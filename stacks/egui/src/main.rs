//! Application de référence BENCH-SPEC v2 en egui / eframe 0.37.0.
//! Renderer : wgpu (défaut d'eframe 0.37, Vulkan lavapipe sous Xvfb) ou glow (EGUI_BENCH_RENDERER=glow,
//! OpenGL Mesa llvmpipe). Rendu logiciel dans les deux cas.
mod canvas;
mod data;
mod scen;

use canvas::Canvas;
use data::*;
use egui::{Color32, FontId, Key, Pos2, Rect, Vec2};
use std::io::Write as _;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

const WARMUP: f64 = 2.0;
const FRAME_60: f64 = 1.0 / 60.0;
const VERSION: &str = "egui 0.37.0 / eframe 0.37.0 / epaint 0.37.0 / wgpu 30.0.1 / glow 0.17.0 / winit 0.30.13 / image 0.25.10 / rustc 1.99.1";
static PROC_START: OnceLock<Instant> = OnceLock::new();

fn marker(s: &str) {
    let mut o = std::io::stdout().lock();
    let _ = writeln!(o, "{s}");
    let _ = o.flush();
}

#[repr(C)]
struct Mallinfo2 { arena: usize, ordblks: usize, smblks: usize, hblks: usize, hblkhd: usize, usmblks: usize, fsmblks: usize, uordblks: usize, fordblks: usize, keepcost: usize }
unsafe extern "C" { fn mallinfo2() -> Mallinfo2; }
fn heap_kb() -> u64 { let m = unsafe { mallinfo2() }; ((m.uordblks + m.hblkhd) / 1024) as u64 }

#[derive(Clone)]
struct Args { scenario: String, duration: f64, out: String, rate: u32, shapes: usize, assets: String }

/// Racine du dépôt : BENCH_ROOT, sinon déduite de l'emplacement du binaire (stacks/egui/target/release/…).
fn bench_root() -> std::path::PathBuf {
    if let Some(r) = std::env::var_os("BENCH_ROOT") { return r.into(); }
    let exe = std::env::current_exe().ok().and_then(|p| p.canonicalize().ok());
    exe.and_then(|p| p.ancestors().nth(5).map(|a| a.to_path_buf())).unwrap_or_else(|| ".".into())
}

