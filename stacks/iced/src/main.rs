//! Application de reference BENCH-SPEC en iced 0.14.0 (rendu logiciel iced_tiny_skia ; variante wgpu/lavapipe en option).
mod data;
mod shapes;
mod widgets;

use data::*;
use iced::widget::{canvas, column, container, image::Handle, row, stack, text, text_editor, Space};
use iced::{window, Background, Border, Color, Element, Length, Padding, Pixels, Point, Size, Subscription, Task, Theme};
use shapes::{GraphProgram, Shapes, ShapesProgram};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use widgets::*;

const WARMUP: f64 = 2.0;
const SUM_H: u64 = 8_799_584;

#[derive(Debug, Clone)]
pub enum Message {
    /// Emis par `window::frames()` a chaque RedrawRequested (traite apres le present de cette frame).
    Frame(Instant),
    Edit(text_editor::Action),
}

#[repr(C)]
struct Mallinfo2 { arena: usize, ordblks: usize, smblks: usize, hblks: usize, hblkhd: usize, usmblks: usize, fsmblks: usize, uordblks: usize, fordblks: usize, keepcost: usize }
extern "C" { fn mallinfo2() -> Mallinfo2; }
fn heap_kb() -> u64 { unsafe { let m = mallinfo2(); ((m.uordblks + m.hblkhd) / 1024) as u64 } }
fn rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1).and_then(|v| v.parse().ok()))).unwrap_or(0)
}

#[derive(Clone)]
struct Args { scenario: String, duration: f64, out: String, rate: u32, shapes: usize, assets: String, no_shadows: bool, no_graph: bool }
fn parse_args() -> Args {
    let mut a = Args { scenario: "idle".into(), duration: 10.0, out: "out.json".into(), rate: 1000, shapes: 5000, assets: "/home/user/eval/assets".into(), no_shadows: false, no_graph: false };
    let v: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < v.len() {
        if v[i] == "--no-shadows" { a.no_shadows = true; i += 1; continue; }
        if v[i] == "--full-damage" { std::env::set_var("ICED_FULL_DAMAGE", "1"); i += 1; continue; }
        if v[i] == "--no-graph" { a.no_graph = true; i += 1; continue; }
        let val = v.get(i + 1).cloned().unwrap_or_default();
        match v[i].as_str() {
            "--scenario" => a.scenario = val,
            "--duration" => a.duration = val.parse().unwrap_or(10.0),
            "--out" => a.out = val,
            "--rate" => a.rate = val.parse().unwrap_or(1000),
            "--shapes" => a.shapes = val.parse().unwrap_or(5000),
            "--assets" => a.assets = val,
            _ => {}
        }
        i += 2;
    }
    a
}
fn pct(v: &[f64], p: f64) -> f64 {
    if v.is_empty() { return 0.0; }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    s[((p / 100.0 * s.len() as f64) as usize).min(s.len() - 1)]
}
fn ping(x: f64, max: f64) -> f64 {
    if max <= 0.0 { return 0.0; }
    let m = x.rem_euclid(2.0 * max);
    if m > max { 2.0 * max - m } else { m }
}

// ---------------------------------------------------------------- core (thread worker + heavy)
struct CoreWork { calls: Arc<AtomicU64>, heavy: Arc<AtomicU64>, hsum: Arc<AtomicU64>, stop: Arc<AtomicBool> }
fn start_core() -> CoreWork {
    let w = CoreWork { calls: Arc::new(AtomicU64::new(0)), heavy: Arc::new(AtomicU64::new(0)), hsum: Arc::new(AtomicU64::new(0)), stop: Arc::new(AtomicBool::new(false)) };
    let (c, s) = (w.calls.clone(), w.stop.clone());
    std::thread::spawn(move || {
        let frame: [u8; 8] = [0xA5, 1, 5, b'h', b'e', b'l', b'l', b'o'];
        let mut out = [0u8; 64];
        let mut acc = 0u32;
        while !s.load(Ordering::Relaxed) {
            for _ in 0..4096 {
                let f = std::hint::black_box(&frame);
                unsafe {
                    let v = benchcore::core_validate(f.as_ptr(), f.len());
                    let mut ol = 0usize;
                    let p = benchcore::core_parse(f.as_ptr(), f.len(), out.as_mut_ptr(), out.len(), &mut ol);
                    let h = benchcore::core_hash(out.as_ptr(), std::hint::black_box(ol));
                    acc ^= h.wrapping_add(v as u32 + p as u32);
                }
            }
            std::hint::black_box(acc);
            c.fetch_add(4096, Ordering::Relaxed);
        }
    });
    let (h, hs, s) = (w.heavy.clone(), w.hsum.clone(), w.stop.clone());
    std::thread::spawn(move || {
        while !s.load(Ordering::Relaxed) {
            let r = benchcore::core_heavy(100);
            hs.fetch_xor(r, Ordering::Relaxed);
            h.fetch_add(1, Ordering::Relaxed);
            let t = Instant::now();
            while t.elapsed() < Duration::from_millis(1000) && !s.load(Ordering::Relaxed) { std::thread::sleep(Duration::from_millis(10)); }
        }
    });
    w
}

