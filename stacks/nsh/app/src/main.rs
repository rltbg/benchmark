//! NSH (Native Shell) reference app: GTK 4 backend, Rust-only app (no QuickJS).
//! This file is the host: CLI, window, frame measurement, markers, JSON output.

mod probe;
mod scenarios;
mod shared;

use std::{cell::RefCell, io::Write, rc::Rc, sync::atomic::Ordering::Relaxed, time::Duration};

use nsh_platform_linux::{
    App, AppOptions, AppView, AssetBase, Root,
    gtk4::{self as gtk, glib, prelude::*},
};
use shared::*;

const WARMUP_S: u64 = 2;

/// Repository root: `BENCH_ROOT`, else deduced from the executable's place
/// (`<root>/stacks/nsh/app/target/release/nshbench`).
fn bench_root() -> std::path::PathBuf {
    if let Some(r) = std::env::var_os("BENCH_ROOT") {
        return r.into();
    }
    let exe = std::env::current_exe().unwrap_or_default();
    exe.ancestors().nth(6).map(|p| p.to_path_buf()).unwrap_or_else(|| ".".into())
}

fn parse_args() -> Config {
    let mut c = Config {
        scenario: "idle".into(),
        duration: 10.0,
        out: "out.json".into(),
        rate: 1000,
        shapes: 5000,
        assets: bench_root().join("assets").to_string_lossy().into_owned(),
    };
    let mut dur_set = false;
    let a: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < a.len() {
        let v = a.get(i + 1).cloned().unwrap_or_default();
        match a[i].as_str() {
            "--scenario" => c.scenario = v,
            "--duration" => {
                c.duration = v.parse().unwrap_or(10.0);
                dur_set = true;
            }
            "--out" => c.out = v,
            "--rate" => c.rate = v.parse().unwrap_or(1000),
            "--shapes" => c.shapes = v.parse().unwrap_or(5000),
            "--assets" => c.assets = v,
            _ => {
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    if !dur_set && c.scenario == "stress" {
        c.duration = 600.0;
    }
    c
}

thread_local! {
    static LAST: RefCell<Option<u64>> = const { RefCell::new(None) };
    static FRAMES: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
    static BEFORE: RefCell<Option<u64>> = const { RefCell::new(None) };
    static PAINT_MS: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
}

fn marker(m: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{m}");
    let _ = out.flush();
}

/// BENCH_PACING (spec v2.1): "60" (default) = GTK's native GdkFrameClock cadence;
/// "uncapped" = GDK_DEBUG=no-vsync, the documented GTK option "Repaint instantly"
/// (https://docs.gtk.org/gtk4/running.html, GDK_DEBUG), set before GTK starts.
fn pacing() -> &'static str {
    match std::env::var("BENCH_PACING").as_deref() {
        Ok("uncapped") => "uncapped",
        _ => "60",
    }
}

fn main() -> glib::ExitCode {
    if pacing() == "uncapped" {
        let cur = std::env::var("GDK_DEBUG").unwrap_or_default();
        if !cur.split([',', ':', ' ']).any(|f| f == "no-vsync") {
            let v = if cur.is_empty() { "no-vsync".to_owned() } else { format!("{cur},no-vsync") };
            // SAFETY: first statement of main, before any thread exists.
            unsafe { std::env::set_var("GDK_DEBUG", v) };
        }
    }
    let cfg = parse_args();
    let _ = T0.get_or_init(std::time::Instant::now);
    let _ = CFG.set(cfg);
    let assets = std::path::PathBuf::from(&cfg_ref().assets);

    let app = gtk::Application::builder()
        .application_id("dev.bench.nsh")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(move |app| {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("nsh bench")
            .default_width(1280)
            .default_height(720)
            .resizable(false)
            .decorated(false)
            .build();
        let view = AppView::new(AppOptions {
            asset_base: Some(AssetBase::Directory(assets.clone())),
            app_id: "dev.bench.nsh".into(),
            title: "nsh bench".into(),
            size: (1280, 720),
            ..Default::default()
        });
        window.set_child(Some(&probe::Probe::new(&view.widget())));
        view.load(App::Rust(Root::new::<scenarios::Bench>(|| ())));
        // The window keeps the view; closing it stops the app (docs: Linux, Embedding in a GTK app).
        let keep = Rc::new(RefCell::new(Some(Rc::clone(&view))));
        {
            let keep = keep.clone();
            window.connect_destroy(move |_| {
                if let Some(v) = keep.borrow_mut().take() {
                    v.stop()
                }
            });
        }
        window.present();

        // Frame end = GdkFrameClock `after-paint`, the last phase of a GTK frame
        // (GSK has rendered and the surface's frame has been ended / submitted to X11;
        // the X server's presentation is not reported back without a compositor).
        // A frame counts only if some widget was snapshotted again in it (probe.rs):
        // a mounted batch, a scroll or a native animation step invalidated it.
        if let Some(clock) = window.frame_clock() {
            clock.connect_before_paint(|_| {
                BEFORE.with(|b| *b.borrow_mut() = Some(now_ns()));
            });
            clock.connect_after_paint(|_| {
                // text: the first frame painted once the engine has laid the text out
                // (its batch is mounted before the next paint).
                if TEXT_LAID_MS.load(Relaxed) != 0 && TEXT_FIRST_MS.load(Relaxed) == 0 {
                    TEXT_FIRST_MS.store((now_ns() / 1_000_000).max(1), Relaxed);
                }
                // Only frames that redrew something count (see probe.rs).
                let drew = probe::take();
                if START_NS.load(Relaxed) == 0 || STOP.load(Relaxed) {
                    return;
                }
                if !drew {
                    EMPTY_PAINTS.fetch_add(1, Relaxed);
                    BEFORE.with(|b| b.borrow_mut().take());
                    return;
                }
                let now = now_ns();
                if let Some(b) = BEFORE.with(|b| b.borrow_mut().take()) {
                    PAINT_MS.with(|p| p.borrow_mut().push((now - b) as f64 / 1e6));
                }
                LAST.with(|l| {
                    if let Some(prev) = l.replace(Some(now)) {
                        FRAMES.with(|f| f.borrow_mut().push((now - prev) as f64 / 1e6));
                    }
                });
            });
        } else {
            eprintln!("no frame clock");
        }

        // Measured window starts after the warm-up.
        let app2 = app.clone();
        glib::timeout_add_local_once(Duration::from_secs(WARMUP_S), move || {
            LAST.with(|l| *l.borrow_mut() = None);
            FRAMES.with(|f| f.borrow_mut().clear());
            PAINT_MS.with(|f| f.borrow_mut().clear());
            reset_counters();
            marker("BENCH:measure_start");
            START_NS.store(now_ns().max(1), Relaxed);
            let app3 = app2.clone();
            let keep2 = keep.clone();
            glib::timeout_add_local_once(Duration::from_secs_f64(cfg_ref().duration), move || {
                STOP.store(true, Relaxed);
                marker("BENCH:measure_end");
                write_json();
                if let Some(v) = keep2.borrow_mut().take() {
                    v.stop();
                }
                app3.quit();
            });
        });
    });
    let code = app.run_with_args::<&str>(&[]);
    std::process::exit(if code == glib::ExitCode::SUCCESS { 0 } else { 1 });
}

fn cfg_ref() -> &'static Config {
    cfg()
}

fn version() -> String {
    format!(
        "nsh-core/nsh-base/nsh-platform-linux 0.2.0 (crates.io, = git tag v0.2.0 669b62b); gtk4-rs 0.11.5; \
         GTK {}.{}.{} (system, runtime); rustc {}",
        gtk::major_version(),
        gtk::minor_version(),
        gtk::micro_version(),
        env!("NSHBENCH_RUSTC")
    )
}

fn write_json() {
    use serde_json::json;
    let c = cfg();
    let frames: Vec<f64> = FRAMES.with(|f| f.borrow().clone());
    let mut ops = serde_json::Map::new();
    let sc = c.scenario.as_str();
    ops.insert("pacing".into(), json!(pacing()));
    ops.insert("paints_without_change".into(), json!(EMPTY_PAINTS.load(Relaxed)));
    if matches!(sc, "list" | "stream" | "stress" | "idle" | "input") {
        ops.insert("sum_heights".into(), json!(sum_heights()));
        ops.insert("list_content_height_reported".into(), json!(f64_load(&CONTENT_H)));
    }
    if matches!(sc, "list" | "stream" | "stress" | "input") {
        ops.insert("final_offset".into(), json!(f64_load(&FINAL_OFFSET)));
        ops.insert("rows_drawn".into(), json!(ROWS_BOUND.load(Relaxed)));
    }
    if sc == "input" {
        ops.insert("key_events".into(), json!(KEY_EVENTS.load(Relaxed)));
    }
    if matches!(sc, "stream" | "stress") {
        ops.insert("updates_generated".into(), json!(GENERATED.load(Relaxed)));
        ops.insert("updates_applied".into(), json!(APPLIED.load(Relaxed)));
        ops.insert("updates_dropped".into(), json!(DROPPED.load(Relaxed)));
        ops.insert("rate".into(), json!(if sc == "stress" { 5000 } else { c.rate }));
    }
    if matches!(sc, "canvas" | "core" | "stress") {
        let n = match sc {
            "core" => 1000,
            _ => c.shapes,
        };
        ops.insert("shapes".into(), json!(n));
        ops.insert("graph_points".into(), json!(if sc == "core" { 0 } else { 10_000 }));
        ops.insert("not_rendered".into(), json!(Vec::<String>::new()));
    }
    if matches!(sc, "core" | "stress") {
        let dur = c.duration.max(0.001);
        ops.insert("calls_per_s".into(), json!(CALLS.load(Relaxed) as f64 / dur));
        ops.insert("heavy_calls".into(), json!(HEAVY.load(Relaxed)));
        let mx = frames.iter().cloned().fold(0.0, f64::max);
        ops.insert("max_frame_stall_ms".into(), json!(mx));
    }
    if sc == "text" {
        ops.insert("time_to_first_display_ms".into(), json!(TEXT_FIRST_MS.load(Relaxed)));
        ops.insert("layout_ms".into(), json!(f64_load(&TEXT_LAYOUT_MS)));
        ops.insert("text_bytes".into(), json!(TEXT_BYTES.load(Relaxed)));
        ops.insert("text_content_height".into(), json!(f64_load(&CONTENT_H)));
        ops.insert("select_all".into(), json!("not done: NSH 0.2.0 has no programmatic selection for text (set_selection is for inputs only); Ctrl+A selects all for the user"));
    }
    if sc == "images" {
        ops.insert("images_decoded".into(), json!(IMAGES_DECODED.load(Relaxed)));
        ops.insert("decode_total_ms".into(), json!(DECODE_US.load(Relaxed) as f64 / 1000.0));
        ops.insert("images_distinct_loaded".into(), json!(distinct_images()));
    }
    if sc == "nav" {
        ops.insert("transitions_done".into(), json!(TRANSITIONS.load(Relaxed)));
        ops.insert("modals_done".into(), json!(MODALS.load(Relaxed)));
    }
    let mut paint: Vec<f64> = PAINT_MS.with(|p| p.borrow().clone());
    if !paint.is_empty() {
        paint.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = paint.len();
        let mean = paint.iter().sum::<f64>() / n as f64;
        ops.insert(
            "paint_phase_ms".into(),
            json!({"n": n, "mean": mean, "p50": paint[n/2], "p95": paint[(n*95/100).min(n-1)], "max": paint[n-1]}),
        );
    }
    let note = SCRIPT_NOTE.lock().unwrap().clone();
    let j = json!({
        "stack": "nsh",
        "version": version(),
        "scenario": c.scenario,
        "duration_s": c.duration,
        "frames": frames.len(),
        "frame_ms": frames,
        "ops": ops,
        "checksum": format!("{:08x}", (CORE_SINK.load(Relaxed) ^ CALLS.load(Relaxed) ^ CHECK.load(Relaxed)) as u32),
        "notes": format!(
            "NSH 0.2.0 GTK4 backend, Rust-only app (no QuickJS). Frame end = GdkFrameClock after-paint on the GTK main thread \
             (last public post-render signal: GSK render done, X11 frame submitted; presentation not reported without a compositor). \
             Only frames whose snapshot ran again (a widget was invalidated: mounted batch, scroll, native animation) are counted; paints that redrew nothing go to ops.paints_without_change. \
             Pacing {}: 60 = GdkFrameClock's own 60 Hz timer (no widget/frame-clock API changes it); uncapped = GDK_DEBUG=no-vsync \
             (documented GTK env option, GDK_DEBUG={}). \
             Rust components run on the NSH engine thread, GTK on the main thread, one batch hop per commit. Warm-up 2 s excluded. \
             GSK_RENDERER={}. {}",
            pacing(),
            std::env::var("GDK_DEBUG").unwrap_or_else(|_| "(unset)".into()),
            std::env::var("GSK_RENDERER").unwrap_or_else(|_| "(default)".into()),
            note
        ),
        "unsupported": null,
    });
    if let Err(e) = std::fs::write(&c.out, serde_json::to_vec(&j).unwrap()) {
        eprintln!("cannot write {}: {e}", c.out);
    }
}