fn parse_args() -> Args {
    let mut a = Args { scenario: "idle".into(), duration: 10.0, out: "out.json".into(), rate: 1000, shapes: 5000, assets: String::new() };
    let v: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i + 1 < v.len() {
        let val = v[i + 1].clone();
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
    if a.assets.is_empty() { a.assets = bench_root().join("assets").to_string_lossy().into_owned(); }
    a
}

// ---------------------------------------------------------------- cœur C-ABI
// benchcore est une dépendance rlib (path) ; on appelle ses `extern "C"` par des pointeurs de fonction
// opacifiés par `black_box`, pour que la LTO ne puisse ni inliner ni éliminer les appels.
type ValidateFn = unsafe extern "C" fn(*const u8, usize) -> i32;
type ParseFn = unsafe extern "C" fn(*const u8, usize, *mut u8, usize, *mut usize) -> i32;
type HashFn = unsafe extern "C" fn(*const u8, usize) -> u32;
type HeavyFn = extern "C" fn(u32) -> u64;

struct CoreWork { calls: Arc<AtomicU64>, heavy: Arc<AtomicU64>, acc: Arc<AtomicU64>, hsum: Arc<AtomicU64>, stop: Arc<AtomicBool> }
fn start_core() -> CoreWork {
    let w = CoreWork { calls: Arc::new(AtomicU64::new(0)), heavy: Arc::new(AtomicU64::new(0)), acc: Arc::new(AtomicU64::new(0)), hsum: Arc::new(AtomicU64::new(0)), stop: Arc::new(AtomicBool::new(false)) };
    let (c, a, s) = (w.calls.clone(), w.acc.clone(), w.stop.clone());
    std::thread::spawn(move || {
        let validate: ValidateFn = std::hint::black_box(benchcore::core_validate as ValidateFn);
        let parse: ParseFn = std::hint::black_box(benchcore::core_parse as ParseFn);
        let hash: HashFn = std::hint::black_box(benchcore::core_hash as HashFn);
        let frame: [u8; 8] = [0xA5, 1, 5, b'h', b'e', b'l', b'l', b'o'];
        let mut out = [0u8; 64];
        let mut acc = 0u64;
        while !s.load(Ordering::Relaxed) {
            for _ in 0..4096 {
                let f = std::hint::black_box(&frame);
                let mut ol = 0usize;
                unsafe {
                    let v = validate(f.as_ptr(), f.len());
                    let p = parse(f.as_ptr(), f.len(), out.as_mut_ptr(), out.len(), &mut ol);
                    let h = hash(out.as_ptr(), ol);
                    acc = acc.wrapping_add(v as u64 + p as u64 + h as u64 + ol as u64);
                }
            }
            c.fetch_add(4096, Ordering::Relaxed);
            a.store(acc, Ordering::Relaxed); // résultat accumulé publié
        }
    });
    let (h, hs, s) = (w.heavy.clone(), w.hsum.clone(), w.stop.clone());
    std::thread::spawn(move || {
        let heavy: HeavyFn = std::hint::black_box(benchcore::core_heavy as HeavyFn);
        let t0 = Instant::now();
        let mut k = 0u32;
        while !s.load(Ordering::Relaxed) {
            let r = heavy(100);
            hs.fetch_xor(r, Ordering::Relaxed);
            h.fetch_add(1, Ordering::Relaxed);
            k += 1;
            // cadence 1 s (début d'appel à début d'appel)
            let next = t0 + Duration::from_secs(k as u64);
            while Instant::now() < next && !s.load(Ordering::Relaxed) { std::thread::sleep(Duration::from_millis(5)); }
        }
    });
    w
}

#[derive(Clone, Copy, PartialEq)]
enum Rend { Wgpu, Glow }

struct App {
    a: Args,
    rend: Rend,
    pacing60: bool,
    m0: Option<Instant>,
    hook_no: u64,
    last_counted: Option<Instant>,
    changed: bool,
    frame_ms: Vec<f64>,
    cpu_usage_sum: f64,
    cpu_usage_n: u64,
    unchanged: u64,
    t: f64,
    ttfd_ms: Option<f64>,
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
    shown_imgs: usize,
    gpu_info: String,
    square_white: bool,
    key_events: u64,
    emoji_font: String,
}

fn rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1).and_then(|v| v.parse().ok()))).unwrap_or(0)
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, a: Args, rend: Rend, pacing60: bool) -> Self {
        cc.egui_ctx.set_pixels_per_point(1.0);
        let mut gpu_info = String::new();
        if let Some(rs) = cc.wgpu_render_state.as_ref() {
            let i = rs.adapter.get_info();
            gpu_info = format!("wgpu adapter={} backend={:?} type={:?} driver={} {}", i.name, i.backend, i.device_type, i.driver, i.driver_info);
        }
        if let Some(gl) = cc.gl.as_ref() {
            use eframe::glow::HasContext;
            gpu_info = unsafe { format!("GL_RENDERER={} GL_VERSION={}", gl.get_parameter_string(eframe::glow::RENDERER), gl.get_parameter_string(eframe::glow::VERSION)) };
        }
        let (mut text, mut images, mut canvas, mut nav) = (None, None, None, None);
        let mut emoji_font = String::new();
        match a.scenario.as_str() {
            "text" => {
                if std::env::var("EGUI_BENCH_NO_COLOR_EMOJI").is_err() { emoji_font = scen::add_color_emoji(&cc.egui_ctx); }
                text = Some(scen::TextView::load(&format!("{}/text1mb.txt", a.assets)));
            }
            "images" => images = Some(scen::Images::new(&a.assets)),
            "canvas" => canvas = Some(Canvas::new(a.shapes, Vec2::new(1000.0, 600.0), true, Some(Vec2::new(1000.0, 100.0)))),
            "core" => canvas = Some(Canvas::new(1000, Vec2::new(1000.0, 600.0), false, None)),
            "stress" => canvas = Some(Canvas::new(5000, Vec2::new(380.0, 580.0), true, Some(Vec2::new(380.0, 100.0)))),
            "nav" => nav = Some(scen::Nav::new()),
            _ => {}
        }
        App {
            a, rend, pacing60, m0: None, hook_no: 0, last_counted: None, changed: false, frame_ms: Vec::new(),
            cpu_usage_sum: 0.0, cpu_usage_n: 0, unchanged: 0, t: 0.0, ttfd_ms: None, model: ListModel::new(), stream: None, canvas, text, images, nav, core: None, burner: None,
            last_off: -1.0, final_off: 0.0, samples: Vec::new(), next_sample: 10.0, base_applied: 0, shown_imgs: 0, gpu_info, square_white: false, key_events: 0, emoji_font,
        }
    }

    fn start_measure(&mut self, now: Instant) {
        marker("BENCH:measure_start");
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
        self.key_events = 0;
    }

    fn finish(&mut self, wall: f64) -> ! {
        marker("BENCH:measure_end");
        let a = &self.a;
        let sc = a.scenario.as_str();
        let mut ops: Vec<String> = Vec::new();
        let mut not_rendered: Vec<&str> = Vec::new();
        let mut checksum = format!("{:08x}", fnv(self.model.total()));
        let mut extra = String::new();
        if matches!(sc, "list" | "stream" | "stress" | "input") {
            ops.push(format!("\"sum_heights\":{},\"final_offset\":{:.3},\"rows_drawn\":{}", self.model.total(), self.final_off, self.model.rows_drawn));
        }
        if sc == "input" {
            ops.push(format!("\"key_events\":{}", self.key_events));
            extra.push_str(" input: carré 200x200 en (100,100) basculé noir/blanc sur egui::Event::Key{key: Space, pressed: true} (chemin winit -> egui-winit -> RawInput), redessiné dans la frame suivante ; focus clavier donné par le harnais (XSetInputFocus), aucune manipulation X11 dans l'app.");
        }
        if let Some(s) = &self.stream {
            s.stop();
            let g = s.generated.load(Ordering::Relaxed);
            let d = s.dropped.load(Ordering::Relaxed);
            let ap = self.model.updates_applied - self.base_applied;
            ops.push(format!("\"updates_generated\":{g},\"updates_applied\":{ap},\"updates_dropped\":{d}"));
        }
        if let Some(c) = &self.canvas {
            ops.push(format!("\"shapes\":{},\"shadows\":{},\"graph_points\":{},\"draw_shapes_last_frame\":{}", c.n_shapes, c.n_shadows, if c.graph.is_some() { 10000 } else { 0 }, c.meshes_last_frame));
            if sc != "core" { not_rendered.push("rounded_clip"); }
        }
        if let Some(c) = &self.core {
            c.stop.store(true, Ordering::Relaxed);
            let calls = c.calls.load(Ordering::Relaxed);
            let stall = self.frame_ms.iter().cloned().fold(0.0, f64::max);
            ops.push(format!("\"calls_per_s\":{:.0},\"core_calls_total\":{},\"heavy_calls\":{},\"max_frame_stall_ms\":{:.3},\"core_acc\":{}", calls as f64 / wall, calls, c.heavy.load(Ordering::Relaxed), stall, c.acc.load(Ordering::Relaxed)));
            checksum = format!("{:016x}", c.hsum.load(Ordering::Relaxed) ^ c.acc.load(Ordering::Relaxed));
        }
        if let Some(t) = &self.text {
            ops.push(format!("\"time_to_first_display_ms\":{:.1},\"layout_ms\":{:.3},\"file_read_ms\":{:.3},\"galley_rows\":{},\"select_all\":{},\"selected_chars\":{}", self.ttfd_ms.unwrap_or(0.0), t.layout_ms.unwrap_or(0.0), t.load_ms, t.galley_rows, t.select_all_done, t.selected_chars));
            extra.push_str(&format!(" text: TextEdit::multiline(&mut &str) (lecture seule, sélectionnable) dans ScrollArea ; layout_ms = durée du premier TextEdit::show (mise en page complète du Mo en un Galley, retour à la ligne à la largeur) ; time_to_first_display_ms = début de main -> retour de la présentation de la 1re frame de texte ; select-all à t=5 s = request_focus + TextEditState::cursor.set_char_range(CCursorRange::select_all) ; polices de repli système via feature eframe `system_fonts` (egui_system_font_provider/fontique : Noto Sans CJK, DejaVu…) + Noto Color Emoji ajoutée explicitement : {}.", self.emoji_font));
            checksum = format!("{:08x}", data::fnv(t.text.len() as u32));
        }
        if let Some(im) = &self.images {
            ops.push(format!("\"images_decoded\":{},\"decode_total_ms\":{:.1},\"cells_drawn_last_frame\":{},\"texture_px\":128", im.decoded, im.decode_total_ms, self.shown_imgs));
        }
        if let Some(n) = &self.nav {
            ops.push(format!("\"transitions_done\":{},\"modals_done\":{}", n.transitions_done, n.modals_done));
            if let Some(b) = &self.burner { b.store(true, Ordering::Relaxed); }
        }
        if !self.samples.is_empty() {
            let s: Vec<String> = self.samples.iter().map(|(t, h, r)| format!("{{\"t\":{t:.1},\"native_heap_kb\":{h},\"rss_kb\":{r}}}")).collect();
            ops.push(format!("\"heap_samples\":[{}]", s.join(",")));
        }
        ops.push(format!("\"pacing\":\"{}\"", if self.pacing60 { "60" } else { "uncapped" }));
        ops.push(format!("\"unchanged_frames_excluded\":{},\"frame_cpu_usage_ms_mean\":{:.3}", self.unchanged, if self.cpu_usage_n > 0 { self.cpu_usage_sum / self.cpu_usage_n as f64 * 1000.0 } else { 0.0 }));
        let nr: Vec<String> = not_rendered.iter().map(|s| json_str(s)).collect();
        ops.push(format!("\"not_rendered\":[{}]", nr.join(",")));
        if !not_rendered.is_empty() {
            extra.push_str(" canvas: rounded_clip non rendu (egui/epaint 0.37 ne clippe que par rectangle : Painter::with_clip_rect -> scissor) ; clip rectangulaire 1000x600 + fond arrondi. Dégradés = Mesh à couleurs de sommets (seul moyen documenté ; Shape::gradient_rect est lui-même un Mesh), anticrénelage = anneau de feathering 1 px construit dans le Mesh ; ombres = epaint::Shadow::as_shape (RectShape blur_width, par forme, ordre de peinture respecté).");
        }
        let (rname, vs) = match self.rend {
            Rend::Wgpu => ("wgpu", "wgpu_options.surface.present_mode = PresentMode::AutoNoVsync (défaut eframe : AutoVsync)"),
            Rend::Glow => ("glow", "glow_options.vsync = false (glutin SwapInterval::DontWait)"),
        };
        let notes = format!(
            "egui/eframe 0.37.0, renderer {rname} ({}), {vs}. frame_ms[k] = intervalle entre deux appels consécutifs de App::raw_input_hook (premier code applicatif après le retour de present/swap_buffers de la frame précédente ; eframe n'expose aucun hook post-présentation), comptés seulement si la frame précédente avait un contenu changé ; {}.{extra} Warm-up 2 s non mesuré.",
            self.gpu_info,
            if self.pacing60 { "pacing 60 (BENCH_PACING=60) : le vsync n'est pas effectif sous Xvfb (AutoVsync/SwapInterval::Wait mesurés > 60 fps), donc cadence par l'API publique Context::request_repaint_after vers la prochaine échéance d'une grille fixe de 16,667 ms (frame en retard = repaint immédiat)" } else { "pacing uncapped (BENCH_PACING=uncapped) : Context::request_repaint() à chaque frame, redessin continu sans plafond" }
        );
        let fm: Vec<String> = self.frame_ms.iter().map(|x| format!("{x:.3}")).collect();
        let version = format!("{VERSION} / renderer {rname}");
        let json = format!(
            "{{\"stack\":\"egui\",\"version\":{},\"scenario\":{},\"duration_s\":{:.1},\"frames\":{},\"frame_ms\":[{}],\"ops\":{{{}}},\"checksum\":\"{}\",\"notes\":{},\"unsupported\":null}}",
            json_str(&version), json_str(sc), a.duration, self.frame_ms.len(), fm.join(","), ops.join(","), checksum, json_str(&notes)
        );
        std::fs::write(&a.out, json).expect("write json");
        std::process::exit(0);
    }

    fn list_off(&mut self, view_h: f64) -> f64 {
        let maxoff = self.model.total() as f64 - view_h;
        let o = if self.m0.is_some() { list_offset(self.t, maxoff) } else { 0.0 };
        self.final_off = o;
        if o != self.last_off { self.changed = true; }
        self.last_off = o;
        o
    }

    /// Coquille : barre de titre, liste virtualisée, barre d'état.
    fn shell(&mut self, ui: &mut egui::Ui, origin: Pos2, off: f64, width: f32) {
        let p = ui.painter();
        let top = Rect::from_min_size(origin, Vec2::new(width, 32.0));
        p.rect_filled(top, 0.0, Color32::from_rgb(45, 60, 100));
        p.text(Pos2::new(top.left() + 12.0, top.center().y), egui::Align2::LEFT_CENTER, "Bench - egui / eframe 0.37", FontId::proportional(14.0), Color32::WHITE);
        let rest = Rect::from_min_size(origin + Vec2::new(0.0, 32.0), Vec2::new(width, 664.0));
        let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(rest));
        lui.set_clip_rect(rest.intersect(ui.clip_rect()));
        scen::draw_list(&mut lui, &mut self.model, off);
        let bot = Rect::from_min_size(origin + Vec2::new(0.0, 696.0), Vec2::new(width, 24.0));
        let p = ui.painter();
        p.rect_filled(bot, 0.0, Color32::from_rgb(30, 32, 40));
        p.text(Pos2::new(bot.left() + 12.0, bot.center().y), egui::Align2::LEFT_CENTER, format!("{} lignes - offset {:.0}", N_ROWS, off), FontId::proportional(13.0), Color32::WHITE);
    }
}

