// Listes : L1 list-scale, L2 list-rich, L3 list-mutate, L4 list-reset, idle, I1 latency-load, I2 latency-wheel.
// Deux implémentations de la virtualisation (variante `impl`) :
//  * `pool`   : anneau de lignes instanciées une fois et repositionnées par Rust (positions exactes, sommes préfixes éparses) ;
//  * `native` : le `ListView` officiel sur un `Model` paresseux (virtualisation de i-slint-core/model/repeater.rs).
use super::{software_not_rendered, Scenario};
use crate::data::*;
use crate::spsc::Spsc;
use crate::{Args, Bench, MainWindow, RowData};
use serde_json::{json, Map, Value};
use slint::{Color, ComponentHandle, Model, ModelNotify, ModelRc, ModelTracker, SharedString, VecModel};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const VIEW_H: f64 = 660.0; // 720 - barre de titre 36 - barre d'état 24
const ORIGIN_BLOCK: usize = 4096; // rangées par bloc d'origine (reste dans la précision de f32 : ~360 k px)

pub fn make_row(i: u32, y: f32, price: f64, dense: bool) -> RowData {
    let f = fnv(i);
    let q = format!("Quote {} \u{2014} {:.2}", i, (fnv(i + 7) % 100000) as f64 / 100.0);
    let reps = if dense { 3 } else { 6 };
    let mut sub = String::with_capacity(q.len() * reps + reps);
    for _ in 0..reps {
        sub.push_str(&q);
        sub.push(' ');
    }
    RowData {
        y,
        h: (if dense { dense_height(i) } else { height(i) }) as f32,
        title: format!("Item #{}", i).into(),
        sub: sub.into(),
        price: format!("{:.2}", price).into(),
        color: Color::from_rgb_u8(f as u8, (f >> 8) as u8, (f >> 16) as u8),
        hue: (f % 360) as f32,
        prog: (fnv(i + 3) % 100) as f32 / 100.0,
        on: fnv(i + 5) & 1 == 1,
    }
}
fn initial_price(i: u32) -> f64 {
    (fnv(i + 7) % 100000) as f64 / 100.0
}
fn blank_row() -> RowData {
    RowData { y: 0.0, h: 0.0, title: "".into(), sub: "".into(), price: "".into(), color: Color::from_argb_u8(0, 0, 0, 0), hue: 0.0, prog: 0.0, on: false }
}

