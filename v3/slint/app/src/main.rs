// Banc v3 Slint : un binaire, tous les scénarios (voir ../NOTES.md et ../OPTIMIZATION-LOG.md).
mod data;
mod ffi;
mod scen;
mod spsc;

use scen::Scenario;
use serde_json::{json, Map, Value};
use slint::winit_030::{winit, CustomApplicationHandler, EventResult, WinitWindowAccessor};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::io::Write;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

slint::include_modules!();

// ---------------------------------------------------------------- compteur d'allocations (ops.alloc_bytes)
// Inactif par défaut (un chargement relaxé par allocation) ; BENCH_ALLOC=1 l'active.
struct CountingAlloc;
static ALLOC_ON: AtomicBool = AtomicBool::new(false);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ALLOC_ON.load(Ordering::Relaxed) {
            ALLOC_BYTES.fetch_add(l.size() as u64, Ordering::Relaxed);
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if ALLOC_ON.load(Ordering::Relaxed) && n > l.size() {
            ALLOC_BYTES.fetch_add((n - l.size()) as u64, Ordering::Relaxed);
        }
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: CountingAlloc = CountingAlloc;

// ---------------------------------------------------------------- arguments
pub struct Args {
    pub scenario: String,
    pub n: Option<u64>,
    pub variant: BTreeMap<String, String>,
    pub duration: f64,
    pub out: String,
    pub assets: String,
}
impl Args {
    pub fn n_or(&self, d: u64) -> u64 {
        self.n.unwrap_or(d)
    }
    pub fn var(&self, k: &str) -> Option<&str> {
        self.variant.get(k).map(|s| s.as_str())
    }
    /// la variante `name` est demandée : `--variant name` / `--variant name=1` ou `--variant <clé>=name`
    pub fn has(&self, name: &str) -> bool {
        self.variant.iter().any(|(k, v)| k == name && v != "0" || v == name)
    }
    /// valeur d'une clé, ou à défaut une des valeurs candidates passées en forme nue (`--variant gradient`)
    pub fn pick<'a>(&'a self, key: &str, cands: &[&'a str], default: &'a str) -> &'a str {
        if let Some(v) = self.var(key) {
            return v;
        }
        for c in cands {
            if self.has(c) {
                return c;
            }
        }
        default
    }
    pub fn variant_string(&self) -> String {
        self.variant.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(",")
    }
}

/// Racine du dépôt : $BENCH_ROOT, sinon déduite de l'emplacement du binaire
/// (<root>/v3/slint/app/target/release/slintapp).
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
    let mut a = Args { scenario: "idle".into(), n: None, variant: BTreeMap::new(), duration: 12.0, out: "out.json".into(), assets: String::new() };
    let v: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < v.len() {
        let nxt = v.get(i + 1).cloned().unwrap_or_default();
        match v[i].as_str() {
            "--scenario" => a.scenario = nxt,
            "--duration" => a.duration = nxt.parse().unwrap_or(12.0),
            "--out" => a.out = nxt,
            "--n" => a.n = nxt.parse().ok(),
            "--variant" => {
                let (k, val) = nxt.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())).unwrap_or((nxt.clone(), "1".into()));
                a.variant.insert(k, val);
            }
            "--assets" => a.assets = nxt,
            _ => {
                i += 1;
                continue; // arguments v2 inconnus (--rate, --shapes) ignorés
            }
        }
        i += 2;
    }
    // SLINTAPP_VARIANT="k=v,k2=v2" : variantes supplémentaires posées par l'environnement (lignes de stacks3.tsv) ;
    // SLINTAPP_DIRECT=1 est un raccourci pour `direct`
    if let Ok(e) = std::env::var("SLINTAPP_VARIANT") {
        for kv in e.split(',').filter(|x| !x.is_empty()) {
            let (k, v) = kv.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())).unwrap_or((kv.to_string(), "1".into()));
            a.variant.entry(k).or_insert(v);
        }
    }
    if std::env::var("SLINTAPP_DIRECT").as_deref() == Ok("1") {
        a.variant.entry("direct".into()).or_insert("1".into());
    }
    if a.assets.is_empty() {
        a.assets = bench_root().join("assets").to_string_lossy().into_owned();
    }
    a
}