impl eframe::App for App {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] { [0.07, 0.075, 0.09, 1.0] }

    /// Appelé une fois par frame, avant les passes egui : c'est ici qu'on horodate la frame
    /// (fin de présentation de la précédente) et qu'on fait avancer l'état.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        let now = Instant::now();
        self.hook_no += 1;
        if let Some(m0) = self.m0 {
            if self.changed {
                self.frame_ms.push((now - self.last_counted.unwrap()).as_secs_f64() * 1000.0);
                self.last_counted = Some(now);
            } else {
                self.unchanged += 1;
            }
            self.t = (now - m0).as_secs_f64();
            if self.t >= self.a.duration { self.finish(self.t); }
        } else if (now - *PROC_START.get().unwrap()).as_secs_f64() >= WARMUP && self.hook_no > 2 {
            self.start_measure(now);
            self.t = 0.0;
        }
        if self.ttfd_ms.is_none() && self.text.as_ref().is_some_and(|t| t.layout_ms.is_some()) {
            self.ttfd_ms = Some((now - *PROC_START.get().unwrap()).as_secs_f64() * 1000.0);
        }
        self.changed = false;
        // entrée clavier (scénario input) : événements de la frame, chemin normal winit -> egui-winit
        if self.a.scenario == "input" {
            for e in &raw.events {
                if let egui::Event::Key { key: Key::Space, pressed: true, repeat: false, .. } = e {
                    self.square_white = !self.square_white;
                    self.key_events += 1;
                    self.changed = true;
                }
            }
        }
        // flux : un lot par frame
        if let Some(s) = &self.stream { if s.apply(&mut self.model) > 0 { self.changed = true; } }
        if let Some(c) = &mut self.canvas { if self.m0.is_some() { c.advance_graph(); } }
        if self.a.scenario == "stress" && self.m0.is_some() && self.t >= self.next_sample {
            self.samples.push((self.t, heap_kb(), rss_kb()));
            self.next_sample += 10.0;
        }
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if ctx.current_pass_index() == 0 {
            if let Some(c) = frame.info().cpu_usage { if self.m0.is_some() { self.cpu_usage_sum += c as f64; self.cpu_usage_n += 1; } }
            if let Some(im) = &mut self.images { if im.poll(ctx) { self.changed = true; } }
        }
        if self.a.scenario != "idle" {
            if self.pacing60 && ctx.current_pass_index() == 0 {
                // grille fixe de 16,667 ms depuis le démarrage : pas de dérive ; frame en retard -> repaint immédiat
                let st = *PROC_START.get().unwrap();
                let el = st.elapsed().as_secs_f64();
                let k = (el / FRAME_60).floor() + 1.0;
                let wait = (k * FRAME_60 - el).max(0.0);
                ctx.request_repaint_after(Duration::from_secs_f64(wait));
            } else if !self.pacing60 {
                ctx.request_repaint();
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let t = self.t;
        let animating = self.m0.is_some();
        let sc = self.a.scenario.clone();
        let full = ui.max_rect();
        match sc.as_str() {
            "idle" => self.shell(ui, full.min, 0.0, 1280.0),
            "list" | "stream" => { let o = self.list_off(664.0); self.shell(ui, full.min, o, 1280.0); }
            "input" => {
                let o = self.list_off(664.0);
                self.shell(ui, full.min, o, 1280.0);
                let sq = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::splat(200.0));
                ui.painter().rect_filled(sq, 0.0, if self.square_white { Color32::WHITE } else { Color32::BLACK });
            }
            "stress" => {
                let o = self.list_off(664.0);
                self.shell(ui, full.min, o, 880.0);
                let right = Rect::from_min_size(Pos2::new(880.0, 0.0), Vec2::new(400.0, 720.0));
                let p = ui.painter().with_clip_rect(right);
                p.rect_filled(right, 0.0, Color32::from_rgb(28, 30, 36));
                let c = self.canvas.as_mut().unwrap();
                c.paint(&p, Pos2::new(890.0, 10.0), t as f32);
                c.paint_graph(&p, Pos2::new(890.0, 600.0));
                if animating { self.changed = true; }
            }
            "canvas" => {
                let p = ui.painter().clone();
                let c = self.canvas.as_mut().unwrap();
                c.paint(&p, Pos2::new(140.0, 8.0), t as f32);
                c.paint_graph(&p, Pos2::new(140.0, 612.0));
                if animating { self.changed = true; }
            }
            "core" => {
                let p = ui.painter().clone();
                let c = self.canvas.as_mut().unwrap();
                c.paint(&p, Pos2::new(140.0, 60.0), t as f32);
                p.text(Pos2::new(140.0, 20.0), egui::Align2::LEFT_TOP, "core: validate+parse+hash en boucle (thread) + core_heavy(100)/s", FontId::proportional(18.0), Color32::WHITE);
                if animating { self.changed = true; }
            }
            "text" => {
                let measuring = self.m0.is_some();
                let tv = self.text.as_mut().unwrap();
                let off = egui::CentralPanel::default().show(ui, |ui| tv.draw(ui, t, measuring)).inner;
                if off != self.last_off || (measuring && t >= 5.0 && t < 5.2) { self.changed = true; }
                self.last_off = off;
            }
            "images" => {
                // zoom 1->3->1 sur 6 s (triangle), pan horizontal sinusoïdal, défilement vertical 3000 px/s aller-retour
                let ph = (t % 6.0) / 6.0;
                let s = 1.0 + 2.0 * (1.0 - (2.0 * ph - 1.0).abs()) as f32;
                let rows = scen::N_IMG.div_ceil(scen::COLS);
                let world_h = rows as f32 * scen::CELL + 8.0;
                let world_w = scen::COLS as f32 * scen::CELL + 8.0;
                let oy = scen::ping(t * 3000.0, (world_h - 720.0 / s) as f64) as f32;
                let ox = (world_w - 1280.0 / s).max(0.0) * (0.5 + 0.5 * ((t as f32) * std::f32::consts::TAU / 4.0).sin());
                let im = self.images.as_mut().unwrap();
                self.shown_imgs = im.draw(ui, ox, oy, s);
                if animating { self.changed = true; }
            }
            "nav" => {
                self.nav.as_mut().unwrap().draw(ui, t);
                if animating { self.changed = true; }
            }
            _ => {}
        }
    }
}