// ================================================================ pool
pub struct PoolInner {
    pub pre: Prefix,
    pub dense: bool,
    pool: usize,
    pub model: Rc<VecModel<RowData>>,
    slots: Vec<(usize, usize)>, // (idx, origine) ; idx == usize::MAX : vide
    pub prices: Vec<f64>,       // vide si le scénario n'a pas de flux de prix
    origin: usize,
    pub rows_set: u64,
    pub final_offset: f64,
    pub price_applied: u64,
    ys: Vec<(usize, u64)>,
}
impl PoolInner {
    pub fn new(n: usize, dense: bool, track_prices: bool) -> Self {
        let pool = if dense { 40 } else { 32 };
        let model = Rc::new(VecModel::from(vec![blank_row(); pool]));
        let prices = if track_prices { (0..n as u32).map(initial_price).collect() } else { vec![] };
        PoolInner { pre: Prefix::build(n, dense), dense, pool, model, slots: vec![(usize::MAX, 0); pool], prices, origin: 0, rows_set: 0, final_offset: 0.0, price_applied: 0, ys: Vec::with_capacity(64) }
    }
    pub fn set_pre(&mut self, pre: Prefix) {
        self.pre = pre;
        for s in self.slots.iter_mut() {
            *s = (usize::MAX, 0);
        }
        self.final_offset = self.final_offset.min(self.max_offset());
    }
    pub fn max_offset(&self) -> f64 {
        (self.pre.total as f64 - VIEW_H).max(0.0)
    }
    fn price_of(&self, i: usize) -> f64 {
        if self.prices.is_empty() { initial_price(i as u32) } else { self.prices[i] }
    }
    /// positionne l'anneau pour le décalage `off` (px) ; renvoie le décalage local à donner au conteneur
    pub fn set_offset(&mut self, off: f64) -> f32 {
        let n = self.pre.n;
        let off = off.clamp(0.0, self.max_offset());
        self.final_offset = off;
        let lo = self.pre.row_at(off as u64).saturating_sub(1);
        let origin = lo - lo % ORIGIN_BLOCK;
        self.origin = origin;
        let y_origin = self.pre.pos(origin);
        let mut y = self.pre.pos(lo);
        let bottom = off + VIEW_H;
        self.ys.clear();
        let mut i = lo;
        while i < n && self.ys.len() < self.pool {
            self.ys.push((i, y - y_origin));
            if y as f64 >= bottom {
                break; // une ligne de marge sous la zone visible
            }
            y += self.pre.h(i) as u64;
            i += 1;
        }
        let mut in_win = [false; 64];
        for k in 0..self.ys.len() {
            let (i, yy) = self.ys[k];
            let slot = i % self.pool;
            in_win[slot] = true;
            if self.slots[slot] != (i, origin) {
                let row = make_row(i as u32, yy as f32, self.price_of(i), self.dense);
                self.model.set_row_data(slot, row);
                self.slots[slot] = (i, origin);
                self.rows_set += 1;
            }
        }
        // anneaux restants : une ligne d'une autre origine aurait un y faux -> la vider ; sinon elle est hors écran
        for slot in 0..self.pool {
            if !in_win[slot] && self.slots[slot].0 != usize::MAX && self.slots[slot].1 != origin {
                self.model.set_row_data(slot, blank_row());
                self.slots[slot] = (usize::MAX, 0);
            }
        }
        (off - y_origin as f64) as f32
    }
    pub fn apply_price(&mut self, idx: usize, price: f64) -> bool {
        if idx >= self.prices.len() {
            return false;
        }
        self.prices[idx] = price;
        let slot = idx % self.pool;
        if self.slots[slot].0 == idx {
            if let Some(mut r) = self.model.row_data(slot) {
                r.price = SharedString::from(format!("{:.2}", price));
                self.model.set_row_data(slot, r);
                self.price_applied += 1;
                return true;
            }
        }
        false
    }
}

// ================================================================ native : modèles paresseux
pub struct LazyModel {
    pub n: usize,
    pub dense: bool,
    notify: ModelNotify,
}
impl LazyModel {
    pub fn new(n: usize, dense: bool) -> Rc<Self> {
        Rc::new(LazyModel { n, dense, notify: ModelNotify::default() })
    }
}
impl Model for LazyModel {
    type Data = RowData;
    fn row_count(&self) -> usize {
        self.n
    }
    fn row_data(&self, row: usize) -> Option<RowData> {
        if row < self.n { Some(make_row(row as u32, 0.0, initial_price(row as u32), self.dense)) } else { None }
    }
    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}
/// modèle dont les lignes sont des identifiants (L4 : jeu de données reconstruit dans un fil de fond)
pub struct IdModel {
    ids: Vec<u32>,
    notify: ModelNotify,
}
impl Model for IdModel {
    type Data = RowData;
    fn row_count(&self) -> usize {
        self.ids.len()
    }
    fn row_data(&self, row: usize) -> Option<RowData> {
        let id = *self.ids.get(row)?;
        Some(make_row(id, 0.0, initial_price(id), false))
    }
    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}