pub fn marker(s: &str) {
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
            _ => "",
        },
        env!("SLINTAPP_RUSTC")
    )
}

// ---------------------------------------------------------------- état partagé UI-thread
thread_local! {
    static DRIVER: RefCell<Option<Driver>> = const { RefCell::new(None) };
    static STALL: Stall = Stall::new();
}
/// Moniteur d'arrêt de la boucle d'événements : minuteur répété de 4 ms sur le thread UI ; on garde le plus
/// long intervalle entre deux déclenchements consécutifs depuis le début de la mesure.
struct Stall {
    last: Cell<Instant>,
    max_ms: Cell<f64>,
    on: Cell<bool>,
}
impl Stall {
    fn new() -> Self {
        Stall { last: Cell::new(Instant::now()), max_ms: Cell::new(0.0), on: Cell::new(false) }
    }
    fn fire(&self) {
        let now = Instant::now();
        if self.on.get() {
            let g = now.duration_since(self.last.get()).as_secs_f64() * 1000.0;
            if g > self.max_ms.get() {
                self.max_ms.set(g);
            }
        }
        self.last.set(now);
    }
}

/// Hook public (feature `unstable-winit-030`) : appelé AVANT que Slint ne voie l'événement.
/// RedrawRequested = début du dessin ; le `about_to_wait` suivant arrive après que Slint a rendu et présenté
/// (softbuffer present_with_damage) -> fin de frame.
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
        match event {
            winit::event::WindowEvent::RedrawRequested => {
                DRIVER.with(|d| {
                    if let Ok(mut g) = d.try_borrow_mut() {
                        if let Some(d) = g.as_mut() {
                            d.drew = true;
                            d.draw_start = Instant::now();
                        }
                    }
                });
            }
            winit::event::WindowEvent::Resized(_) => {
                DRIVER.with(|d| {
                    if let Ok(mut g) = d.try_borrow_mut() {
                        if let Some(d) = g.as_mut() {
                            d.resized_at = Some(Instant::now());
                            d.resize_count += 1;
                        }
                    }
                });
            }
            _ => {}
        }
        EventResult::Propagate
    }
    fn about_to_wait(&mut self, _el: &winit::event_loop::ActiveEventLoop) -> EventResult {
        DRIVER.with(|d| {
            if let Ok(mut g) = d.try_borrow_mut() {
                if let Some(d) = g.as_mut() {
                    d.tick();
                }
            }
        });
        EventResult::Propagate
    }
}

pub struct Driver {
    app: MainWindow,
    args: Args,
    drew: bool,
    draw_start: Instant,
    pending: bool,
    requested_at: Instant,
    proc_start: Instant,
    start: Instant,
    t0: Option<Instant>,
    last_end: Option<Instant>,
    frame_ms: Vec<f64>,
    sc: Box<dyn Scenario>,
    unthrottled: bool,
    renderer: String,
    focus_done: bool,
    ready_done: bool,
    setup_ms: Option<f64>,
    resized_at: Option<Instant>,
    resize_count: u64,
    resize_frame_ms: Vec<f64>,
    keys: Rc<Cell<u64>>,
    self_resize: Option<(u32, f64)>, // (nombre de redimensionnements restants, dernier instant)
}

const WARMUP: f64 = 2.0;

impl Driver {
    /// Donne le focus clavier X11 à notre fenêtre (pas de WM sous Xvfb) : X envoie FocusIn, winit marque la fenêtre
    /// active et transmet les KeyPress à Slint (chemin d'événements normal).
    fn take_x11_focus(&mut self) {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let mut id: Option<u32> = None;
        self.app.window().with_winit_window(|w: &winit::window::Window| {
            if let Ok(h) = w.window_handle() {
                id = match h.as_raw() {
                    RawWindowHandle::Xlib(x) => Some(x.window as u32),
                    RawWindowHandle::Xcb(x) => Some(x.window.get()),
                    _ => None,
                };
            }
        });
        if let Some(win) = id {
            use x11rb::connection::Connection;
            use x11rb::protocol::xproto::{ConnectionExt, InputFocus};
            if let Ok((conn, _)) = x11rb::connect(None) {
                let _ = conn.set_input_focus(InputFocus::PARENT, win, x11rb::CURRENT_TIME);
                let _ = conn.flush();
                let _ = conn.get_input_focus().map(|c| c.reply());
            }
        }
        self.focus_done = true;
    }

