// Données, scripts et simulations du banc (indépendants du rendu).
use crate::{Args, ImgData, MainWindow, RowData, ShapeData};
use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::collections::VecDeque;
use std::fmt::Write;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

pub const N: usize = 100_000;
const POOL: usize = 32; // slots de lignes instanciées (anneau i % POOL)
const ORIGIN_BLOCK: usize = 16;

pub fn fnv(i: u32) -> u32 {
    let mut h: u32 = 2166136261;
    for b in i.to_le_bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    h
}
pub fn height(i: usize) -> u32 {
    48 + 16 * (fnv(i as u32) % 6)
}
pub fn sum_heights() -> u64 {
    (0..N).map(|i| height(i) as u64).sum()
}

// ---------------------------------------------------------------- core C-ABI
// Le cœur est lié comme crate rlib (dépendance path) : une seule libstd, aucun hack d'édition de
// liens. Les fonctions restent les `extern "C"` #[no_mangle] du cœur ; on les appelle par des
// pointeurs de fonction `unsafe extern "C" fn` opacifiés par black_box, pour que la LTO ne
// puisse pas les inliner dans la boucle : chaque appel est un vrai appel avec l'ABI C, comme
// depuis un autre langage.
type ValidateFn = unsafe extern "C" fn(*const u8, usize) -> i32;
type ParseFn = unsafe extern "C" fn(*const u8, usize, *mut u8, usize, *mut usize) -> i32;
type HashFn = unsafe extern "C" fn(*const u8, usize) -> u32;
type HeavyFn = unsafe extern "C" fn(u32) -> u64;

struct CoreSim {
    calls: Arc<AtomicU64>,
    heavy: Arc<AtomicU64>,
    acc: Arc<AtomicU64>,
    calls0: u64,
    heavy0: u64,
}
impl CoreSim {
    fn start() -> Self {
        let calls = Arc::new(AtomicU64::new(0));
        let heavy = Arc::new(AtomicU64::new(0));
        let acc = Arc::new(AtomicU64::new(0));
        let (c, a) = (calls.clone(), acc.clone());
        std::thread::spawn(move || {
            let core_validate: ValidateFn = std::hint::black_box(benchcore::core_validate as ValidateFn);
            let core_parse: ParseFn = std::hint::black_box(benchcore::core_parse as ParseFn);
            let core_hash: HashFn = std::hint::black_box(benchcore::core_hash as HashFn);
            let frame: [u8; 8] = [0xA5, 1, 5, b'h', b'e', b'l', b'l', b'o'];
            let mut out = [0u8; 64];
            let mut x = 0u64;
            loop {
                for _ in 0..1024 {
                    let mut ol = 0usize;
                    let v = unsafe { core_validate(std::hint::black_box(frame.as_ptr()), frame.len()) };
                    let p = unsafe { core_parse(frame.as_ptr(), frame.len(), out.as_mut_ptr(), out.len(), &mut ol) };
                    let h = unsafe { core_hash(out.as_ptr(), ol) };
                    x = x.wrapping_add(v as u64 + p as u64 + h as u64 + ol as u64);
                }
                c.fetch_add(1024, Ordering::Relaxed);
                a.store(x, Ordering::Relaxed);
            }
        });
        let h = heavy.clone();
        let core_heavy: HeavyFn = std::hint::black_box(benchcore::core_heavy as HeavyFn);
        std::thread::spawn(move || loop {
            let t = Instant::now();
            let r = unsafe { core_heavy(100) };
            std::hint::black_box(r);
            h.fetch_add(1, Ordering::Relaxed);
            let e = t.elapsed();
            if e < Duration::from_secs(1) {
                std::thread::sleep(Duration::from_secs(1) - e);
            }
        });
        CoreSim { calls, heavy, acc, calls0: 0, heavy0: 0 }
    }
}

