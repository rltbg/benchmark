//! Canvas : N formes en `Mesh` à couleurs de sommets (dégradé linéaire 2 couleurs, anticrénelage par
//! anneau de « feathering » de 1 px comme le tessellateur d'epaint), ombres = `epaint::Shadow`
//! (RectShape::blur_width, flou par forme), polyligne = `Shape::line` (tessellée + feathering natif).
//! Clip : `Painter::with_clip_rect` = rectangle uniquement (pas de clip arrondi dans egui 0.37).
use crate::data::fnv;
use egui::epaint::{Mesh, Shadow, Vertex, WHITE_UV};
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

struct Sim {
    tmpl_start: u32,
    tmpl_len: u32,
    idx_start: u32,
    idx_len: u32,
    x0: f32,
    y0: f32,
    vx: f32,
    vy: f32,
    w: f32,
    h: f32,
    kind: u8,
    shadow: bool,
}

pub struct Canvas {
    sims: Vec<Sim>,
    tv: Vec<Vertex>, // sommets modèles (locaux) concaténés
    ti: Vec<u32>,    // indices locaux
    pub region: Vec2,
    pub graph: Option<Graph>,
    pub n_shapes: usize,
    pub n_shadows: usize,
    pub meshes_last_frame: usize,
}
pub struct Graph {
    pub vals: Vec<f32>,
    pub head: usize,
    pub size: Vec2,
    pub frame: u32,
}

fn rgb(h: u32) -> Color32 {
    Color32::from_rgb((h & 255) as u8, ((h >> 8) & 255) as u8, ((h >> 16) & 255) as u8)
}
fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}
fn tri(u: f32, r: f32) -> f32 {
    if r <= 0.0 {
        return 0.0;
    }
    let m = u.rem_euclid(2.0 * r);
    if m > r { 2.0 * r - m } else { m }
}

/// Normales « miter » sortantes (longueur telle que le décalage soit de 1 unité perpendiculairement
/// à chaque arête) d'un polygone convexe.
fn miter_normals(o: &[Pos2], ctr: Pos2) -> Vec<Vec2> {
    let n = o.len();
    (0..n)
        .map(|i| {
            let p = o[(i + n - 1) % n];
            let c = o[i];
            let q = o[(i + 1) % n];
            let mut n1 = (c - p).rot90().normalized();
            let mut n2 = (q - c).rot90().normalized();
            if n1.dot(c - ctr) < 0.0 { n1 = -n1; }
            if n2.dot(c - ctr) < 0.0 { n2 = -n2; }
            let m = (n1 + n2).normalized();
            let cos = m.dot(n1).max(0.3);
            m / cos
        })
        .collect()
}