// ---------------------------------------------------------------- images
struct Images {
    req_tx: std::sync::mpsc::Sender<usize>,
    res_rx: Receiver<(usize, u32, u32, Vec<u8>, f64)>,
    handles: Vec<Option<Handle>>,
    requested: Vec<bool>,
    decoded: u64,
    decode_total_ms: f64,
}
impl Images {
    fn new(dir: &str) -> Self {
        let (req_tx, req_rx) = channel::<usize>();
        let (res_tx, res_rx) = channel();
        let req_rx = Arc::new(Mutex::new(req_rx));
        for _ in 0..2 {
            let (rx, tx, d) = (req_rx.clone(), res_tx.clone(), dir.to_string());
            std::thread::spawn(move || loop {
                let Ok(i) = rx.lock().unwrap().recv() else { return };
                let t = Instant::now();
                if let Ok(img) = image::open(format!("{d}/images/img{i:03}.png")) {
                    let img = img.to_rgba8();
                    let (w, h) = (img.width(), img.height());
                    let _ = tx.send((i, w, h, img.into_raw(), t.elapsed().as_secs_f64() * 1000.0));
                }
            });
        }
        Self { req_tx, res_rx, handles: (0..N_IMG).map(|_| None).collect(), requested: vec![false; N_IMG], decoded: 0, decode_total_ms: 0.0 }
    }
    fn poll(&mut self) -> bool {
        let mut any = false;
        while let Ok((i, w, h, px, ms)) = self.res_rx.try_recv() {
            self.handles[i] = Some(Handle::from_rgba(w, h, px));
            self.decoded += 1;
            self.decode_total_ms += ms;
            any = true;
        }
        any
    }
    fn request(&mut self, i: usize) {
        if i < N_IMG && !self.requested[i] {
            self.requested[i] = true;
            let _ = self.req_tx.send(i);
        }
    }
}

// ---------------------------------------------------------------- navigation
#[derive(Clone, Copy, Debug)]
enum Step { Push(usize, usize), Pop(usize, usize), Modal }
struct Nav { steps: Vec<(Step, f64)>, cur: usize, step_start: f64, transitions_done: u64, modals_done: u64 }
impl Nav {
    fn new() -> Self {
        let mut steps = Vec::new();
        for _ in 0..5 {
            steps.push((Step::Push(0, 1), 0.3));
            steps.push((Step::Push(1, 2), 0.3));
            steps.push((Step::Pop(2, 1), 0.3));
            steps.push((Step::Pop(1, 0), 0.3));
            steps.push((Step::Modal, 0.2));
            steps.push((Step::Modal, 0.2));
        }
        Self { steps, cur: 0, step_start: 0.0, transitions_done: 0, modals_done: 0 }
    }
    /// Avance dans la sequence ; retourne (etape, progression lissee 0..1, index d'etape).
    fn advance(&mut self, t: f64) -> (Step, f32, usize) {
        loop {
            let (st, d) = self.steps[self.cur % self.steps.len()];
            if t - self.step_start >= d {
                match st { Step::Modal => self.modals_done += 1, _ => self.transitions_done += 1 }
                self.step_start += d;
                self.cur += 1;
            } else {
                break;
            }
        }
        let (st, d) = self.steps[self.cur % self.steps.len()];
        let f = ((t - self.step_start) / d).clamp(0.0, 1.0) as f32;
        (st, f * f * (3.0 - 2.0 * f), self.cur)
    }
}
fn nav_screen<'a>(idx: usize) -> Element<'a, Message> {
    let hue = [Color::from_rgb8(30, 50, 80), Color::from_rgb8(60, 40, 80), Color::from_rgb8(40, 70, 60)][idx];
    let mut col = column![text(format!("Ecran {}", idx + 1)).size(28).color(Color::WHITE)].spacing(6).padding(Padding { top: 16.0, left: 24.0, right: 0.0, bottom: 0.0 });
    for k in 0..24 {
        let w = 300.0 + 40.0 * (((k + idx) as f32) * 0.7).sin();
        col = col.push(
            container(text(format!("Ligne {k} de l'ecran {}", idx + 1)).size(14).color(Color::from_rgb8(230, 230, 230)))
                .width(Length::Fixed(w))
                .height(Length::Fixed(20.0))
                .padding(Padding { top: 0.0, left: 8.0, right: 0.0, bottom: 0.0 })
                .style(|_| container::Style { background: Some(Background::Color(Color::from_rgba8(255, 255, 255, 40.0 / 255.0))), border: Border { radius: 4.0.into(), ..Border::default() }, ..Default::default() }),
        );
    }
    container(col).width(Length::Fill).height(Length::Fill).style(move |_| container::Style { background: Some(Background::Color(hue)), ..Default::default() }).into()
}

