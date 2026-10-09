//! Scenario-driven stand-in for llama-server, for Saltnitor's tests (REQ-TST-011).
//!
//! * In-process: [`spawn`] a [`Scenario`] on a loopback port and read what it saw from
//!   [`Handle::recorder`].
//! * Process-level: the `fake-llama-server` binary (see README.md).
//! * Real-world check: `fake-llama-server record` captures a live router read-only.
mod engine;
pub mod record;
mod recorder;
mod scenario;

use std::net::SocketAddr;
use std::sync::Arc;

pub use engine::CrashMode;
pub use recorder::Recorder;
pub use scenario::{Fault, ModelSpec, ModelsShape, OomRule, Scenario};

/// A running in-process fake. Dropping it stops the server.
pub struct Handle {
    pub addr: SocketAddr,
    pub recorder: Recorder,
    shared: Arc<engine::Shared>,
    task: tokio::task::JoinHandle<()>,
}

impl Handle {
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Currently loaded model ids, oldest first.
    pub fn loaded(&self) -> Vec<String> {
        self.shared.loaded.lock().expect("loaded lock").clone()
    }

    /// Raw-chunk streams currently open (see `Fault::RawChunks`).
    pub fn in_flight(&self) -> usize {
        self.shared
            .in_flight
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Serve `scenario` on a fresh `127.0.0.1` port.
pub async fn spawn(scenario: Scenario) -> Handle {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    let recorder = Recorder::default();
    let shared = engine::Shared::new(scenario, recorder.clone(), CrashMode::DropConnection);
    let app = engine::router(shared.clone());
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Handle {
        addr,
        recorder,
        shared,
        task,
    }
}

/// Serve on an existing listener until the process ends (used by the binary).
pub async fn serve_on(
    listener: tokio::net::TcpListener,
    scenario: Scenario,
    recorder: Recorder,
    crash: CrashMode,
) -> std::io::Result<()> {
    axum::serve(
        listener,
        engine::router(engine::Shared::new(scenario, recorder, crash)),
    )
    .await
}
