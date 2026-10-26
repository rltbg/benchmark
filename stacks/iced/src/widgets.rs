//! Widgets personnalises (iced::advanced) : liste virtualisee, grille d'images, couche d'ombres, glissement (nav).
use crate::data::*;
use crate::Message;
use iced::advanced::image::{self as aimage, Renderer as _};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Quad};
use iced::advanced::text::{self as atext, Renderer as _, Text};
use iced::advanced::widget::{tree, Tree};
use iced::advanced::{mouse, Renderer as _, Widget};
use iced::widget::image::Handle;
use iced::{alignment, Background, Border, Color, Element, Font, Length, Pixels, Point, Rectangle, Shadow, Size, Vector};
use std::sync::atomic::{AtomicU64, Ordering};

/// Nombre de lignes dessinees (cumul sur les draws) : remis a zero au debut de la mesure.
pub static ROWS_DRAWN: AtomicU64 = AtomicU64::new(0);
/// Nombre de passes `draw` du widget racine de scenario (preuve qu'une frame est reellement dessinee).
pub static DRAWS: AtomicU64 = AtomicU64::new(0);
pub static CELLS_DRAWN_LAST: AtomicU64 = AtomicU64::new(0);

/// Parite de frame (alternee par App::on_frame) pour le mode diagnostic --full-damage.
pub static FD_PARITY: AtomicU64 = AtomicU64::new(0);
/// Bornes de couche : en mode --full-damage, 0,25 px de moins une frame sur deux (voir FullDamage).
pub fn lbounds(b: Rectangle) -> Rectangle {
    if full_damage() { Rectangle { height: b.height - 0.25 * (FD_PARITY.load(Ordering::Relaxed) & 1) as f32, ..b } } else { b }
}
pub fn full_damage() -> bool { static E: std::sync::OnceLock<bool> = std::sync::OnceLock::new(); *E.get_or_init(|| std::env::var("ICED_FULL_DAMAGE").is_ok()) }
fn rgb(h: u32) -> Color {
    Color::from_rgb8((h & 255) as u8, ((h >> 8) & 255) as u8, ((h >> 16) & 255) as u8)
}

fn txt(content: String, size: f32, w: f32, h: f32, font: Font, ax: atext::Alignment, ay: alignment::Vertical) -> Text<String, Font> {
    Text {
        content,
        bounds: Size::new(w, h),
        size: Pixels(size),
        line_height: atext::LineHeight::Absolute(Pixels(size * 1.25)),
        font,
        align_x: ax,
        align_y: ay,
        shaping: atext::Shaping::Basic,
        wrapping: atext::Wrapping::None,
    }
}

// ------------------------------------------------------------------ liste virtualisee
pub struct VList<'a> {
    pub model: &'a ListModel,
    pub offset: f64,
    pub height: f32,
}