// ---------------------------------------------------------------- application
struct App {
    a: Args,
    proc_start: Instant,
    start: Option<Instant>,
    m0: Option<Instant>,
    frame_no: u64,
    last_counted: Option<Instant>,
    changed: bool,
    frame_ms: Vec<f64>,
    frame_at_ms: Vec<f64>,
    last_at: Option<Instant>,
    unchanged: u64,
    t: f64,
    ttfd_ms: f64,
    draws_at_start: u64,
    draws_prev: u64,
    frames_without_draw: u64,
    model: ListModel,
    stream: Option<Stream>,
    base_applied: u64,
    shapes: Option<Shapes>,
    content: Option<text_editor::Content>,
    text_layout_ms: f64,
    text_lines: usize,
    text_cur_line: i64,
    select_all_done: bool,
    images: Option<Images>,
    img_view: (f32, f32, f32),
    nav: Option<Nav>,
    nav_state: Option<(Step, f32, usize)>,
    core: Option<CoreWork>,
    burner: Option<Arc<AtomicBool>>,
    last_off: f64,
    off: f64,
    samples: Vec<(f64, u64, u64)>,
    next_sample: f64,
    shapes_t: f32,
}

const LINE_H: f32 = 18.0;

impl App {
    fn new(a: Args, proc_start: Instant) -> Self {
        let mut shapes = None;
        let mut content = None;
        let mut text_layout_ms = 0.0;
        let mut text_lines = 0;
        let mut images = None;
        let mut nav = None;
        match a.scenario.as_str() {
            "text" => {
                let t = Instant::now();
                let s = std::fs::read_to_string(format!("{}/text1mb.txt", a.assets)).expect("text1mb.txt");
                let c = text_editor::Content::with_text(&s);
                text_layout_ms = t.elapsed().as_secs_f64() * 1000.0;
                text_lines = c.line_count();
                content = Some(c);
            }
            "images" => images = Some(Images::new(&a.assets)),
            "canvas" => shapes = Some(Shapes::new(a.shapes, Size::new(1000.0, 600.0), !a.no_shadows, if a.no_graph { None } else { Some(Size::new(1000.0, 100.0)) })),
            "core" => shapes = Some(Shapes::new(1000, Size::new(1000.0, 600.0), false, None)),
            "stress" => shapes = Some(Shapes::new(a.shapes, Size::new(380.0, 580.0), true, Some(Size::new(380.0, 100.0)))),
            "nav" => nav = Some(Nav::new()),
            _ => {}
        }
        App {
            a, proc_start, start: None, m0: None, frame_no: 0, last_counted: None, changed: false, frame_ms: Vec::new(), frame_at_ms: Vec::new(), last_at: None, unchanged: 0, t: 0.0, ttfd_ms: 0.0,
            draws_at_start: 0, draws_prev: 0, frames_without_draw: 0,
            model: ListModel::new(), stream: None, base_applied: 0, shapes, content, text_layout_ms, text_lines, text_cur_line: 0, select_all_done: false, images, img_view: (0.0, 0.0, 1.0),
            nav, nav_state: None, core: None, burner: None, last_off: -1.0, off: 0.0, samples: Vec::new(), next_sample: 10.0, shapes_t: 0.0,
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        if self.a.scenario == "idle" { Subscription::none() } else { window::frames().map(Message::Frame) }
    }

    fn start_measure(&mut self, now: Instant) {
        self.m0 = Some(now);
        self.last_counted = Some(now);
        match self.a.scenario.as_str() {
            "stream" => self.stream = Some(Stream::start(self.a.rate)),
            "stress" => { self.stream = Some(Stream::start(5000)); self.core = Some(start_core()); }
            "core" => self.core = Some(start_core()),
            "nav" => {
                let stop = Arc::new(AtomicBool::new(false));
                let s = stop.clone();
                std::thread::spawn(move || { let mut x = 1u64; while !s.load(Ordering::Relaxed) { for _ in 0..100000 { x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)); } } });
                self.burner = Some(stop);
            }
            _ => {}
        }
        self.base_applied = self.model.updates_applied;
        ROWS_DRAWN.store(0, Ordering::Relaxed);
        self.draws_at_start = DRAWS.load(Ordering::Relaxed);
        self.draws_prev = self.draws_at_start;
        self.nav = self.nav.as_ref().map(|_| Nav::new());
        self.text_cur_line = 0;
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Edit(action) => {
                // lecture seule : toute edition est ignoree ; deplacements/selection/molette appliques
                if !action.is_edit() {
                    if let Some(c) = self.content.as_mut() { c.perform(action); }
                }
            }
            Message::Frame(at) => return self.on_frame(at),
        }
        Task::none()
    }

    fn on_frame(&mut self, at: Instant) -> Task<Message> {
        let mut task = Task::none();
        // `now` : apres le present de la frame dont RedrawRequested(at) est l'origine (message traite dans AboutToWait)
        let now = Instant::now();
        FD_PARITY.fetch_add(1, Ordering::Relaxed);
        self.frame_no += 1;
        if self.frame_no == 2 { self.ttfd_ms = (now - self.proc_start).as_secs_f64() * 1000.0; }
        let st = *self.start.get_or_insert(now);
        if self.m0.is_none() && (now - st).as_secs_f64() >= WARMUP && self.frame_no > 2 {
            self.start_measure(now);
        }
        if let Some(m0) = self.m0 {
            self.t = (now - m0).as_secs_f64();
            let draws = DRAWS.load(Ordering::Relaxed);
            if draws == self.draws_prev { self.frames_without_draw += 1; }
            self.draws_prev = draws;
            if let Some(la) = self.last_at { self.frame_at_ms.push((at - la).as_secs_f64() * 1000.0); }
            self.last_at = Some(at);
            if self.changed {
                let lc = self.last_counted.unwrap();
                self.frame_ms.push((now - lc).as_secs_f64() * 1000.0);
                self.last_counted = Some(now);
            } else if self.frame_no > 1 {
                self.unchanged += 1;
            }
            if self.t >= self.a.duration { self.finish(self.t); }
        }
        self.changed = false;
        let t = self.t;
        let measuring = self.m0.is_some();
        match self.a.scenario.clone().as_str() {
            "list" | "stream" | "stress" => {
                let maxoff = self.model.total() as f64 - 664.0;
                let o = if measuring { list_offset(t, maxoff) } else { 0.0 };
                self.off = o;
                if o != self.last_off { self.changed = true; }
                self.last_off = o;
                if let Some(s) = &self.stream {
                    if s.apply(&mut self.model) > 0 { self.changed = true; }
                }
                if self.a.scenario == "stress" {
                    self.shapes_t = t as f32;
                    if let Some(sh) = self.shapes.as_mut() { sh.step_graph(); }
                    self.changed = true;
                    if measuring && self.t >= self.next_sample {
                        self.samples.push((self.t, heap_kb(), rss_kb()));
                        self.next_sample += 10.0;
                    }
                }
            }
            "canvas" | "core" => {
                self.shapes_t = t as f32;
                if let Some(sh) = self.shapes.as_mut() { sh.step_graph(); }
                self.changed = true;
            }
            "text" => {
                if let Some(c) = self.content.as_mut() {
                    let max = (c.line_count() as f64 * LINE_H as f64 - 720.0).max(0.0);
                    let off = if measuring { ping(t * 3000.0, max) } else { 0.0 };
                    let line = (off / LINE_H as f64) as i64;
                    let d = line - self.text_cur_line;
                    if d != 0 {
                        c.perform(text_editor::Action::Scroll { lines: d as i32 });
                        self.text_cur_line = line;
                        self.changed = true;
                    }
                    if measuring && t >= 5.0 && !self.select_all_done {
                        c.perform(text_editor::Action::SelectAll);
                        task = iced::widget::operation::focus("text"); // la selection n'est dessinee que si le widget a le focus
                        self.select_all_done = true;
                        self.changed = true;
                    }
                    self.off = off;
                }
            }
            "images" => {
                let ph = (t % 6.0) / 6.0;
                let s = 1.0 + 2.0 * (1.0 - (2.0 * ph - 1.0).abs()) as f32;
                let rows = (N_IMG + COLS - 1) / COLS;
                let world_h = rows as f32 * CELL + 8.0;
                let world_w = COLS as f32 * CELL + 8.0;
                let oy = ping(t * 3000.0, (world_h - 720.0 / s) as f64) as f32;
                let ox = (world_w - 1280.0 / s).max(0.0) * (0.5 + 0.5 * ((t as f32) * std::f32::consts::TAU / 4.0).sin());
                self.img_view = (ox, oy, s);
                let r0 = ((oy / CELL).floor() as i32).max(0) as usize;
                let r1 = (((oy + 720.0 / s) / CELL).ceil() as usize + 1).min(rows);
                let im = self.images.as_mut().unwrap();
                for r in r0.saturating_sub(2)..(r1 + 2).min(rows) {
                    for c in 0..COLS { im.request(r * COLS + c); }
                }
                im.poll();
                self.changed = true;
            }
            "nav" => {
                if let Some(n) = self.nav.as_mut() { self.nav_state = Some(n.advance(t)); }
                self.changed = true;
            }
            _ => {}
        }
        task
    }

    // ------------------------------------------------------------ vues
    fn bar<'a>(&self, label: String, h: f32, fill: Color) -> Element<'a, Message> {
        container(text(label).size(14).color(Color::WHITE))
            .width(Length::Fill)
            .height(Length::Fixed(h))
            .padding(Padding { top: 0.0, left: 12.0, right: 0.0, bottom: 0.0 })
            .align_y(iced::alignment::Vertical::Center)
            .style(move |_| container::Style { background: Some(Background::Color(fill)), ..Default::default() })
            .into()
    }
    fn shell(&self, width: f32) -> Element<'_, Message> {
        let list: Element<'_, Message> = VList { model: &self.model, offset: self.off, height: 664.0 }.into();
        column![
            self.bar("Bench - iced 0.14.0 (tiny-skia)".into(), 32.0, Color::from_rgb8(45, 60, 100)),
            list,
            self.bar(format!("{} lignes - offset {:.0}", N_ROWS, self.off), 24.0, Color::from_rgb8(30, 32, 40)),
        ]
        .width(Length::Fixed(width))
        .into()
    }
    fn shapes_panel<'a>(&'a self, region: Size, graph: bool) -> Element<'a, Message> {
        let sh = self.shapes.as_ref().unwrap();
        let t = self.shapes_t;
        let par = if full_damage() { 0.25 * (FD_PARITY.load(Ordering::Relaxed) & 1) as f32 } else { 0.0 };
        let cv = canvas(ShapesProgram { shapes: sh, t }).width(Length::Fixed(region.width)).height(Length::Fixed(region.height - par));
        let area: Element<'a, Message> = if sh.n_shadows > 0 {
            stack![sh.shadow_layer(t), cv].width(Length::Fixed(region.width)).height(Length::Fixed(region.height)).into()
        } else {
            cv.into()
        };
        let area = container(area).width(Length::Fixed(region.width)).height(Length::Fixed(region.height)).style(|_| container::Style {
            background: Some(Background::Color(Color::from_rgb8(24, 26, 32))),
            border: Border { radius: 12.0.into(), ..Border::default() },
            ..Default::default()
        });
        let mut col = column![area];
        if graph {
            if let Some(g) = &sh.graph {
                col = col.push(Space::new().height(Length::Fixed(4.0)));
                col = col.push(canvas(GraphProgram { graph: g }).width(Length::Fixed(g.size.width)).height(Length::Fixed(g.size.height - par)));
            }
        }
        col.into()
    }

    fn view(&self) -> Element<'_, Message> {
        let root: Element<'_, Message> = match self.a.scenario.as_str() {
            "idle" | "list" | "stream" => self.shell(1280.0),
            "stress" => row![self.shell(880.0), container(self.shapes_panel(Size::new(380.0, 580.0), true)).padding(Padding { top: 10.0, left: 10.0, right: 0.0, bottom: 0.0 }).width(Length::Fixed(400.0)).height(Length::Fill)
                .style(|_| container::Style { background: Some(Background::Color(Color::from_rgb8(28, 30, 36))), ..Default::default() })].into(),
            "canvas" => container(self.shapes_panel(Size::new(1000.0, 600.0), true)).padding(Padding { top: 8.0, left: 140.0, right: 0.0, bottom: 0.0 }).width(Length::Fill).height(Length::Fill).into(),
            "core" => container(column![text("core: boucle validate+parse+hash en thread").size(18).color(Color::WHITE), Space::new().height(Length::Fixed(20.0)), self.shapes_panel(Size::new(1000.0, 600.0), false)])
                .padding(Padding { top: 20.0, left: 140.0, right: 0.0, bottom: 0.0 }).width(Length::Fill).height(Length::Fill).into(),
            "text" => text_editor(self.content.as_ref().unwrap())
                .id("text")
                .on_action(Message::Edit)
                .size(Pixels(14.0))
                .line_height(iced::widget::text::LineHeight::Absolute(Pixels(LINE_H)))
                .wrapping(iced::widget::text::Wrapping::None)
                .padding(0)
                .height(Length::Fill)
                .into(),
            "images" => {
                let im = self.images.as_ref().unwrap();
                let (ox, oy, s) = self.img_view;
                ImageGrid { handles: &im.handles, ox, oy, s }.into()
            }
            "nav" => self.nav_view(),
            _ => Space::new().into(),
        };
        let root: Element<'_, Message> = if full_damage() { FullDamage { content: root }.into() } else { root };
        container(root).width(Length::Fill).height(Length::Fill).style(|_| container::Style { background: Some(Background::Color(Color::from_rgb(0.07, 0.075, 0.09))), ..Default::default() }).into()
    }

    fn nav_view(&self) -> Element<'_, Message> {
        let Some((st, e, _cur)) = self.nav_state else { return nav_screen(0) };
        let w = 1280.0f32;
        let slide = |c: Element<'static, Message>, dx: f32| -> Element<'static, Message> { Slide { content: c, dx }.into() };
        match st {
            Step::Push(a, b) => stack![slide(nav_screen(a), -e * w * 0.3), slide(nav_screen(b), (1.0 - e) * w)].into(),
            Step::Pop(a, b) => stack![slide(nav_screen(b), -(1.0 - e) * w * 0.3), slide(nav_screen(a), e * w)].into(),
            Step::Modal => {
                let open = _cur % 2 == 0;
                let al = if open { e } else { 1.0 - e };
                let scrim = container(Space::new()).width(Length::Fill).height(Length::Fill).style(move |_| container::Style { background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 150.0 * al / 255.0))), ..Default::default() });
                let dialog = container(text("Modale").size(26).color(Color::from_rgba8(0, 0, 0, al)))
                    .width(Length::Fixed(420.0)).height(Length::Fixed(240.0))
                    .center_x(Length::Fixed(420.0)).center_y(Length::Fixed(240.0))
                    .style(move |_| container::Style { background: Some(Background::Color(Color::from_rgba8(245, 245, 250, al))), border: Border { radius: 12.0.into(), ..Border::default() }, ..Default::default() });
                stack![nav_screen(0), scrim, container(dialog).center_x(Length::Fill).center_y(Length::Fill)].into()
            }
        }
    }

    // ------------------------------------------------------------ fin / JSON
    fn finish(&mut self, wall: f64) -> ! {
        let a = &self.a;
        let mut ops = String::new();
        let mut checksum = format!("{:08x}", fnv(self.model.total()));
        let mut extra_notes = String::new();
        let sc = a.scenario.as_str();
        if matches!(sc, "list" | "stream" | "stress" | "idle") {
            let _ = write!(ops, "\"sum_heights\":{},\"final_offset\":{:.3},\"rows_drawn\":{}", self.model.total(), self.off, ROWS_DRAWN.load(Ordering::Relaxed));
        }
        if let Some(s) = &self.stream {
            s.stop();
            let g = s.generated.load(Ordering::Relaxed);
            let d = s.dropped.load(Ordering::Relaxed);
            let ap = self.model.updates_applied - self.base_applied;
            let _ = write!(ops, ",\"updates_generated\":{g},\"updates_applied\":{ap},\"updates_dropped\":{d},\"updates_pending_at_end\":{}", g.saturating_sub(ap + d));
        }
        if let Some(sh) = &self.shapes {
            if !ops.is_empty() { ops.push(','); }
            let _ = write!(ops, "\"shapes\":{},\"shadows\":{},\"graph_points\":{},\"clip\":\"rectangulaire (bornes du widget canvas) ; arrondi 1000x600 non supporte\",\"shadow_impl\":\"Quad.shadow (fill_quad) hors canvas, sous les formes\"", sh.sims.len(), sh.n_shadows, if sh.graph.is_some() { 10000 } else { 0 });
        }
        if let Some(c) = &self.core {
            c.stop.store(true, Ordering::Relaxed);
            let calls = c.calls.load(Ordering::Relaxed);
            if !ops.is_empty() { ops.push(','); }
            let _ = write!(ops, "\"calls_per_s\":{:.0},\"core_calls_total\":{},\"heavy_calls\":{},\"max_frame_stall_ms\":{:.3}", calls as f64 / wall, calls, c.heavy.load(Ordering::Relaxed), self.frame_ms.iter().cloned().fold(0.0, f64::max));
            checksum = format!("{:016x}", c.hsum.load(Ordering::Relaxed));
        }
        if self.content.is_some() {
            let _ = write!(ops, "\"time_to_first_display_ms\":{:.1},\"layout_ms\":{:.3},\"lines\":{},\"select_all\":\"{}\",\"wrapping\":\"none\"", self.ttfd_ms, self.text_layout_ms, self.text_lines, if self.select_all_done { "Action::SelectAll performed (t=5s)" } else { "not reached" });
            extra_notes.push_str(" text: widget text_editor (cosmic-text) en lecture seule (Edit ignore), Wrapping::None, taille 14 px, interligne 18 px ; defilement par Action::Scroll{lines} (granularite = 1 ligne) ; layout_ms = lecture fichier + Content::with_text (cosmic-text set_text, mise en page paresseuse) ; time_to_first_display_ms = debut de main jusqu'a la 2e frame.");
        }
        if let Some(im) = &self.images {
            let _ = write!(ops, "\"images_decoded\":{},\"decode_total_ms\":{:.1},\"cells_drawn_last_frame\":{},\"decode_threads\":2", im.decoded, im.decode_total_ms, CELLS_DRAWN_LAST.load(Ordering::Relaxed));
        }
        if let Some(n) = &self.nav {
            let _ = write!(ops, "\"transitions_done\":{},\"modals_done\":{},\"background_load\":\"1 thread de calcul\"", n.transitions_done, n.modals_done);
            if let Some(b) = &self.burner { b.store(true, Ordering::Relaxed); }
        }
        if !self.samples.is_empty() {
            let s: Vec<String> = self.samples.iter().map(|(t, h, r)| format!("{{\"t\":{t:.1},\"managed_heap_kb\":{h},\"rss_kb\":{r}}}")).collect();
            let _ = write!(ops, ",\"heap_samples\":[{}]", s.join(","));
        }
        let _ = write!(ops, "{}\"unchanged_frames_excluded\":{},\"redraw_start_interval_p50_ms\":{:.3},\"redraw_start_interval_p95_ms\":{:.3},\"frame_events\":{},\"frames_without_custom_draw\":{},\"custom_draws\":{}",
            if ops.is_empty() { "" } else { "," }, self.unchanged, pct(&self.frame_at_ms, 50.0), pct(&self.frame_at_ms, 95.0), self.frame_at_ms.len() + 1, self.frames_without_draw, DRAWS.load(Ordering::Relaxed) - self.draws_at_start);
        let renderer = renderer_name();
        let notes = format!("iced 0.14.0, renderer: {renderer}. Redessin continu via la subscription window::frames() (un message par RedrawRequested) : update -> view -> RedrawRequested -> draw -> present, la boucle winit redemande une frame des que le message est traite ; aucun timer ni vsync. frame_ms[k] = intervalle entre deux traitements consecutifs de Message::Frame dans update() ; ce message est emis par RedrawRequested mais traite dans AboutToWait, donc APRES le present (softbuffer / swap wgpu) de cette frame : c'est le meilleur marqueur de fin de frame observable depuis l'app (iced n'expose pas de hook post-present ni de timestamp de presentation reelle), et il inclut donc update(), view(), layout, draw, raster tiny-skia, present et la relance de la boucle. ops.redraw_start_interval_* = intervalles entre les Instant de RedrawRequested (debut de frame). Seules les frames dont le contenu a change sont comptees (ops.unchanged_frames_excluded) ; iced ne genere pas de frames 'fantomes' en redessin continu. Warm-up 2 s non mesure.{extra_notes}");
        let fm: Vec<String> = self.frame_ms.iter().map(|x| format!("{x:.3}")).collect();
        let json = format!("{{\"stack\":\"iced\",\"scenario\":\"{}\",\"duration_s\":{:.1},\"frames\":{},\"frame_ms\":[{}],\"ops\":{{{}}},\"checksum\":\"{}\",\"notes\":\"{}\"}}", a.scenario, a.duration, self.frame_ms.len(), fm.join(","), ops, checksum, notes.replace('"', "'"));
        std::fs::write(&a.out, json).expect("write json");
        std::process::exit(0);
    }
}

