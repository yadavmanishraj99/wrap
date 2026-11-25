//! Async data access: one shared tokio runtime plus the saavnapi client.
//!
//! Reactor's `spawn_background` runs a closure on a worker thread and
//! delivers its returned `Message` to the component. The closure
//! blocks on the runtime handle, so UI code never awaits — every
//! network call is `runtime_handle().block_on(client().<call>().await)`.

use std::sync::OnceLock;

use saavnapi::SaavnClient;
use tokio::runtime::{Handle, Runtime};

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// The shared tokio runtime handle (created on first use).
pub fn runtime_handle() -> Handle {
    RUNTIME
        .get_or_init(|| Runtime::new().expect("tokio runtime"))
        .handle()
        .clone()
}

/// A fresh saavnapi client (stateless — one per call is fine).
pub fn client() -> SaavnClient {
    SaavnClient::new()
}
