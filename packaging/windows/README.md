# Windows build

On Windows (x64) with Rust and the WebView2 Runtime installed:

```powershell
cargo build --release -p llmouser
Compress-Archive target\release\llmouser.exe LLMouser-<version>-windows-x64.zip
```

`build.rs` embeds `assets/icon.ico` and `llmouser.manifest` (Common Controls 6,
per-monitor DPI, UTF-8) through `winresource`, which needs `rc.exe` (Visual
Studio Build Tools) or `llvm-rc`.

## Cross-compiling from macOS/Linux

```bash
cargo install cargo-xwin           # downloads the Windows SDK/CRT on first use
brew install llvm lld              # clang-cl, llvm-rc, lld-link
cargo xwin build --release -p llmouser --target x86_64-pc-windows-msvc
```

Without the `lld` formula, a symlink named `lld-link` to rustup's `rust-lld`
(`~/.rustup/toolchains/*/lib/rustlib/<host>/bin/rust-lld`) on `PATH` works: lld
picks the COFF flavour from its program name.

The E2E suite (`cargo test -p llmouser --test e2e`) must run on Windows itself:
WebView2 has no headless mode and does not run under Wine.
