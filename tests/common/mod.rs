//! Shared rig for the proxy integration tests: Saltnitor's real router in front of the
//! scenario-driven fake runtime.
#![allow(
    dead_code,
    reason = "each integration-test crate uses a different subset of these helpers"
)]
use fake_llama_server::{Fault, Handle, Scenario};
use saltnitor::control_api::{ControlApi, ProfileMeta, serve};
use saltnitor::events::Event;
use saltnitor::proxy_stream::ProxyLimits;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

pub fn scenario(fault: Fault) -> Scenario {
    let mut s = Scenario {
        models_shape: fake_llama_server::ModelsShape::Legacy,
        ..Scenario::default()
    };
    s.models.push(fake_llama_server::ModelSpec {
        id: "A".into(),
        loaded: true,
    });
    s.routes.insert("POST /v1/chat/completions".into(), fault);
    s
}

pub fn raw(items: &[&str], delay_ms: u64) -> Fault {
    Fault::RawChunks {
        items: items.iter().map(|s| s.to_string()).collect(),
        delay_ms,
        code: 200,
        content_type: "text/event-stream".into(),
        headers: vec![],
    }
}

pub fn profile() -> ProfileMeta {
    ProfileMeta {
        model: "a.gguf".into(),
        offload: false,
        est_vram_gb: Some(0.1),
        est_ram_gb: Some(0.1),
    }
}

pub struct Rig {
    pub base: String,
    pub fake: Handle,
    pub http: reqwest::Client,
    /// Held so the event channel stays open until a test takes it with [`Rig::take_events`].
    rx: Option<mpsc::Receiver<Event>>,
}
impl Rig {
    /// The receiving end of the router's event channel (log lines, state changes).
    pub fn take_events(&mut self) -> mpsc::Receiver<Event> {
        self.rx.take().expect("events already taken")
    }
}

pub async fn rig(s: Scenario, upstream_bearer: Option<&str>, limits: ProxyLimits) -> Rig {
    let fake = fake_llama_server::spawn(s).await;
    let (tx, rx) = mpsc::channel(256);
    let api = Arc::new(
        ControlApi::new(
            HashMap::from([("A".to_string(), profile())]),
            fake.base_url(),
            upstream_bearer.map(str::to_string),
            None,
            0.0,
            0.0,
            tx,
        )
        .unwrap()
        .limits(limits)
        .unwrap(),
    );
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tokio::spawn(serve(api, addr));
    let base = format!("http://{addr}");
    let http = reqwest::Client::new();
    for _ in 0..100 {
        if http.get(format!("{base}/healthz")).send().await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Rig {
        base,
        fake,
        http,
        rx: Some(rx),
    }
}

pub fn limits() -> ProxyLimits {
    ProxyLimits {
        connect: Duration::from_secs(5),
        first_byte: Duration::from_secs(600),
        idle: Duration::from_secs(120),
        max_body_bytes: 33_554_432,
    }
}
