//! Project automation, in Rust, no dependencies:
//!
//! ```text
//! cargo xtask build [macos] [linux] [windows]   packages into build/
//! cargo xtask test  [unit] [macos] [linux] [windows]
//! cargo xtask lint                              fmt, clippy -D warnings, cargo audit, cargo deny
//! cargo xtask coverage                          llvm-cov report into coverage/
//! ```
//!
//! Runs from a Mac: Linux work happens in the Docker image from
//! `packaging/linux/Dockerfile`; the Windows binary is cross-built with
//! cargo-xwin, its E2E suite needs a Windows machine.
#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let (command, rest) = match args.split_first() {
        Some((c, r)) => (c.as_str(), r.to_vec()),
        None => ("help", vec![]),
    };
    let result = match command {
        "build" => build(&rest),
        "test" => test(&rest),
        "lint" => lint(),
        "coverage" => coverage(),
        _ => {
            eprintln!("usage: cargo xtask build|test|lint|coverage [targets...]");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

type Result<T> = std::result::Result<T, String>;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn version() -> String {
    let manifest = fs::read_to_string(root().join("Cargo.toml")).expect("Cargo.toml");
    manifest
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("version = ")
                .map(|v| v.trim_matches('"').to_string())
        })
        .expect("workspace version")
}

/// Run a command in the workspace root; fail with its name on a non-zero exit.
fn run(program: &str, args: &[&str]) -> Result<()> {
    run_in(&root(), program, args, &[])
}

fn run_in(dir: &Path, program: &str, args: &[&str], envs: &[(&str, String)]) -> Result<()> {
    println!("+ {program} {}", args.join(" "));
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(dir);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let status = cmd
        .status()
        .map_err(|e| format!("cannot start {program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {} failed ({status})", args.join(" ")))
    }
}

fn has(program: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {program}"))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn selected<'a>(rest: &'a [String], all: &'a [&'a str]) -> Result<Vec<&'a str>> {
    if rest.is_empty() {
        return Ok(all.to_vec());
    }
    let mut out = Vec::new();
    for r in rest {
        match all.iter().find(|a| *a == r) {
            Some(a) => out.push(*a),
            None => {
                return Err(format!(
                    "unknown target {r}; expected one of {}",
                    all.join(", ")
                ))
            }
        }
    }
    Ok(out)
}

fn docker_platform() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "linux/arm64"
    } else {
        "linux/amd64"
    }
}

fn docker_image(platform: &str) -> Result<String> {
    let tag = format!("llmouser-linux-{}", platform.replace('/', "-"));
    run(
        "docker",
        &[
            "build",
            "--platform",
            platform,
            "-t",
            &tag,
            "packaging/linux",
        ],
    )?;
    Ok(tag)
}

/// Run a shell command inside the Linux build image with the cargo registry cached.
fn docker_run(platform: &str, target_dir: &str, envs: &[&str], script: &str) -> Result<()> {
    let image = docker_image(platform)?;
    let volume = format!("llmouser-cargo-{}", platform.replace('/', "-"));
    let _ = Command::new("docker")
        .args(["volume", "create", &volume])
        .output();
    let src = format!("{}:/src", root().display());
    let registry = format!("{volume}:/usr/local/cargo/registry");
    let target = format!("CARGO_TARGET_DIR=/src/target/{target_dir}");
    let mut args = vec![
        "run",
        "--rm",
        "--platform",
        platform,
        "-v",
        &src,
        "-v",
        &registry,
        "-e",
        &target,
    ];
    for e in envs {
        args.push("-e");
        args.push(e);
    }
    args.extend(["bash", "-c"]);
    let mut args: Vec<String> = args.into_iter().map(String::from).collect();
    args.push(image);
    // image goes before the command
    let image_pos = args.len() - 1;
    let image = args.remove(image_pos);
    let insert_at = args.iter().position(|a| a == "bash").unwrap();
    args.insert(insert_at, image);
    args.push(script.to_string());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run("docker", &refs)
}

// ---- build ----------------------------------------------------------------

fn build(rest: &[String]) -> Result<()> {
    for target in selected(rest, &["macos", "linux", "windows"])? {
        match target {
            "macos" => build_macos()?,
            "linux" => build_linux()?,
            "windows" => build_windows()?,
            _ => unreachable!(),
        }
    }
    Ok(())
}