// ================================================================ scénario liste statique
#[derive(PartialEq, Clone, Copy)]
enum Impl {
    Pool,
    Native,
}
enum Extra {
    None,
    Reset(ResetState),
    Heavy(Arc<AtomicBool>, Arc<AtomicU64>),
}
struct ResetState {
    n: usize,
    next_at: f64,
    generation: u32,
    build_ms: Vec<f64>,
    swap_ms: Vec<f64>,
    on_ui: bool,
    rx: std::sync::mpsc::Receiver<ResetResult>,
    tx: std::sync::mpsc::Sender<ResetResult>,
    in_flight: bool,
}
enum ResetData {
    Pre(Prefix),
    Ids(Vec<u32>),
}
struct ResetResult {
    data: ResetData,
    build_ms: f64,
}

pub struct ListSc {
    imp: Impl,
    inner: Rc<RefCell<PoolInner>>,
    scroller: Option<Scroller>,
    n: usize,
    dense: bool,
    scenario: String,
    extra: Extra,
    native_total: f64,
    native_final: f64,
    model_build_ms: f64,
    shell: bool,
    wheel_events: Rc<Cell<u64>>,
    renderer: String,
    k: u64,
}

fn parse_load(args: &Args) -> String {
    args.pick("load", &["idle", "fling", "cpuhog", "heavy", "mutate"], "idle").to_string()
}

pub fn build_idle(args: &Args, app: &MainWindow) -> Result<Box<dyn Scenario>, String> {
    let b = app.global::<Bench>();
    b.set_chrome(true);
    let inner = Rc::new(RefCell::new(PoolInner::new(100_000, false, false)));
    let local = inner.borrow_mut().set_offset(0.0);
    b.set_rows(ModelRc::from(inner.borrow().model.clone()));
    b.set_scroll_y(local);
    b.set_scn("pool-list".into());
    let _ = args;
    Ok(Box::new(ListSc {
        imp: Impl::Pool,
        inner,
        scroller: None,
        n: 100_000,
        dense: false,
        scenario: "idle".into(),
        extra: Extra::None,
        native_total: 0.0,
        native_final: 0.0,
        model_build_ms: 0.0,
        shell: true,
        wheel_events: Rc::new(Cell::new(0)),
        renderer: String::new(),
        k: 6,
    }))
}

