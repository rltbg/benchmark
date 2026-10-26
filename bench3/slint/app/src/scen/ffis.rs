// FFI et fils : F1 ffi-calls, F2 ffi-callbacks, F3 ffi-block.
use super::gfx::ShapesState;
use super::lists::{PoolInner, VIEW_H};
use super::Scenario;
use crate::data::*;
use crate::ffi::Core;
use crate::spsc::Spsc;
use crate::{Args, Bench, MainWindow};
use serde_json::{json, Map, Value};
use slint::{ComponentHandle, ModelRc};
use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ================================================================ F2 / F3 : liste de 100 k lignes
thread_local! {
    static INNER: RefCell<Option<Rc<RefCell<PoolInner>>>> = const { RefCell::new(None) };
}
static INVOKE_APPLIED: AtomicU64 = AtomicU64::new(0);
static INVOKE_FAILED: AtomicU64 = AtomicU64::new(0);

#[derive(PartialEq, Clone, Copy)]
enum Src {
    Queue,
    Coalesce,
    Invoke,
}
/// contexte partagé avec le rappel natif (jamais d'appel Slint depuis le fil producteur, sauf Src::Invoke)
struct CbCtx {
    src: Src,
    q: Arc<Spsc<(u32, f64)>>,
    dropped: AtomicU64,
    coalesced: AtomicU64,
    latest: Vec<AtomicU64>,
    dirty: Vec<AtomicBool>,
}
unsafe extern "C" fn on_event(user: *mut c_void, index: u32, price: f64) {
    let c = unsafe { &*(user as *const CbCtx) };
    match c.src {
        Src::Queue => {
            if !c.q.push((index, price)) {
                c.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
        Src::Coalesce => {
            let i = index as usize % c.latest.len();
            c.latest[i].store(price.to_bits(), Ordering::Relaxed);
            if c.dirty[i].swap(true, Ordering::Release) {
                c.coalesced.fetch_add(1, Ordering::Relaxed); // l'événement précédent sur cette ligne n'a jamais été vu : remplacé
            }
        }
        Src::Invoke => {
            let r = slint::invoke_from_event_loop(move || {
                INNER.with(|i| {
                    if let Some(inn) = i.borrow().as_ref() {
                        inn.borrow_mut().apply_price(index as usize, price);
                    }
                });
                INVOKE_APPLIED.fetch_add(1, Ordering::Relaxed);
            });
            if r.is_err() {
                INVOKE_FAILED.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

pub struct ListFfi {
    core: Core,
    scenario: String,
    inner: Rc<RefCell<PoolInner>>,
    sc: Scroller,
    // F2
    src: Src,
    rate: u64,
    ctx: *mut CbCtx,
    started: bool,
    applied_unique: u64,
    drained: u64,
    produced: u64,
    // F3
    heavy_ms: u32,
    on_ui: bool,
    heavy_stop: Arc<AtomicBool>,
    heavy_calls: Arc<AtomicU64>,
    next_ui_heavy: f64,
    ui_heavy_calls: u64,
}

pub fn build_list_ffi(args: &Args, app: &MainWindow, _renderer: &str) -> Result<Box<dyn Scenario>, String> {
    let b = app.global::<Bench>();
    let f2 = args.scenario == "ffi-callbacks";
    let inner = Rc::new(RefCell::new(PoolInner::new(100_000, false, f2)));
    let local = inner.borrow_mut().set_offset(0.0);
    b.set_chrome(true);
    b.set_rows(ModelRc::from(inner.borrow().model.clone()));
    b.set_scroll_y(local);
    b.set_scn("pool-list".into());
    INNER.with(|i| *i.borrow_mut() = Some(inner.clone()));
    let src = match args.pick("src", &["queue", "coalesce", "invoke"], "queue") {
        "coalesce" => Src::Coalesce,
        "invoke" => Src::Invoke,
        _ => Src::Queue,
    };
    let ctx = Box::into_raw(Box::new(CbCtx {
        src,
        q: Spsc::<(u32, f64)>::new(1 << 21),
        dropped: AtomicU64::new(0),
        coalesced: AtomicU64::new(0),
        latest: (0..100_000).map(|_| AtomicU64::new(0)).collect(),
        dirty: (0..100_000).map(|_| AtomicBool::new(false)).collect(),
    }));
    let heavy_ms = if f2 { 0 } else { args.n_or(100) as u32 };
    Ok(Box::new(ListFfi {
        core: Core::get(),
        scenario: args.scenario.clone(),
        inner,
        sc: Scroller::new(Script::constant(if f2 { 3000.0 } else { 20000.0 })),
        src,
        rate: args.n_or(10_000),
        ctx,
        started: false,
        applied_unique: 0,
        drained: 0,
        produced: 0,
        heavy_ms,
        on_ui: args.has("ui"),
        heavy_stop: Arc::new(AtomicBool::new(false)),
        heavy_calls: Arc::new(AtomicU64::new(0)),
        next_ui_heavy: 0.0,
        ui_heavy_calls: 0,
    }))
}

impl ListFfi {
    /// applique les événements reçus (appelé une fois par frame, et une dernière fois avant le décompte final)
    fn drain_events(&mut self) {
        let ctx = unsafe { &*self.ctx };
        match self.src {
            Src::Queue => {
                let mut n = 0u64;
                let mut inn = self.inner.borrow_mut();
                ctx.q.drain(usize::MAX, |(i, p)| {
                    inn.apply_price(i as usize, p);
                    n += 1;
                });
                drop(inn);
                self.drained += n;
            }
            Src::Coalesce => {
                let mut inn = self.inner.borrow_mut();
                let mut u = 0u64;
                for i in 0..ctx.dirty.len() {
                    if ctx.dirty[i].load(Ordering::Relaxed) && ctx.dirty[i].swap(false, Ordering::Acquire) {
                        let p = f64::from_bits(ctx.latest[i].load(Ordering::Relaxed));
                        inn.apply_price(i, p);
                        u += 1;
                    }
                }
                drop(inn);
                self.applied_unique += u;
            }
            Src::Invoke => {}
        }
    }
}
impl Scenario for ListFfi {
    fn begin_measure(&mut self, app: &MainWindow) {
        self.sc.reset();
        let local = self.inner.borrow_mut().set_offset(0.0);
        app.global::<Bench>().set_scroll_y(local);
        if self.scenario == "ffi-callbacks" {
            let ok = unsafe { (self.core.producer_start)(Some(on_event), self.ctx as *mut c_void, self.rate, 100_000) };
            self.started = ok == 1;
        } else if !self.on_ui {
            let (stop, calls, ms, core) = (self.heavy_stop.clone(), self.heavy_calls.clone(), self.heavy_ms, self.core);
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let r = unsafe { (core.heavy)(ms) };
                    std::hint::black_box(r);
                    calls.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    }
    fn step(&mut self, t: f64, app: &MainWindow, measuring: bool) {
        if measuring && self.scenario == "ffi-callbacks" {
            self.drain_events();
        }
        if measuring && self.scenario == "ffi-block" && self.on_ui && t >= self.next_ui_heavy {
            // variante `ui` : appel sur le fil UI, une fois par seconde (gel mesuré par ui_stall_ms_max)
            self.next_ui_heavy = t + 1.0;
            let r = unsafe { (self.core.heavy)(self.heavy_ms) };
            std::hint::black_box(r);
            self.ui_heavy_calls += 1;
        }
        let off = self.sc.advance(if measuring { t } else { 0.0 }, self.inner.borrow().max_offset());
        let local = self.inner.borrow_mut().set_offset(off);
        app.global::<Bench>().set_scroll_y(local);
        let _ = VIEW_H;
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        ops.insert("sum_heights".into(), sum_heights(100_000, false).into());
        ops.insert("checksum_data".into(), format!("{:08x}", checksum_heights(100_000, false)).into());
        if self.scenario == "ffi-callbacks" {
            // fin de mesure : arrêt du producteur, puis dernier lot (les événements émis depuis la dernière frame ne
            // sont pas « perdus » ; sans cela le retard d'une frame ferait 99,86 % sur 12 s)
            if self.started {
                self.produced = (self.core.producer_stop)();
                self.started = false;
            }
            self.drain_events();
            let produced = self.produced;
            let ctx = unsafe { &*self.ctx };
            let (applied, extra): (u64, Value) = match self.src {
                Src::Queue => (self.drained, json!({"mechanism": "lock-free SPSC ring (2^21 x 16 B) drained once per frame on the UI thread, every event applied individually"})),
                Src::Coalesce => {
                    let co = ctx.coalesced.load(Ordering::Relaxed);
                    (self.applied_unique + co, json!({"mechanism": "latest-value-wins per row (AtomicU64 price + dirty flag); coalesced events are counted as applied", "applied_unique": self.applied_unique, "coalesced": co}))
                }
                Src::Invoke => (INVOKE_APPLIED.load(Ordering::Relaxed), json!({"mechanism": "slint::invoke_from_event_loop per event (naive)", "invoke_failed": INVOKE_FAILED.load(Ordering::Relaxed)})),
            };
            ops.insert("updates_generated".into(), produced.into());
            ops.insert("updates_applied".into(), applied.into());
            ops.insert("updates_dropped".into(), ctx.dropped.load(Ordering::Relaxed).into());
            ops.insert("src".into(), match self.src { Src::Queue => "queue", Src::Coalesce => "coalesce", Src::Invoke => "invoke" }.into());
            ops.insert("src_detail".into(), extra);
            ops.insert("rows_visible_updated".into(), self.inner.borrow().price_applied.into());
            ops.insert("rate".into(), self.rate.into());
        } else {
            ops.insert("heavy_ms".into(), self.heavy_ms.into());
            ops.insert("heavy_calls".into(), (self.heavy_calls.load(Ordering::Relaxed) + self.ui_heavy_calls).into());
            ops.insert("heavy_on_ui_thread".into(), self.on_ui.into());
        }
    }
    fn notes(&self) -> String {
        if self.scenario == "ffi-callbacks" {
            "ffi-callbacks: core_producer_start(cb, user, rate=N, range=100000) through a black_box'ed extern \"C\" fn pointer; the native producer thread only touches lock-free structures in the callback (no Slint call). 100 k-row ring list scrolling at 3000 px/s. updates_applied counts events reflected in the row store (visible rows are updated through VecModel::set_row_data) or coalesced explicitly (src=coalesce).".into()
        } else {
            "ffi-block: core_heavy(N ms) called back-to-back on a background std::thread (or once per second on the UI thread with variant ui) while the 100 k-row list scrolls at 20000 px/s; ui_stall_ms_max from the 4 ms UI-thread timer.".into()
        }
    }
    fn stop(&mut self) {
        if self.started {
            (self.core.producer_stop)();
            self.started = false;
        }
        self.heavy_stop.store(true, Ordering::Relaxed);
    }
}

// ================================================================ F1 ffi-calls
#[derive(Default)]
struct WorkerStats {
    calls: [AtomicU64; 4],
    nanos: [AtomicU64; 4],
    bytes: [AtomicU64; 4],
}
pub struct CallsSc {
    shapes: ShapesState,
    stop: Arc<AtomicBool>,
    stats: Vec<Arc<WorkerStats>>,
    phase: Arc<AtomicU64>,
    threads: usize,
    size: usize,
    handles: Vec<std::thread::JoinHandle<()>>,
    started: Arc<AtomicBool>,
    last_phase: u64,
    slice: f64,
}
const FN_NAMES: [&str; 4] = ["core_noop", "core_add", "core_hash", "core_fill"];

pub fn build_calls(args: &Args, app: &MainWindow) -> Result<Box<dyn Scenario>, String> {
    let threads = args.var("threads").and_then(|v| v.parse().ok()).unwrap_or(1usize);
    let size = args.n_or(1024) as usize;
    let shapes = ShapesState::new(app, 1000, "solid", true);
    app.global::<Bench>().set_chrome(false);
    let stop = Arc::new(AtomicBool::new(false));
    let started = Arc::new(AtomicBool::new(false));
    let phase = Arc::new(AtomicU64::new(0));
    let core = Core::get();
    let hash_buf: Arc<Vec<u8>> = Arc::new({
        let mut v = vec![0u8; size];
        unsafe { (core.fill)(v.as_mut_ptr(), size, 7) };
        v
    });
    let mut stats = vec![];
    let mut handles = vec![];
    for _ in 0..threads {
        let st = Arc::new(WorkerStats::default());
        stats.push(st.clone());
        let (stop, started, phase, hb) = (stop.clone(), started.clone(), phase.clone(), hash_buf.clone());
        handles.push(std::thread::spawn(move || {
            let mut out = vec![0u8; size]; // tampon de sortie créé une fois
            let mut acc: u64 = 0;
            while !started.load(Ordering::Acquire) {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let mut cur = phase.load(Ordering::Relaxed) as usize % 4;
            let mut t_phase = Instant::now();
            let (mut calls, mut bytes) = (0u64, 0u64);
            while !stop.load(Ordering::Relaxed) {
                let ph = phase.load(Ordering::Relaxed) as usize % 4;
                if ph != cur {
                    st.calls[cur].fetch_add(calls, Ordering::Relaxed);
                    st.bytes[cur].fetch_add(bytes, Ordering::Relaxed);
                    st.nanos[cur].fetch_add(t_phase.elapsed().as_nanos() as u64, Ordering::Relaxed);
                    calls = 0;
                    bytes = 0;
                    cur = ph;
                    t_phase = Instant::now();
                }
                // une rafale ~ 100 µs de travail avant de relire la phase
                let burst = match cur {
                    0 | 1 => 4096,
                    2 | 3 => (65536 / size.max(1)).clamp(1, 4096),
                    _ => 1,
                };
                for k in 0..burst {
                    match cur {
                        0 => acc = acc.wrapping_add((core.noop)()),
                        1 => acc = acc.wrapping_add((core.add)(std::hint::black_box(k as u64), 3)),
                        2 => acc = acc.wrapping_add(unsafe { (core.hash)(hb.as_ptr(), hb.len()) } as u64),
                        _ => unsafe { (core.fill)(out.as_mut_ptr(), out.len(), k as u32 + 1) },
                    }
                }
                calls += burst as u64;
                if cur >= 2 {
                    bytes += burst as u64 * size as u64;
                }
            }
            st.calls[cur].fetch_add(calls, Ordering::Relaxed);
            st.bytes[cur].fetch_add(bytes, Ordering::Relaxed);
            st.nanos[cur].fetch_add(t_phase.elapsed().as_nanos() as u64, Ordering::Relaxed);
            std::hint::black_box(acc);
        }));
    }
    Ok(Box::new(CallsSc { shapes, stop, stats, phase, threads, size, handles, started, last_phase: 0, slice: (args.duration / 4.0).max(0.25) }))
}

impl Scenario for CallsSc {
    fn begin_measure(&mut self, _app: &MainWindow) {
        self.started.store(true, Ordering::Release);
    }
    fn step(&mut self, t: f64, app: &MainWindow, measuring: bool) {
        self.shapes.step(t, app);
        if measuring {
            // 4 fonctions en tranches de durée égale sur la fenêtre de mesure (12 s -> 3 s chacune)
            let ph = ((t / (self.slice)) as u64).min(3);
            self.phase.store(ph, Ordering::Relaxed);
            self.last_phase = ph;
        }
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        let mut per_fn = Map::new();
        for f in 0..4 {
            // débit agrégé = somme sur les fils des appels / temps passé par ce fil dans la phase
            let (mut cps, mut mbps, mut total_calls) = (0.0f64, 0.0f64, 0u64);
            for s in &self.stats {
                let c = s.calls[f].load(Ordering::Relaxed);
                let ns = s.nanos[f].load(Ordering::Relaxed);
                let by = s.bytes[f].load(Ordering::Relaxed);
                if ns > 0 {
                    cps += c as f64 / (ns as f64 / 1e9);
                    mbps += by as f64 / (ns as f64 / 1e9) / 1e6;
                }
                total_calls += c;
            }
            per_fn.insert(FN_NAMES[f].into(), json!({"calls_per_s": cps, "mb_per_s": mbps, "calls": total_calls}));
        }
        ops.insert("per_function".into(), Value::Object(per_fn));
        ops.insert("threads".into(), self.threads.into());
        ops.insert("buffer_bytes".into(), self.size.into());
        ops.insert("shapes".into(), 1000.into());
        let mut tot = 0.0;
        for f in 0..4 {
            for s in &self.stats {
                let ns = s.nanos[f].load(Ordering::Relaxed);
                if ns > 0 {
                    tot += s.calls[f].load(Ordering::Relaxed) as f64 / (ns as f64 / 1e9);
                }
            }
        }
        ops.insert("calls_per_s".into(), tot.into());
    }
    fn notes(&self) -> String {
        "ffi-calls: worker threads call core_noop, core_add, core_hash (buffer of N bytes created once) and core_fill (output buffer created once) through black_box'ed extern \"C\" fn pointers, one function per 3 s slice of the 12 s window (phase set by the UI frame step; workers re-read it every ~100 us of work). Rates = sum over threads of calls / time spent in the phase. The UI animates 1000 `solid` shapes (positions bound to one `t` property).".into()
    }
    fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}
