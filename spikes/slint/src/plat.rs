//! Petites abstractions de plateforme : journal `SPIKE:`, horloge, nom de plateforme, autorun.

pub fn platform() -> &'static str {
    if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "ios") {
        if cfg!(target_abi = "sim") || cfg!(target_arch = "x86_64") { "ios-sim" } else { "ios" }
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_family = "wasm") {
        "web"
    } else {
        "linux"
    }
}

pub fn is_mobile() -> bool {
    cfg!(any(target_os = "android", target_os = "ios"))
}

// ---------- journal ----------

#[cfg(target_os = "android")]
pub fn log(line: &str) {
    use std::ffi::{CString, c_char, c_int};
    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }
    if let Ok(t) = CString::new(line.replace('\0', " ")) {
        unsafe { __android_log_write(4, c"SPIKE".as_ptr(), t.as_ptr()) };
    }
}

#[cfg(target_family = "wasm")]
pub fn log(line: &str) {
    web_sys::console::log_1(&line.into());
}

#[cfg(not(any(target_os = "android", target_family = "wasm")))]
pub fn log(line: &str) {
    use std::io::Write;
    let mut o = std::io::stdout().lock();
    let _ = writeln!(o, "{line}");
    let _ = o.flush();
}

// ---------- horloge ----------

#[cfg(not(target_family = "wasm"))]
pub fn now_ms() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static T0: OnceLock<Instant> = OnceLock::new();
    T0.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

#[cfg(target_family = "wasm")]
pub fn now_ms() -> f64 {
    web_sys::window().and_then(|w| w.performance()).map(|p| p.now()).unwrap_or(0.0)
}

// ---------- autorun ----------

/// Bureau : variable d'environnement SPIKE_AUTORUN=1 (ou argument --autorun) ; iOS : argument --autorun.
#[cfg(not(any(target_os = "android", target_family = "wasm")))]
pub fn autorun_requested() -> bool {
    std::env::var("SPIKE_AUTORUN").map(|v| v == "1").unwrap_or(false) || std::env::args().any(|a| a == "--autorun")
}

/// Web : paramètre d'URL ?autorun=1.
#[cfg(target_family = "wasm")]
pub fn autorun_requested() -> bool {
    web_sys::window().and_then(|w| w.location().search().ok()).map(|s| s.contains("autorun=1")).unwrap_or(false)
}

/// Android : extra d'intent `autorun=1` (am start --es autorun 1), lu en JNI brut (voir android_intent.rs).
#[cfg(target_os = "android")]
pub fn autorun_requested() -> bool {
    crate::android_intent::autorun_extra()
}