pub fn build_static(args: &Args, app: &MainWindow, renderer: &str) -> Result<Box<dyn Scenario>, String> {
    let b = app.global::<Bench>();
    let sc = args.scenario.as_str();
    let load = parse_load(args);
    if sc.starts_with("latency") && load == "mutate" {
        // charge « mutate » : L3 à 100 k mises à jour/s sous le carré de latence
        let mut a2 = Args { scenario: "list-mutate".into(), n: Some(args.var("rate").and_then(|v| v.parse().ok()).unwrap_or(100_000)), variant: args.variant.clone(), duration: args.duration, out: args.out.clone(), assets: args.assets.clone() };
        a2.variant.remove("load");
        return build_mutate(&a2, app).map(|mut s| {
            let _ = &mut s;
            s
        });
    }
    let dflt = if sc == "list-rich" { "pool" } else { "native" };
    let imp = if args.pick("impl", &["pool", "native"], dflt) == "native" { Impl::Native } else { Impl::Pool };
    let (n, dense, script, k) = match sc {
        "list-scale" => (args.n_or(100_000) as usize, false, Some(Script::l1()), 6u64),
        "list-rich" => (100_000usize, true, Some(Script::constant(20000.0)), args.n_or(6)),
        "list-reset" => (args.n_or(100_000) as usize, false, Some(Script::constant(10000.0)), 6),
        _ => {
            // latency-load / latency-wheel : 100 k lignes ; charge `fling` = flings en continu
            let s = if sc == "latency-load" && load == "fling" { Some(Script::fling(150000.0)) } else { None };
            (100_000usize, false, s, 6)
        }
    };
    b.set_chrome(true);
    b.set_dense(dense);
    if dense {
        b.set_puces(((k.saturating_sub(6)) / 2) as i32);
    }
    let t_build = Instant::now();
    let wheel_events = Rc::new(Cell::new(0u64));
    let (inner, native_total) = match imp {
        Impl::Pool => {
            let inner = Rc::new(RefCell::new(PoolInner::new(n, dense, false)));
            let local = inner.borrow_mut().set_offset(0.0);
            b.set_rows(ModelRc::from(inner.borrow().model.clone()));
            b.set_scroll_y(local);
            b.set_scn(if dense { "pool-dense" } else { "pool-list" }.into());
            // molette (I2) : appliquée dans le rappel, sans attendre une frame
            let w = app.as_weak();
            let inn = inner.clone();
            let we = wheel_events.clone();
            let direct = args.has("direct");
            b.on_wheel(move |dy| {
                we.set(we.get() + 1);
                if let Some(app) = w.upgrade() {
                    let mut i = inn.borrow_mut();
                    let off = i.final_offset - dy as f64;
                    let local = i.set_offset(off);
                    app.global::<Bench>().set_scroll_y(local);
                    if direct {
                        use slint::winit_030::{winit, WinitWindowAccessor};
                        app.window().with_winit_window(|w: &winit::window::Window| w.request_redraw());
                    }
                }
            });
            (inner, 0.0)
        }
        Impl::Native => {
            // le modèle ne contient aucune donnée : lignes construites à la demande ; hauteur totale exacte = pour borner le défilement
            let total = Prefix::build(n, dense).total as f64;
            b.set_lv_rows(ModelRc::from(LazyModel::new(n, dense) as Rc<dyn Model<Data = RowData>>));
            b.set_scn(if dense { "native-dense" } else { "native-list" }.into());
            let inner = Rc::new(RefCell::new(PoolInner::new(0, dense, false)));
            (inner, total)
        }
    };
    let model_build_ms = t_build.elapsed().as_secs_f64() * 1000.0;
    let mut extra = Extra::None;
    if sc == "list-reset" {
        let (tx, rx) = std::sync::mpsc::channel();
        extra = Extra::Reset(ResetState { n, next_at: 2.0, generation: 0, build_ms: vec![], swap_ms: vec![], on_ui: args.var("build") == Some("ui"), rx, tx, in_flight: false });
    }
    if sc == "latency-load" && load == "heavy" {
        // core_heavy(100) en boucle sur un fil de fond
        let stop = Arc::new(AtomicBool::new(false));
        let count = Arc::new(AtomicU64::new(0));
        let (s2, c2) = (stop.clone(), count.clone());
        let core = crate::ffi::Core::get();
        std::thread::spawn(move || {
            while !s2.load(Ordering::Relaxed) {
                let r = unsafe { (core.heavy)(100) };
                std::hint::black_box(r);
                c2.fetch_add(1, Ordering::Relaxed);
            }
        });
        extra = Extra::Heavy(stop, count);
    }
    let mut scroller = script.map(Scroller::new);
    if let Some(s) = scroller.as_mut() {
        s.reset();
    }
    Ok(Box::new(ListSc {
        imp,
        inner,
        scroller,
        n,
        dense,
        scenario: sc.to_string(),
        extra,
        native_total,
        native_final: 0.0,
        model_build_ms,
        shell: true,
        wheel_events,
        renderer: renderer.to_string(),
        k,
    }))
}

