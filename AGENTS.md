# AGENTS.md — how LLMouser is designed and built

This file is for anyone (human or agent) changing the code. It records the two
lenses every change is judged through, the shape of the codebase, and the rules
that keep it native, safe and tested.

## 1. Design philosophy

### 1.1 TRIZ: resolve contradictions, do not compromise them

Engineering decisions here are framed as contradictions and resolved with TRIZ
inventive principles instead of picking a middle ground. The ones that shaped
the architecture:

| Contradiction | Resolution (TRIZ principle) | Where |
|---|---|---|
| A browser must look and behave native on three OSes, yet one person must maintain it | **1 Segmentation** + **2 Taking out**: everything that is not pixels lives in one safe crate (`llmouser-browser`); each OS gets only a thin shell of its own toolkit. **6 Universality**: one automation protocol, one test suite, three shells. | `crates/browser`, `crates/app/src/{macos,linux,windows}` |
| Generated pages must be fully interactive (scripts, forms, links) yet must never touch the real internet | **24 Intermediary**: the system webview renders, but every navigation it attempts is intercepted and turned into a generation. **9 Preliminary anti-action**: a CSP is injected ahead of the content. **11 Beforehand cushioning**: a webview-level request blocker is the second layer when the CSP is not enough (content rules on WebKit, resource filtering on WebView2). | `page::prepare_document`, each shell's navigation delegate |
| The LLM takes seconds, yet the interface must feel immediate | **10 Prior action**: pages are cached per history entry so Back/Forward never regenerate. **15 Dynamics**: the Go control becomes Stop while loading. **21 Skipping**: cancelling aborts the HTTP request at once and a late result is discarded by sequence number. **20 Continuity**: tabs generate concurrently. | `browser::Browser`, `engine::Engine` |
| Windows tabs, macOS tabs and GNOME tabs are different things, yet the model must be one | **17 Another dimension**: on macOS a tab *is* a window in a native tab group; on GNOME an `AdwTabView` page; on Windows a native tab control item. The model only knows "tabs". | shells |
| Silent PDF export needs platform code, yet the browser logic must stay safe | **3 Local quality**: `unsafe` is confined to the shells; the browser crate `#![forbid(unsafe_code)]`. | `crates/app` |
| The API key must persist, yet must never be shown again | **2 Taking out**: the key never leaves the browser crate; the UI only learns `has_api_key`. | `settings::PublicSettings` |
| Users upgrading from the Electron app should not retype anything, yet tests must be isolated | **26 Copying**: the old settings file format is read as-is on first run; an explicit data directory (tests) never inherits it. | `settings::SettingsStore::open_default` |
| Native UIs cannot be unit-tested, yet every path must be tested | **25 Self-service**: each shell answers an automation protocol by operating its *own real widgets*; the suite is one set of Rust integration tests for all shells. | `browser::automation`, `crates/app/tests/e2e` |

When you add a feature, name the contradiction it resolves in the PR description
using this vocabulary. If there is no contradiction, it is probably not worth the
code.

### 1.2 Jakob Nielsen: the ten heuristics are acceptance criteria

Every user-facing change is checked against Nielsen's heuristics. How they are
currently honoured:

1. **Visibility of system status** — the status bar says what is happening
   (`Loading …`, `Loaded …`, `Stopped …`, `Saved PDF: …`); tabs show a loading
   state; the Go control turns into Stop.
2. **Match between system and the real world** — it works like a browser:
   address bar, tabs, Back/Forward, favicons, page titles in the title bar.
   The LLM is spoken to in HTTP (`GET`, `Host`, `Referer`), not in prose.
3. **User control and freedom** — Stop cancels generation instantly; Back/Forward
   restore exact pages; closed tabs can be reopened; Settings has Close that
   discards edits; Escape closes About.
4. **Consistency and standards** — native menus with the platform's standard
   roles and shortcuts (Cmd/Ctrl+T, W, S, R, L, [ ]); macOS window tabs and
   Services; GNOME header bar and `AdwTabBar`; Windows accelerators.
