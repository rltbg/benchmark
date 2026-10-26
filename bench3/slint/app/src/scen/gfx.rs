// Graphisme : G1 gfx-shapes, G2 gfx-paths, G3 gfx-chart, G4 gfx-images, G5 gfx-layers.
// Slint n'a pas d'API canvas : chaque forme est un élément du graphe de scène (Rectangle, Path, Image) ; seul le tracé de
// courbe (G3) a une alternative idiomatique, l'image rendue côté Rust (SharedPixelBuffer) comme l'exemple `plotter` de Slint.
use super::{software_not_rendered, Scenario};
use crate::data::*;
use crate::{Args, Bench, GroupData, ImgItem, MainWindow, MoverData, PathData, ShapeData};
use serde_json::{json, Map, Value};
use slint::{Color, ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};
use std::rc::Rc;
use std::time::Instant;

fn col(v: u32) -> Color {
    Color::from_rgb_u8(v as u8, (v >> 8) as u8, (v >> 16) as u8)
}

// ================================================================ G1 shapes
pub struct ShapesState {
    model: Rc<VecModel<ShapeData>>,
    base: Vec<ShapeData>,
    bind: bool,
    last_sum: f64,
}
impl ShapesState {
    pub fn new(app: &MainWindow, n: usize, mode: &str, bind: bool) -> Self {
        let (w, h) = (1000.0f32, 600.0f32);
        let mut base = Vec::with_capacity(n);
        for i in 0..n {
            let f = |k: u32| fnv((i as u32).wrapping_mul(7).wrapping_add(k));
            let size = 12.0 + (f(1) % 29) as f32;
            let r = match i % 3 {
                0 => 0.0,
                1 => size / 2.0,
                _ => size / 4.0,
            };
            let sp = |v: u32| ((v % 401) as f32 - 200.0) * 1.0;
            let x0 = (f(2) % 1000) as f32 / 1000.0 * (w - size);
            let y0 = (f(3) % 1000) as f32 / 1000.0 * (h - size);
            base.push(ShapeData { x0, y0, vx: sp(f(6)), vy: sp(f(7)), x: x0, y: y0, w: size, r, c1: col(f(4)), c2: col(f(5)) });
        }
        let model = Rc::new(VecModel::from(base.clone()));
        let b = app.global::<Bench>();
        b.set_shapes(ModelRc::from(model.clone()));
        b.set_shape_bind(bind);
        b.set_shape_mode(mode.into());
        b.set_scn("shapes".into());
        ShapesState { model, base, bind, last_sum: 0.0 }
    }
    pub fn step(&mut self, t: f64, app: &MainWindow) {
        let b = app.global::<Bench>();
        if self.bind {
            b.set_t(t as f32);
        } else {
            // v0 : Rust recalcule et pousse les 2 coordonnées de chacune des N formes (N x set_row_data)
            let t = t as f32;
            for (i, s) in self.base.iter().enumerate() {
                let mut s2 = s.clone();
                s2.x = reflect(s.x0 + s.vx * t, 1000.0 - s.w);
                s2.y = reflect(s.y0 + s.vy * t, 600.0 - s.w);
                self.model.set_row_data(i, s2);
            }
            if let Some(l) = self.base.last() {
                self.last_sum = (l.x0 + l.y0) as f64;
            }
        }
    }
}
pub struct ShapesSc {
    st: ShapesState,
    n: usize,
    mode: String,
    renderer: String,
    bind: bool,
}
pub fn build_shapes(args: &Args, app: &MainWindow, renderer: &str) -> Result<Box<dyn Scenario>, String> {
    let n = args.n_or(1000) as usize;
    let mode = args.pick("feature", &["solid", "gradient", "shadow", "clip", "all"], "solid").to_string();
    let bind = args.pick("drive", &["bind", "model"], "bind") == "bind";
    app.global::<Bench>().set_chrome(false);
    Ok(Box::new(ShapesSc { st: ShapesState::new(app, n, &mode, bind), n, mode, renderer: renderer.to_string(), bind }))
}
impl Scenario for ShapesSc {
    fn step(&mut self, t: f64, app: &MainWindow, _m: bool) {
        self.st.step(t, app);
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        ops.insert("shapes".into(), self.n.into());
        ops.insert("feature".into(), self.mode.clone().into());
        ops.insert("drive".into(), (if self.bind { "bind" } else { "model" }).into());
        if self.renderer == "software" {
            let mut u: Vec<&str> = vec![];
            match self.mode.as_str() {
                "gradient" => u.push("gradient-rounded"),
                "shadow" => u.push("shadow"),
                "clip" => u.push("rounded-clip"),
                "all" => {
                    u.push("gradient-rounded");
                    u.push("shadow");
                    u.push("rounded-clip");
                }
                _ => {}
            }
            ops.insert("not_rendered".into(), json!(software_not_rendered(&u)));
        }
    }
    fn notes(&self) -> String {
        format!("gfx-shapes feature={}: one Rectangle per shape in the scene graph (1/3 rectangles, 1/3 circles, 1/3 rounded; speed from fnv, rebound by reflection in 1000x600). drive=bind: x/y are bindings of one global `t` property (1 property written per frame, N*2 bindings re-evaluated lazily at render); drive=model: Rust pushes x,y of every shape each frame with VecModel::set_row_data. feature=shadow: drop-shadow-blur 8px offset 2,2 on every shape; clip: the 1000x600 container has clip:true + border-radius 24px.", self.mode)
    }
}