    fn request_redraw(&self) {
        if self.unthrottled {
            // API publique documentée : WinitWindowAccessor::with_winit_window + winit Window::request_redraw
            // (contourne le Timer 60 Hz interne de Slint sur X11)
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

    fn tick(&mut self) {
        let now = Instant::now();
        if self.drew {
            self.drew = false;
            self.pending = false;
            self.sc.on_frame_end(now, now.duration_since(self.draw_start));
            if self.setup_ms.is_none() {
                self.setup_ms = Some(now.duration_since(self.proc_start).as_secs_f64() * 1000.0);
            }
            if let Some(r) = self.resized_at.take() {
                if self.t0.is_some() {
                    self.resize_frame_ms.push(now.duration_since(r).as_secs_f64() * 1000.0);
                }
            }
            if self.t0.is_some() {
                if let Some(l) = self.last_end {
                    self.frame_ms.push(now.duration_since(l).as_secs_f64() * 1000.0);
                }
                self.last_end = Some(now);
            }
            if !self.focus_done {
                self.take_x11_focus();
            }
            if !self.ready_done {
                self.ready_done = true;
                self.sc.on_ready(&self.app);
            }
        }
        let el = now.duration_since(self.start).as_secs_f64();
        if self.t0.is_none() && el >= WARMUP && self.ready_done {
            self.t0 = Some(now);
            self.last_end = None;
            self.sc.begin_measure(&self.app);
            STALL.with(|s| {
                s.max_ms.set(0.0);
                s.last.set(now);
                s.on.set(true);
            });
            self.keys.set(0);
            self.resize_frame_ms.clear();
            if std::env::var("BENCH_ALLOC").as_deref() == Ok("1") {
                ALLOC_BYTES.store(0, Ordering::Relaxed);
                ALLOC_COUNT.store(0, Ordering::Relaxed);
                ALLOC_ON.store(true, Ordering::Relaxed);
            }
            marker("BENCH:measure_start");
        }
        let t = self.t0.map(|t0| now.duration_since(t0).as_secs_f64()).unwrap_or(el);
        if self.t0.is_some() && t >= self.args.duration {
            self.finish(t);
            return;
        }
        // redimensionnement déclenché par l'app elle-même (variante selfresize=1, pour valider U1 sans harnais)
        if let (Some((left, last)), true) = (self.self_resize, self.t0.is_some()) {
            if left > 0 && t - last >= 0.06 {
                let big = left % 2 == 0;
                let (w, h) = if big { (1280.0, 720.0) } else { (1100.0, 650.0) };
                self.app.window().set_size(slint::LogicalSize::new(w, h));
                self.self_resize = Some((left - 1, t));
            }
        }
        if self.sc.animated() && (!self.pending || now.duration_since(self.requested_at) > Duration::from_millis(250)) {
            self.sc.step(t, &self.app, self.t0.is_some());
            self.request_redraw();
            self.pending = true;
            self.requested_at = now;
        }
    }

    fn finish(&mut self, t: f64) {
        marker("BENCH:measure_end");
        ALLOC_ON.store(false, Ordering::Relaxed);
        let mut ops = Map::new();
        self.sc.ops(t, &self.app, &mut ops);
        ops.insert("n".into(), self.args.n.map(Value::from).unwrap_or(Value::Null));
        ops.insert("variant".into(), self.args.variant_string().into());
        ops.insert("ui_stall_ms_max".into(), STALL.with(|s| s.max_ms.get()).into());
        ops.insert("setup_ms".into(), self.setup_ms.unwrap_or(0.0).into());
        ops.insert("key_events".into(), self.keys.get().into());
        ops.insert("pacing".into(), (if self.unthrottled { "uncapped" } else { "60" }).into());
        ops.insert("renderer".into(), self.renderer.clone().into());
        if !self.resize_frame_ms.is_empty() || self.resize_count > 0 {
            ops.insert("resize_frame_ms".into(), json!(self.resize_frame_ms));
            ops.insert("resize_events".into(), self.resize_count.into());
        }
        if std::env::var("BENCH_ALLOC").as_deref() == Ok("1") {
            ops.insert("alloc_bytes".into(), ALLOC_BYTES.load(Ordering::Relaxed).into());
            ops.insert("alloc_count".into(), ALLOC_COUNT.load(Ordering::Relaxed).into());
        } else {
            ops.insert("alloc_bytes".into(), Value::Null);
        }
        if !ops.contains_key("not_rendered") {
            ops.insert("not_rendered".into(), json!([]));
        }
        let notes = format!(
            "renderer={}; pacing={}; frame_ms = interval between consecutive frame ends; frame end = first winit about_to_wait after a RedrawRequested (Slint renders and presents synchronously while handling RedrawRequested: softbuffer present_with_damage), observed through the public slint::winit_030::CustomApplicationHandler (unstable feature); set_rendering_notifier is not usable as frame end (AfterRendering fires before present; the software renderer returns Unsupported). {} ui_stall_ms_max = longest gap between two firings of a 4 ms repeated slint::Timer on the UI thread (includes render time of a frame). setup_ms = first frame end since the start of main(). 2 s warm-up not recorded. {}",
            self.renderer,
            if self.unthrottled { "uncapped" } else { "60" },
            if self.unthrottled {
                "uncapped: redraw requested from about_to_wait via WinitWindowAccessor::with_winit_window(|w| w.request_redraw()), bypassing Slint's X11 TimerBasedFrameThrottle through public APIs only."
            } else {
                "BENCH_PACING=60: native Slint cadence (slint::Window::request_redraw; Slint's winit/X11 TimerBasedFrameThrottle uses a repeated Timer of 1000000/refresh_millihertz ms in integer ms: 16 ms, i.e. 62.5 Hz)."
            },
            self.sc.notes()
        );
        let j = json!({
            "stack": "slint", "version": version_string(&self.renderer), "scenario": self.args.scenario, "duration_s": t,
            "frames": self.frame_ms.len(), "frame_ms": self.frame_ms,
            "ops": Value::Object(ops), "checksum": self.sc.checksum(), "notes": notes, "unsupported": Value::Null,
        });
        self.sc.stop();
        std::fs::write(&self.args.out, serde_json::to_vec(&j).unwrap()).expect("write json");
        std::process::exit(0);
    }
}

fn write_unsupported(args: &Args, reason: &str, renderer: &str) -> ! {
    marker("BENCH:measure_start");
    marker("BENCH:measure_end");
    let j = json!({
        "stack": "slint", "version": version_string(renderer), "scenario": args.scenario, "duration_s": 0.0, "frames": 0, "frame_ms": [],
        "ops": {"n": args.n, "variant": args.variant_string(), "not_rendered": []}, "checksum": "00000000",
        "notes": format!("unsupported: {reason}"), "unsupported": reason,
    });
    std::fs::write(&args.out, serde_json::to_vec(&j).unwrap()).expect("write json");
    std::process::exit(0);
}

fn main() {
    let proc_start = Instant::now();
    let args = parse_args();
    let renderer = std::env::var("SLINTAPP_RENDERER").unwrap_or_else(|_| "skia-software".into());
    if let Some(reason) = scen::unsupported(&args, &renderer) {
        write_unsupported(&args, &reason, &renderer);
    }
    let pacing = std::env::var("BENCH_PACING").ok();
    let unthrottled = match pacing.as_deref() {
        Some("uncapped") => true,
        Some("60") | None => false,
        Some(o) => {
            eprintln!("BENCH_PACING={o} inconnu, 60 utilisé");
            false
        }
    };
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name(renderer.clone())
        .with_winit_custom_application_handler(Handler)
        .select()
        .expect("backend select");

    let app = MainWindow::new().unwrap();
    let keys = Rc::new(Cell::new(0u64));
    {
        // variante `direct` : en plus du chemin normal, demande une frame tout de suite par l'API winit publique
        // (le TimerBasedFrameThrottle de Slint attend sinon 16 ms avant la 1re frame suivant un événement)
        let direct = args.has("direct");
        let redraw = {
            let w = app.as_weak();
            move || {
                if direct {
                    if let Some(a) = w.upgrade() {
                        a.window().with_winit_window(|w: &winit::window::Window| w.request_redraw());
                    }
                }
            }
        };
        let k = keys.clone();
        let r1 = redraw.clone();
        app.global::<Bench>().on_space_pressed(move || {
            k.set(k.get() + 1);
            r1();
        });
        let k = keys.clone();
        app.global::<Bench>().on_edited_text(move || {
            k.set(k.get() + 1);
            redraw();
        });
    }
    let b = app.global::<Bench>();
    b.set_scn(args.scenario.clone().into());
    b.set_variant(args.variant_string().into());
    b.set_explicit(args.var("sizes") != Some("implicit"));
    let is_idle = args.scenario == "idle";
    b.set_square(!is_idle);
    let sc = match scen::build(&args, &app, &renderer) {
        Ok(s) => s,
        Err(reason) => write_unsupported(&args, &reason, &renderer),
    };
    app.show().unwrap();
    app.invoke_focus_keys();
    let start = Instant::now();
    if is_idle {
        // idle : coquille d'app, aucun redessin continu ; deux Timers mono-coup (début / fin de mesure), pas de moniteur 4 ms
        let dur = args.duration;
        let out = args.out.clone();
        let scen = args.scenario.clone();
        let rname = renderer.clone();
        let t_start = slint::Timer::default();
        t_start.start(slint::TimerMode::SingleShot, Duration::from_secs_f64(WARMUP), || marker("BENCH:measure_start"));
        let t_end = slint::Timer::default();
        t_end.start(slint::TimerMode::SingleShot, Duration::from_secs_f64(WARMUP + dur), move || {
            marker("BENCH:measure_end");
            let j = json!({"stack":"slint","version":version_string(&rname),"scenario":scen,"duration_s":dur,"frames":0,"frame_ms":[],
              "ops":{"sum_heights": data::sum_heights(100_000, false), "not_rendered": [], "pacing": "60", "n": Value::Null, "variant": "", "key_events": 0, "ui_stall_ms_max": Value::Null, "setup_ms": Value::Null},
              "checksum":"00000000","unsupported":null,
              "notes": format!("renderer={}; idle: shell (title bar, first list page, status bar) drawn once; no redraw loop, the event loop sleeps (two single-shot Timers mark start/end)", rname)});
            std::fs::write(&out, serde_json::to_vec(&j).unwrap()).unwrap();
            std::process::exit(0);
        });
        slint::run_event_loop().unwrap();
        return;
    }
    let self_resize = if args.var("selfresize").is_some() { Some((100u32, 0.0f64)) } else { None };
    DRIVER.with(|d| {
        *d.borrow_mut() = Some(Driver {
            app, args, drew: false, draw_start: Instant::now(), pending: false, requested_at: Instant::now(), proc_start, start, t0: None,
            last_end: None, frame_ms: vec![], sc, unthrottled, renderer, focus_done: false, ready_done: false, setup_ms: None,
            resized_at: None, resize_count: 0, resize_frame_ms: vec![], keys, self_resize,
        });
    });
    // moniteur d'arrêt de la boucle d'événements (spec v3 §2) ; il réveille aussi la boucle pour `tick`
    let stall = slint::Timer::default();
    stall.start(slint::TimerMode::Repeated, Duration::from_millis(4), || STALL.with(|s| s.fire()));
    slint::run_event_loop().unwrap();
    drop(stall);
}