impl Canvas {
    pub fn new(n: usize, region: Vec2, shadows: bool, graph: Option<Vec2>) -> Self {
        let mut sims = Vec::with_capacity(n);
        let mut tv: Vec<Vertex> = Vec::new();
        let mut ti: Vec<u32> = Vec::new();
        let mut n_shadows = 0;
        for i in 0..n as u32 {
            let h = fnv(i);
            let kind = (i % 3) as u8; // 0 rect, 1 cercle, 2 rect arrondi (1/3 chacun)
            let w = 20.0 + (fnv(i + 3) % 40) as f32;
            let hh = if kind == 1 { w } else { 16.0 + (fnv(i + 4) % 40) as f32 };
            let ang = (fnv(i + 1) % 360) as f32 * std::f32::consts::PI / 180.0;
            let sp = 60.0 + (fnv(i + 2) % 180) as f32;
            let c1 = rgb(h);
            let c2 = rgb(fnv(i + 11));
            let gdir = match fnv(i + 5) % 3 { 0 => Vec2::new(1.0, 0.0), 1 => Vec2::new(0.0, 1.0), _ => Vec2::new(0.7071, 0.7071) };
            let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(w, hh));
            let outline: Vec<Pos2> = match kind {
                0 => vec![rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()],
                1 => {
                    let r = w / 2.0;
                    let seg = 24;
                    (0..seg).map(|k| { let a = k as f32 / seg as f32 * std::f32::consts::TAU; Pos2::new(r + r * a.cos(), r + r * a.sin()) }).collect()
                }
                _ => {
                    let r = 8.0f32.min(w / 2.0).min(hh / 2.0);
                    let corners = [(rect.right_top() + Vec2::new(-r, r), -90.0f32), (rect.right_bottom() + Vec2::new(-r, -r), 0.0), (rect.left_bottom() + Vec2::new(r, -r), 90.0), (rect.left_top() + Vec2::new(r, r), 180.0)];
                    let mut v = Vec::new();
                    for (c, a0) in corners {
                        for s in 0..=4 {
                            let a = (a0 + s as f32 * 22.5).to_radians();
                            v.push(Pos2::new(c.x + r * a.cos(), c.y + r * a.sin()));
                        }
                    }
                    v
                }
            };
            // dégradé linéaire exact : couleur = fonction affine de la position, interpolée par le GPU/raster
            let grad = |p: Pos2| -> Color32 {
                let d = gdir.x * (p.x / w) + gdir.y * (p.y / hh);
                lerp(c1, c2, d / (gdir.x + gdir.y))
            };
            let ctr = Pos2::new(w / 2.0, hh / 2.0);
            let nm = miter_normals(&outline, ctr);
            let m = outline.len() as u32;
            let tmpl_start = tv.len() as u32;
            // [0] centre, [1..=m] anneau intérieur (−0,5 px, opaque), [m+1..=2m] anneau extérieur (+0,5 px, transparent)
            tv.push(Vertex { pos: ctr, uv: WHITE_UV, color: grad(ctr) });
            for (p, n) in outline.iter().zip(&nm) {
                let pi = *p - *n * 0.5;
                tv.push(Vertex { pos: pi, uv: WHITE_UV, color: grad(pi) });
            }
            for (p, n) in outline.iter().zip(&nm) {
                tv.push(Vertex { pos: *p + *n * 0.5, uv: WHITE_UV, color: Color32::TRANSPARENT });
            }
            let i0 = ti.len() as u32;
            for k in 0..m {
                let a = 1 + k;
                let b = 1 + (k + 1) % m;
                ti.extend_from_slice(&[0, a, b]); // éventail intérieur
                ti.extend_from_slice(&[a, b, a + m, b, b + m, a + m]); // bande de feathering
            }
            let mut s = Sim { tmpl_start, tmpl_len: 2 * m + 1, idx_start: i0, idx_len: ti.len() as u32 - i0, x0: 0.0, y0: 0.0, vx: 0.0, vy: 0.0, w, h: hh, kind, shadow: false };
            s.x0 = (fnv(i + 20) % 10000) as f32 / 10000.0 * (region.x - w).max(0.0);
            s.y0 = (fnv(i + 21) % 10000) as f32 / 10000.0 * (region.y - hh).max(0.0);
            s.vx = sp * ang.cos();
            s.vy = sp * ang.sin();
            if shadows && i % 10 == 0 { s.shadow = true; n_shadows += 1; }
            sims.push(s);
        }
        let g = graph.map(|size| Graph { vals: (0..10_000).map(|k| 0.5 + 0.4 * ((k as f32) * 0.05).sin()).collect(), head: 0, size, frame: 0 });
        Self { sims, tv, ti, region, graph: g, n_shapes: n, n_shadows, meshes_last_frame: 0 }
    }

    /// Dessine les formes (origin = coin haut-gauche de la zone, clip = zone) à l'instant t.
    /// Ordre de peinture respecté : l'ombre d'une forme est sous elle mais au-dessus des formes précédentes
    /// (le mesh est coupé à chaque forme ombrée : ~N/10 meshes + N/10 ombres par frame).
    pub fn paint(&mut self, painter: &Painter, origin: Pos2, t: f32) {
        let clip = Rect::from_min_size(origin, self.region);
        let p = painter.with_clip_rect(clip.intersect(painter.clip_rect()));
        p.rect_filled(clip, 16.0, Color32::from_rgb(24, 26, 32)); // fond arrondi (le CLIP reste rectangulaire)
        let sh = Shadow { offset: [2, 2], blur: 8, spread: 0, color: Color32::from_black_alpha(120) };
        let mut shapes: Vec<Shape> = Vec::with_capacity(self.n_shadows * 2 + 2);
        let mut m = Mesh::default();
        m.reserve_vertices(self.tv.len() / 4 + 64);
        for s in &self.sims {
            let pos = Self::pos(s, origin, self.region, t);
            if s.shadow {
                if !m.is_empty() { shapes.push(Shape::mesh(std::mem::take(&mut m))); }
                let cr = match s.kind { 1 => s.w / 2.0, 2 => 8.0, _ => 0.0 };
                shapes.push(sh.as_shape(Rect::from_min_size(pos, Vec2::new(s.w, s.h)), cr as u8).into());
            }
            let off = pos.to_vec2();
            let base = m.vertices.len() as u32;
            for v in &self.tv[s.tmpl_start as usize..(s.tmpl_start + s.tmpl_len) as usize] {
                m.vertices.push(Vertex { pos: v.pos + off, uv: v.uv, color: v.color });
            }
            for k in &self.ti[s.idx_start as usize..(s.idx_start + s.idx_len) as usize] {
                m.indices.push(base + *k);
            }
        }
        if !m.is_empty() { shapes.push(Shape::mesh(m)); }
        self.meshes_last_frame = shapes.len();
        p.extend(shapes);
    }
    fn pos(s: &Sim, origin: Pos2, region: Vec2, t: f32) -> Pos2 {
        Pos2::new(origin.x + tri(s.x0 + s.vx * t, region.x - s.w), origin.y + tri(s.y0 + s.vy * t, region.y - s.h))
    }

    /// Avance la polyligne d'un point (appelé une fois par frame, hors passes egui).
    pub fn advance_graph(&mut self) {
        let Some(g) = self.graph.as_mut() else { return };
        g.frame += 1;
        let v = (fnv(g.frame) % 1000) as f32 / 1000.0 * 0.3 + 0.35 + 0.3 * ((g.frame as f32) * 0.02).sin();
        g.vals[g.head] = v;
        g.head = (g.head + 1) % g.vals.len();
    }

    /// Graphe temps réel : polyligne de 10 000 points, retessellée à chaque frame.
    pub fn paint_graph(&self, painter: &Painter, origin: Pos2) {
        let Some(g) = self.graph.as_ref() else { return };
        let n = g.vals.len();
        let dx = g.size.x / (n - 1) as f32;
        let pts: Vec<Pos2> = (0..n).map(|k| Pos2::new(origin.x + k as f32 * dx, origin.y + g.size.y - g.vals[(g.head + k) % n] * g.size.y)).collect();
        let r = Rect::from_min_size(origin, g.size);
        let p = painter.with_clip_rect(r.intersect(painter.clip_rect()));
        p.rect_filled(r, 0.0, Color32::from_rgb(16, 18, 22));
        p.add(Shape::line(pts, Stroke::new(1.0, Color32::from_rgb(90, 220, 140))));
    }
}
