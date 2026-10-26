// Texte : T1 text-big, T2 text-labels, T3 text-edit, T4 text-rich.
use super::Scenario;
use crate::data::*;
use crate::{Args, Bench, MainWindow};
use serde_json::{json, Map, Value};
use slint::{ComponentHandle, Model, ModelRc, SharedString, StyledText, VecModel};
use std::fmt::Write;
use std::rc::Rc;
use std::time::{Duration, Instant};

fn read_text(args: &Args, mb: u64) -> Result<String, String> {
    let name = format!("text{}mb.txt", mb);
    let p = std::path::Path::new(&args.assets).join(&name);
    std::fs::read_to_string(&p).map_err(|e| format!("lecture de {} impossible : {e} (générer avec harness/gen_assets.py --text-mb {mb})", p.display()))
}

// ================================================================ T1
pub struct BigSc {
    mb: u64,
    sc: Scroller,
    built: Instant,
    first_display_ms: Option<f64>,
    layout_ms: f64,
    selected_all: bool,
    content_h: f32,
    visible_h: f32,
    final_off: f64,
    bytes: usize,
}
pub fn build_big(args: &Args, app: &MainWindow) -> Result<Box<dyn Scenario>, String> {
    let mb = args.n_or(1);
    let t = read_text(args, mb)?;
    let bytes = t.len();
    let b = app.global::<Bench>();
    b.set_chrome(false);
    let built = Instant::now();
    b.set_big_text(SharedString::from(t.as_str()));
    drop(t);
    b.set_scn("text-big".into());
    Ok(Box::new(BigSc { mb, sc: Scroller::new(Script::constant(30000.0)), built, first_display_ms: None, layout_ms: 0.0, selected_all: false, content_h: 0.0, visible_h: 0.0, final_off: 0.0, bytes }))
}
impl Scenario for BigSc {
    fn on_frame_end(&mut self, now: Instant, dur: Duration) {
        if self.first_display_ms.is_none() {
            self.first_display_ms = Some(now.duration_since(self.built).as_secs_f64() * 1000.0);
            self.layout_ms = dur.as_secs_f64() * 1000.0;
        }
    }
    fn begin_measure(&mut self, app: &MainWindow) {
        self.sc.reset();
        app.set_text_y(0.0);
    }
    fn step(&mut self, t: f64, app: &MainWindow, measuring: bool) {
        self.content_h = app.get_text_content_h();
        self.visible_h = app.get_text_visible_h();
        let max = (self.content_h - self.visible_h).max(0.0) as f64;
        let off = self.sc.advance(if measuring { t } else { 0.0 }, max);
        self.final_off = off;
        app.set_text_y(-(off as f32));
        if measuring && !self.selected_all && t >= 5.0 {
            self.selected_all = true;
            app.invoke_select_all();
        }
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        ops.insert("megabytes".into(), self.mb.into());
        ops.insert("bytes".into(), self.bytes.into());
        let fd = self.first_display_ms.unwrap_or(-1.0);
        ops.insert("time_to_first_display_ms".into(), fd.into());
        // la mise en page n'a pas lieu pendant le dessin (la 1re frame ne dure que `first_frame_ms`) mais pendant show()
        ops.insert("first_frame_ms".into(), self.layout_ms.into());
        ops.insert("layout_ms".into(), (fd - self.layout_ms).max(0.0).into());
        ops.insert("content_height".into(), (self.content_h as f64).into());
        ops.insert("final_offset".into(), self.final_off.into());
        ops.insert("select_all".into(), self.selected_all.into());
    }
    fn notes(&self) -> String {
        "text-big: std-widgets TextEdit (read-only, word-wrap, 14 px) holding the whole document as one SharedString; select-all() called at t = 5 s; content-y driven by Rust (30000 px/s, bounce). time_to_first_display_ms = from setting the text to the end of the first frame; layout_ms = duration of that first frame (layout + paint).".into()
    }
}

// ================================================================ T2
pub struct LabelsSc {
    n: usize,
    model: Rc<VecModel<SharedString>>,
    counter: u32,
    buf: String,
}
pub fn build_labels(args: &Args, app: &MainWindow) -> Result<Box<dyn Scenario>, String> {
    let n = args.n_or(500) as usize;
    let model = Rc::new(VecModel::from(vec![SharedString::from("0"); n]));
    let b = app.global::<Bench>();
    b.set_chrome(false);
    b.set_labels(ModelRc::from(model.clone()));
    b.set_scn("labels".into());
    Ok(Box::new(LabelsSc { n, model, counter: 0, buf: String::with_capacity(32) }))
}
impl Scenario for LabelsSc {
    fn step(&mut self, _t: f64, _app: &MainWindow, _m: bool) {
        self.counter = self.counter.wrapping_add(1);
        for i in 0..self.n {
            self.buf.clear();
            let _ = write!(self.buf, "{} {:08x}", self.counter, fnv(self.counter.wrapping_add(i as u32)));
            self.model.set_row_data(i, SharedString::from(self.buf.as_str()));
        }
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        ops.insert("labels".into(), self.n.into());
        ops.insert("text_updates".into(), (self.counter as u64 * self.n as u64).into());
    }
    fn notes(&self) -> String {
        "text-labels: N Text elements in an 11-column grid (112x14 px cells, 11 px font) bound to a VecModel<SharedString>; each frame every label gets a new text (counter + 8 hex digits of fnv) through set_row_data. Labels beyond the 1280x720 window are culled by the renderer.".into()
    }
}

