// Arbre d'interface, liaisons, animations : U1 ui-tree, U2 ui-bind, U3 ui-anim.
use super::gfx::mover;
use super::Scenario;
use crate::data::*;
use crate::{Args, Bench, MainWindow};
use serde_json::{Map, Value};
use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::fmt::Write;
use std::rc::Rc;

// ================================================================ U1
pub struct TreeSc {
    leaves: usize,
}
pub fn build_tree(args: &Args, app: &MainWindow) -> Result<Box<dyn Scenario>, String> {
    let leaves = args.n_or(1000) as usize;
    let b = app.global::<Bench>();
    b.set_chrome(false);
    b.set_tree_n(leaves.div_ceil(10) as i32);
    b.set_scn("tree".into());
    let manual = args.pick("layout", &["layout", "manual"], "layout") == "manual";
    b.set_mode((if manual { "manual" } else { "layout" }).into());
    Ok(Box::new(TreeSc { leaves }))
}
impl Scenario for TreeSc {
    fn animated(&self) -> bool {
        false
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        ops.insert("leaves".into(), self.leaves.into());
        ops.insert("containers".into(), self.leaves.div_ceil(10).into());
        ops.insert("elements".into(), (self.leaves * 2 + self.leaves.div_ceil(10)).into());
    }
    fn notes(&self) -> String {
        "ui-tree: N = number of leaves; N/10 containers (HorizontalLayout of 10 leaves = Rectangle + Text) as a `for` repeater inside a VerticalLayout inside a Flickable (so that the window keeps no minimum size); everything is instantiated (no virtualization). The resize storm is done by the harness (xdotool) or by the app with --variant selfresize=1 (Window::set_size, 100 steps); ops.resize_frame_ms = delay between each Resized event and the end of the next frame.".into()
    }
}

// ================================================================ U2
pub struct BindSc {
    n: usize,
    derived: bool,
    model: Option<Rc<VecModel<SharedString>>>,
    counter: u32,
    buf: String,
}
pub fn build_bind(args: &Args, app: &MainWindow) -> Result<Box<dyn Scenario>, String> {
    let n = args.n_or(1000) as usize;
    let derived = args.pick("bind", &["model", "derived"], "model") == "derived";
    let b = app.global::<Bench>();
    b.set_chrome(false);
    let model = if derived {
        b.set_nlabels(n as i32);
        b.set_scn("bind-derived".into());
        None
    } else {
        let m = Rc::new(VecModel::from((0..n).map(|i| SharedString::from(i.to_string())).collect::<Vec<_>>()));
        b.set_labels(ModelRc::from(m.clone()));
        b.set_scn("bind-model".into());
        Some(m)
    };
    Ok(Box::new(BindSc { n, derived, model, counter: 0, buf: String::with_capacity(16) }))
}
impl Scenario for BindSc {
    fn step(&mut self, _t: f64, app: &MainWindow, _m: bool) {
        self.counter = self.counter.wrapping_add(1);
        if self.derived {
            // 1 propriété écrite ; N liaisons `Bench.tick + i` se réévaluent
            app.global::<Bench>().set_tick(self.counter as i32);
        } else if let Some(m) = &self.model {
            // N propriétés changent : N x set_row_data -> row_changed -> liaison `text` de chaque Text
            for i in 0..self.n {
                self.buf.clear();
                let _ = write!(self.buf, "{}", self.counter.wrapping_add(i as u32));
                m.set_row_data(i, SharedString::from(self.buf.as_str()));
            }
        }
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        ops.insert("labels".into(), self.n.into());
        ops.insert("bind".into(), (if self.derived { "derived" } else { "model" }).into());
        ops.insert("property_changes".into(), (self.counter as u64 * self.n as u64).into());
        ops.insert("counter_final".into(), self.counter.into());
    }
    fn notes(&self) -> String {
        if self.derived {
            "ui-bind bind=derived: Text { text: Bench.tick + i } for N instances: one property written per frame, the N dependent bindings are invalidated and re-evaluated when rendered (labels outside the window are culled and never re-read).".into()
        } else {
            "ui-bind bind=model: a VecModel<SharedString> bound by `for s[i] in labels: Text { text: s }`; each frame Rust calls set_row_data on all N rows (N row_changed notifications -> N `text` property updates).".into()
        }
    }
}

// ================================================================ U3
pub struct AnimSc {
    n: usize,
}
pub fn build_anim(args: &Args, app: &MainWindow) -> Result<Box<dyn Scenario>, String> {
    let n = args.n_or(1000) as usize;
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let f = |k: u32| fnv((i as u32).wrapping_mul(61).wrapping_add(k));
        let c = Color::from_rgb_u8(f(1) as u8, (f(1) >> 8) as u8, (f(1) >> 16) as u8);
        v.push(mover((f(2) % 1260) as f32, (f(3) % 1260) as f32, (f(4) % 700) as f32, (f(5) % 700) as f32, 1000 + (f(6) % 2000) as i32, c, 0.2 + (f(7) % 40) as f32 / 100.0));
    }
    let b = app.global::<Bench>();
    b.set_chrome(false);
    b.set_movers(ModelRc::from(Rc::new(VecModel::from(v))));
    b.set_go(false);
    b.set_scn("movers".into());
    Ok(Box::new(AnimSc { n }))
}
impl Scenario for AnimSc {
    fn animated(&self) -> bool {
        false // animé par le moteur d'animation de Slint (animate + iteration-count: -1) : aucun code par frame
    }
    fn on_ready(&mut self, app: &MainWindow) {
        // le changement de `go` après la première frame déclenche les animations infinies de x, y et opacity
        app.global::<Bench>().set_go(true);
    }
    fn ops(&mut self, _t: f64, _app: &MainWindow, ops: &mut Map<String, Value>) {
        ops.insert("movers".into(), self.n.into());
        ops.insert("animated_properties".into(), (self.n * 3).into());
    }
    fn notes(&self) -> String {
        "ui-anim: N Rectangles each with `animate x / y / opacity { duration; iteration-count: -1; direction: alternate }` (engine animations; the only Rust call is Bench.go = true after the first frame). Redraws are requested by Slint's animation driver, the app requests none.".into()
    }
}
