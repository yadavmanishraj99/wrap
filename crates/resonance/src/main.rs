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
mod cache;
mod catalog;
mod data;
mod image_cache;
mod pages;
mod player;
mod store;
mod views;

use windows::core::HSTRING;
use windows_reactor::App;

fn main() {
    // Tell Windows which app this process is, so the media session
    // (and the taskbar) shows "Resonance" instead of "Unknown app".
    // The name fully resolves once the app is installed/packaged
    // with a matching registered shortcut; for a `cargo run` build
    // this is best-effort.
    let app_id = HSTRING::from(player::APP_USER_MODEL_ID);
    unsafe {
        let _ = windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(&app_id);
    }
    App::run_component::<app::Shell>(()).unwrap();
}
