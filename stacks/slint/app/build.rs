fn main() {
    slint_build::compile("ui/app.slint").unwrap();
    // C-ABI core: static library produced by /home/user/eval/core
    println!("cargo:rustc-link-search=native=/home/user/eval/core/target/release");
    println!("cargo:rustc-link-lib=static=benchcore");
    // libbenchcore.a embeds its own copy of libstd (Rust staticlib): tolerate duplicate std symbols
    println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");
    println!("cargo:rerun-if-changed=ui/app.slint");
    println!("cargo:rerun-if-changed=/home/user/eval/core/target/release/libbenchcore.a");
}
