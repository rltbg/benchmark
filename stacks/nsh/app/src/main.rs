//! NSH (Native Shell) reference app: GTK 4 backend, Rust-only app (no QuickJS).
//! This file is the host: CLI, window, frame measurement, JSON output.

mod scenarios;
mod shared;

use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering::Relaxed, time::Duration};

use nsh_platform_linux::{
    App, AppOptions, AppView, AssetBase, Root,
    gtk4::{self as gtk, glib, prelude::*},
};
use shared::*;

const WARMUP_S: u64 = 2;

fn parse_args() -> Config {
    let mut c = Config {
        scenario: "idle".into(),
        duration: 10.0,
        out: "out.json".into(),
        rate: 1000,
        shapes: 5000,
        assets: "/home/user/eval/assets".into(),
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
    static PAINTS_TOTAL: RefCell<u64> = const { RefCell::new(0) };
}

fn main() -> glib::ExitCode {
    let cfg = parse_args();
    let _ = T0.get_or_init(std::time::Instant::now);
    let _ = CFG.set(cfg);
    let cfg = cfg_ref();
    let assets = std::path::PathBuf::from(&cfg.assets);

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
        window.set_child(Some(&view.widget()));
        #[cfg(not(feature = "tsx"))]
        view.load(App::Rust(Root::new::<scenarios::Bench>(|| ())));
        #[cfg(feature = "tsx")]
        tsx_load(&view);
        window.present();

        // Frame end = GdkFrameClock `after-paint` (GSK has rendered the frame
        // on the CPU; the X11 present is not awaited). The frame clock only
        // paints when a widget was invalidated by a mounted batch / scroll.
        if let Some(clock) = window.frame_clock() {
            clock.connect_before_paint(|_| {
                BEFORE.with(|b| *b.borrow_mut() = Some(now_ns()));
            });
            clock.connect_after_paint(|_| {
                if START_NS.load(Relaxed) == 0 || STOP.load(Relaxed) {
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
        let keep = Rc::new(RefCell::new(Some(view)));
        #[cfg(feature = "tsx")]
        let host_ch = keep.borrow().as_ref().unwrap().host();
        glib::timeout_add_local_once(Duration::from_secs(WARMUP_S), move || {
            #[cfg(feature = "tsx")]
            {
                let _ = host_ch.send("start", serde_json::Value::Null);
                if cfg_ref().scenario == "stream" {
                    spawn_producer(cfg_ref().rate);
                }
            }
            LAST.with(|l| *l.borrow_mut() = None);
            FRAMES.with(|f| f.borrow_mut().clear());
            PAINT_MS.with(|f| f.borrow_mut().clear());
            reset_counters();
            START_NS.store(now_ns().max(1), Relaxed);
            let app3 = app2.clone();
            let keep2 = keep.clone();
            glib::timeout_add_local_once(Duration::from_secs_f64(cfg_ref().duration), move || {
                STOP.store(true, Relaxed);
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

/// TSX variant: the bundle (compiled by `nsh build`) runs on QuickJS; the host gives it its
/// launch parameters, drains the stream's bounded queue on request and takes its `ops` posts.
#[cfg(feature = "tsx")]
fn tsx_load(view: &AppView) {
    use nsh_platform_linux::Bundle;
    use serde_json::{Value, json};
    let c = cfg();
    let ch = view.host();
    let mut p = serde_json::Map::new();
    p.insert("scenario".into(), json!(c.scenario));
    p.insert("rate".into(), json!(c.rate));
    ch.set_params(p);
    ch.handle("ops", |call| {
        let mut call = call;
        let d = call.take_data();
        let g = |k: &str| d.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        f64_store(&FINAL_OFFSET, g("offset"));
        f64_store(&CONTENT_H, g("content"));
        ROWS_BOUND.store(g("rowsBound") as u64, Relaxed);
        APPLIED.store(g("applied") as u64, Relaxed);
    });
    ch.handle("drain", |call| {
        let batch = std::mem::take(&mut *QUEUE.lock().unwrap());
        let mut flat: Vec<Value> = Vec::with_capacity(batch.len() * 2);
        for (i, c) in batch {
            flat.push(json!(i));
            flat.push(json!(c));
        }
        call.resolve(Value::Array(flat));
    });
    let path = std::env::var("NSH_BUNDLE").unwrap_or_else(|_| "bundle.nshc".into());
    // SAFETY: the bytecode is compiled from this benchmark's own source by `nsh build`.
    let bundle = unsafe { Bundle::read_trusted(std::path::Path::new(&path)) }.expect("read bundle");
    view.load(App::Js(bundle));
}

fn cfg_ref() -> &'static Config {
    cfg()
}

fn write_json() {
    use serde_json::json;
    let c = cfg();
    let frames: Vec<f64> = FRAMES.with(|f| f.borrow().clone());
    let mut ops = serde_json::Map::new();
    let sc = c.scenario.as_str();
    let uses_list = matches!(sc, "list" | "stream" | "stress" | "idle");
    if uses_list {
        ops.insert("sum_heights".into(), json!(sum_heights()));
    }
    if matches!(sc, "list" | "stream" | "stress") {
        ops.insert("final_offset".into(), json!(f64_load(&FINAL_OFFSET)));
        ops.insert("content_height_estimate_final".into(), json!(f64_load(&CONTENT_H)));
        ops.insert("rows_drawn".into(), json!(ROWS_BOUND.load(Relaxed)));
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
        let fr = CANVAS_FRAMES.load(Relaxed).max(1);
        ops.insert("canvas_record_avg_ms".into(), json!(CANVAS_RECORD_US.load(Relaxed) as f64 / fr as f64 / 1000.0));
    }
    if matches!(sc, "core" | "stress") {
        let dur = c.duration.max(0.001);
        ops.insert("calls_per_s".into(), json!(CALLS.load(Relaxed) as f64 / dur));
        ops.insert("heavy_calls".into(), json!(HEAVY.load(Relaxed)));
        let mx = frames.iter().cloned().fold(0.0, f64::max);
        ops.insert("max_frame_stall_ms".into(), json!(mx));
    }
    if sc == "stress" {
        ops.insert("managed_heap_samples".into(), json!([]));
    }
    if sc == "text" {
        ops.insert("time_to_first_display_ms".into(), json!(TEXT_FIRST_MS.load(Relaxed)));
        ops.insert("layout_ms".into(), json!(TEXT_LAYOUT_MS.load(Relaxed)));
        ops.insert("selected_bytes_after_select_all".into(), json!(TEXT_SEL.load(Relaxed)));
    }
    if sc == "images" {
        ops.insert("images_decoded".into(), json!(IMAGES_DECODED.load(Relaxed)));
        ops.insert("decode_total_ms".into(), json!(DECODE_US.load(Relaxed) as f64 / 1000.0));
    }
    if sc == "nav" {
        ops.insert("transitions_done".into(), json!(TRANSITIONS.load(Relaxed)));
    }
    let mut paint: Vec<f64> = PAINT_MS.with(|p| p.borrow().clone());
    if !paint.is_empty() {
        paint.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = paint.len();
        let mean = paint.iter().sum::<f64>() / n as f64;
        ops.insert("paint_phase_ms".into(), json!({"n": n, "mean": mean, "p50": paint[n/2], "p95": paint[(n*95/100).min(n-1)], "max": paint[n-1]}));
    }
    let note = SCRIPT_NOTE.lock().unwrap().clone();
    let mut j = json!({
        "stack": "nsh",
        "scenario": c.scenario,
        "duration_s": c.duration,
        "frames": frames.len(),
        "frame_ms": frames,
        "ops": ops,
        "checksum": format!("{:08x}", (CORE_SINK.load(Relaxed) ^ CALLS.load(Relaxed)) as u32),
        "notes": format!(
            "NSH GTK4 backend (Rust-only app, no QuickJS). Frame end = GdkFrameClock after-paint on the GTK main thread \
             (GSK render finished on CPU, X11 present not awaited); only painted frames counted (the frame clock paints only \
             when a mounted batch/scroll invalidated widgets, and a tick callback alone paints nothing). Rust components run \
             on the NSH engine thread, GTK on the main thread, batches hop once per commit. Warm-up 2 s excluded. \
             GSK_RENDERER={}. {}",
            std::env::var("GSK_RENDERER").unwrap_or_else(|_| "(default)".into()), note),
    });
    #[cfg(not(feature = "tsx"))]
    if let Some(r) = scenarios::unsupported(sc) {
        j["unsupported"] = json!(r);
    }
    #[cfg(feature = "tsx")]
    {
        j["stack"] = json!("nsh-tsx");
        if !matches!(sc, "list" | "stream") {
            j["unsupported"] = json!("TSX variant implemented for list and stream only");
        }
    }
    if let Err(e) = std::fs::write(&c.out, serde_json::to_vec(&j).unwrap()) {
        eprintln!("cannot write {}: {e}", c.out);
    }
}