5. **Error prevention** — settings are validated before saving (endpoint must be
   http(s), tokens a positive integer, search URL must contain `{query}`);
   navigation without a key fails fast with guidance instead of a network error.
6. **Recognition rather than recall** — provider presets fill endpoint and model;
   placeholders describe every field; the API-key field says whether a key is
   stored.
7. **Flexibility and efficiency of use** — plain words in the address bar search;
   bare hosts get `https://`; keyboard shortcuts for every command; the universe
   rules are a free-text field for power users.
8. **Aesthetic and minimalist design** — a toolbar, a tab strip, a page, a status
   line. Nothing else.
9. **Help users recognize, diagnose and recover from errors** — a failed
   generation renders an error page with the exact message, a Try Again button
   and, for credential errors, an Open Settings button; the status line repeats
   the error in the user's language.
10. **Help and documentation** — a new tab is blank, like a browser; the README
    covers setup; the Help menu links home; a missing key is explained in place.

New UI must state which heuristics it serves and must not regress any.

## 2. Codebase shape

```
crates/browser   llmouser-browser: the browser itself minus pixels.
                 tabs & history model, LLM providers (OpenAI, Anthropic, mock),
                 settings, translations (en/ru/zh), page preparation,
                 async engine, automation protocol. #![forbid(unsafe_code)].
crates/app       llmouser: the native shells.
  src/session.rs shared glue between the model and any shell (safe)
  src/macos      AppKit + WKWebView via objc2 (native window tabs, unified toolbar)
  src/linux      GTK4 + libadwaita + WebKitGTK 6.0
  src/windows    Win32 + WebView2 via the windows/webview2-com crates
  tests/e2e      the end-to-end suite, one spec set for every shell
crates/xtask     `cargo xtask build|test|lint|coverage` (std only, no deps)
packaging/       macOS Info.plist, Debian container + cargo-deb metadata,
                 Windows resources and cross-build notes
```

Names describe what things are. Do not introduce a "core", "utils" or
"common" crate: say what it is.

## 3. Rules

- **Unsafe** only in `crates/app/src/{macos,linux,windows}`, wrapping toolkit
  calls, each block as small as possible. `llmouser-browser` and `session.rs`
  forbid it.
- **Every user-visible path is E2E tested.** A new button, menu item, shortcut,
  status message or page gets a spec in `crates/app/tests/e2e` in the same
  change, and every shell must implement whatever automation command the spec
  needs. Unit tests cover the browser crate; they do not replace E2E coverage.
- **Translations are complete**: a new string is added to `en.rs`, `ru.rs` and
  `zh.rs` in the same change; `i18n::tests` checks placeholders.
- **Nothing reaches the network** except the configured LLM endpoint. Any new
  way for page content to load resources must be blocked in both layers
  (CSP and webview-level) and tested.
- **Security hygiene**: `cargo audit` and `cargo deny check` are clean; clippy
  runs with `-D warnings`; dependencies are pinned to current releases; secrets
  never appear in UI state or logs.
- **Coverage** is measured with `cargo llvm-cov` (unit + E2E, the app process is
  instrumented too) and the report is committed under `coverage/`.

## 4. Working on it

```bash
cargo xtask test                          # unit + macOS E2E + Linux E2E (Docker) + Windows check
cargo xtask build                         # dmg + deb (Docker) + Windows zip (cargo-xwin)
cargo test -p llmouser-browser            # unit tests of the browser crate
cargo test -p llmouser --test e2e         # E2E against this platform's shell (mock LLM)
LLMOUSER_MOCK=1 cargo run                 # run with the offline provider
cargo xtask coverage                      # llvm-cov report into coverage/
cargo xtask lint                          # fmt + clippy + audit + deny
```

The Linux shell is built and its E2E suite run in the Docker image from
`packaging/linux/Dockerfile` (Ubuntu 24.04); under Docker or CI set
`WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1` because WebKit's bubblewrap sandbox needs user
namespaces. The Windows binary can be cross-compiled with `cargo xwin` (see
`packaging/windows/README.md`); its E2E suite needs a real Windows machine.
