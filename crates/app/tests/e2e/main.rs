//! End-to-end suite: launches the real app (mock LLM, isolated settings) and
//! drives its native widgets through the automation channel. Every user-facing
//! path has a test here; the same specs run against each platform shell.

mod harness;

mod about;
mod cancel;
mod context_menu;
mod errors;
mod history;
mod keyboard;
mod language;
mod links;
mod menu;
mod navigate;
mod network;
mod pdf;
mod search;
mod settings;
mod start_page;
mod tabs;
mod universe;
