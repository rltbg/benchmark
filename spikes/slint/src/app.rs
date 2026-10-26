use crate::{core, plat};
use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

slint::include_modules!();

const N_ROWS: usize = 100_000;
const SLOTS: usize = 64;
const N_SHAPES: usize = 2000;
const SEED: u32 = 0x2545_F491;

fn xorshift32(x: &mut u32) -> u32 {
    *x ^= *x << 13;
    *x ^= *x >> 17;
    *x ^= *x << 5;
    *x
}

fn row_color(i: usize) -> Color {
    let h = core::fnv_u32(i as u32 ^ 0x9E37_79B9);
    Color::from_rgb_u8(80 + (h & 0x7F) as u8, 80 + ((h >> 8) & 0x7F) as u8, 80 + ((h >> 16) & 0x7F) as u8)
}

// ---------- liste virtualisée ----------

struct ListState {
    prefix: Vec<u32>,       // prefix[i] = ordonnée du haut de la ligne i ; prefix[N] = hauteur totale
    prices: Vec<f64>,       // prix courant de chaque ligne
    slot_idx: [i64; SLOTS], // ligne affichée dans chaque slot (anneau i % SLOTS), -1 si vide
    rows: Rc<VecModel<RowData>>,
    last_offset: f32,
    last_view_h: f32,
    first: usize,
    last: usize,
}

impl ListState {
    fn new() -> Self {
        let mut prefix = Vec::with_capacity(N_ROWS + 1);
        let mut y = 0u32;
        let mut prices = Vec::with_capacity(N_ROWS);
        for i in 0..N_ROWS {
            prefix.push(y);
            let h = core::fnv_u32(i as u32);
            y += 48 + 16 * (h % 6);
            prices.push((core::fnv_u32(i as u32 ^ 0x5555_5555) % 100_000) as f64 / 100.0);
        }
        prefix.push(y);
        let empty = RowData { y: 0.0, h: 0.0, title: SharedString::new(), price: SharedString::new(), color: Color::from_argb_u8(0, 0, 0, 0) };
        ListState {
            prefix,
            prices,
            slot_idx: [-1; SLOTS],
            rows: Rc::new(VecModel::from(vec![empty; SLOTS])),
            last_offset: f32::NAN,
            last_view_h: f32::NAN,
            first: 0,
            last: 0,
        }
    }
    fn height(&self, i: usize) -> u32 {
        self.prefix[i + 1] - self.prefix[i]
    }
    fn row_data(&self, i: usize) -> RowData {
        RowData {
            y: self.prefix[i] as f32,
            h: self.height(i) as f32,
            title: format!("Item #{i}").into(),
            price: format!("{:.2}", self.prices[i]).into(),
            color: row_color(i),
        }
    }
    /// Recalcule les lignes visibles pour (décalage, hauteur de vue) en pixels logiques.
    fn refresh(&mut self, offset: f32, view_h: f32) {
        if offset == self.last_offset && view_h == self.last_view_h {
            return;
        }
        self.last_offset = offset;
        self.last_view_h = view_h;
        let offset = offset.max(0.0);
        let lo = self.prefix.partition_point(|&p| (p as f32) <= offset).saturating_sub(1).min(N_ROWS - 1);
        let hi = self.prefix.partition_point(|&p| (p as f32) < offset + view_h).min(N_ROWS - 1);
        let first = lo.saturating_sub(1);
        let last = (hi + 1).min(N_ROWS - 1).min(first + SLOTS - 1);
        for i in first..=last {
            let s = i % SLOTS;
            if self.slot_idx[s] != i as i64 {
                self.slot_idx[s] = i as i64;
                self.rows.set_row_data(s, self.row_data(i));
            }
        }
        for s in 0..SLOTS {
            let i = self.slot_idx[s];
            if i >= 0 && ((i as usize) < first || (i as usize) > last) {
                self.slot_idx[s] = -1;
                self.rows.set_row_data(s, RowData { y: 0.0, h: 0.0, ..self.rows.row_data(s).unwrap() });
            }
        }
        self.first = first;
        self.last = last;
    }
    fn set_price(&mut self, i: usize, p: f64) {
        self.prices[i] = p;
        let s = i % SLOTS;
        if self.slot_idx[s] == i as i64 {
            let mut d = self.rows.row_data(s).unwrap();
            d.price = format!("{p:.2}").into();
            self.rows.set_row_data(s, d);
        }
    }
}