// ================================================================ G2 paths
pub struct PathsSc {
    n: usize,
}
pub fn build_paths(args: &Args, app: &MainWindow) -> Result<Box<dyn Scenario>, String> {
    let n = args.n_or(100) as usize;
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let f = |k: u32| fnv((i as u32).wrapping_mul(131).wrapping_add(k));
        let p = |k: u32| (f(k) % 120) as f32;
        let mut cmd = format!("M {} {}", p(0), p(1));
        for s in 0..8u32 {
            let b = 2 + s * 6;
            cmd.push_str(&format!(" C {} {} {} {} {} {}", p(b), p(b + 1), p(b + 2), p(b + 3), p(b + 4), p(b + 5)));
        }
        let sp = |v: u32| ((v % 201) as f32 - 100.0) * 1.0;
        v.push(PathData { x0: (f(100) % 880) as f32, y0: (f(101) % 480) as f32, vx: sp(f(102)), vy: sp(f(103)), cmd: cmd.into(), c: col(f(104)) });
    }
    let b = app.global::<Bench>();
    b.set_paths(ModelRc::from(Rc::new(VecModel::from(v))));
    b.set_scn("paths".into());
    b.set_chrome(false);
    Ok(Box::new(PathsSc { n }))
}
impl Scenario for PathsSc {
    fn step(&mut self, t: f64, app: &MainWindow, _m: bool) {
        app.global::<Bench>().set_t(t as f32);
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        ops.insert("paths".into(), self.n.into());
    }
    fn notes(&self) -> String {
        "gfx-paths: one Path element per curve (M + 8 cubic C segments in a 120x120 viewbox, stroke 2 px, static command string), translated by a binding on the global `t`. Dashed strokes: unsupported (no stroke-dash in Slint Path).".into()
    }
}