impl ListSc {
    fn total(&self) -> f64 {
        match self.imp {
            Impl::Pool => self.inner.borrow().pre.total as f64,
            Impl::Native => self.native_total,
        }
    }
    fn do_reset(&mut self, t: f64, app: &MainWindow) {
        let imp = self.imp;
        let dense = self.dense;
        let Extra::Reset(rs) = &mut self.extra else { return };
        // 1. résultat prêt ? -> échange sur le fil UI
        if let Ok(res) = rs.rx.try_recv() {
            let t0 = Instant::now();
            match res.data {
                ResetData::Pre(pre) => {
                    self.native_total = pre.total as f64;
                    self.inner.borrow_mut().set_pre(pre);
                }
                ResetData::Ids(ids) => {
                    let m = Rc::new(IdModel { ids, notify: ModelNotify::default() });
                    self.native_total = m.ids.iter().map(|&i| height(i) as u64).sum::<u64>() as f64;
                    app.global::<Bench>().set_lv_rows(ModelRc::from(m as Rc<dyn Model<Data = RowData>>));
                }
            }
            rs.build_ms.push(res.build_ms);
            rs.swap_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
            rs.in_flight = false;
        }
        // 2. nouveau remplacement toutes les 2 s
        if t >= rs.next_at && !rs.in_flight {
            rs.next_at += 2.0;
            rs.generation += 1;
            let (n, generation) = (rs.n, rs.generation);
            let work = move || -> ResetResult {
                let t0 = Instant::now();
                let data = match imp {
                    Impl::Pool => ResetData::Pre(Prefix::build(n, dense)),
                    Impl::Native => ResetData::Ids((0..n as u32).map(|i| i.wrapping_add(generation * 7919)).collect()),
                };
                ResetResult { data, build_ms: t0.elapsed().as_secs_f64() * 1000.0 }
            };
            if rs.on_ui {
                // v0 naïve : reconstruction sur le fil UI (gel mesuré par ui_stall_ms_max)
                let r = work();
                rs.tx.send(r).ok();
            } else {
                let tx = rs.tx.clone();
                rs.in_flight = true;
                std::thread::spawn(move || {
                    let r = work();
                    tx.send(r).ok();
                });
                return;
            }
            // chemin UI : appliquer immédiatement
            if let Ok(res) = rs.rx.try_recv() {
                let t0 = Instant::now();
                match res.data {
                    ResetData::Pre(pre) => {
                        self.native_total = pre.total as f64;
                        self.inner.borrow_mut().set_pre(pre);
                    }
                    ResetData::Ids(ids) => {
                        let m = Rc::new(IdModel { ids, notify: ModelNotify::default() });
                        self.native_total = m.ids.iter().map(|&i| height(i) as u64).sum::<u64>() as f64;
                        app.global::<Bench>().set_lv_rows(ModelRc::from(m as Rc<dyn Model<Data = RowData>>));
                    }
                }
                rs.build_ms.push(res.build_ms);
                rs.swap_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
            }
        }
    }
}