// ---------- mises à jour de prix : 1 000 / s ----------

/// Tâche de fond : xorshift32 (graine 0x2545F491), index = x % 100000, prix = x / 100.0, 1 000 par seconde.
/// Natif : vrai fil. Web (wasm sans fil) : produit les mises à jour dues à chaque tick de l'UI (voir NOTES.md).
struct PriceFeed {
    queue: Arc<Mutex<Vec<(u32, f64)>>>,
    #[cfg(target_family = "wasm")]
    state: RefCell<(u32, f64, u64)>, // xorshift, t0, produites
}

impl PriceFeed {
    #[cfg(not(target_family = "wasm"))]
    fn start() -> PriceFeed {
        let queue = Arc::new(Mutex::new(Vec::<(u32, f64)>::new()));
        let q = queue.clone();
        std::thread::spawn(move || {
            let t0 = std::time::Instant::now();
            let mut x = SEED;
            let mut produced: u64 = 0;
            loop {
                let due = t0.elapsed().as_millis() as u64;
                if due > produced {
                    let mut batch = Vec::with_capacity((due - produced) as usize);
                    while produced < due {
                        let v = xorshift32(&mut x);
                        batch.push((v % N_ROWS as u32, v as f64 / 100.0));
                        produced += 1;
                    }
                    let mut g = q.lock().unwrap();
                    if g.len() < 100_000 {
                        g.extend(batch);
                    }
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        PriceFeed { queue }
    }
    #[cfg(target_family = "wasm")]
    fn start() -> PriceFeed {
        PriceFeed { queue: Arc::new(Mutex::new(Vec::new())), state: RefCell::new((SEED, plat::now_ms(), 0)) }
    }
    fn drain(&self) -> Vec<(u32, f64)> {
        #[cfg(target_family = "wasm")]
        {
            let mut st = self.state.borrow_mut();
            let due = (plat::now_ms() - st.1) as u64;
            let mut out = Vec::new();
            while st.2 < due {
                let v = xorshift32(&mut st.0);
                out.push((v % N_ROWS as u32, v as f64 / 100.0));
                st.2 += 1;
            }
            return out;
        }
        #[cfg(not(target_family = "wasm"))]
        std::mem::take(&mut *self.queue.lock().unwrap())
    }
}

// ---------- formes ----------

fn make_shapes() -> Vec<ShapeData> {
    let mut x = 0x1234_5678u32;
    let mut rnd = move || xorshift32(&mut x) as f32 / u32::MAX as f32;
    let pal = |r: f32| Color::from_rgb_u8((60.0 + 195.0 * r) as u8, (60.0 + 195.0 * ((r * 7.0) % 1.0)) as u8, (60.0 + 195.0 * ((r * 13.0) % 1.0)) as u8);
    (0..N_SHAPES)
        .map(|i| ShapeData {
            x0: rnd() * 2000.0,
            y0: rnd() * 2000.0,
            vx: 40.0 + rnd() * 220.0,
            vy: 40.0 + rnd() * 220.0,
            w: 12.0 + rnd() * 28.0,
            kind: (i % 3) as i32, // 1/3 rectangles, 1/3 cercles, 1/3 rectangles arrondis
            c1: pal(rnd()),
            c2: pal(rnd()),
        })
        .collect()
}

// ---------- application ----------

struct Fps {
    frames: Rc<Cell<u64>>, // incrémenté par le notifier de rendu, sinon par le tick
    from_notifier: bool,
    last_count: u64,
    last_ms: f64,
}

pub fn run() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    let autorun = plat::autorun_requested();

    // cœur + autotest
    let selftest = core::selftest();
    let banner = |st: &Result<(), String>| {
        format!("slint · {} · cœur={} · autotest={}", plat::platform(), core::NAME, if st.is_ok() { "OK" } else { "ÉCHEC" })
    };
    ui.set_banner(banner(&selftest).into());
    ui.set_mobile(plat::is_mobile());

    // onglet 1
    let list = Rc::new(RefCell::new(ListState::new()));
    ui.set_total_h(list.borrow().prefix[N_ROWS] as f32);
    ui.set_rows(ModelRc::from(list.borrow().rows.clone()));
    let feed = PriceFeed::start();

    // onglet 2
    ui.set_shapes(ModelRc::from(Rc::new(VecModel::from(make_shapes()))));

    // onglet 3
    let weak = ui.as_weak();
    let process = {
        let weak = weak.clone();
        move |text: &str| {
            let Some(ui) = weak.upgrade() else { return };
            let f = core::frame(text);
            let valid = core::validate(&f);
            let payload = core::parse(&f);
            ui.set_frame_hex(f.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ").into());
            ui.set_res_validate(valid.to_string().into());
            ui.set_res_payload(match &payload {
                Some(p) => format!("{:?} ({} o)", String::from_utf8_lossy(p), p.len()),
                None => "(parse a échoué)".to_string(),
            }
            .into());
            ui.set_res_hash(format!("0x{:08X}", core::hash(&f)).into());
        }
    };
    process("");
    {
        let process = process.clone();
        ui.on_edited(move |t| process(t.as_str()));
    }

    // charge 100 ms : fil de fond (natif) ; en wasm (pas de fil) elle bloque le fil principal
    let heavy_gap = Arc::new(Mutex::new((false, 0.0f64))); // (en cours, plus grand intervalle entre ticks)
    let start_heavy = {
        let weak = weak.clone();
        let heavy_gap = heavy_gap.clone();
        move || {
            let Some(ui) = weak.upgrade() else { return };
            if ui.get_heavy_running() {
                return;
            }
            ui.set_heavy_running(true);
            ui.set_heavy_status("charge en cours…".into());
            *heavy_gap.lock().unwrap() = (true, 0.0);
            plat::log("SPIKE:heavy start");
            let t0 = plat::now_ms();
            let finish = {
                let heavy_gap = heavy_gap.clone();
                move |ui: MainWindow, sum: u64| {
                    let gap = {
                        let mut g = heavy_gap.lock().unwrap();
                        g.0 = false;
                        g.1
                    };
                    let el = plat::now_ms() - t0;
                    ui.set_heavy_running(false);
                    ui.set_heavy_status(format!("terminée en {el:.0} ms (somme {sum:016x}) ; plus grand intervalle entre ticks UI : {gap:.0} ms").into());
                    plat::log(&format!("SPIKE:heavy done elapsed_ms={el:.0} max_ui_gap_ms={gap:.0}"));
                }
            };
            #[cfg(not(target_family = "wasm"))]
            {
                let w2 = weak.clone();
                std::thread::spawn(move || {
                    let sum = core::heavy(100);
                    let _ = w2.upgrade_in_event_loop(move |ui| finish(ui, sum));
                });
            }
            #[cfg(target_family = "wasm")]
            {
                let sum = core::heavy(100);
                finish(ui, sum);
            }
        }
    };
    {
        let start_heavy = start_heavy.clone();
        ui.on_heavy_clicked(move || start_heavy());
    }

    // onglets
    let tab_changed = {
        let weak = weak.clone();
        move |n: i32| {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_tab(n);
            plat::log(&format!("SPIKE:tab index={} name={}", n + 1, ["list", "canvas", "core"][n as usize]));
        }
    };
    {
        let tc = tab_changed.clone();
        ui.on_tab_changed(move |n| tc(n));
    }

    // FPS : notifier de rendu si le renderer le permet, sinon comptage des ticks
    let frames = Rc::new(Cell::new(0u64));
    let first_frame = Rc::new(Cell::new(false));
    let from_notifier = {
        let frames = frames.clone();
        let first_frame = first_frame.clone();
        ui.window()
            .set_rendering_notifier(move |state, _| {
                if matches!(state, slint::RenderingState::AfterRendering) {
                    frames.set(frames.get() + 1);
                    first_frame.set(true);
                }
            })
            .is_ok()
    };
    let fps_state = Rc::new(RefCell::new(Fps { frames: frames.clone(), from_notifier, last_count: 0, last_ms: plat::now_ms() }));

    // minuterie principale (~60 Hz) : animation, prix, liste, FPS, parcours automatique
    let t_start = plat::now_ms();
    let ready_logged = Rc::new(Cell::new(false));
    let notifier_dead = Rc::new(Cell::new(false));
    let auto_t0 = Rc::new(Cell::new(f64::NAN));
    let auto_step = Rc::new(Cell::new(0usize));
    let done_at = Rc::new(Cell::new(f64::NAN));
    let last_tick = Rc::new(Cell::new(plat::now_ms()));
    let timer = Timer::default();
    {
        let weak = weak.clone();
        let list = list.clone();
        let heavy_gap = heavy_gap.clone();
        let selftest = selftest.clone();
        let process = process.clone();
        let start_heavy = start_heavy.clone();
        let tab_changed = tab_changed.clone();
        timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
            let Some(ui) = weak.upgrade() else { return };
            let now = plat::now_ms();
            // intervalle entre ticks pendant la charge
            let gap = now - last_tick.get();
            last_tick.set(now);
            {
                let mut g = heavy_gap.lock().unwrap();
                if g.0 && gap > g.1 {
                    g.1 = gap;
                }
            }
            // animation du canvas
            ui.set_anim_t(((now - t_start) / 1000.0) as f32);
            // prix
            {
                let mut l = list.borrow_mut();
                for (i, p) in feed.drain() {
                    l.set_price(i as usize, p);
                }
                l.refresh(-ui.get_list_scroll(), ui.get_list_view_h());
            }
            // FPS
            {
                let mut f = fps_state.borrow_mut();
                // le notifier a été accepté mais n'a rien rendu en 1 s (ex. skia-software) : repli sur les ticks
                if f.from_notifier && f.frames.get() == 0 && now - t_start > 1000.0 {
                    f.from_notifier = false;
                    notifier_dead.set(true);
                }
                if !f.from_notifier {
                    f.frames.set(f.frames.get() + 1);
                }
                if now - f.last_ms >= 1000.0 {
                    let n = f.frames.get();
                    let fps = ((n - f.last_count) as f64 * 1000.0 / (now - f.last_ms)).round() as i32;
                    f.last_count = n;
                    f.last_ms = now;
                    ui.set_fps(fps);
                    if ui.get_tab() == 1 {
                        plat::log(&format!("SPIKE:fps tab=canvas value={fps}"));
                    }
                }
            }
            // première frame -> ready + autotest
            if !ready_logged.get() && (first_frame.get() || ((!from_notifier || notifier_dead.get()) && now - t_start > 1000.0)) {
                ready_logged.set(true);
                plat::log(&format!("SPIKE:ready platform={} stack=slint core={}", plat::platform(), core::NAME));
                plat::log(&format!("SPIKE:info first_frame_source={} fps_source={}", if first_frame.get() { "rendering-notifier" } else { "timer-1s" }, if first_frame.get() { "rendering-notifier" } else { "timer-tick" }));
                match &selftest {
                    Ok(()) => plat::log("SPIKE:selftest ok"),
                    Err(e) => plat::log(&format!("SPIKE:selftest fail {e}")),
                }
                if autorun {
                    auto_t0.set(now);
                    plat::log("SPIKE:autorun start");
                    tab_changed(0);
                }
            }
            // parcours automatique : liste 3 s, canvas 3 s, cœur 2 s
            let t0 = auto_t0.get();
            if autorun && !t0.is_nan() {
                let t = (now - t0) / 1000.0;
                if t < 3.0 {
                    ui.set_list_scroll(-(t as f32 * 6000.0)); // défilement simulé 6000 px/s
                }
                let step = auto_step.get();
                let at = |s: f64| t >= s;
                if step == 0 && at(3.0) {
                    auto_step.set(1);
                    tab_changed(1);
                } else if step == 1 && at(6.0) {
                    auto_step.set(2);
                    tab_changed(2);
                } else if (2..=6).contains(&step) && at(6.0 + 0.2 * (step - 1) as f64) {
                    // saisie simulée de « hello » : une lettre toutes les 200 ms
                    let txt: String = "hello".chars().take(step - 1).collect();
                    ui.set_input_text(txt.as_str().into());
                    process(&txt);
                    auto_step.set(step + 1);
                } else if step == 7 && at(7.2) {
                    auto_step.set(8);
                    start_heavy();
                } else if step == 8 && at(8.0) {
                    auto_step.set(9);
                    plat::log("SPIKE:done");
                    done_at.set(now);
                }
            }
            // bureau : quitte 4 s après la fin du parcours (SPIKE_KEEP=1 pour rester ouvert)
            #[cfg(not(any(target_os = "android", target_os = "ios", target_family = "wasm")))]
            if !done_at.get().is_nan() && now - done_at.get() > 4000.0 && std::env::var_os("SPIKE_KEEP").is_none() {
                let _ = slint::quit_event_loop();
            }
        });
    }
    ui.run()
}
