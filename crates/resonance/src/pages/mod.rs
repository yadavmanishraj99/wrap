//! Pages for the shell's NavigationView. Each page is a plain view
//! function over the shared `Shell` state — no per-page component,
//! so navigation is instant and the player state never resets.

pub mod album;
pub mod artist;
pub mod details;
pub mod home;
pub mod library;
pub mod playlist;
pub mod queue;
pub mod search;
pub mod settings;
pub mod song;