impl Scenario for ListSc {
    fn animated(&self) -> bool {
        self.scroller.is_some() || matches!(self.extra, Extra::Reset(_))
    }
    fn begin_measure(&mut self, app: &MainWindow) {
        if let Some(s) = self.scroller.as_mut() {
            s.reset();
        }
        if let Extra::Reset(rs) = &mut self.extra {
            rs.next_at = 2.0;
        }
        match self.imp {
            Impl::Pool => {
                let local = self.inner.borrow_mut().set_offset(0.0);
                app.global::<Bench>().set_scroll_y(local);
            }
            Impl::Native => app.global::<Bench>().set_lv_y(0.0),
        }
    }
    fn step(&mut self, t: f64, app: &MainWindow, measuring: bool) {
        self.do_reset(if measuring { t } else { -1.0 }, app);
        let total = self.total();
        if let Some(s) = self.scroller.as_mut() {
            let off = s.advance(t.max(0.0), (total - VIEW_H).max(0.0));
            match self.imp {
                Impl::Pool => {
                    let local = self.inner.borrow_mut().set_offset(off);
                    app.global::<Bench>().set_scroll_y(local);
                }
                Impl::Native => {
                    self.native_final = off;
                    app.global::<Bench>().set_lv_y(-(off as f32));
                }
            }
        }
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        let imp = if self.imp == Impl::Pool { "pool" } else { "native" };
        ops.insert("impl".into(), imp.into());
        ops.insert("rows".into(), self.n.into());
        ops.insert("sum_heights".into(), sum_heights(self.n.min(100_000_000), self.dense).into());
        ops.insert("checksum_data".into(), format!("{:08x}", checksum_heights(self.n, self.dense)).into());
        ops.insert("model_build_ms".into(), self.model_build_ms.into());
        let (fo, rs) = match self.imp {
            Impl::Pool => {
                let i = self.inner.borrow();
                (i.final_offset, i.rows_set)
            }
            Impl::Native => (self.native_final, 0),
        };
        ops.insert("final_offset".into(), fo.into());
        ops.insert("rows_drawn".into(), rs.into());
        ops.insert("wheel_events".into(), self.wheel_events.get().into());
        match &self.extra {
            Extra::Reset(r) => {
                ops.insert("resets".into(), r.build_ms.len().into());
                ops.insert("rebuild_ms".into(), json!(r.build_ms));
                ops.insert("swap_ms".into(), json!(r.swap_ms));
                ops.insert("reset_on_ui_thread".into(), r.on_ui.into());
            }
            Extra::Heavy(_, c) => {
                ops.insert("heavy_calls".into(), c.load(Ordering::Relaxed).into());
            }
            Extra::None => {}
        }
        if self.renderer == "software" {
            ops.insert("not_rendered".into(), json!(software_not_rendered(&[])));
        }
        let _ = self.k;
        let _ = self.shell;
    }
    fn notes(&self) -> String {
        match self.imp {
            Impl::Pool => format!("impl=pool: ring of {} instantiated rows repositioned from Rust (exact positions from sparse prefix sums, step {}, origin block {} rows), VecModel::set_row_data on the rows that enter the window; the scroll container y = -(offset - origin) is one property per frame.", if self.dense { 40 } else { 32 }, STEP, ORIGIN_BLOCK),
            Impl::Native => "impl=native: std-widgets ListView (ScrollView) over a lazy slint::Model (row_data builds the row on demand, row_count = N, 0 bytes of per-row storage); content-y driven each frame by Bench.lv-y; the repeater estimates the height of non-instantiated rows by the instantiated average (i-slint-core model/repeater.rs update_visible_instances), so the offset in px is not exact for variable heights.".into(),
        }
    }
    fn stop(&mut self) {
        if let Extra::Heavy(s, _) = &self.extra {
            s.store(true, Ordering::Relaxed);
        }
    }
}

// ================================================================ L3 list-mutate
#[derive(Clone, Copy, Default)]
struct Op {
    kind: u8, // 0 valeur, 1 insertion, 2 suppression, 3 déplacement
    a: u32,
    b: u32,
    p: f64,
}
#[derive(Clone, Copy)]
struct Rec {
    id: u32,
    price: f64,
}
pub struct MutModel {
    recs: RefCell<Vec<Rec>>,
    notify: ModelNotify,
}
impl Model for MutModel {
    type Data = RowData;
    fn row_count(&self) -> usize {
        self.recs.borrow().len()
    }
    fn row_data(&self, row: usize) -> Option<RowData> {
        let r = *self.recs.borrow().get(row)?;
        Some(make_row(r.id, 0.0, r.price, false))
    }
    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}
impl MutModel {
    fn apply(&self, op: &Op, first_vis: usize, next_id: &mut u32) -> bool {
        let mut recs = self.recs.borrow_mut();
        let len = recs.len();
        if len == 0 {
            return false;
        }
        let fv = first_vis.min(len - 1);
        match op.kind {
            0 => {
                let idx = op.a as usize % len;
                recs[idx].price = op.p;
                drop(recs);
                self.notify.row_changed(idx);
            }
            1 => {
                let idx = fv + op.a as usize % (len - fv + 1);
                recs.insert(idx.min(len), Rec { id: *next_id, price: op.p });
                *next_id += 1;
                drop(recs);
                self.notify.row_added(idx.min(len), 1);
            }
            2 => {
                if len - fv < 2 {
                    return false;
                }
                let idx = fv + op.a as usize % (len - fv);
                recs.remove(idx);
                drop(recs);
                self.notify.row_removed(idx, 1);
            }
            _ => {
                if len - fv < 2 {
                    return false;
                }
                let from = fv + op.a as usize % (len - fv);
                let r = recs.remove(from);
                let to = fv + op.b as usize % (len - fv);
                recs.insert(to, r);
                drop(recs);
                self.notify.row_removed(from, 1);
                self.notify.row_added(to, 1);
            }
        }
        true
    }
}

