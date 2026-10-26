//! Scenarios : liste virtualisee, texte, images, navigation.
use crate::data::*;
use egui::{Align2, Color32, ColorImage, FontId, Painter, Pos2, Rect, Sense, TextureHandle, TextureOptions, Ui, Vec2};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

fn rgb(h: u32) -> Color32 {
    Color32::from_rgb((h & 255) as u8, ((h >> 8) & 255) as u8, ((h >> 16) & 255) as u8)
}

// ---------------------------------------------------------------- liste
pub fn paint_row(p: &Painter, m: &ListModel, i: usize, x0: f32, x1: f32, y: f32) {
    let h = row_h(i as u32) as f32;
    let r = Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + h));
    p.rect_filled(r, 0.0, if i % 2 == 0 { Color32::from_rgb(34, 36, 42) } else { Color32::from_rgb(40, 42, 49) });
    p.circle_filled(Pos2::new(x0 + 32.0, y + h / 2.0), 16.0, rgb(fnv(i as u32)));
    p.text(Pos2::new(x0 + 64.0, y + 8.0), Align2::LEFT_TOP, format!("Item #{i}"), FontId::proportional(17.0), Color32::WHITE);
    let q = fnv(i as u32 + 7) % 100000;
    let sub = format!("Quote {} \u{2014} {}.{:02}", i, q / 100, q % 100);
    // sous-titre sur une ligne, clippé à la zone texte (à gauche du prix)
    let tr = Rect::from_min_max(Pos2::new(x0 + 64.0, y), Pos2::new(x1 - 120.0, y + h));
    let pc = p.with_clip_rect(tr.intersect(p.clip_rect()));
    pc.text(Pos2::new(x0 + 64.0, y + 30.0), Align2::LEFT_TOP, sub, FontId::proportional(13.0), Color32::from_gray(170));
    let pr = m.prices[i];
    p.text(Pos2::new(x1 - 12.0, y + h / 2.0), Align2::RIGHT_CENTER, format!("{}.{:02}", pr / 100, pr % 100), FontId::monospace(16.0), Color32::from_rgb(120, 230, 150));
}

/// Liste virtualisee a hauteurs variables : ScrollArea (barre + clip) + sommes prefixes + recherche binaire.
pub fn draw_list(ui: &mut Ui, m: &mut ListModel, off: f64) {
    let total = m.total() as f32;
    egui::ScrollArea::vertical().id_salt("list").auto_shrink([false, false]).vertical_scroll_offset(off as f32).show_viewport(ui, |ui, _vp| {
        ui.set_height(total);
        let clip = ui.clip_rect();
        let p = ui.painter().clone();
        let (x0, x1) = (clip.left(), clip.right() - 12.0);
        let first = m.row_at(off);
        let bottom = off + clip.height() as f64;
        let mut i = first;
        let mut drawn = 0;
        while i < N_ROWS && (m.prefix[i] as f64) < bottom {
            let y = clip.top() + (m.prefix[i] as f64 - off) as f32; // coordonnee ecran calculee en f64 (pas de perte f32 a 8,8 Mpx)
            paint_row(&p, m, i, x0, x1, y);
            i += 1;
            drawn += 1;
        }
        m.rows_drawn += drawn;
    });
}

// ---------------------------------------------------------------- texte
/// Texte 1 Mo : idiome documenté de `TextEdit` pour un texte « sélectionnable mais non éditable »
/// (`TextEdit::multiline(&mut &str)`, cf. docs.rs/egui/0.37.0/egui/widgets/text_edit/struct.TextEdit.html),
/// dans une `ScrollArea` verticale ; un seul Galley pour tout le Mo (mis en cache par egui, lignes
/// hors clip éliminées par `coarse_tessellation_culling`).
pub struct TextView {
    pub text: String,
    pub load_ms: f64,
    pub layout_ms: Option<f64>,
    pub content_h: f32,
    pub select_all_done: bool,
    pub selected_chars: usize,
    pub galley_rows: usize,
}
impl TextView {
    pub fn load(path: &str) -> Self {
        let t = Instant::now();
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("text1mb.txt ({path}): {e}"));
        Self { text, load_ms: t.elapsed().as_secs_f64() * 1000.0, layout_ms: None, content_h: 0.0, select_all_done: false, selected_chars: 0, galley_rows: 0 }
    }
    /// Retourne l'offset scripté appliqué (aller-retour 3000 px/s).
    pub fn draw(&mut self, ui: &mut Ui, t: f64, measuring: bool) -> f64 {
        let view_h = ui.available_height() as f64;
        let max = (self.content_h as f64 - view_h).max(0.0);
        let off = ping(t * 3000.0, max);
        let want_select = measuring && t >= 5.0 && !self.select_all_done;
        let t0 = Instant::now();
        let text = &self.text;
        let out = egui::ScrollArea::vertical().id_salt("text").auto_shrink([false, false]).animated(false).vertical_scroll_offset(off as f32).show(ui, |ui| {
            let mut s: &str = text;
            let out = egui::TextEdit::multiline(&mut s).id_salt("text1mb").desired_width(f32::INFINITY).font(egui::TextStyle::Body).show(ui);
            if want_select {
                // « tout sélectionner » par API publique : focus + TextEditState::cursor (CCursorRange::select_all)
                out.response.request_focus();
                let mut st = out.state.clone();
                st.cursor.set_char_range(Some(egui::text::CCursorRange::select_all(&out.galley)));
                st.store(ui.ctx(), out.response.id);
            }
            (out.galley.rows.len(), out.cursor_range.map(|r| { let r = r.as_sorted_char_range(); r.end.0 - r.start.0 }).unwrap_or(0))
        });
        if self.layout_ms.is_none() { self.layout_ms = Some(t0.elapsed().as_secs_f64() * 1000.0); }
        if want_select { self.select_all_done = true; }
        self.galley_rows = out.inner.0;
        self.selected_chars = out.inner.1;
        self.content_h = out.content_size.y;
        off
    }
}