// ================================================================ G3 chart
fn gval(k: u32) -> f32 {
    0.5 + 0.35 * (k as f32 / 200.0).sin() + 0.1 * ((fnv(k) % 1000) as f32 / 1000.0 - 0.5)
}
pub struct ChartSc {
    pts: std::collections::VecDeque<f32>,
    n: usize,
    next: u32,
    path: bool,
    drawn_segments: u64,
    last_draw_ms: f64,
    max_draw_ms: f64,
    sum_draw_ms: f64,
    frames: u64,
}
pub fn build_chart(args: &Args, app: &MainWindow) -> Result<Box<dyn Scenario>, String> {
    let n = args.n_or(10_000) as usize;
    let path = args.pick("render", &["pixbuf", "path"], "pixbuf") == "path";
    let mut pts = std::collections::VecDeque::with_capacity(n + 1024);
    for k in 0..n as u32 {
        pts.push_back(gval(k));
    }
    let b = app.global::<Bench>();
    b.set_scn("chart".into());
    b.set_chrome(false);
    Ok(Box::new(ChartSc { pts, n, next: n as u32, path, drawn_segments: 0, last_draw_ms: 0.0, max_draw_ms: 0.0, sum_draw_ms: 0.0, frames: 0 }))
}
const CW: usize = 1000;
const CH: usize = 300;
fn line(buf: &mut [u8], x0: i32, y0: i32, x1: i32, y1: i32) {
    // Bresenham 1 px, couleur #40e080
    let (mut x, mut y) = (x0, y0);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        if (x as u32) < CW as u32 && (y as u32) < CH as u32 {
            let o = (y as usize * CW + x as usize) * 4;
            buf[o] = 0x40;
            buf[o + 1] = 0xe0;
            buf[o + 2] = 0x80;
            buf[o + 3] = 0xff;
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}
impl Scenario for ChartSc {
    fn step(&mut self, _t: f64, app: &MainWindow, _m: bool) {
        // 1 000 points ajoutés par frame (les plus anciens sortent)
        for _ in 0..1000 {
            self.pts.push_back(gval(self.next));
            self.next = self.next.wrapping_add(1);
            self.pts.pop_front();
        }
        let t0 = Instant::now();
        let b = app.global::<Bench>();
        let n = self.pts.len();
        if self.path {
            let mut s = String::with_capacity(n * 14 + 8);
            use std::fmt::Write;
            let sx = (CW - 1) as f32 / (n - 1).max(1) as f32;
            for (j, v) in self.pts.iter().enumerate() {
                let _ = write!(s, "{}{:.1} {:.1} ", if j == 0 { "M " } else { "L " }, j as f32 * sx, (1.0 - v) * (CH - 1) as f32);
            }
            b.set_chart_path(s.into());
        } else {
            let mut pb = SharedPixelBuffer::<Rgba8Pixel>::new(CW as u32, CH as u32);
            let buf = pb.make_mut_bytes();
            for px in buf.chunks_exact_mut(4) {
                px[0] = 0x10;
                px[1] = 0x14;
                px[2] = 0x18;
                px[3] = 0xff;
            }
            let sx = (CW - 1) as f32 / (n - 1).max(1) as f32;
            let mut prev: Option<(i32, i32)> = None;
            for (j, v) in self.pts.iter().enumerate() {
                let p = ((j as f32 * sx) as i32, ((1.0 - v) * (CH - 1) as f32) as i32);
                if let Some(q) = prev {
                    line(buf, q.0, q.1, p.0, p.1);
                }
                prev = Some(p);
            }
            b.set_chart(Image::from_rgba8(pb));
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        self.last_draw_ms = ms;
        self.sum_draw_ms += ms;
        self.max_draw_ms = self.max_draw_ms.max(ms);
        self.frames += 1;
        self.drawn_segments += n as u64 - 1;
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        ops.insert("points".into(), self.n.into());
        ops.insert("render".into(), (if self.path { "path" } else { "pixbuf" }).into());
        ops.insert("points_added_per_frame".into(), 1000.into());
        ops.insert("segments_drawn".into(), self.drawn_segments.into());
        ops.insert("chart_build_ms_mean".into(), (self.sum_draw_ms / self.frames.max(1) as f64).into());
        ops.insert("chart_build_ms_max".into(), self.max_draw_ms.into());
    }
    fn notes(&self) -> String {
        if self.path {
            "gfx-chart render=path: the polyline is a Path element whose commands string (M/L x N points) is rebuilt and re-parsed every frame (O(N) string + parse).".into()
        } else {
            "gfx-chart render=pixbuf: each frame Rust rasterizes N-1 one-pixel Bresenham segments into a fresh 1000x300 SharedPixelBuffer shown by an Image (the approach of Slint's own `plotter` example); no anti-aliasing. The N points window slides by 1000 points per frame.".into()
        }
    }
}

// ================================================================ G4 images
pub struct ImagesSc {
    n: usize,
    big: Option<BigState>,
    decode_ms: f64,
    decoded: usize,
    renderer: String,
}
struct BigState {
    imgs: Vec<Image>,
    switch_at: f64,
    cur: usize,
    load_ms: Vec<f64>,
}
pub fn load_png(path: &std::path::Path) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
    let im = image::open(path).ok()?.into_rgba8();
    let (w, h) = im.dimensions();
    Some(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(im.as_raw(), w, h))
}
pub fn build_images(args: &Args, app: &MainWindow) -> Result<Box<dyn Scenario>, String> {
    let b = app.global::<Bench>();
    b.set_chrome(false);
    if args.has("big") {
        let t0 = Instant::now();
        // décodage des deux images avant la mesure ; échange 4096 -> 8192 à mi-fenêtre
        let p = std::path::Path::new(&args.assets);
        let mut imgs = vec![];
        let mut load_ms = vec![];
        for f in ["big4096.png", "big8192.png"] {
            let t = Instant::now();
            let pb = load_png(&p.join(f)).ok_or_else(|| format!("image {f} illisible"))?;
            imgs.push(Image::from_rgba8(pb));
            load_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        b.set_big_image(imgs[0].clone());
        b.set_scn("bigimage".into());
        let _ = t0;
        return Ok(Box::new(ImagesSc { n: 1, big: Some(BigState { imgs, switch_at: args.duration / 2.0, cur: 0, load_ms }), decode_ms: 0.0, decoded: 2, renderer: String::new() }));
    }
    let n = args.n_or(100) as usize;
    let nd = n.min(600);
    let t0 = Instant::now();
    // décodage une seule fois, sur 3 fils ; Image::from_rgba8 sur le fil UI (Image n'est pas Send)
    let p = std::path::PathBuf::from(&args.assets).join("images");
    let mut handles = vec![];
    for k in 0..3usize {
        let p = p.clone();
        handles.push(std::thread::spawn(move || {
            let mut out = vec![];
            let mut i = k;
            while i < nd {
                out.push((i, image::open(p.join(format!("img{:03}.png", i))).ok().map(|im| {
                    let im = im.into_rgba8();
                    let (w, h) = im.dimensions();
                    (im.into_raw(), w, h)
                })));
                i += 3;
            }
            out
        }));
    }
    let mut decoded: Vec<Option<Image>> = vec![None; nd];
    for h in handles {
        for (i, r) in h.join().unwrap() {
            if let Some((raw, w, hh)) = r {
                decoded[i] = Some(Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&raw, w, hh)));
            }
        }
    }
    let ndec = decoded.iter().filter(|x| x.is_some()).count();
    if ndec == 0 {
        return Err(format!("aucune image décodée depuis {}", p.display()));
    }
    let decode_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let f = |k: u32| fnv((i as u32).wrapping_mul(53).wrapping_add(k));
        let sp = |x: u32| ((x % 201) as f32 - 100.0) * 1.0;
        v.push(ImgItem {
            img: decoded[i % nd].clone().or_else(|| decoded.iter().flatten().next().cloned()).unwrap(),
            x0: (f(1) % 1000) as f32,
            y0: (f(2) % 600) as f32,
            vx: sp(f(3)),
            vy: sp(f(4)),
            sc: 0.25 + (f(5) % 76) as f32 / 100.0,
            rot: (f(6) % 360) as f32,
            rotv: ((f(7) % 181) as f32 - 90.0),
            op: 0.3 + (f(8) % 71) as f32 / 100.0,
        });
    }
    b.set_imgs(ModelRc::from(Rc::new(VecModel::from(v))));
    b.set_scn("images".into());
    Ok(Box::new(ImagesSc { n, big: None, decode_ms, decoded: ndec, renderer: String::new() }))
}
impl Scenario for ImagesSc {
    fn step(&mut self, t: f64, app: &MainWindow, measuring: bool) {
        let b = app.global::<Bench>();
        b.set_t(t as f32);
        if let Some(bg) = self.big.as_mut() {
            if measuring && bg.cur == 0 && t >= bg.switch_at {
                bg.cur = 1;
                b.set_big_image(bg.imgs[1].clone());
            }
            // pan (aller-retour), zoom 0.5 -> 1.5 -> 0.5, rotation continue
            let side = if bg.cur == 0 { 4096.0 } else { 8192.0 };
            let ph = (t * 0.5).rem_euclid(2.0);
            let tri = if ph < 1.0 { ph } else { 2.0 - ph };
            let zoom = 0.1 + 0.15 * (1.0 - (t * 0.8).cos() as f32) as f64;
            b.set_big_zoom(zoom as f32);
            b.set_big_x((-(side * zoom - 1000.0).max(0.0) * tri) as f32);
            b.set_big_y((-(side * zoom - 600.0).max(0.0) * (1.0 - tri)) as f32);
            b.set_big_rot((t * 20.0) as f32);
        }
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        ops.insert("images".into(), self.n.into());
        ops.insert("images_decoded".into(), self.decoded.into());
        ops.insert("decode_ms".into(), self.decode_ms.into());
        if let Some(bg) = &self.big {
            ops.insert("big_load_ms".into(), json!(bg.load_ms));
            ops.insert("big_final_side".into(), (if bg.cur == 0 { 4096 } else { 8192 }).into());
        }
        if self.renderer == "software" {
            ops.insert("not_rendered".into(), json!(software_not_rendered(&["rotation"])));
        }
    }
    fn notes(&self) -> String {
        "gfx-images: N Image elements sharing <= 600 decoded textures (decoded once on 3 threads with image 0.25.10, Image::from_rgba8 on the UI thread because slint::Image is not Send); position/rotation/opacity are bindings on the global `t` (transform-rotation, opacity, scaled width/height). variant big: single Image 4096^2 then 8192^2 (loaded before the measure, swapped at mid-window) with pan, zoom and rotation bound to properties set each frame.".into()
    }
}

