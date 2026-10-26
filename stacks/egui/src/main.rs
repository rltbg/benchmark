//! Application de reference BENCH-SPEC en egui / eframe 0.37 (glow, rendu logiciel Mesa llvmpipe).
mod canvas;
mod data;
mod scen;

use canvas::Canvas;
use data::*;
use egui::{Color32, FontId, Frame, Pos2, Rect, Vec2};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const WARMUP: f64 = 2.0;
const SUM_H: u64 = 8_799_584;

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
        // drapeaux de diagnostic (hors spec, jamais utilises pour les mesures officielles)
        if v[i] == "--no-shadows" { a.no_shadows = true; i += 1; continue; }
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

struct App {
    a: Args,
    proc_start: Instant,
    m0: Option<Instant>,
    frame_no: u64,
    last_counted: Option<Instant>,
    changed: bool,
    frame_ms: Vec<f64>,
    ui_end_prev: Option<Instant>,
    ui_end_ms: Vec<f64>,
    cpu_usage_sum: f64,
    cpu_usage_n: u64,
    unchanged: u64,
    t: f64,
    ttfd_ms: f64,
    // etat par scenario
    model: ListModel,
    stream: Option<Stream>,
    canvas: Option<Canvas>,
    text: Option<scen::TextView>,
    images: Option<scen::Images>,
    nav: Option<scen::Nav>,
    core: Option<CoreWork>,
    burner: Option<Arc<AtomicBool>>,
    last_off: f64,
    final_off: f64,
    samples: Vec<(f64, u64, u64)>,
    next_sample: f64,
    base_applied: u64,
    font_info: String,
    shown_imgs: usize,
    gpu_info: String,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, a: Args) -> Self {
        let proc_start = Instant::now();
        cc.egui_ctx.set_pixels_per_point(1.0);
        let mut font_info = String::new();
        let mut text = None;
        let mut images = None;
        let mut canvas = None;
        let mut nav = None;
        match a.scenario.as_str() {
            "text" => {
                let l = scen::install_fallback_fonts(&cc.egui_ctx);
                font_info = format!("fallback fonts: {l:?}");
                text = Some(scen::TextView::load(&format!("{}/text1mb.txt", a.assets)));
            }
            "images" => images = Some(scen::Images::new(&a.assets)),
            "canvas" => canvas = Some(Canvas::new(a.shapes, Vec2::new(1000.0, 600.0), !a.no_shadows, if a.no_graph { None } else { Some(Vec2::new(1000.0, 100.0)) })),
            "core" => canvas = Some(Canvas::new(1000, Vec2::new(1000.0, 600.0), false, None)),
            "stress" => canvas = Some(Canvas::new(a.shapes, Vec2::new(380.0, 580.0), true, Some(Vec2::new(380.0, 100.0)))),
            "nav" => nav = Some(scen::Nav::new()),
            _ => {}
        }
        App {
            a, proc_start, m0: None, frame_no: 0, last_counted: None, changed: false, frame_ms: Vec::new(), ui_end_prev: None, ui_end_ms: Vec::new(),
            cpu_usage_sum: 0.0, cpu_usage_n: 0, unchanged: 0, t: 0.0, ttfd_ms: 0.0, model: ListModel::new(), stream: None, canvas, text, images, nav, core: None, burner: None,
            last_off: -1.0, final_off: 0.0, samples: Vec::new(), next_sample: 10.0, base_applied: 0, font_info, shown_imgs: 0, gpu_info: String::new(),
        }
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
        self.model.rows_drawn = 0;
    }

    fn finish(&mut self, wall: f64) -> ! {
        let a = &self.a;
        let mut ops = String::new();
        let mut checksum = format!("{:08x}", fnv(self.model.total()));
        let mut extra_notes = String::new();
        let sc = a.scenario.as_str();
        if matches!(sc, "list" | "stream" | "stress" | "idle") {
            let _ = write!(ops, "\"sum_heights\":{},\"final_offset\":{:.3},\"rows_drawn\":{}", self.model.total(), self.final_off, self.model.rows_drawn);
        }
        if let Some(s) = &self.stream {
            s.stop();
            let g = s.generated.load(Ordering::Relaxed);
            let d = s.dropped.load(Ordering::Relaxed);
            let ap = self.model.updates_applied - self.base_applied;
            let _ = write!(ops, ",\"updates_generated\":{g},\"updates_applied\":{ap},\"updates_dropped\":{d},\"updates_pending_at_end\":{}", g.saturating_sub(ap + d));
        }
        if let Some(c) = &self.canvas {
            if !ops.is_empty() { ops.push(','); }
            let _ = write!(ops, "\"shapes\":{},\"shadows\":{},\"graph_points\":{}", c.n_shapes, c.n_shadows, if c.graph.is_some() { 10000 } else { 0 });
        }
        if let Some(c) = &self.core {
            c.stop.store(true, Ordering::Relaxed);
            let calls = c.calls.load(Ordering::Relaxed);
            if !ops.is_empty() { ops.push(','); }
            let _ = write!(ops, "\"calls_per_s\":{:.0},\"core_calls_total\":{},\"heavy_calls\":{},\"max_frame_stall_ms\":{:.3}", calls as f64 / wall, calls, c.heavy.load(Ordering::Relaxed), self.frame_ms.iter().cloned().fold(0.0, f64::max));
            checksum = format!("{:016x}", c.hsum.load(Ordering::Relaxed));
        }
        if let Some(t) = &self.text {
            let _ = write!(ops, "\"time_to_first_display_ms\":{:.1},\"layout_ms\":{:.3},\"lines\":{},\"select_all\":\"unsupported\"", self.ttfd_ms, t.layout_ms, t.lines.len());
            extra_notes.push_str(&format!(" text: {}. layout_ms = lecture du fichier + decoupage en {} lignes (pas de layout global du Mo : egui met en page ligne par ligne a la demande).", self.font_info, t.lines.len()));
        }
        if let Some(im) = &self.images {
            let _ = write!(ops, "\"images_decoded\":{},\"decode_total_ms\":{:.1},\"cells_drawn_last_frame\":{}", im.decoded, im.decode_total_ms, self.shown_imgs);
            let _ = im.dir();
        }
        if let Some(n) = &self.nav {
            let _ = write!(ops, "\"transitions_done\":{},\"modals_done\":{},\"background_load\":\"1 thread de calcul\"", n.transitions_done, n.modals_done);
            if let Some(b) = &self.burner { b.store(true, Ordering::Relaxed); }
        }
        if !self.samples.is_empty() {
            let s: Vec<String> = self.samples.iter().map(|(t, h, r)| format!("{{\"t\":{t:.1},\"managed_heap_kb\":{h},\"rss_kb\":{r}}}")).collect();
            let _ = write!(ops, ",\"heap_samples\":[{}]", s.join(","));
        }
        let ui_p50 = pct(&self.ui_end_ms, 50.0); let ui_p95 = pct(&self.ui_end_ms, 95.0);
        let _ = write!(ops, "{}\"unchanged_frames_excluded\":{},\"ui_end_interval_p50_ms\":{:.3},\"ui_end_interval_p95_ms\":{:.3},\"frame_cpu_usage_ms_mean\":{:.3}", if ops.is_empty() { "" } else { "," }, self.unchanged, ui_p50, ui_p95, if self.cpu_usage_n > 0 { self.cpu_usage_sum / self.cpu_usage_n as f64 * 1000.0 } else { 0.0 });
        let gpu = self.gpu_info.replace('"', "'");
        let renderer = if cfg!(feature = "glow") { "glow (OpenGL, Mesa llvmpipe, glutin/GLX)" } else { "wgpu (Vulkan lavapipe)" };
        let notes = format!("egui/eframe 0.37.0 {renderer}, vsync=false (swap interval 0 / PresentMode::AutoNoVsync demande). frame_ms[k] = intervalle entre les DEBUTS de App::logic() de deux frames comptees consecutives ; ce debut suit le retour de swap_buffers de la frame precedente (eframe n'expose pas de hook post-swap) donc l'intervalle inclut update+tessellation+raster llvmpipe+swap. Seules les frames dont le contenu a change sont comptees (offset/anim/prix visibles) ; request_repaint() a chaque frame ; eframe ne genere pas de frames 'fantomes'.{extra_notes} warm-up 2 s non mesure. gpu: {gpu}.");
        let fm: Vec<String> = self.frame_ms.iter().map(|x| format!("{x:.3}")).collect();
        let json = format!("{{\"stack\":\"egui\",\"scenario\":\"{}\",\"duration_s\":{:.1},\"frames\":{},\"frame_ms\":[{}],\"ops\":{{{}}},\"checksum\":\"{}\",\"notes\":\"{}\"}}", a.scenario, a.duration, self.frame_ms.len(), fm.join(","), ops, checksum, notes.replace('"', "'"));
        std::fs::write(&a.out, json).expect("write json");
        std::process::exit(0);
    }
}

