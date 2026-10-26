// Slint benchmark app (voir ../../STACK-NOTES.md)
mod sim;
use sim::*;

use slint::winit_030::{winit, CustomApplicationHandler, EventResult, WinitWindowAccessor};
use std::cell::RefCell;
use std::io::Write;
use std::time::{Duration, Instant};

slint::include_modules!();

pub struct Args {
    pub scenario: String,
    pub duration: f64,
    pub out: String,
    pub rate: u32,
    pub shapes: usize,
    pub assets: String,
}

/// Racine du dépôt : $BENCH_ROOT, sinon déduite de l'emplacement du binaire
/// (<root>/stacks/slint/app/target/release/slintapp).
fn bench_root() -> std::path::PathBuf {
    if let Some(r) = std::env::var_os("BENCH_ROOT") {
        return r.into();
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .and_then(|p| p.ancestors().nth(6).map(|a| a.to_path_buf()))
        .unwrap_or_else(|| ".".into())
}

fn parse_args() -> Args {
    let mut a = Args { scenario: "idle".into(), duration: 10.0, out: "out.json".into(), rate: 0, shapes: 0, assets: String::new() };
    let v: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < v.len() {
        let nxt = v.get(i + 1).cloned().unwrap_or_default();
        match v[i].as_str() {
            "--scenario" => a.scenario = nxt,
            "--duration" => a.duration = nxt.parse().unwrap_or(10.0),
            "--out" => a.out = nxt,
            "--rate" => a.rate = nxt.parse().unwrap_or(0),
            "--shapes" => a.shapes = nxt.parse().unwrap_or(0),
            "--assets" => a.assets = nxt,
            _ => {
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    if a.assets.is_empty() {
        a.assets = bench_root().join("assets").to_string_lossy().into_owned();
    }
    a
}

fn marker(s: &str) {
    let mut o = std::io::stdout().lock();
    let _ = writeln!(o, "{}", s);
    let _ = o.flush();
}

pub fn version_string(renderer: &str) -> String {
    format!(
        "slint 1.18.1 (backend winit 0.30.13 x11, renderer {}{}); {}; image 0.25.10",
        renderer,
        match renderer {
            "skia-software" | "skia" => " via skia-safe/skia-bindings 0.153.3, softbuffer 0.4.8",
            "software" => " (i-slint-renderer-software 1.18.1), softbuffer 0.4.8",
            "femtovg" => " (femtovg/OpenGL, Mesa llvmpipe)",
            _ => "",
        },
        env!("SLINTAPP_RUSTC")
    )
}

thread_local! {
    static DRIVER: RefCell<Option<Driver>> = const { RefCell::new(None) };
}

/// Hook public (feature `unstable-winit-030`) : toutes les méthodes sont appelées AVANT que
/// Slint ne voie l'événement. RedrawRequested -> début du draw ; le about_to_wait suivant
/// arrive après que Slint a rendu et présenté (softbuffer present / swap GL) -> fin de frame.
struct Handler;
impl CustomApplicationHandler for Handler {
    fn window_event(
        &mut self,
        _el: &winit::event_loop::ActiveEventLoop,
        _id: winit::window::WindowId,
        _w: Option<&winit::window::Window>,
        _s: Option<&slint::Window>,
        event: &winit::event::WindowEvent,
    ) -> EventResult {
        if matches!(event, winit::event::WindowEvent::RedrawRequested) {
            DRIVER.with(|d| {
                if let Some(d) = d.borrow_mut().as_mut() {
                    d.drew = true;
                    d.draw_start = Instant::now();
                }
            });
        }
        EventResult::Propagate
    }
    fn about_to_wait(&mut self, _el: &winit::event_loop::ActiveEventLoop) -> EventResult {
        DRIVER.with(|d| {
            if let Some(d) = d.borrow_mut().as_mut() {
                d.tick();
            }
        });
        EventResult::Propagate
    }
}

pub struct Driver {
    pub app: MainWindow,
    pub args: Args,
    pub drew: bool,
    pub draw_start: Instant,
    start: Instant,
    t0: Option<Instant>,
    last_end: Option<Instant>,
    frame_ms: Vec<f64>,
    sc: Scenario,
    unthrottled: bool,
    renderer: String,
}

const WARMUP: f64 = 2.0;

impl Driver {
    fn tick(&mut self) {
        let now = Instant::now();
        if self.drew {
            self.drew = false;
            self.sc.on_frame_end(now, now.duration_since(self.draw_start));
            if self.t0.is_some() {
                if let Some(l) = self.last_end {
                    self.frame_ms.push(now.duration_since(l).as_secs_f64() * 1000.0);
                }
                self.last_end = Some(now);
            }
        }
        let el = now.duration_since(self.start).as_secs_f64();
        if self.t0.is_none() && el >= WARMUP {
            self.t0 = Some(now);
            self.last_end = None;
            self.sc.begin_measure(&self.app);
            marker("BENCH:measure_start");
        }
        let t = self.t0.map(|t0| now.duration_since(t0).as_secs_f64()).unwrap_or(el);
        if self.t0.is_some() && t >= self.args.duration {
            self.finish(t);
            return;
        }
        self.sc.step(&self.app, t, self.t0.is_some());
        if self.sc.wants_redraw(t, self.t0.is_some()) {
            if self.unthrottled {
                // API publique documentée : WinitWindowAccessor::with_winit_window + winit
                // Window::request_redraw (pas le Timer 60 Hz interne de Slint sur X11)
                let mut done = false;
                self.app.window().with_winit_window(|w: &winit::window::Window| {
                    w.request_redraw();
                    done = true;
                });
                if !done {
                    self.app.window().request_redraw();
                }
            } else {
                self.app.window().request_redraw();
            }
        }
    }

    fn finish(&mut self, t: f64) {
        marker("BENCH:measure_end");
        let mut ops = self.sc.ops(t, &self.renderer);
        if self.sc.has_core() {
            let mx = self.frame_ms.iter().cloned().fold(0.0, f64::max);
            ops["max_frame_stall_ms"] = mx.into();
        }
        let notes = format!(
            "renderer={}; unthrottled_redraw={}; frame_ms = interval between consecutive frame ends; frame end = first winit about_to_wait after a RedrawRequested (Slint renders and presents synchronously while handling RedrawRequested: softbuffer present_with_damage for software/skia-software, eglSwapBuffers for femtovg), observed through the public slint::winit_030::CustomApplicationHandler; set_rendering_notifier is not usable as frame end (AfterRendering fires before present, and the software renderer returns Unsupported). {} ; 2 s warm-up not recorded. {}",
            self.renderer,
            self.unthrottled,
            if self.unthrottled {
                "continuous redraw requested from about_to_wait via WinitWindowAccessor::with_winit_window(|w| w.request_redraw()), which bypasses Slint's internal X11 TimerBasedFrameThrottle (60 Hz timer) using public documented APIs only (feature unstable-winit-030)"
            } else {
                "redraw requested via slint::Window::request_redraw -> Slint's X11 TimerBasedFrameThrottle paces at the monitor refresh (60 Hz)"
            },
            self.sc.notes(&self.renderer)
        );
        let j = serde_json::json!({
            "stack": "slint", "version": version_string(&self.renderer), "scenario": self.args.scenario, "duration_s": t,
            "frames": self.frame_ms.len(), "frame_ms": self.frame_ms,
            "ops": ops, "checksum": self.sc.checksum(), "notes": notes, "unsupported": null,
        });
        std::fs::write(&self.args.out, serde_json::to_vec(&j).unwrap()).expect("write json");
        std::process::exit(0);
    }
}

fn main() {
    let args = parse_args();
    let renderer = std::env::var("SLINTAPP_RENDERER").unwrap_or_else(|_| "skia-software".into());
    let unthrottled = std::env::var("SLINTAPP_THROTTLE").map(|v| v != "1").unwrap_or(true);
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name(renderer.clone())
        .with_winit_custom_application_handler(Handler)
        .select()
        .expect("backend select");

    let app = MainWindow::new().unwrap();
    let sc = Scenario::new(&args, &app);
    app.show().unwrap();
    let animated = sc.animated();
    let start = Instant::now();
    let dur = args.duration;
    let out = args.out.clone();
    let scen = args.scenario.clone();
    let rname = renderer.clone();
    let scen_is_nav = scen == "nav";
    DRIVER.with(|d| {
        *d.borrow_mut() = Some(Driver { app, args, drew: false, draw_start: Instant::now(), start, t0: None, last_end: None, frame_ms: vec![], sc, unthrottled, renderer });
    });
    if !animated {
        // idle : aucune animation ni redessin continu ; deux Timers mono-coup (début / fin de mesure)
        let t_start = slint::Timer::default();
        t_start.start(slint::TimerMode::SingleShot, Duration::from_secs_f64(WARMUP), || marker("BENCH:measure_start"));
        let t_end = slint::Timer::default();
        t_end.start(slint::TimerMode::SingleShot, Duration::from_secs_f64(WARMUP + dur), move || {
            marker("BENCH:measure_end");
            let j = serde_json::json!({"stack":"slint","version":version_string(&rname),"scenario":scen,"duration_s":dur,"frames":0,"frame_ms":[],
              "ops":{"sum_heights": sim::sum_heights(), "not_rendered": []},"checksum":"00000000","unsupported":null,
              "notes": format!("renderer={}; idle: shell (title bar, first list page, status bar) drawn once; no redraw loop, the event loop sleeps (two single-shot Timers mark start/end)", rname)});
            std::fs::write(&out, serde_json::to_vec(&j).unwrap()).unwrap();
            std::process::exit(0);
        });
        slint::run_event_loop().unwrap();
        return;
    }
    // nav : le redessin n'est demandé que pendant les animations ; un timer 2 ms réveille la
    // boucle pour lancer les transitions suivantes et détecter la fin (pas un pacing de rendu).
    let wake = slint::Timer::default();
    if scen_is_nav {
        wake.start(slint::TimerMode::Repeated, Duration::from_millis(2), || {});
    }
    slint::run_event_loop().unwrap();
    drop(wake);
}