impl<'a> Widget<Message, iced::Theme, iced::Renderer> for VList<'a> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(self.height))
    }
    fn layout(&mut self, _tree: &mut Tree, _r: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        layout::atomic(limits, Length::Fill, Length::Fixed(self.height))
    }
    fn draw(&self, _tree: &Tree, r: &mut iced::Renderer, _theme: &iced::Theme, _style: &renderer::Style, layout: Layout<'_>, _cursor: mouse::Cursor, _vp: &Rectangle) {
        let b = layout.bounds();
        DRAWS.fetch_add(1, Ordering::Relaxed);
        let m = self.model;
        let off = self.offset;
        let first = m.row_at(off);
        let bottom = off + b.height as f64;
        let x0 = b.x;
        let x1 = b.x + b.width;
        let mut i = first;
        let mut drawn = 0u64;
        // iced_tiny_skia ne clippe le texte que par les bornes de COUCHE (le clip local de fill_text ne sert qu'au test d'intersection) :
        // les lignes coupees en haut/bas de la liste sont donc dessinees dans leur propre couche aux bornes de la ligne visible.
        let draw_row = |r: &mut iced::Renderer, i: usize, y: f32, h: f32, clip: Rectangle| {
            let bg = if i % 2 == 0 { Color::from_rgb8(34, 36, 42) } else { Color::from_rgb8(40, 42, 49) };
            r.fill_quad(Quad { bounds: Rectangle::new(Point::new(x0, y), Size::new(b.width, h)), ..Quad::default() }, bg);
            // pastille ronde 32 px
            r.fill_quad(
                Quad { bounds: Rectangle::new(Point::new(x0 + 16.0, y + h / 2.0 - 16.0), Size::new(32.0, 32.0)), border: Border { radius: 16.0.into(), ..Border::default() }, ..Quad::default() },
                rgb(fnv(i as u32)),
            );
            r.fill_text(txt(format!("Item #{i}"), 17.0, 300.0, 22.0, Font::DEFAULT, atext::Alignment::Left, alignment::Vertical::Top), Point::new(x0 + 64.0, y + 8.0), Color::WHITE, clip);
            let q = fnv(i as u32 + 7) % 100000;
            let sub = format!("Quote {} \u{2014} {}.{:02}", i, q / 100, q % 100);
            let sub3 = format!("{sub}   {sub}   {sub}   {sub}");
            r.fill_text(txt(sub3, 13.0, 4000.0, 18.0, Font::DEFAULT, atext::Alignment::Left, alignment::Vertical::Top), Point::new(x0 + 64.0, y + 32.0), Color::from_rgb8(170, 170, 170), clip);
            let pr = m.prices[i];
            r.fill_text(
                txt(format!("{}.{:02}", pr / 100, pr % 100), 16.0, 110.0, 20.0, Font::MONOSPACE, atext::Alignment::Right, alignment::Vertical::Top),
                Point::new(x1 - 12.0 - 110.0, y + h / 2.0 - 10.0),
                Color::from_rgb8(120, 230, 150),
                clip,
            );
        };
        r.with_layer(lbounds(b), |r| {
            while i < N_ROWS && (m.prefix[i] as f64) < bottom {
                let h = row_h(i as u32) as f32;
                let y = b.y + (m.prefix[i] as f64 - off) as f32; // position ecran en f64 puis f32
                let rect = Rectangle::new(Point::new(x0, y), Size::new(b.width, h));
                let clip = rect.intersection(&b).unwrap_or(rect);
                if y < b.y || y + h > b.y + b.height {
                    r.with_layer(clip, |r| draw_row(r, i, y, h, rect));
                } else {
                    draw_row(r, i, y, h, rect);
                }
                i += 1;
                drawn += 1;
            }
        });
        ROWS_DRAWN.fetch_add(drawn, Ordering::Relaxed);
    }
}
impl<'a> From<VList<'a>> for Element<'a, Message> {
    fn from(w: VList<'a>) -> Self {
        Element::new(w)
    }
}

// ------------------------------------------------------------------ couche d'ombres (quads avec Shadow)
pub struct ShadowLayer {
    /// (x, y, w, h, rayon) en coordonnees locales a la couche
    pub rects: Vec<(f32, f32, f32, f32, f32)>,
    pub size: Size,
}
impl Widget<Message, iced::Theme, iced::Renderer> for ShadowLayer {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size.width), Length::Fixed(self.size.height))
    }
    fn layout(&mut self, _t: &mut Tree, _r: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        layout::atomic(limits, Length::Fixed(self.size.width), Length::Fixed(self.size.height))
    }
    fn draw(&self, _t: &Tree, r: &mut iced::Renderer, _th: &iced::Theme, _s: &renderer::Style, layout: Layout<'_>, _c: mouse::Cursor, _vp: &Rectangle) {
        let b = layout.bounds();
        r.with_layer(lbounds(b), |r| {
            for &(x, y, w, h, rad) in &self.rects {
                r.fill_quad(
                    Quad {
                        bounds: Rectangle::new(Point::new(b.x + x, b.y + y), Size::new(w, h)),
                        border: Border { radius: rad.into(), ..Border::default() },
                        shadow: Shadow { color: Color::from_rgba8(0, 0, 0, 110.0 / 255.0), offset: Vector::new(2.0, 2.0), blur_radius: 8.0 },
                        snap: false,
                    },
                    Color::TRANSPARENT,
                );
            }
        });
    }
}
impl From<ShadowLayer> for Element<'_, Message> {
    fn from(w: ShadowLayer) -> Self {
        Element::new(w)
    }
}

// ------------------------------------------------------------------ grille d'images (zoom/pan scriptes)
pub const COLS: usize = 9;
pub const CELL: f32 = 136.0;
pub const N_IMG: usize = 600;