impl eframe::App for App {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] { [0.07, 0.075, 0.09, 1.0] }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let now = Instant::now();
        self.frame_no += 1;
        if self.frame_no == 1 {
            #[cfg(feature = "glow")]
            if let Some(gl) = frame.gl() {
                use eframe::glow::HasContext;
                self.gpu_info = unsafe { format!("GL_RENDERER={} GL_VERSION={}", gl.get_parameter_string(eframe::glow::RENDERER), gl.get_parameter_string(eframe::glow::VERSION)) };
            }
            #[cfg(all(feature = "wgpu", not(feature = "glow")))]
            if let Some(rs) = frame.wgpu_render_state() { let i = rs.adapter.get_info(); self.gpu_info = format!("wgpu adapter={} backend={:?} type={:?}", i.name, i.backend, i.device_type); }
        }
        if self.frame_no == 2 { self.ttfd_ms = (now - self.proc_start).as_secs_f64() * 1000.0; }
        if let Some(c) = frame.info().cpu_usage { if self.m0.is_some() { self.cpu_usage_sum += c as f64; self.cpu_usage_n += 1; } }
        let st = *START.get_or_init(|| now);
        if self.m0.is_none() && (now - st).as_secs_f64() >= WARMUP && self.frame_no > 2 {
            self.start_measure(now);
        }
        if let Some(m0) = self.m0 {
            self.t = (now - m0).as_secs_f64();
            if self.changed {
                let lc = self.last_counted.unwrap();
                self.frame_ms.push((now - lc).as_secs_f64() * 1000.0);
                self.last_counted = Some(now);
            } else if self.frame_no > 1 { self.unchanged += 1; }
            if self.t >= self.a.duration { self.finish(self.t); }
        }
        // flux : un lot par frame
        if let Some(s) = &self.stream {
            let n = s.apply(&mut self.model);
            if n > 0 { self.changed = true; }
        }
        if let Some(im) = &mut self.images { if im.poll(ctx) { self.changed = true; } }
        if let (Some(m0), Some(_)) = (self.m0, &self.core) {
            if self.a.scenario == "stress" && self.t >= self.next_sample {
                self.samples.push((self.t, heap_kb(), rss_kb()));
                self.next_sample += 10.0;
            }
            let _ = m0;
        }
        self.changed = false;
        if self.a.scenario != "idle" { ctx.request_repaint(); }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let t = self.t;
        let sc = self.a.scenario.clone();
        match sc.as_str() {
            "idle" => self.shell(ui, 0.0, 880.0 + 400.0, false),
            "list" | "stream" => { let o = self.list_off(t, 664.0); self.shell(ui, o, 1280.0, true); }
            "stress" => {
                let o = self.list_off(t, 664.0);
                let left = Rect::from_min_size(ui.max_rect().min, Vec2::new(880.0, 720.0));
                let right = Rect::from_min_size(Pos2::new(880.0, 0.0), Vec2::new(400.0, 720.0));
                let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(left));
                lui.set_clip_rect(left);
                self.shell(&mut lui, o, 880.0, true);
                let p = ui.painter().with_clip_rect(right);
                p.rect_filled(right, 0.0, Color32::from_rgb(28, 30, 36));
                let tf = t as f32;
                let c = self.canvas.as_mut().unwrap();
                c.paint(&p, Pos2::new(890.0, 10.0), tf);
                c.paint_graph(&p, Pos2::new(890.0, 600.0));
                self.changed = true;
            }
            "canvas" => {
                let p = ui.painter().clone();
                let c = self.canvas.as_mut().unwrap();
                c.paint(&p, Pos2::new(140.0, 8.0), t as f32);
                c.paint_graph(&p, Pos2::new(140.0, 612.0));
                self.changed = true;
            }
            "core" => {
                let p = ui.painter().clone();
                let c = self.canvas.as_mut().unwrap();
                c.paint(&p, Pos2::new(140.0, 60.0), t as f32);
                p.text(Pos2::new(140.0, 20.0), egui::Align2::LEFT_TOP, "core: boucle validate+parse+hash en thread", FontId::proportional(18.0), Color32::WHITE);
                self.changed = true;
            }
            "text" => {
                let tv = self.text.as_ref().unwrap();
                let off = egui::CentralPanel::no_frame().show(ui, |ui| tv.draw(ui, t)).inner;
                self.changed = off != self.last_off;
                self.last_off = off;
            }
            "images" => {
                // zoom 1->3->1 sur 6 s (triangle), pan horizontal sinusoidal, defilement 3000 px/s en aller-retour
                let ph = (t % 6.0) / 6.0;
                let s = 1.0 + 2.0 * (1.0 - (2.0 * ph - 1.0).abs()) as f32;
                let rows = (scen::N_IMG + scen::COLS - 1) / scen::COLS;
                let world_h = rows as f32 * scen::CELL + 8.0;
                let world_w = scen::COLS as f32 * scen::CELL + 8.0;
                let oy = ping(t * 3000.0, (world_h - 720.0 / s) as f64) as f32;
                let ox = (world_w - 1280.0 / s).max(0.0) * (0.5 + 0.5 * ((t as f32) * std::f32::consts::TAU / 4.0).sin());
                let im = self.images.as_mut().unwrap();
                self.shown_imgs = im.draw(ui, ox, oy, s);
                self.changed = true;
            }
            "nav" => {
                let n = self.nav.as_mut().unwrap();
                n.draw(ui, t);
                self.changed = true;
            }
            _ => {}
        }
        if self.m0.is_some() {
            let now = Instant::now();
            if let Some(p) = self.ui_end_prev { self.ui_end_ms.push((now - p).as_secs_f64() * 1000.0); }
            self.ui_end_prev = Some(now);
        }
        let _ = ctx;
    }
}

