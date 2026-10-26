// Records the compiler's version for the JSON "version" field.
fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let v = std::process::Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let v = v.trim().trim_start_matches("rustc ").to_owned();
    println!("cargo:rustc-env=NSHBENCH_RUSTC={v}");
    println!("cargo:rerun-if-changed=build.rs");
}