// ================================================================ G5 layers
pub struct LayersSc {
    n: usize,
}
pub fn build_layers(args: &Args, app: &MainWindow) -> Result<Box<dyn Scenario>, String> {
    let n = args.n_or(16) as usize;
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let f = |k: u32| fnv((i as u32).wrapping_mul(97).wrapping_add(k));
        let sp = |x: u32| ((x % 201) as f32 - 100.0) * 1.0;
        v.push(GroupData { x0: (f(1) % 840) as f32, y0: (f(2) % 480) as f32, vx: sp(f(3)), vy: sp(f(4)), c1: col(f(5)), c2: col(f(6)), c3: col(f(7)) });
    }
    let b = app.global::<Bench>();
    b.set_groups(ModelRc::from(Rc::new(VecModel::from(v))));
    b.set_scn("layers".into());
    b.set_chrome(false);
    Ok(Box::new(LayersSc { n }))
}
impl Scenario for LayersSc {
    fn step(&mut self, t: f64, app: &MainWindow, _m: bool) {
        app.global::<Bench>().set_t(t as f32);
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        ops.insert("groups".into(), self.n.into());
    }
    fn notes(&self) -> String {
        "gfx-layers: N groups of 3 overlapping shapes under a Rectangle with opacity 0.5 (Slint renders an offscreen layer per semi-transparent group); positions bound to the global `t`. Blur: unsupported (Slint has no blur other than drop-shadow-blur).".into()
    }
}

// MoverData est déclaré ici pour la construction des animations (uis.rs) ; ré-export
pub fn mover(xa: f32, xb: f32, ya: f32, yb: f32, dur_ms: i32, c: Color, oa: f32) -> MoverData {
    MoverData { xa, xb, ya, yb, dur: dur_ms as i64, c, oa }
}