/// LLMouser.app and a .dmg with Apple's stock tools (sips, iconutil, hdiutil, codesign).
fn build_macos() -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err("the macOS package can only be built on macOS".into());
    }
    let version = version();
    let out = root().join("build/macos");
    let app = out.join("LLMouser.app");
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(app.join("Contents/MacOS")).map_err(|e| e.to_string())?;
    fs::create_dir_all(app.join("Contents/Resources")).map_err(|e| e.to_string())?;
    let iconset = out.join("icon.iconset");
    fs::create_dir_all(&iconset).map_err(|e| e.to_string())?;

    run("cargo", &["build", "--release", "-p", "llmouser"])?;
    fs::copy(
        root().join("target/release/llmouser"),
        app.join("Contents/MacOS/LLMouser"),
    )
    .map_err(|e| e.to_string())?;

    for size in [16u32, 32, 128, 256, 512] {
        for (scale, suffix) in [(1u32, ""), (2, "@2x")] {
            let px = (size * scale).to_string();
            let file = iconset.join(format!("icon_{size}x{size}{suffix}.png"));
            run(
                "sips",
                &[
                    "-z",
                    &px,
                    &px,
                    "assets/icon-mac.png",
                    "--out",
                    file.to_str().unwrap(),
                ],
            )?;
        }
    }
    run(
        "iconutil",
        &[
            "-c",
            "icns",
            iconset.to_str().unwrap(),
            "-o",
            app.join("Contents/Resources/LLMouser.icns")
                .to_str()
                .unwrap(),
        ],
    )?;
    let _ = fs::remove_dir_all(&iconset);

    let plist =
        fs::read_to_string(root().join("packaging/macos/Info.plist")).map_err(|e| e.to_string())?;
    fs::write(
        app.join("Contents/Info.plist"),
        plist.replace("@VERSION@", &version),
    )
    .map_err(|e| e.to_string())?;
    fs::write(app.join("Contents/PkgInfo"), "APPL????").map_err(|e| e.to_string())?;
    // Ad-hoc signature so Gatekeeper reports "unsigned" rather than "damaged".
    run(
        "codesign",
        &["--force", "--deep", "--sign", "-", app.to_str().unwrap()],
    )?;
    let dmg = out.join(format!("LLMouser-{version}-macos.dmg"));
    run(
        "hdiutil",
        &[
            "create",
            "-volname",
            "LLMouser",
            "-srcfolder",
            app.to_str().unwrap(),
            "-ov",
            "-format",
            "UDZO",
            dmg.to_str().unwrap(),
        ],
    )?;
    println!("built {} and {}", app.display(), dmg.display());
    Ok(())
}

/// Ubuntu 24.04 x64 .deb via cargo-deb inside Docker.
fn build_linux() -> Result<()> {
    let version = version();
    docker_run("linux/amd64", "linux-amd64", &[], "cargo deb -p llmouser")?;
    let deb = format!("llmouser_{version}-1_amd64.deb");
    let out = root().join("build/linux");
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    fs::copy(
        root().join("target/linux-amd64/debian").join(&deb),
        out.join(&deb),
    )
    .map_err(|e| e.to_string())?;
    println!("built {}", out.join(deb).display());
    Ok(())
}

/// Windows x64 binary: native `cargo build` on Windows, cargo-xwin elsewhere.
fn build_windows() -> Result<()> {
    let version = version();
    let mut envs: Vec<(&str, String)> = Vec::new();
    if cfg!(target_os = "windows") {
        run("cargo", &["build", "--release", "-p", "llmouser"])?;
    } else {
        if !has("cargo-xwin") {
            return Err("cargo-xwin is not installed: cargo install cargo-xwin".into());
        }
        let mut path = env::var("PATH").unwrap_or_default();
        for extra in ["/opt/homebrew/opt/lld/bin", "/opt/homebrew/opt/llvm/bin"] {
            path = format!("{extra}:{path}");
        }
        if !has("lld-link") {
            // rustup's lld answers to the name lld-link (COFF flavour by program name).
            let shim = root().join("target/xtask-bin");
            fs::create_dir_all(&shim).map_err(|e| e.to_string())?;
            let link = shim.join("lld-link");
            let _ = fs::remove_file(&link);
            let rust_lld = find_rust_lld().ok_or("rust-lld not found in ~/.rustup")?;
            symlink(&rust_lld, &link)?;
            path = format!("{}:{path}", shim.display());
        }
        envs.push(("PATH", path));
        run_in(
            &root(),
            "cargo",
            &[
                "xwin",
                "build",
                "--release",
                "-p",
                "llmouser",
                "--target",
                "x86_64-pc-windows-msvc",
            ],
            &envs,
        )?;
    }
    let exe = if cfg!(target_os = "windows") {
        root().join("target/release/llmouser.exe")
    } else {
        root().join("target/x86_64-pc-windows-msvc/release/llmouser.exe")
    };
    let out = root().join("build/windows");
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    fs::copy(&exe, out.join("LLMouser.exe")).map_err(|e| e.to_string())?;
    let zip = format!("LLMouser-{version}-windows-x64.zip");
    let _ = fs::remove_file(out.join(&zip));
    run_in(&out, "zip", &["-q", &zip, "LLMouser.exe"], &[])?;
    println!("built {}", out.join(zip).display());
    Ok(())
}