static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// aller-retour 0..max
fn ping(x: f64, max: f64) -> f64 {
    if max <= 0.0 { return 0.0; }
    let m = x.rem_euclid(2.0 * max);
    if m > max { 2.0 * max - m } else { m }
}

impl App {
    fn list_off(&mut self, t: f64, view_h: f64) -> f64 {
        let maxoff = self.model.total() as f64 - view_h;
        let o = if self.m0.is_some() { list_offset(t, maxoff) } else { 0.0 };
        self.final_off = o;
        if o != self.last_off || self.stream.as_ref().map_or(false, |_| self.model.updates_applied > 0) { self.changed = true; }
        self.last_off = o;
        o
    }
    /// Coquille : barre de titre, liste virtualisee, barre d'etat.
    fn shell(&mut self, ui: &mut egui::Ui, off: f64, width: f32, _animated: bool) {
        let bar = |ui: &mut egui::Ui, h: f32, fill: Color32, text: String| {
            let r = Rect::from_min_size(ui.cursor().min, Vec2::new(width, h));
            let (r, _) = ui.allocate_exact_size(r.size(), egui::Sense::hover());
            ui.painter().rect_filled(r, 0.0, fill);
            ui.painter().text(Pos2::new(r.left() + 12.0, r.center().y), egui::Align2::LEFT_CENTER, text, FontId::proportional(14.0), Color32::WHITE);
        };
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        bar(ui, 32.0, Color32::from_rgb(45, 60, 100), "Bench - egui / eframe 0.37".into());
        let rest = Rect::from_min_size(ui.cursor().min, Vec2::new(width, 664.0));
        let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(rest));
        lui.set_clip_rect(rest);
        scen::draw_list(&mut lui, &mut self.model, off);
        ui.allocate_rect(rest, egui::Sense::hover());
        bar(ui, 24.0, Color32::from_rgb(30, 32, 40), format!("{} lignes - offset {:.0}", N_ROWS, off));
        let _ = Frame::NONE;
    }
}