pub struct MutateSc {
    model: Rc<MutModel>,
    q: Arc<Spsc<Op>>,
    stop: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
    gen0: u64,
    drop0: u64,
    applied: u64,
    applied0: u64,
    structural_applied: u64,
    next_id: u32,
    sc: Scroller,
    final_offset: f64,
    avg_h: f64,
    rate: u64,
    producer: Option<std::thread::JoinHandle<()>>,
    started: Arc<AtomicBool>,
    max_batch: usize,
    renderer: String,
    total_before: usize,
    model_build_ms: f64,
}

pub fn build_mutate(args: &Args, app: &MainWindow) -> Result<Box<dyn Scenario>, String> {
    let b = app.global::<Bench>();
    let rate = args.n_or(1000);
    let t_build = Instant::now();
    let recs: Vec<Rec> = (0..100_000u32).map(|i| Rec { id: i, price: initial_price(i) }).collect();
    let model = Rc::new(MutModel { recs: RefCell::new(recs), notify: ModelNotify::default() });
    b.set_chrome(true);
    b.set_lv_rows(ModelRc::from(model.clone() as Rc<dyn Model<Data = RowData>>));
    b.set_scn("native-list".into());
    let model_build_ms = t_build.elapsed().as_secs_f64() * 1000.0;
    let q = Spsc::<Op>::new(1 << 20); // file bornée à 1 Mi (spec : 1 M)
    let stop = Arc::new(AtomicBool::new(false));
    let started = Arc::new(AtomicBool::new(false));
    let generation = Arc::new(AtomicU64::new(0));
    let dropped = Arc::new(AtomicU64::new(0));
    let (q2, s2, st2, g2, d2) = (q.clone(), stop.clone(), started.clone(), generation.clone(), dropped.clone());
    let producer = std::thread::spawn(move || {
        // attend le début de la mesure : le producteur démarre quand `started` passe à true
        while !st2.load(Ordering::Acquire) {
            if s2.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let t0 = Instant::now();
        let mut x = SEED;
        let (mut vals, mut structs) = (0u64, 0u64);
        let srate = (rate / 100).max(1);
        while !s2.load(Ordering::Relaxed) {
            let el = t0.elapsed().as_nanos() as u64;
            let due_v = (el as u128 * rate as u128 / 1_000_000_000) as u64;
            let due_s = (el as u128 * srate as u128 / 1_000_000_000) as u64;
            let (mut ng, mut nd) = (0u64, 0u64);
            while vals < due_v {
                xorshift(&mut x);
                let op = Op { kind: 0, a: x % 100_000, b: 0, p: x as f64 / 100.0 };
                ng += 1;
                if !q2.push(op) {
                    nd += 1;
                }
                vals += 1;
            }
            while structs < due_s {
                let k = xorshift(&mut x);
                let a = xorshift(&mut x);
                let bb = xorshift(&mut x);
                let op = Op { kind: 1 + (k % 3) as u8, a, b: bb, p: k as f64 / 100.0 };
                ng += 1;
                if !q2.push(op) {
                    nd += 1;
                }
                structs += 1;
            }
            g2.fetch_add(ng, Ordering::Relaxed);
            d2.fetch_add(nd, Ordering::Relaxed);
            std::thread::sleep(Duration::from_micros(500));
        }
    });
    Ok(Box::new(MutateSc {
        model,
        q,
        stop,
        generation,
        dropped,
        gen0: 0,
        drop0: 0,
        applied: 0,
        applied0: 0,
        structural_applied: 0,
        next_id: 100_000,
        sc: Scroller::new(Script::constant(3000.0)),
        final_offset: 0.0,
        avg_h: 88.0,
        rate,
        producer: Some(producer),
        started,
        max_batch: 0,
        renderer: String::new(),
        total_before: 100_000,
        model_build_ms,
    }))
}

impl MutateSc {
    /// lot d'opérations appliqué à chaque frame (file lock-free drainée sur le fil UI)
    fn drain_ops(&mut self) {
        let fv = (self.final_offset / self.avg_h) as usize;
        let mut batch = 0usize;
        let model = self.model.clone();
        let (mut applied, mut st) = (0u64, 0u64);
        let nid = &mut self.next_id;
        self.q.drain(usize::MAX, |op| {
            if model.apply(&op, fv, nid) {
                applied += 1;
                if op.kind != 0 {
                    st += 1;
                }
            }
            batch += 1;
        });
        self.applied += applied;
        self.structural_applied += st;
        self.max_batch = self.max_batch.max(batch);
    }
}
impl Scenario for MutateSc {
    fn begin_measure(&mut self, app: &MainWindow) {
        self.sc.reset();
        app.global::<Bench>().set_lv_y(0.0);
        self.started.store(true, Ordering::Release);
    }
    fn step(&mut self, t: f64, app: &MainWindow, measuring: bool) {
        self.drain_ops();
        // 2. défilement 3 000 px/s
        let total = self.model.recs.borrow().len() as f64 * self.avg_h;
        let off = self.sc.advance(if measuring { t } else { 0.0 }, (total - VIEW_H).max(0.0));
        self.final_offset = off;
        app.global::<Bench>().set_lv_y(-(off as f32));
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        let generated = self.generation.load(Ordering::Relaxed);
        let dropped = self.dropped.load(Ordering::Relaxed);
        // opérations encore en file à la fin : appliquées au prochain lot, non perdues, mais non appliquées au décompte
        ops.insert("updates_generated".into(), (generated - self.gen0).into());
        ops.insert("updates_applied".into(), (self.applied - self.applied0).into());
        ops.insert("updates_dropped".into(), (dropped - self.drop0).into());
        ops.insert("structural_applied".into(), self.structural_applied.into());
        ops.insert("queue_left".into(), self.q.len().into());
        ops.insert("max_batch".into(), self.max_batch.into());
        ops.insert("rate_values_per_s".into(), self.rate.into());
        ops.insert("rate_structural_per_s".into(), (self.rate / 100).max(1).into());
        ops.insert("rows_final".into(), self.model.recs.borrow().len().into());
        ops.insert("final_offset".into(), self.final_offset.into());
        ops.insert("model_build_ms".into(), self.model_build_ms.into());
        ops.insert("sum_heights".into(), sum_heights(100_000, false).into());
        ops.insert("checksum_data".into(), format!("{:08x}", checksum_heights(100_000, false)).into());
        ops.insert("impl".into(), "native".into());
        let _ = (&self.renderer, self.total_before);
    }
    fn notes(&self) -> String {
        "list-mutate: std-widgets ListView on a custom slint::Model (Vec<Rec> + ModelNotify: row_changed for values, row_added/row_removed for insert/delete/move at an index >= the estimated first visible row). Producer = background thread writing into a bounded lock-free SPSC ring of 2^20 ops (drop + count when full), no Slint call off the UI thread; the queue is drained completely on the UI thread once per frame. Structural index = first_visible + draw % (len - first_visible), first_visible estimated as offset / 88 px. updates_applied counts ops applied to the model (value ops on any row, notified through row_changed whether or not the row is instantiated).".into()
    }
    fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.producer.take() {
            let _ = h.join();
        }
    }
}