fn find_rust_lld() -> Option<PathBuf> {
    let home = env::var_os("HOME")?;
    let toolchains = Path::new(&home).join(".rustup/toolchains");
    for tc in fs::read_dir(toolchains).ok()?.flatten() {
        let rustlib = tc.path().join("lib/rustlib");
        for host in fs::read_dir(rustlib).ok()?.flatten() {
            let candidate = host.path().join("bin/rust-lld");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(unix)]
fn symlink(from: &Path, to: &Path) -> Result<()> {
    std::os::unix::fs::symlink(from, to).map_err(|e| e.to_string())
}

#[cfg(not(unix))]
fn symlink(_from: &Path, _to: &Path) -> Result<()> {
    Err("symlink shim is only used on unix hosts".into())
}

// ---- test -----------------------------------------------------------------

fn test(rest: &[String]) -> Result<()> {
    for step in selected(rest, &["unit", "macos", "linux", "windows"])? {
        match step {
            "unit" => {
                run("cargo", &["test", "-p", "llmouser-browser"])?;
                run("cargo", &["test", "-p", "llmouser", "--bin", "llmouser"])?;
            }
            "macos" => {
                if cfg!(target_os = "macos") {
                    run("cargo", &["test", "-p", "llmouser", "--test", "e2e"])?;
                } else {
                    println!("skipping macOS E2E: not on macOS");
                }
            }
            "linux" => {
                if cfg!(target_os = "linux") && !has("docker") {
                    run(
                        "cargo",
                        &[
                            "test",
                            "-p",
                            "llmouser",
                            "--test",
                            "e2e",
                            "--",
                            "--test-threads=1",
                        ],
                    )?;
                } else {
                    // WebKit's bubblewrap sandbox needs user namespaces, absent in containers.
                    docker_run(
                        docker_platform(),
                        "linux-test",
                        &[
                            "WEBKIT_DISABLE_COMPOSITING_MODE=1",
                            "WEBKIT_DISABLE_DMABUF_RENDERER=1",
                            "WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1",
                            "GTK_A11Y=none",
                        ],
                        "cargo test -p llmouser-browser && xvfb-run -a -s '-screen 0 1600x1000x24' dbus-run-session -- \
                         cargo test -p llmouser --test e2e -- --test-threads=1",
                    )?;
                }
            }
            "windows" => {
                if cfg!(target_os = "windows") {
                    run(
                        "cargo",
                        &[
                            "test",
                            "-p",
                            "llmouser",
                            "--test",
                            "e2e",
                            "--",
                            "--test-threads=1",
                        ],
                    )?;
                } else {
                    run("rustup", &["target", "add", "x86_64-pc-windows-msvc"])?;
                    run(
                        "cargo",
                        &[
                            "check",
                            "-p",
                            "llmouser",
                            "--target",
                            "x86_64-pc-windows-msvc",
                        ],
                    )?;
                    println!(
                        "note: the Windows E2E suite must run on Windows: cargo xtask test windows"
                    );
                }
            }
            _ => unreachable!(),
        }
    }
    Ok(())
}

// ---- lint & coverage -------------------------------------------------------

fn lint() -> Result<()> {
    for (tool, package) in [("cargo-audit", "cargo-audit"), ("cargo-deny", "cargo-deny")] {
        if !has(tool) {
            run("cargo", &["install", package, "--locked"])?;
        }
    }
    run("cargo", &["fmt", "--all", "--", "--check"])?;
    run(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run("cargo", &["audit"])?;
    run("cargo", &["deny", "check"])
}

/// Line coverage from unit tests and the E2E suite (the launched app is
/// instrumented too). Writes coverage/{lcov.info,html,summary.txt}.
fn coverage() -> Result<()> {
    if !has("cargo-llvm-cov") {
        run("cargo", &["install", "cargo-llvm-cov", "--locked"])?;
    }
    let out = root().join("coverage");
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    run("cargo", &["llvm-cov", "clean", "--workspace"])?;
    run("cargo", &["llvm-cov", "--workspace", "--no-report"])?;
    run(
        "cargo",
        &[
            "llvm-cov",
            "report",
            "--lcov",
            "--output-path",
            "coverage/lcov.info",
        ],
    )?;
    run(
        "cargo",
        &[
            "llvm-cov",
            "report",
            "--html",
            "--output-dir",
            "coverage/html",
        ],
    )?;
    let summary = Command::new("cargo")
        .args(["llvm-cov", "report", "--summary-only"])
        .current_dir(root())
        .output()
        .map_err(|e| e.to_string())?;
    fs::write(out.join("summary.txt"), &summary.stdout).map_err(|e| e.to_string())?;
    print!("{}", String::from_utf8_lossy(&summary.stdout));
    Ok(())
}