fn renderer_name() -> String {
    let be = std::env::var("ICED_BACKEND").unwrap_or_default();
    if cfg!(feature = "wgpu") {
        format!("iced_renderer fallback, ICED_BACKEND={be} (wgpu 27 / Vulkan lavapipe si 'wgpu', iced_tiny_skia sinon)")
    } else {
        "iced_tiny_skia 0.14.1 (CPU natif, softbuffer), sans wgpu compile".into()
    }
}

fn main() {
    let proc_start = Instant::now();
    let a = parse_args();
    #[cfg(feature = "wgpu")]
    {
        if std::env::var("ICED_BACKEND").is_err() { std::env::set_var("ICED_BACKEND", "wgpu"); }
        if std::env::var("ICED_PRESENT_MODE").is_err() { std::env::set_var("ICED_PRESENT_MODE", "no_vsync"); }
    }
    if a.scenario == "idle" {
        let a2 = a.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs_f64(WARMUP + a2.duration));
            let t = ListModel::new().total();
            let json = format!("{{\"stack\":\"iced\",\"scenario\":\"idle\",\"duration_s\":{:.1},\"frames\":0,\"frame_ms\":[],\"ops\":{{\"sum_heights\":{}}},\"checksum\":\"{:08x}\",\"notes\":\"iced 0.14.0 ({}) ; aucune subscription de frames, aucune animation : la coquille (barre de titre, liste 1re page, barre d'etat) n'est dessinee que lors des quelques redraws initiaux ; un thread dort puis ecrit le JSON.\"}}", a2.duration, t, fnv(t), renderer_name());
            std::fs::write(&a2.out, json).unwrap();
            std::process::exit(0);
        });
    }
    let a2 = a.clone();
    let r = iced::application(move || (App::new(a2.clone(), proc_start), Task::none()), App::update, App::view)
        .subscription(App::subscription)
        .title("iced bench")
        .theme(|_: &App| Theme::Dark)
        .window_size(Size::new(1280.0, 720.0))
        .resizable(false)
        .position(window::Position::Specific(Point::new(0.0, 0.0)))
        .scale_factor(|_: &App| 1.0)
        .run();
    if let Err(e) = r {
        eprintln!("iced error: {e}");
        std::process::exit(2);
    }
}