pub struct ImageGrid<'a> {
    pub handles: &'a [Option<Handle>],
    pub ox: f32,
    pub oy: f32,
    pub s: f32,
}
impl<'a> Widget<Message, iced::Theme, iced::Renderer> for ImageGrid<'a> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }
    fn layout(&mut self, _t: &mut Tree, _r: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        layout::atomic(limits, Length::Fill, Length::Fill)
    }
    fn draw(&self, _t: &Tree, r: &mut iced::Renderer, _th: &iced::Theme, _s: &renderer::Style, layout: Layout<'_>, _c: mouse::Cursor, _vp: &Rectangle) {
        let area = layout.bounds();
        DRAWS.fetch_add(1, Ordering::Relaxed);
        let (ox, oy, s) = (self.ox, self.oy, self.s);
        let rows = (N_IMG + COLS - 1) / COLS;
        let r0 = ((oy / CELL).floor() as i32).max(0) as usize;
        let r1 = (((oy + area.height / s) / CELL).ceil() as usize + 1).min(rows);
        let c0 = ((ox / CELL).floor() as i32).max(0) as usize;
        let c1 = (((ox + area.width / s) / CELL).ceil() as usize + 1).min(COLS);
        let mut shown = 0u64;
        r.with_layer(lbounds(area), |r| {
            r.fill_quad(Quad { bounds: area, ..Quad::default() }, Color::from_rgb8(20, 22, 26));
            for rr in r0..r1 {
                for c in c0..c1 {
                    let i = rr * COLS + c;
                    if i >= N_IMG {
                        continue;
                    }
                    let wx = 4.0 + c as f32 * CELL;
                    let wy = 4.0 + rr as f32 * CELL;
                    let rect = Rectangle::new(Point::new(area.x + (wx - ox) * s, area.y + (wy - oy) * s), Size::new(128.0 * s, 128.0 * s));
                    match &self.handles[i] {
                        Some(h) => {
                            let mut img = aimage::Image::new(h.clone());
                            img.filter_method = iced::widget::image::FilterMethod::Linear;
                            img.snap = false;
                            r.draw_image(img, rect, area);
                        }
                        None => r.fill_quad(Quad { bounds: rect, ..Quad::default() }, Color::from_rgb8(50, 50, 50)),
                    }
                    shown += 1;
                }
            }
        });
        CELLS_DRAWN_LAST.store(shown, Ordering::Relaxed);
    }
}
impl<'a> From<ImageGrid<'a>> for Element<'a, Message> {
    fn from(w: ImageGrid<'a>) -> Self {
        Element::new(w)
    }
}

// ------------------------------------------------------------------ diagnostic : regions de dommage
/// Enveloppe racine (option --full-damage / ICED_FULL_DAMAGE=1, hors spec) : dessine l'enfant dans une couche dont les bornes
/// changent d'un quart de pixel a chaque draw ; iced_tiny_skia (`Layer::damage`) renvoie alors les bornes entieres de la couche
/// comme region de dommage, ce qui fusionne toutes les regions fragmentees en une seule (equivalent a « tout redessiner »).
pub struct FullDamage<'a> {
    pub content: Element<'a, Message>,
}
impl<'a> Widget<Message, iced::Theme, iced::Renderer> for FullDamage<'a> {
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn layout(&mut self, tree: &mut Tree, r: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        let child = self.content.as_widget_mut().layout(&mut tree.children[0], r, limits);
        let size = child.size();
        layout::Node::with_children(size, vec![child])
    }
    fn draw(&self, tree: &Tree, r: &mut iced::Renderer, th: &iced::Theme, style: &renderer::Style, layout: Layout<'_>, cursor: mouse::Cursor, vp: &Rectangle) {
        let b = layout.bounds();
        let child = layout.children().next().unwrap();
        r.with_layer(lbounds(b), |r| {
            self.content.as_widget().draw(&tree.children[0], r, th, style, child, cursor, vp);
        });
    }
}
impl<'a> From<FullDamage<'a>> for Element<'a, Message> {
    fn from(w: FullDamage<'a>) -> Self {
        Element::new(w)
    }
}

// ------------------------------------------------------------------ glissement horizontal d'un enfant (nav)
pub struct Slide<'a> {
    pub content: Element<'a, Message>,
    pub dx: f32,
}
impl<'a> Widget<Message, iced::Theme, iced::Renderer> for Slide<'a> {
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn layout(&mut self, tree: &mut Tree, r: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        let child = self.content.as_widget_mut().layout(&mut tree.children[0], r, limits);
        let size = child.size();
        layout::Node::with_children(size, vec![child])
    }
    fn draw(&self, tree: &Tree, r: &mut iced::Renderer, th: &iced::Theme, style: &renderer::Style, layout: Layout<'_>, cursor: mouse::Cursor, vp: &Rectangle) {
        DRAWS.fetch_add(1, Ordering::Relaxed);
        let b = layout.bounds();
        let child = layout.children().next().unwrap();
        let dx = self.dx;
        let vp2 = Rectangle { x: vp.x - dx, ..*vp };
        r.with_layer(lbounds(b), |r| {
            r.with_translation(Vector::new(dx, 0.0), |r| {
                self.content.as_widget().draw(&tree.children[0], r, th, style, child, cursor, &vp2);
            });
        });
    }
}
impl<'a> From<Slide<'a>> for Element<'a, Message> {
    fn from(w: Slide<'a>) -> Self {
        Element::new(w)
    }
}

/// Fond uni plein (utilise sous les widgets).
#[allow(dead_code)]
pub fn bg(c: Color) -> Background {
    Background::Color(c)
}
