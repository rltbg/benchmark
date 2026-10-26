// Compile l'UI Slint et, avec la feature core-pulse, le C généré par F* Pulse + KaRaMeL.
use std::path::Path;

fn main() {
    // UI : style fluent explicite (indépendant de la présence de Qt sur la machine de build)
    let cfg = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    slint_build::compile_with_config("ui/app.slint", cfg).expect("compilation du .slint");

    println!("cargo:rerun-if-changed=ui/app.slint");
    println!("cargo:rerun-if-changed=core-c");

    if std::env::var_os("CARGO_FEATURE_CORE_PULSE").is_some() {
        let target = std::env::var("TARGET").unwrap();
        let mut b = cc::Build::new();
        b.file("core-c/Core_Pulse.c").include("core-c").opt_level(2).warnings(true).extra_warnings(true);
        // wasm32-unknown-unknown n'a pas de libc : le C n'utilise que stdint/stdbool/size_t
        // (les en-têtes générés incluent <stdlib.h> et <string.h> par habitude) -> cales vides.
        if target == "wasm32-unknown-unknown" {
            b.flag("-ffreestanding").include(Path::new("core-c/wasm-shim"));
        }
        b.compile("core_pulse");
    }
}
