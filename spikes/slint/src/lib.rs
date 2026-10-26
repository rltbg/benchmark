//! Spike multiplateforme Slint : une seule base de code (Rust + ui/app.slint) pour
//! Linux, Windows, macOS, iOS (simulateur), Android et web (wasm).
//! Spécification : docs/SPIKE-SPEC.md. Les têtes de plateforme ne contiennent que le point d'entrée.

mod app;
pub mod core;
pub mod plat;

#[cfg(target_os = "android")]
mod android_intent;

pub use app::run;

/// Point d'entrée web : `wasm-pack build --target web` appelle cette fonction au chargement du module.
#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn wasm_start() {
    if let Err(e) = run() {
        plat::log(&format!("SPIKE:error {e}"));
    }
}

/// Point d'entrée Android (NativeActivity via android-activity, ré-exporté par Slint).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    android_intent::set_app(app.clone());
    slint::android::init(app).unwrap();
    if let Err(e) = run() {
        plat::log(&format!("SPIKE:error {e}"));
    }
}
