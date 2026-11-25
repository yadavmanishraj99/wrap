#![windows_subsystem = "windows"]

//! Resonance — native Windows music streaming app.
//!
//! UI: WinUI 3 via `windows-reactor` (declarative components, typed
//! messages, WinUI controls). Data: `saavnapi` (JioSaavn). Audio:
//! WinRT `MediaPlayer`.
//!
//! Built incrementally: the shell (sidebar navigation, header search,
//! bottom player) first, then Home, Search, Library and Settings
//! pages. This crate is Windows-only — build and run on Windows:
//! `cargo run -p resonance` (needs the Windows App Runtime installed;
//! see README.md).

mod app;
mod data;
mod pages;
mod player;
mod store;
mod views;

use windows_reactor::App;

fn main() {
    App::run_component::<app::Shell>(()).unwrap();
}