fn main() {
    PROC_START.get_or_init(Instant::now);
    let a = parse_args();
    // BENCH_PACING=60 (défaut) : request_repaint_after vers la prochaine échéance d'une grille 16,667 ms ;
    // BENCH_PACING=uncapped : request_repaint() à chaque frame.
    let pacing60 = std::env::var("BENCH_PACING").as_deref() != Ok("uncapped");
    let rend = match std::env::var("EGUI_BENCH_RENDERER").as_deref() { Ok("glow") => Rend::Glow, _ => Rend::Wgpu };
    if a.scenario == "idle" {
        // idle : aucune animation ni request_repaint ; ce thread place les marqueurs et écrit le JSON
        let a2 = a.clone();
        std::thread::spawn(move || {
            let st = *PROC_START.get().unwrap();
            std::thread::sleep((st + Duration::from_secs_f64(WARMUP)).saturating_duration_since(Instant::now()));
            marker("BENCH:measure_start");
            std::thread::sleep(Duration::from_secs_f64(a2.duration));
            marker("BENCH:measure_end");
            let rn = if rend == Rend::Glow { "glow" } else { "wgpu" };
            let notes = format!("egui/eframe 0.37.0 renderer {rn} ; aucune frame redemandée (pas de request_repaint) : coquille (barre de titre + 1re page de liste + barre d'état) dessinée par les frames initiales d'eframe avant la mesure ; 0 frame pendant la mesure.");
            let json = format!("{{\"stack\":\"egui\",\"version\":{},\"scenario\":\"idle\",\"duration_s\":{:.1},\"frames\":0,\"frame_ms\":[],\"ops\":{{\"sum_heights\":{},\"not_rendered\":[],\"pacing\":\"{}\"}},\"checksum\":\"{:08x}\",\"notes\":{},\"unsupported\":null}}",
                json_str(&format!("{VERSION} / renderer {rn}")), a2.duration, ListModel::new().total(), if pacing60 { "60" } else { "uncapped" }, fnv(ListModel::new().total()), json_str(&notes));
            std::fs::write(&a2.out, json).unwrap();
            std::process::exit(0);
        });
    }
    let mut opts = eframe::NativeOptions::default();
    opts.viewport = egui::ViewportBuilder::default().with_inner_size([1280.0, 720.0]).with_resizable(false).with_position([0.0, 0.0]).with_title("egui bench");
    opts.persist_window = false;
    match rend {
        Rend::Glow => { opts.renderer = eframe::Renderer::Glow; opts.glow_options.vsync = false; }
        Rend::Wgpu => {
            opts.renderer = eframe::Renderer::Wgpu;
            // diagnostic seulement (vérification de l'effet du vsync) : EGUI_BENCH_PRESENT=vsync garde AutoVsync
            if std::env::var("EGUI_BENCH_PRESENT").as_deref() != Ok("vsync") {
                opts.wgpu_options.surface.present_mode = eframe::egui_wgpu::wgpu::PresentMode::AutoNoVsync;
            }
        }
    }
    if std::env::var("EGUI_BENCH_PRESENT").as_deref() == Ok("vsync") { opts.glow_options.vsync = true; }
    let a2 = a.clone();
    eframe::run_native("egui bench", opts, Box::new(move |cc| Ok(Box::new(App::new(cc, a2.clone(), rend, pacing60))))).unwrap();
}
