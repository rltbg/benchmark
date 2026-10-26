//! Canvas : N formes (iced::widget::canvas Frame/Path/Gradient), graphe temps reel ; ombres via ShadowLayer (quads).
use crate::data::fnv;
use crate::widgets::{ShadowLayer, DRAWS};
use crate::Message;
use iced::mouse;
use iced::widget::canvas::{self, gradient, Fill, Frame, Gradient, Path, Stroke, Style};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme};
use std::sync::atomic::Ordering;

pub struct Sim {
    x0: f32,
    y0: f32,
    vx: f32,
    vy: f32,
    w: f32,
    h: f32,
    kind: u8,
    c1: Color,
    c2: Color,
    gdir: (f32, f32),
    pub shadow: bool,
}
pub struct Graph {
    pub vals: Vec<f32>,
    pub head: usize,
    pub size: Size,
    pub frame: u32,
}
pub struct Shapes {
    pub sims: Vec<Sim>,
    pub region: Size,
    pub graph: Option<Graph>,
    pub n_shadows: usize,
}

fn rgb(h: u32) -> Color {
    Color::from_rgb8((h & 255) as u8, ((h >> 8) & 255) as u8, ((h >> 16) & 255) as u8)
}
fn tri(u: f32, r: f32) -> f32 {
    if r <= 0.0 {
        return 0.0;
    }
    let m = u.rem_euclid(2.0 * r);
    if m > r { 2.0 * r - m } else { m }
}

impl Shapes {
    pub fn new(n: usize, region: Size, shadows: bool, graph: Option<Size>) -> Self {
        let mut sims = Vec::with_capacity(n);
        let mut n_shadows = 0;
        for i in 0..n as u32 {
            let h = fnv(i);
            let kind = (i % 3) as u8; // 0 rect, 1 cercle, 2 rect arrondi
            let w = 20.0 + (fnv(i + 3) % 40) as f32;
            let hh = if kind == 1 { w } else { 16.0 + (fnv(i + 4) % 40) as f32 };
            let ang = (fnv(i + 1) % 360) as f32 * std::f32::consts::PI / 180.0;
            let sp = 60.0 + (fnv(i + 2) % 180) as f32;
            let gdir = match fnv(i + 5) % 3 { 0 => (1.0, 0.0), 1 => (0.0, 1.0), _ => (0.7071, 0.7071) };
            let shadow = shadows && i % 10 == 0;
            if shadow {
                n_shadows += 1;
            }
            sims.push(Sim {
                x0: (fnv(i + 20) % 10000) as f32 / 10000.0 * (region.width - w).max(0.0),
                y0: (fnv(i + 21) % 10000) as f32 / 10000.0 * (region.height - hh).max(0.0),
                vx: sp * ang.cos(),
                vy: sp * ang.sin(),
                w,
                h: hh,
                kind,
                c1: rgb(h),
                c2: rgb(fnv(i + 11)),
                gdir,
                shadow,
            });
        }
        let graph = graph.map(|size| Graph { vals: (0..10_000).map(|k| 0.5 + 0.4 * ((k as f32) * 0.05).sin()).collect(), head: 0, size, frame: 0 });
        Self { sims, region, graph, n_shadows }
    }

    #[inline]
    fn pos(&self, s: &Sim, t: f32) -> (f32, f32) {
        (tri(s.x0 + s.vx * t, self.region.width - s.w), tri(s.y0 + s.vy * t, self.region.height - s.h))
    }

    /// Un point ajoute par frame au graphe (appele depuis update).
    pub fn step_graph(&mut self) {
        if let Some(g) = self.graph.as_mut() {
            g.frame += 1;
            let v = (fnv(g.frame) % 1000) as f32 / 1000.0 * 0.3 + 0.35 + 0.3 * ((g.frame as f32) * 0.02).sin();
            g.vals[g.head] = v;
            g.head = (g.head + 1) % g.vals.len();
        }
    }

    pub fn shadow_layer(&self, t: f32) -> ShadowLayer {
        let rects = self
            .sims
            .iter()
            .filter(|s| s.shadow)
            .map(|s| {
                let (x, y) = self.pos(s, t);
                let rad = match s.kind { 1 => s.w / 2.0, 2 => 8.0, _ => 0.0 };
                (x, y, s.w, s.h, rad)
            })
            .collect();
        ShadowLayer { rects, size: self.region }
    }
}

pub struct ShapesProgram<'a> {
    pub shapes: &'a Shapes,
    pub t: f32,
}
impl<'a> canvas::Program<Message> for ShapesProgram<'a> {
    type State = ();
    fn draw(&self, _st: &(), renderer: &Renderer, _th: &Theme, bounds: Rectangle, _c: mouse::Cursor) -> Vec<canvas::Geometry> {
        DRAWS.fetch_add(1, Ordering::Relaxed);
        let mut frame = Frame::new(renderer, bounds.size());
        for s in &self.shapes.sims {
            let (x, y) = self.shapes.pos(s, self.t);
            let path = match s.kind {
                0 => Path::rectangle(Point::new(x, y), Size::new(s.w, s.h)),
                1 => Path::circle(Point::new(x + s.w / 2.0, y + s.h / 2.0), s.w / 2.0),
                _ => Path::rounded_rectangle(Point::new(x, y), Size::new(s.w, s.h), 8.0.into()),
            };
            // degrade lineaire 2 couleurs le long de gdir sur la boite de la forme
            let start = Point::new(x, y);
            let end = Point::new(x + s.w * s.gdir.0, y + s.h * s.gdir.1);
            let lin = gradient::Linear::new(start, end).add_stop(0.0, s.c1).add_stop(1.0, s.c2);
            frame.fill(&path, Fill { style: Style::Gradient(Gradient::Linear(lin)), ..Fill::default() });
        }
        vec![frame.into_geometry()]
    }
}

pub struct GraphProgram<'a> {
    pub graph: &'a Graph,
}
impl<'a> canvas::Program<Message> for GraphProgram<'a> {
    type State = ();
    fn draw(&self, _st: &(), renderer: &Renderer, _th: &Theme, bounds: Rectangle, _c: mouse::Cursor) -> Vec<canvas::Geometry> {
        DRAWS.fetch_add(1, Ordering::Relaxed);
        let g = self.graph;
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, g.size, Color::from_rgb8(16, 18, 22));
        let n = g.vals.len();
        let dx = g.size.width / (n - 1) as f32;
        let path = Path::new(|b| {
            for k in 0..n {
                let val = g.vals[(g.head + k) % n];
                let p = Point::new(k as f32 * dx, g.size.height - val * g.size.height);
                if k == 0 { b.move_to(p) } else { b.line_to(p) }
            }
        });
        frame.stroke(&path, Stroke::default().with_width(1.0).with_color(Color::from_rgb8(90, 220, 140)));
        vec![frame.into_geometry()]
    }
}
