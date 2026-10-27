// Scénarios v3 : une implémentation de `Scenario` par famille ; `build` choisit selon `--scenario`.
use crate::{Args, MainWindow};
use serde_json::{Map, Value};
use std::time::{Duration, Instant};

pub mod ffis;
pub mod gfx;
pub mod lists;
pub mod text;
pub mod uis;

pub trait Scenario {
    /// true : le pilote avance l'état et demande une frame à chaque fin de frame (animation par code).
    /// false : scénario statique ou animé par le moteur (animations Slint, événements) : le pilote ne redessine pas.
    fn animated(&self) -> bool {
        true
    }
    /// appelé une fois à la fin de la première frame (focus, démarrage d'animations moteur…)
    fn on_ready(&mut self, _app: &MainWindow) {}
    fn begin_measure(&mut self, _app: &MainWindow) {}
    /// `t` : secondes depuis le début de la mesure (avant : temps de warm-up)
    fn step(&mut self, _t: f64, _app: &MainWindow, _measuring: bool) {}
    fn on_frame_end(&mut self, _now: Instant, _dur: Duration) {}
    fn ops(&mut self, _t: f64, _app: &MainWindow, _ops: &mut Map<String, Value>) {}
    fn notes(&self) -> String {
        String::new()
    }
    fn checksum(&self) -> String {
        "00000000".into()
    }
    fn stop(&mut self) {}
}

/// Raison d'indisponibilité décidée avant toute création de fenêtre (capacité absente de la stack).
pub fn unsupported(args: &Args, _renderer: &str) -> Option<String> {
    match args.scenario.as_str() {
        "gfx-paths" if args.has("dashed") => {
            Some("Slint's Path element has no stroke-dash property (no dash pattern in slint 1.18.1 builtin_elements.rs Path)".into())
        }
        "gfx-layers" if args.has("blur") => {
            Some("Slint has no per-layer blur: the only blur is drop-shadow-blur (BoxShadow) and the opacity layer cannot be blurred (builtin_elements.rs)".into())
        }
        _ => None,
    }
}

pub fn build(args: &Args, app: &MainWindow, renderer: &str) -> Result<Box<dyn Scenario>, String> {
    let s = args.scenario.as_str();
    match s {
        "list-scale" | "list-rich" | "list-reset" | "latency-load" | "latency-wheel" => lists::build_static(args, app, renderer),
        "list-mutate" => lists::build_mutate(args, app),
        "ffi-callbacks" | "ffi-block" => ffis::build_list_ffi(args, app, renderer),
        "ffi-calls" => ffis::build_calls(args, app),
        "text-big" => text::build_big(args, app),
        "text-labels" => text::build_labels(args, app),
        "text-edit" => text::build_edit(args, app),
        "text-rich" => text::build_rich(args, app),
        "gfx-shapes" => gfx::build_shapes(args, app, renderer),
        "gfx-paths" => gfx::build_paths(args, app),
        "gfx-chart" => gfx::build_chart(args, app),
        "gfx-images" => gfx::build_images(args, app, renderer),
        "gfx-layers" => gfx::build_layers(args, app),
        "ui-tree" => uis::build_tree(args, app),
        "ui-bind" => uis::build_bind(args, app),
        "ui-anim" => uis::build_anim(args, app),
        "idle" => lists::build_idle(args, app),
        other => Err(format!("scénario inconnu : {other}")),
    }
}

/// primitives que le moteur logiciel natif de Slint 1.18.1 ne dessine pas (docs « backends-and-renderers » + code)
pub fn software_not_rendered(variant_uses: &[&str]) -> Vec<&'static str> {
    let mut v = vec![];
    for u in variant_uses {
        match *u {
            "shadow" => v.push("drop-shadow (software renderer: draw_box_shadow is a TODO)"),
            "gradient-rounded" => v.push("linear-gradient on rounded rectangles (software renderer: gradient drawn on the bounding rectangle without radius)"),
            "rounded-clip" => v.push("border-radius combined with clip: true (software renderer ignores the radius)"),
            "rotation" => v.push("rotation / scaling transforms (software renderer: not supported)"),
            _ => {}
        }
    }
    v
}
