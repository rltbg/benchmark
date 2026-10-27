fn main() {
    slint_build::compile("ui/app.slint").expect("slint compile");
    // version exacte du compilateur, reportée dans le champ "version" du JSON
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let v = std::process::Command::new(rustc).arg("--version").output().ok()
        .and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();
    println!("cargo:rustc-env=SLINTAPP_RUSTC={}", v.trim());
    println!("cargo:rerun-if-changed=ui/app.slint");
}
