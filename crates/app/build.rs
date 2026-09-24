fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(target_os = "windows")]
    embed_with_winresource();
    #[cfg(not(target_os = "windows"))]
    embed_cross();
}

/// Native Windows build: winresource finds rc.exe itself.
#[cfg(target_os = "windows")]
fn embed_with_winresource() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/icon.ico");
    res.set_manifest(include_str!("../../packaging/windows/llmouser.manifest"));
    if let Err(e) = res.compile() {
        println!("cargo:warning=windows resources not embedded: {e}");
    }
}

/// Cross-compiling to Windows (cargo-xwin): compile the .rc with llvm-rc and
/// link the .res into the binary.
#[cfg(not(target_os = "windows"))]
fn embed_cross() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let rc_dir = manifest_dir.join("../../packaging/windows");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("llmouser.res");
    println!(
        "cargo:rerun-if-changed={}",
        rc_dir.join("llmouser.rc").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        rc_dir.join("llmouser.manifest").display()
    );
    let status = std::process::Command::new("llvm-rc")
        .current_dir(&rc_dir)
        .arg("/fo")
        .arg(&out)
        .arg("llmouser.rc")
        .status();
    match status {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bins={}", out.display()),
        other => println!("cargo:warning=windows resources not embedded (llvm-rc): {other:?}"),
    }
}