// ---------------------------------------------------------------- scroll script
struct ScrollScript {
    pos: f64,
    last_t: f64,
}
impl ScrollScript {
    fn new() -> Self {
        ScrollScript { pos: 0.0, last_t: 0.0 }
    }
    // distance signée parcourue entre a et b (s)
    fn dist(a: f64, b: f64) -> f64 {
        let mut d = 0.0;
        let mut a = a;
        while a < b {
            if a < 4.0 {
                let e = b.min(4.0);
                d += 3000.0 * (e - a);
                a = e;
            } else {
                let k = ((a - 4.0) / 1.5).floor();
                let s = 4.0 + 1.5 * k;
                let e = b.min(s + 1.5);
                let dir = if (k as i64) % 2 == 0 { 1.0 } else { -1.0 };
                d += dir * 20000.0 / 2.0 * ((-2.0 * (a - s)).exp() - (-2.0 * (e - s)).exp());
                a = e;
            }
        }
        d
    }
    fn advance(&mut self, t: f64, max: f64) -> f64 {
        // clamp aux bornes à chaque pas
        self.pos = (self.pos + Self::dist(self.last_t, t)).clamp(0.0, max);
        self.last_t = t;
        self.pos
    }
}

// ---------------------------------------------------------------- list
struct ListSim {
    prefix: Vec<u32>,
    prices: Vec<f64>,
    model: Rc<VecModel<RowData>>,
    slots: [(usize, usize); POOL], // (idx, origin) ; idx usize::MAX = vide
    view_h: f32,
    script: ScrollScript,
    rows_drawn: u64,
    final_offset: f64,
    cur: (usize, usize),
}
impl ListSim {
    fn new(view_h: f32) -> Self {
        let mut prefix = Vec::with_capacity(N + 1);
        let mut s = 0u32;
        prefix.push(0);
        let mut prices = Vec::with_capacity(N);
        for i in 0..N {
            s += height(i);
            prefix.push(s);
            prices.push((fnv(i as u32 + 7) % 100000) as f64 / 100.0);
        }
        let blank = RowData { y: 0.0, h: 0.0, title: "".into(), sub: "".into(), price: "".into(), color: Color::from_argb_u8(0, 0, 0, 0) };
        let model = Rc::new(VecModel::from(vec![blank; POOL]));
        ListSim { prefix, prices, model, slots: [(usize::MAX, 0); POOL], view_h, script: ScrollScript::new(), rows_drawn: 0, final_offset: 0.0, cur: (0, 0) }
    }
    fn make_row(&self, i: usize, origin: usize) -> RowData {
        let f = fnv(i as u32);
        let q = format!("Quote {} \u{2014} {:.2}", i, (fnv(i as u32 + 7) % 100000) as f64 / 100.0);
        let mut sub = String::with_capacity(q.len() * 6 + 6);
        for _ in 0..6 {
            sub.push_str(&q);
            sub.push(' ');
        }
        RowData {
            y: (self.prefix[i] - self.prefix[origin]) as f32,
            h: height(i) as f32,
            title: format!("Item #{}", i).into(),
            sub: sub.into(),
            price: format!("{:.2}", self.prices[i]).into(),
            color: Color::from_rgb_u8(f as u8, (f >> 8) as u8, (f >> 16) as u8),
        }
    }
    /// positionne la fenêtre glissante pour un offset ; renvoie scroll-y local
    fn set_offset(&mut self, off: f64) -> f32 {
        let total = self.prefix[N] as f64;
        let off = off.clamp(0.0, (total - self.view_h as f64).max(0.0));
        self.final_offset = off;
        let offu = off as u32;
        let bot = (off + self.view_h as f64).ceil() as u32;
        let lo = self.prefix.partition_point(|&p| p <= offu).saturating_sub(1);
        let hi = self.prefix.partition_point(|&p| p < bot).saturating_sub(1).min(N - 1);
        let lo = lo.saturating_sub(1);
        let hi = (hi + 1).min(N - 1);
        let origin = lo - lo % ORIGIN_BLOCK;
        self.cur = (lo, hi);
        for slot in 0..POOL {
            let i = lo + (slot + POOL - lo % POOL) % POOL;
            let want = if i <= hi { (i, origin) } else { (usize::MAX, 0) };
            if self.slots[slot] != want {
                let row = if want.0 == usize::MAX {
                    RowData { y: 0.0, h: 0.0, title: "".into(), sub: "".into(), price: "".into(), color: Color::from_argb_u8(0, 0, 0, 0) }
                } else {
                    self.make_row(want.0, want.1)
                };
                self.model.set_row_data(slot, row);
                self.slots[slot] = want;
            }
        }
        (off as u32 - self.prefix[origin]) as f32 + (off.fract() as f32)
    }
    fn apply_price(&mut self, idx: usize, price: f64) -> bool {
        self.prices[idx] = price;
        let slot = idx % POOL;
        if self.slots[slot].0 == idx {
            let (i, o) = self.slots[slot];
            let mut r = self.model.row_data(slot).unwrap();
            r.price = SharedString::from(format!("{:.2}", self.prices[i]));
            let _ = o;
            self.model.set_row_data(slot, r);
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------- stream
struct Stream {
    rx: mpsc::Receiver<(u32, f64)>,
    generated: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
    applied: u64,
    gen0: u64,
    drop0: u64,
    applied0: u64,
}
impl Stream {
    fn start(rate: u32) -> Self {
        let (tx, rx) = mpsc::sync_channel::<(u32, f64)>(100_000);
        let generated = Arc::new(AtomicU64::new(0));
        let dropped = Arc::new(AtomicU64::new(0));
        let (g, d) = (generated.clone(), dropped.clone());
        std::thread::spawn(move || {
            let start = Instant::now();
            let mut x: u32 = 0x2545F491;
            let mut sent = 0u64;
            loop {
                let target = (start.elapsed().as_secs_f64() * rate as f64) as u64;
                let (mut ng, mut nd) = (0u64, 0u64);
                while sent < target {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    ng += 1;
                    match tx.try_send((x % 100000, x as f64 / 100.0)) {
                        Ok(()) => {}
                        Err(mpsc::TrySendError::Full(_)) => nd += 1,
                        Err(mpsc::TrySendError::Disconnected(_)) => return,
                    }
                    sent += 1;
                }
                g.fetch_add(ng, Ordering::Relaxed);
                d.fetch_add(nd, Ordering::Relaxed);
                std::thread::sleep(Duration::from_micros(500));
            }
        });
        Stream { rx, generated, dropped, applied: 0, gen0: 0, drop0: 0, applied0: 0 }
    }
}

// ---------------------------------------------------------------- canvas
struct CanvasSim {
    model: Rc<VecModel<ShapeData>>,
    base: Vec<ShapeData>,
    vel: Vec<(f32, f32)>,
    w: f32,
    h: f32,
    graph: Option<Graph>,
    sum: f64,
}
struct Graph {
    pts: VecDeque<f32>,
    frame: u32,
    buf: String,
}
fn reflect(p: f32, l: f32) -> f32 {
    let m = p.rem_euclid(2.0 * l);
    if m > l {
        2.0 * l - m
    } else {
        m
    }
}
impl CanvasSim {
    fn new(n: usize, w: f32, h: f32, graph: bool) -> Self {
        let mut base = Vec::with_capacity(n);
        let mut vel = Vec::with_capacity(n);
        for i in 0..n {
            let f = |k: u32| fnv((i as u32).wrapping_mul(7).wrapping_add(k));
            let size = 12.0 + (f(1) % 29) as f32;
            let kind = i % 3;
            let r = match kind {
                0 => 0.0,
                1 => size / 2.0,
                _ => size / 4.0,
            };
            let c = |v: u32| Color::from_rgb_u8(v as u8, (v >> 8) as u8, (v >> 16) as u8);
            base.push(ShapeData {
                x: (f(2) % 1000) as f32 / 1000.0 * (w - size),
                y: (f(3) % 1000) as f32 / 1000.0 * (h - size),
                w: size,
                r,
                c1: c(f(4)),
                c2: c(f(5)),
                shadow: i % 10 == 0,
            });
            let sp = |v: u32| ((v % 401) as f32 - 200.0) * 1.0;
            vel.push((sp(f(6)), sp(f(7))));
        }
        let model = Rc::new(VecModel::from(base.clone()));
        let g = if graph {
            let mut pts = VecDeque::with_capacity(10_001);
            for k in 0..10_000u32 {
                pts.push_back(gval(k));
            }
            Some(Graph { pts, frame: 10_000, buf: String::with_capacity(160_000) })
        } else {
            None
        };
        CanvasSim { model, base, vel, w, h, graph: g, sum: 0.0 }
    }
    fn step(&mut self, t: f32, app: &MainWindow) {
        let n = self.base.len();
        for i in 0..n {
            let mut s = self.base[i].clone();
            let (vx, vy) = self.vel[i];
            s.x = reflect(s.x + vx * t, self.w - s.w);
            s.y = reflect(s.y + vy * t, self.h - s.w);
            self.model.set_row_data(i, s);
        }
        if n > 0 {
            self.sum = self.model.row_data(n - 1).map(|s| (s.x + s.y) as f64).unwrap_or(0.0);
        }
        if let Some(g) = &mut self.graph {
            g.pts.pop_front();
            g.pts.push_back(gval(g.frame));
            g.frame = g.frame.wrapping_add(1);
            g.buf.clear();
            for (k, v) in g.pts.iter().enumerate() {
                let c = if k == 0 { 'M' } else { 'L' };
                let _ = write!(g.buf, "{}{}.{} {:.1} ", c, k / 10, k % 10, v);
            }
            app.set_graph(SharedString::from(g.buf.as_str()));
        }
    }
}
fn gval(k: u32) -> f32 {
    50.0 + 35.0 * ((k as f32) * 0.02).sin() + 10.0 * (((fnv(k) % 1000) as f32) / 1000.0 - 0.5)
}

// ---------------------------------------------------------------- scenario
pub struct Scenario {
    kind: String,
    list: Option<ListSim>,
    stream: Option<Stream>,
    canvas: Option<CanvasSim>,
    core: Option<CoreSim>,
    anim: bool,
    max_stall: f64,
    last_step: Option<Instant>,
    rate: u32,
    shapes: usize,
    measure_start: Option<Instant>,
    text: Option<TextSim>,
    images: Option<ImagesSim>,
    nav: Option<NavSim>,
    key_events: Rc<std::cell::Cell<u64>>,
    pub step_ms_sum: f64,
    pub step_n: u64,
    pub draw_ms_sum: f64,
    pub draw_n: u64,
}

// ---------------------------------------------------------------- text
struct TextSim {
    text: Option<String>,
    set_at: Option<Instant>,
    first_display_ms: f64,
    layout_ms: f64,
    set_ms: f64,
    pos: f64,
    selected: bool,
    max: f64,
    sel_done: bool,
}
// ---------------------------------------------------------------- images
thread_local! {
    static IMG_MODEL: std::cell::RefCell<Option<Rc<VecModel<ImgData>>>> = const { std::cell::RefCell::new(None) };
}
struct ImagesSim {
    decoded: Arc<AtomicU64>,
    decode_us: Arc<AtomicU64>,
    started: bool,
    dir: String,
}
fn decode_all(dir: String, weak: slint::Weak<MainWindow>, decoded: Arc<AtomicU64>, decode_us: Arc<AtomicU64>) {
    let next = Arc::new(AtomicU64::new(0));
    for _ in 0..2 {
        let (dir, weak, decoded, decode_us, next) = (dir.clone(), weak.clone(), decoded.clone(), decode_us.clone(), next.clone());
        std::thread::spawn(move || loop {
            let i = next.fetch_add(1, Ordering::Relaxed) as usize;
            if i >= 600 {
                break;
            }
            let t = Instant::now();
            let path = format!("{}/images/img{:03}.png", dir, i);
            // slint::Image n'est pas Send et Image::load_from_path remplit un cache local au
            // thread : la doc recommande de décoder hors thread UI avec la crate `image`, puis de
            // transférer un SharedPixelBuffer (Send) et de créer l'Image dans le thread UI.
            let Ok(img) = image::open(&path) else { continue };
            let buf = img.into_rgba8();
            // réduction 256 -> 128 (moyenne 2x2)
            let (w, h) = (buf.width() as usize, buf.height() as usize);
            let (tw, th) = (128usize, 128usize);
            let mut out = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(tw as u32, th as u32);
            {
                let src = buf.as_raw();
                let dst = out.make_mut_slice();
                let (fx, fy) = (w / tw, h / th);
                for y in 0..th {
                    for x in 0..tw {
                        let (mut r, mut g, mut b, mut a, mut n) = (0u32, 0u32, 0u32, 0u32, 0u32);
                        for dy in 0..fy {
                            for dx in 0..fx {
                                let o = ((y * fy + dy) * w + x * fx + dx) * 4;
                                r += src[o] as u32;
                                g += src[o + 1] as u32;
                                b += src[o + 2] as u32;
                                a += src[o + 3] as u32;
                                n += 1;
                            }
                        }
                        dst[y * tw + x] = slint::Rgba8Pixel { r: (r / n) as u8, g: (g / n) as u8, b: (b / n) as u8, a: (a / n) as u8 };
                    }
                }
            }
            decode_us.fetch_add(t.elapsed().as_micros() as u64, Ordering::Relaxed);
            decoded.fetch_add(1, Ordering::Relaxed);
            let _ = weak.upgrade_in_event_loop(move |_app| {
                IMG_MODEL.with(|m| {
                    if let Some(m) = m.borrow().as_ref() {
                        m.set_row_data(i, ImgData { img: slint::Image::from_rgba8(out), loaded: true });
                    }
                });
            });
        });
    }
}
// ---------------------------------------------------------------- nav
struct NavSim {
    anim_until: f64,
    step: i64,
    done: u64,
    screen: i32,
}
fn spawn_burner() {
    std::thread::spawn(|| {
        let mut x = 1u64;
        loop {
            for _ in 0..100_000 {
                x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407));
            }
        }
    });
}

impl Scenario {
    pub fn new(a: &Args, app: &MainWindow) -> Self {
        let kind = a.scenario.clone();
        let mut sc = Scenario { kind: kind.clone(), list: None, stream: None, canvas: None, core: None, anim: true, max_stall: 0.0, last_step: None, rate: a.rate, shapes: a.shapes, measure_start: None, text: None, images: None, nav: None, key_events: Rc::new(std::cell::Cell::new(0)), step_ms_sum: 0.0, step_n: 0, draw_ms_sum: 0.0, draw_n: 0 };
        match kind.as_str() {
            "idle" | "list" | "stream" | "input" => {
                let mut l = ListSim::new(660.0);
                app.set_chrome(true);
                app.set_show_list(true);
                app.set_list_w(1280.0);
                let sy = l.set_offset(0.0);
                app.set_scroll_y(sy);
                app.set_rows(ModelRc::from(l.model.clone()));
                sc.list = Some(l);
                sc.anim = kind != "idle";
                if kind == "stream" {
                    sc.rate = if a.rate == 0 { 1000 } else { a.rate };
                    sc.stream = Some(Stream::start(sc.rate));
                }
                if kind == "input" {
                    app.set_show_square(true);
                    app.invoke_focus_keys();
                    let k = sc.key_events.clone();
                    app.on_space_pressed(move || k.set(k.get() + 1));
                }
            }
            "canvas" => {
                let n = if a.shapes == 0 { 5000 } else { a.shapes };
                sc.shapes = n;
                app.set_chrome(false);
                app.set_show_list(false);
                app.set_show_canvas(true);
                app.set_show_graph(true);
                let c = CanvasSim::new(n, 1000.0, 600.0, true);
                app.set_shapes(ModelRc::from(c.model.clone()));
                sc.canvas = Some(c);
            }
            "core" => {
                let n = 1000;
                sc.shapes = n;
                app.set_chrome(false);
                app.set_show_list(false);
                app.set_show_canvas(true);
                let c = CanvasSim::new(n, 1000.0, 600.0, false);
                app.set_shapes(ModelRc::from(c.model.clone()));
                sc.canvas = Some(c);
                sc.core = Some(CoreSim::start());
            }
            "stress" => {
                let n = if a.shapes == 0 { 5000 } else { a.shapes };
                sc.shapes = n;
                sc.rate = if a.rate == 0 { 5000 } else { a.rate };
                app.set_chrome(false);
                app.set_show_list(true);
                app.set_list_w(880.0);
                app.set_show_canvas(true);
                app.set_canvas_x(880.0);
                app.set_canvas_y(0.0);
                app.set_canvas_w(400.0);
                app.set_canvas_h(720.0);
                let mut l = ListSim::new(720.0);
                let sy = l.set_offset(0.0);
                app.set_scroll_y(sy);
                app.set_rows(ModelRc::from(l.model.clone()));
                sc.list = Some(l);
                let c = CanvasSim::new(n, 400.0, 720.0, false);
                app.set_shapes(ModelRc::from(c.model.clone()));
                sc.canvas = Some(c);
                sc.stream = Some(Stream::start(sc.rate));
                sc.core = Some(CoreSim::start());
            }
            "text" => {
                let txt = std::fs::read_to_string(format!("{}/text1mb.txt", a.assets)).expect("text1mb.txt");
                app.set_chrome(true);
                app.set_show_list(false);
                app.set_show_text(true);
                sc.text = Some(TextSim { text: Some(txt), set_at: None, first_display_ms: -1.0, layout_ms: -1.0, set_ms: 0.0, pos: 0.0, selected: false, max: 0.0, sel_done: false });
            }
            "images" => {
                app.set_chrome(true);
                app.set_show_list(false);
                app.set_show_images(true);
                let m = Rc::new(VecModel::from(vec![ImgData { img: Default::default(), loaded: false }; 600]));
                IMG_MODEL.with(|c| *c.borrow_mut() = Some(m.clone()));
                app.set_images(ModelRc::from(m));
                sc.images = Some(ImagesSim { decoded: Arc::new(AtomicU64::new(0)), decode_us: Arc::new(AtomicU64::new(0)), started: false, dir: a.assets.clone() });
            }
            "nav" => {
                app.set_chrome(true);
                app.set_show_list(false);
                app.set_show_nav(true);
                sc.nav = Some(NavSim { anim_until: 0.0, step: -1, done: 0, screen: 0 });
                spawn_burner();
            }
            _ => {
                eprintln!("scenario non supporté: {}", kind);
                sc.anim = false;
            }
        }
        sc
    }
    pub fn animated(&self) -> bool {
        self.anim
    }
    /// Redessin continu demandé ? (nav : seulement tant qu'une animation est active, sinon
    /// on compterait des « frames » vides sans changement de contenu)
    pub fn wants_redraw(&self, t: f64, measuring: bool) -> bool {
        if let Some(n) = &self.nav {
            return measuring && t < n.anim_until;
        }
        self.anim
    }
    pub fn has_core(&self) -> bool {
        self.core.is_some()
    }
    pub fn begin_measure(&mut self, app: &MainWindow) {
        self.key_events.set(0);
        if self.kind == "input" {
            app.invoke_focus_keys(); // la fenêtre est affichée : (re)donner le focus clavier au FocusScope
        }
        if let Some(l) = &mut self.list {
            l.script = ScrollScript::new();
            l.rows_drawn = 0;
        }
        if let Some(s) = &mut self.stream {
            s.gen0 = s.generated.load(Ordering::Relaxed);
            s.drop0 = s.dropped.load(Ordering::Relaxed);
            s.applied0 = s.applied;
        }
        if let Some(c) = &mut self.core {
            c.calls0 = c.calls.load(Ordering::Relaxed);
            c.heavy0 = c.heavy.load(Ordering::Relaxed);
        }
        self.max_stall = 0.0;
        self.last_step = None;
        self.measure_start = Some(Instant::now());
    }
    pub fn step(&mut self, app: &MainWindow, t: f64, measuring: bool) {
        let now = Instant::now();
        self.step_inner(app, t, measuring, now);
        if measuring {
            self.step_ms_sum += now.elapsed().as_secs_f64() * 1000.0;
            self.step_n += 1;
        }
    }
    fn step_inner(&mut self, app: &MainWindow, t: f64, measuring: bool, now: Instant) {
        if measuring {
            if let Some(l) = self.last_step {
                self.max_stall = self.max_stall.max(now.duration_since(l).as_secs_f64() * 1000.0);
            }
            self.last_step = Some(now);
        }
        // flux : appliquer le lot de mises à jour
        let mut dirty = false;
        if let (Some(s), Some(l)) = (&mut self.stream, &mut self.list) {
            for (idx, price) in s.rx.try_iter() {
                s.applied += 1;
                if l.apply_price(idx as usize, price) {
                    dirty = true;
                }
            }
        }
        let _ = dirty;
        if let Some(l) = &mut self.list {
            if self.kind != "idle" {
                let max = l.prefix[N] as f64 - l.view_h as f64;
                let off = l.script.advance(t, max);
                let sy = l.set_offset(off);
                app.set_scroll_y(sy);
            }
        }
        if let Some(c) = &mut self.canvas {
            c.step(t as f32, app);
        }
        if let Some(x) = &mut self.text {
            if x.set_at.is_none() {
                x.set_at = Some(Instant::now());
                app.set_big_text(SharedString::from(x.text.take().unwrap()));
                x.set_ms = x.set_at.unwrap().elapsed().as_secs_f64() * 1000.0;
            } else if x.first_display_ms >= 0.0 {
                if x.max == 0.0 {
                    x.max = (app.get_text_content_h() - app.get_text_visible_h()).max(1.0) as f64;
                }
                if measuring {
                    // aller-retour continu 3000 px/s
                    let d = (3000.0 * t) % (2.0 * x.max);
                    x.pos = if d > x.max { 2.0 * x.max - d } else { d };
                    app.set_text_y(-(x.pos as f32));
                    if t >= 5.0 && !x.sel_done {
                        x.sel_done = true;
                        app.invoke_select_all();
                        x.selected = true;
                    }
                }
            }
        }
        if let Some(im) = &mut self.images {
            if !im.started {
                im.started = true;
                decode_all(im.dir.clone(), app.as_weak(), im.decoded.clone(), im.decode_us.clone());
            }
            if measuring {
                let cycle = (t % 6.0) / 3.0; // 0..2
                let z = 1.0 + 2.0 * (if cycle <= 1.0 { cycle } else { 2.0 - cycle });
                let cell = 136.0 * z;
                let total = (600.0f64 / 9.0).ceil() * cell;
                let maxy = (total - 660.0).max(1.0);
                let d = (3000.0 * t) % (2.0 * maxy);
                let y = if d > maxy { 2.0 * maxy - d } else { d };
                let maxx = (9.0 * cell - 1280.0).max(0.0);
                let x = maxx * (0.5 + 0.5 * (2.0 * std::f64::consts::PI * t / 6.0).sin());
                app.set_zoom(z as f32);
                app.set_pan_x(x as f32);
                app.set_pan_y(y as f32);
            }
        }
        if let Some(n) = &mut self.nav {
            if measuring {
                let k = (t / 0.3).floor() as i64;
                while n.step < k {
                    n.step += 1;
                    if n.step > 0 {
                        n.done += 1; // la transition précédente est terminée
                    }
                    // cycle de 6 : push, push, modal open, modal close, pop, pop
                    let slot_start = n.step as f64 * 0.3;
                    n.anim_until = slot_start + if n.step % 6 == 2 || n.step % 6 == 3 { 0.2 } else { 0.3 } + 0.02;
                    match n.step % 6 {
                        0 => { n.screen = 1; app.set_screen(1); }
                        1 => { n.screen = 2; app.set_screen(2); }
                        2 => app.set_modal(true),
                        3 => app.set_modal(false),
                        4 => { n.screen = 1; app.set_screen(1); }
                        _ => { n.screen = 0; app.set_screen(0); }
                    }
                }
            }
        }
    }
    pub fn on_frame_end(&mut self, now: Instant, draw: Duration) {
        if self.measure_start.is_some() {
            self.draw_ms_sum += draw.as_secs_f64() * 1000.0;
            self.draw_n += 1;
            if let Some(l) = &mut self.list {
                // lignes de la fenêtre virtualisée (visibles + 1 de marge de chaque côté) de cette frame
                l.rows_drawn += (l.cur.1 - l.cur.0 + 1) as u64;
            }
        }
        if let Some(x) = &mut self.text {
            if x.first_display_ms < 0.0 {
                if let Some(s) = x.set_at {
                    x.first_display_ms = now.duration_since(s).as_secs_f64() * 1000.0;
                    x.layout_ms = x.first_display_ms - draw.as_secs_f64() * 1000.0;
                }
            }
        }
    }
    pub fn ops(&self, t: f64, renderer: &str) -> serde_json::Value {
        let mut o = serde_json::json!({ "sum_heights": sum_heights() });
        let m = o.as_object_mut().unwrap();
        let sw = renderer == "software";
        let mut nr: Vec<&str> = vec![];
        if self.canvas.is_some() && sw {
            // limites documentées du moteur logiciel de Slint (backends_and_renderers) + code 1.18.1
            nr.extend(["drop_shadow", "linear_gradient_with_border_radius (circles and rounded rects drawn as square gradients)", "rounded_clip (clip of the rounded panel is rectangular)"]);
        }
        m.insert("not_rendered".into(), nr.into());
        if self.kind == "input" {
            m.insert("key_events".into(), self.key_events.get().into());
        }
        m.insert("app_step_ms_mean".into(), (self.step_ms_sum / self.step_n.max(1) as f64).into());
        m.insert("draw_ms_mean".into(), (self.draw_ms_sum / self.draw_n.max(1) as f64).into());
        if let Some(l) = &self.list {
            m.insert("final_offset".into(), l.final_offset.into());
            m.insert("rows_drawn".into(), l.rows_drawn.into());
            m.insert("pool_rows".into(), (POOL as u64).into());
        }
        if let Some(s) = &self.stream {
            m.insert("rate".into(), self.rate.into());
            m.insert("updates_generated".into(), (s.generated.load(Ordering::Relaxed) - s.gen0).into());
            m.insert("updates_applied".into(), (s.applied - s.applied0).into());
            m.insert("updates_dropped".into(), (s.dropped.load(Ordering::Relaxed) - s.drop0).into());
        }
        if self.canvas.is_some() {
            m.insert("shapes".into(), self.shapes.into());
            m.insert("graph_points".into(), (if self.kind == "canvas" { 10000 } else { 0 }).into());
            m.insert("shadows_rendered".into(), (!sw).into());
        }
        if let Some(x) = &self.text {
            m.insert("time_to_first_display_ms".into(), x.first_display_ms.into());
            m.insert("layout_ms".into(), x.layout_ms.into());
            m.insert("set_text_ms".into(), x.set_ms.into());
            m.insert("select_all_called".into(), x.selected.into());
            m.insert("max_scroll_px".into(), x.max.into());
        }
        if let Some(i) = &self.images {
            m.insert("images_decoded".into(), i.decoded.load(Ordering::Relaxed).into());
            m.insert("decode_total_ms".into(), (i.decode_us.load(Ordering::Relaxed) as f64 / 1000.0).into());
            m.insert("decode_threads".into(), 2.into());
        }
        if let Some(n) = &self.nav {
            m.insert("transitions_done".into(), n.done.into());
        }
        if let Some(c) = &self.core {
            let calls = c.calls.load(Ordering::Relaxed) - c.calls0;
            m.insert("calls".into(), calls.into());
            m.insert("calls_per_s".into(), ((calls as f64) / t).into());
            m.insert("heavy_calls".into(), (c.heavy.load(Ordering::Relaxed) - c.heavy0).into());
            m.insert("max_frame_stall_ms".into(), self.max_stall.into());
        }
        o
    }
    pub fn checksum(&self) -> String {
        let mut h = 0u32;
        if let Some(l) = &self.list {
            h ^= fnv(l.final_offset as u32);
        }
        if let Some(c) = &self.canvas {
            h ^= fnv(c.sum as u32);
        }
        if let Some(c) = &self.core {
            h ^= fnv(c.acc.load(Ordering::Relaxed) as u32);
        }
        format!("{:08x}", h)
    }
    pub fn notes(&self, renderer: &str) -> String {
        let sw = renderer == "software";
        match self.kind.as_str() {
            "idle" => "idle".into(),
            "text" => "TextEdit (std-widgets, fluent style) read-only, word-wrap, content-y driven by script; layout_ms = time_to_first_display minus the first draw (RedrawRequested->frame end) = set_text + lazy layout before draw; time_to_first_display_ms = text set -> end of first frame".into(),
            "images" => "grid 9 columns of 600 Rectangle+Image elements (all instantiated; the renderer culls off-screen items); decode on 2 worker threads (image crate PNG decode + 2x2 box downscale to 128px, SharedPixelBuffer sent with Weak::upgrade_in_event_loop, Image created on the UI thread); zoom implemented by resizing cells (portable: the software renderer has no scale transform); scroll 3000px/s and zoom 1-3-1 over 6s run simultaneously".into(),
            "input" => "list scenario + 200x200 square at (100,100) on top; a FocusScope key-pressed handler in .slint toggles the square colour black/white on each Space (normal Slint/winit key event path); key_events counts Space presses during measurement; redraw is continuous (list scrolling) so the change is shown on the next frame".into(),
            "nav" => "3 screens as Rectangles whose x has animate 300ms ease-in-out; modal opacity animate 200ms; sequence cycle push,push,modal-open,modal-close,pop,pop at one step per 300ms; transitions_done counts steps whose animation window elapsed; one background thread burns a core".into(),
            "canvas" | "core" | "stress" => format!("canvas = one Slint Rectangle element per shape (no immediate-mode canvas API in Slint; repeater over VecModel, all rows set_row_data each frame), @linear-gradient background, drop-shadow-* on 1/10; graph = Path element, commands string rebuilt each frame (10000 pts); {}", if sw { "software renderer: drop-shadow not drawn (draw_box_shadow is a TODO), gradient + border-radius drawn square, clip ignores border-radius (all documented limits)" } else { "all primitives rendered by this renderer" }),
            _ => "virtualised list: ring of 32 Row elements fed by a VecModel, window computed from prefix-sum heights (binary search); offset applied by a single scroll-y property".into(),
        }
    }
}