/// aller-retour 0..max
pub fn ping(x: f64, max: f64) -> f64 {
    if max <= 0.0 { return 0.0; }
    let m = x.rem_euclid(2.0 * max);
    if m > max { 2.0 * max - m } else { m }
}

// ---------------------------------------------------------------- images
pub struct Images {
    req_tx: Sender<usize>,
    res_rx: Receiver<(usize, ColorImage, f64)>,
    pub tex: Vec<Option<TextureHandle>>,
    requested: Vec<bool>,
    pub decoded: u64,
    pub decode_total_ms: f64,
}
pub const COLS: usize = 9;
pub const CELL: f32 = 136.0;
pub const N_IMG: usize = 600;
impl Images {
    pub fn new(dir: &str) -> Self {
        let (req_tx, req_rx) = channel::<usize>();
        let (res_tx, res_rx) = channel();
        let req_rx = Arc::new(Mutex::new(req_rx));
        for _ in 0..2 {
            let (rx, tx, d) = (req_rx.clone(), res_tx.clone(), dir.to_string());
            std::thread::spawn(move || loop {
                let Ok(i) = rx.lock().unwrap().recv() else { return };
                let t = Instant::now();
                let path = format!("{d}/images/img{i:03}.png");
                if let Ok(img) = image::open(&path) {
                    // vignette 128x128 (spec) calculée hors thread UI
                    let img = image::imageops::resize(&img.to_rgba8(), 128, 128, image::imageops::FilterType::Triangle);
                    let ci = ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
                    let _ = tx.send((i, ci, t.elapsed().as_secs_f64() * 1000.0));
                }
            });
        }
        Self { req_tx, res_rx, tex: (0..N_IMG).map(|_| None).collect(), requested: vec![false; N_IMG], decoded: 0, decode_total_ms: 0.0 }
    }
    /// Recupere les images decodees (max 16 uploads de textures 128x128 par frame).
    pub fn poll(&mut self, ctx: &egui::Context) -> bool {
        let mut any = false;
        for _ in 0..16 {
            match self.res_rx.try_recv() {
                Ok((i, ci, ms)) => {
                    self.tex[i] = Some(ctx.load_texture(format!("img{i}"), ci, TextureOptions::LINEAR));
                    self.decoded += 1;
                    self.decode_total_ms += ms;
                    any = true;
                }
                Err(_) => break,
            }
        }
        any
    }
    fn request(&mut self, i: usize) {
        if i < N_IMG && !self.requested[i] {
            self.requested[i] = true;
            let _ = self.req_tx.send(i);
        }
    }
    /// Grille 9 colonnes, vignettes 128x128 ; vue = origine monde (ox, oy), echelle s (zoom/pan scripte).
    pub fn draw(&mut self, ui: &mut Ui, ox: f32, oy: f32, s: f32) -> usize {
        let area = ui.max_rect();
        let p = ui.painter().with_clip_rect(area);
        p.rect_filled(area, 0.0, Color32::from_rgb(20, 22, 26));
        let rows = (N_IMG + COLS - 1) / COLS;
        let r0 = ((oy / CELL).floor() as i32).max(0) as usize;
        let r1 = (((oy + area.height() / s) / CELL).ceil() as usize + 1).min(rows);
        let c0 = ((ox / CELL).floor() as i32).max(0) as usize;
        let c1 = (((ox + area.width() / s) / CELL).ceil() as usize + 1).min(COLS);
        let mut shown = 0;
        // prefetch : 2 rangees avant/apres
        for r in r0.saturating_sub(2)..(r1 + 2).min(rows) {
            for c in 0..COLS { self.request(r * COLS + c); }
        }
        for r in r0..r1 {
            for c in c0..c1 {
                let i = r * COLS + c;
                if i >= N_IMG { continue; }
                let wx = 4.0 + c as f32 * CELL;
                let wy = 4.0 + r as f32 * CELL;
                let rect = Rect::from_min_size(area.min + Vec2::new((wx - ox) * s, (wy - oy) * s), Vec2::splat(128.0 * s));
                match &self.tex[i] {
                    Some(t) => { p.image(t.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE); }
                    None => { p.rect_filled(rect, 0.0, Color32::from_gray(50)); }
                }
                shown += 1;
            }
        }
        let _ = Sense::hover();
        shown
    }
}