fn main() {
    let a = parse_args();
    let _ = SUM_H;
    if a.scenario == "idle" {
        // idle : aucune animation ; un thread dort puis ecrit le JSON
        let a2 = a.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs_f64(WARMUP + a2.duration));
            let json = format!("{{\"stack\":\"egui\",\"scenario\":\"idle\",\"duration_s\":{:.1},\"frames\":0,\"frame_ms\":[],\"ops\":{{\"sum_heights\":{}}},\"checksum\":\"{:08x}\",\"notes\":\"egui glow ; aucune frame redemandee (pas de request_repaint) ; coquille = barre de titre + liste (1re page) + barre d'etat, dessinee une fois (quelques frames initiales d'eframe non comptees).\"}}", a2.duration, ListModel::new().total(), fnv(ListModel::new().total()));
            std::fs::write(&a2.out, json).unwrap();
            std::process::exit(0);
        });
    }
    let mut opts = eframe::NativeOptions::default();
    opts.viewport = egui::ViewportBuilder::default().with_inner_size([1280.0, 720.0]).with_resizable(false).with_position([0.0, 0.0]).with_title("egui bench");
    #[cfg(feature = "glow")]
    { opts.glow_options.vsync = false; }
    #[cfg(all(feature = "wgpu", not(feature = "glow")))]
    {
        opts.renderer = eframe::Renderer::Wgpu;
        opts.wgpu_options.surface.present_mode = eframe::egui_wgpu::wgpu::PresentMode::AutoNoVsync;
    }
    let a2 = a.clone();
    eframe::run_native("egui bench", opts, Box::new(move |cc| Ok(Box::new(App::new(cc, a2.clone()))))).unwrap();
}
