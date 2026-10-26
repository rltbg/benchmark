// Slint benchmark app (voir ../STACK-NOTES.md)
mod sim;
use sim::*;

use slint::winit_030::{winit, CustomApplicationHandler, EventResult, WinitWindowAccessor};
use std::cell::RefCell;
use std::sync::atomic::Ordering;
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

fn parse_args() -> Args {
    let mut a = Args { scenario: "idle".into(), duration: 10.0, out: "out.json".into(), rate: 0, shapes: 0, assets: "/home/user/eval/assets".into() };
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
    a
}

thread_local! {
    static DRIVER: RefCell<Option<Driver>> = const { RefCell::new(None) };
}

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
        }
        let t = self.t0.map(|t0| now.duration_since(t0).as_secs_f64()).unwrap_or(el);
        if self.t0.is_some() && t >= self.args.duration {
            self.finish(t);
            return;
        }
        self.sc.step(&self.app, t, self.t0.is_some());
        if self.sc.wants_redraw(t, self.t0.is_some()) {
            if self.unthrottled {
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
        let ops = self.sc.ops(t);
        let notes = format!(
            "renderer={}; unthrottled_redraw={}; frame_ms = interval between consecutive about_to_wait calls that follow a winit RedrawRequested (i.e. after Slint render + softbuffer/GL present returned, measured from the winit CustomApplicationHandler); redraw requested continuously from about_to_wait (bypasses Slint's 60Hz Timer frame throttle when unthrottled=true); first 2s warm-up not recorded. {}",
            self.renderer, self.unthrottled, self.sc.notes()
        );
        let j = serde_json::json!({
            "stack": "slint", "scenario": self.args.scenario, "duration_s": t,
            "frames": self.frame_ms.len(), "frame_ms": self.frame_ms,
            "ops": ops, "checksum": self.sc.checksum(), "notes": notes,
        });
        std::fs::write(&self.args.out, serde_json::to_vec(&j).unwrap()).expect("write json");
        std::process::exit(0);
    }
}

fn main() {
    let args = parse_args();
    let renderer = std::env::var("SLINTAPP_RENDERER").unwrap_or_else(|_| "software".into());
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
        // idle : aucune animation, on dort et on quitte via un Timer
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::SingleShot, Duration::from_secs_f64(WARMUP + dur), move || {
            let j = serde_json::json!({"stack":"slint","scenario":scen,"duration_s":dur,"frames":0,"frame_ms":[],
              "ops":{"sum_heights": sim::sum_heights()},"checksum":"00000000",
              "notes": format!("renderer={}; idle: no redraw loop, event loop sleeps (Timer wakes once at the end)", rname)});
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
    let _ = (Ordering::Relaxed, &wake);
}