// ---------------------------------------------------------------- navigation
#[derive(Clone, Copy, Debug)]
pub enum Step { Push(usize, usize), Pop(usize, usize), Modal }
pub struct Nav {
    pub steps: Vec<(Step, f64)>,
    pub transitions_done: u64,
    pub modals_done: u64,
    cur: usize,
    step_start: f64,
    pub frames_in_motion: u64,
}
impl Nav {
    pub fn new() -> Self {
        let mut steps = Vec::new();
        for _ in 0..5 {
            steps.push((Step::Push(0, 1), 0.3));
            steps.push((Step::Push(1, 2), 0.3));
            steps.push((Step::Pop(2, 1), 0.3));
            steps.push((Step::Pop(1, 0), 0.3));
            steps.push((Step::Modal, 0.2)); // ouverture
            steps.push((Step::Modal, 0.2)); // fermeture
        }
        Self { steps, transitions_done: 0, modals_done: 0, cur: 0, step_start: 0.0, frames_in_motion: 0 }
    }
    fn screen(p: &Painter, rect: Rect, idx: usize, t: f64) {
        let hue = [Color32::from_rgb(30, 50, 80), Color32::from_rgb(60, 40, 80), Color32::from_rgb(40, 70, 60)][idx];
        p.rect_filled(rect, 0.0, hue);
        p.text(rect.min + Vec2::new(24.0, 16.0), Align2::LEFT_TOP, format!("Ecran {}", idx + 1), FontId::proportional(28.0), Color32::WHITE);
        for k in 0..24 {
            let y = rect.top() + 70.0 + k as f32 * 26.0;
            let w = 300.0 + 40.0 * (((k + idx) as f32) * 0.7).sin();
            p.rect_filled(Rect::from_min_size(Pos2::new(rect.left() + 24.0, y), Vec2::new(w, 20.0)), 4.0, Color32::from_white_alpha(40));
            p.text(Pos2::new(rect.left() + 32.0, y + 2.0), Align2::LEFT_TOP, format!("Ligne {k} de l'ecran {}", idx + 1), FontId::proportional(14.0), Color32::from_gray(230));
        }
        let _ = t;
    }
    /// Retourne true tant qu'une animation est en cours (toujours, la sequence boucle).
    pub fn draw(&mut self, ui: &mut Ui, t: f64) {
        let area = ui.max_rect();
        let p = ui.painter().with_clip_rect(area);
        // avance dans la sequence
        loop {
            let (_, d) = self.steps[self.cur % self.steps.len()];
            if t - self.step_start >= d {
                let (st, d) = self.steps[self.cur % self.steps.len()];
                match st { Step::Modal => { self.modals_done += 1 } _ => { self.transitions_done += 1 } }
                self.step_start += d;
                self.cur += 1;
            } else { break; }
        }
        let (st, d) = self.steps[self.cur % self.steps.len()];
        let f = ((t - self.step_start) / d).clamp(0.0, 1.0) as f32;
        let e = f * f * (3.0 - 2.0 * f); // smoothstep
        let w = area.width();
        let sl = |x: f32| area.translate(Vec2::new(x, 0.0));
        match st {
            Step::Push(a, b) => { Self::screen(&p, sl(-e * w * 0.3), a, t); Self::screen(&p, sl((1.0 - e) * w), b, t); }
            Step::Pop(a, b) => { Self::screen(&p, sl(-(1.0 - e) * w * 0.3), b, t); Self::screen(&p, sl(e * w), a, t); }
            Step::Modal => {
                let idx = (self.cur / 6) % 3;
                let _ = idx;
                // ecran courant = celui atteint apres les 4 transitions (ecran 0)
                Self::screen(&p, area, 0, t);
                let open = self.cur % 2 == 0;
                let a = if open { e } else { 1.0 - e };
                p.rect_filled(area, 0.0, Color32::from_black_alpha((150.0 * a) as u8));
                let r = Rect::from_center_size(area.center(), Vec2::new(420.0, 240.0));
                p.rect_filled(r, 12.0, Color32::from_rgba_unmultiplied(245, 245, 250, (255.0 * a) as u8));
                p.text(r.center(), Align2::CENTER_CENTER, "Modale", FontId::proportional(26.0), Color32::from_black_alpha((255.0 * a) as u8));
            }
        }
        self.frames_in_motion += 1;
    }
}