// ================================================================ T3
pub struct EditSc {
    mid: i32,
    len: usize,
}
pub fn build_edit(args: &Args, app: &MainWindow) -> Result<Box<dyn Scenario>, String> {
    let t = read_text(args, 1)?;
    let mut mid = t.len() / 2;
    while !t.is_char_boundary(mid) {
        mid += 1;
    }
    let len = t.len();
    let b = app.global::<Bench>();
    b.set_chrome(false);
    b.set_edit_text(SharedString::from(t.as_str()));
    b.set_edit_nowrap(args.pick("wrap", &["none", "word"], "word") == "none");
    b.set_scn("text-edit".into());
    Ok(Box::new(EditSc { mid: mid as i32, len }))
}
impl Scenario for EditSc {
    fn animated(&self) -> bool {
        false // piloté par les touches
    }
    fn on_ready(&mut self, app: &MainWindow) {
        // curseur au milieu du document ; focus clavier sur l'éditeur
        app.invoke_edit_focus(self.mid);
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        ops.insert("document_bytes".into(), self.len.into());
        ops.insert("cursor_byte".into(), self.mid.into());
    }
    fn notes(&self) -> String {
        "text-edit: std-widgets TextEdit with the 1 MB document, caret placed in the middle (set-selection-offsets), keyboard focus given to the editor (and X11 focus by SetInputFocus under Xvfb). The 200x200 square toggles inside the TextEdit `edited` callback, i.e. after the character has been applied to the text; ops.key_events counts those callbacks.".into()
    }
}

// ================================================================ T4
pub struct RichSc {
    n: usize,
    sc: Scroller,
    md_bytes: usize,
    first: Option<f64>,
    built: Instant,
    content_h: f32,
    final_off: f64,
}
const LATIN: [&str; 8] = ["lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit"];
const CJK: [&str; 4] = ["日本語", "漢字テキスト", "中文字符", "한국어"];
const EMOJI: [&str; 4] = ["🚀", "😀", "👩‍👩‍👧‍👦", "🎉"];
const ARABIC: [&str; 3] = ["مرحبا", "بالعالم", "نص عربي"];
const HEBREW: [&str; 3] = ["שלום", "עולם", "טקסט"];
fn rich_markdown(n: usize) -> String {
    let mut s = String::with_capacity(n * 24);
    for i in 0..n as u32 {
        let f = fnv(i);
        let word = match f % 10 {
            0 | 1 | 2 | 3 | 4 => LATIN[(f >> 4) as usize % 8],
            5 | 6 => CJK[(f >> 4) as usize % 4],
            7 => EMOJI[(f >> 4) as usize % 4],
            8 => ARABIC[(f >> 4) as usize % 3],
            _ => HEBREW[(f >> 4) as usize % 3],
        };
        match (f >> 8) % 7 {
            0 => s.push_str(word),
            1 => {
                let _ = write!(s, "**{}**", word);
            }
            2 => {
                let _ = write!(s, "*{}*", word);
            }
            3 => {
                let _ = write!(s, "<font color=\"#{:06x}\">{}</font>", fnv(i + 1) & 0xffffff, word);
            }
            4 => {
                let _ = write!(s, "`{}`", word);
            }
            5 => {
                let _ = write!(s, "<u>{}</u>", word);
            }
            _ => {
                let _ = write!(s, "~~{}~~", word);
            }
        }
        s.push(' ');
        if i % 40 == 39 {
            s.push_str("\n\n");
        }
    }
    s
}
pub fn build_rich(args: &Args, app: &MainWindow) -> Result<Box<dyn Scenario>, String> {
    let n = args.n_or(1000) as usize;
    let md = rich_markdown(n);
    let b = app.global::<Bench>();
    b.set_chrome(false);
    let st = StyledText::from_markdown(&md).map_err(|e| format!("StyledText::from_markdown : {e}"))?;
    b.set_rich(st);
    b.set_scn("text-rich".into());
    Ok(Box::new(RichSc { n, sc: Scroller::new(Script::constant(10000.0)), md_bytes: md.len(), first: None, built: Instant::now(), content_h: 0.0, final_off: 0.0 }))
}
impl Scenario for RichSc {
    fn on_frame_end(&mut self, now: Instant, _d: Duration) {
        if self.first.is_none() {
            self.first = Some(now.duration_since(self.built).as_secs_f64() * 1000.0);
        }
    }
    fn begin_measure(&mut self, app: &MainWindow) {
        self.sc.reset();
        app.global::<Bench>().set_scroll_y(0.0);
    }
    fn step(&mut self, t: f64, app: &MainWindow, measuring: bool) {
        self.content_h = app.get_rich_h();
        let max = (self.content_h - 720.0).max(0.0) as f64;
        let off = self.sc.advance(if measuring { t } else { 0.0 }, max);
        self.final_off = off;
        app.global::<Bench>().set_scroll_y(off as f32);
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        ops.insert("spans".into(), self.n.into());
        ops.insert("markdown_bytes".into(), self.md_bytes.into());
        ops.insert("time_to_first_display_ms".into(), self.first.unwrap_or(-1.0).into());
        ops.insert("content_height".into(), (self.content_h as f64).into());
        ops.insert("final_offset".into(), self.final_off.into());
        ops.insert("not_rendered".into(), json!(["font family and font size changes per span (StyledText has only default-font-family / default-font-size, no per-span size or family)"]));
    }
    fn notes(&self) -> String {
        "text-rich: PARTIAL. One StyledText element (CommonMark subset + <font color> / <u>) built with StyledText::from_markdown from N spans (bold, italic, code, strike-through, underline, colour, plain; Latin, CJK, emoji, Arabic, Hebrew), scrolled by moving the element inside a clipping Rectangle. Per-span font family and font size are not expressible in StyledText: they are not rendered (reported in ops.not_rendered).".into()
    }
}
