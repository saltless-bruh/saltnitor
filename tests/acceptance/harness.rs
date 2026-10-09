//! Black-box harness: launch the built binary in a pty with a throw-away `$HOME`, start the
//! fake runtime, and talk HTTP to the control port. No sleeps where a poll works.

use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fake_llama_server::{Fault, Handle, ModelsShape, Scenario};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

/// Process start budget (brief: ≥ 10 s).
pub const START_TIMEOUT: Duration = Duration::from_secs(20);
/// §6 `cancel_propagation_ms`.
pub const CANCEL_PROPAGATION: Duration = Duration::from_millis(1000);
/// How often readiness and recorder polls run.
pub const POLL: Duration = Duration::from_millis(20);
/// The profile every scenario serves.
pub const MODEL: &str = "A";
/// The v1 `control_token` used by every authenticated scenario.
pub const TOKEN: &str = "acceptance-secret-token";

pub fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_saltnitor")
}

/// Bind `127.0.0.1:0`, read the port, release it.
pub fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind 127.0.0.1:0")
        .local_addr()
        .expect("local addr")
        .port()
}

pub fn port_is_free(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A private directory under the system temp dir, removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> Self {
        let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "saltnitor-acceptance-{}-{}-{}",
            std::process::id(),
            n,
            tag
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn write(&self, rel: &str, text: &str) -> PathBuf {
        let p = self.0.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(&p, text).expect("write file");
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A schema-v1 config: control port, optional token, router base, and one profile `A`.
pub fn v1_config(control_port: u16, token: Option<&str>, router_base: &str) -> String {
    let token_line = token
        .map(|t| format!("control_token = \"{t}\"\n"))
        .unwrap_or_default();
    format!(
        "control_port = {control_port}\n{token_line}router_base = \"{router_base}\"\n\n\
         [profiles.{MODEL}]\nmodel = \"a.gguf\"\nest_vram_gb = 1.0\nest_ram_gb = 1.0\n"
    )
}

/// Chat scenario: model `A` resident on a legacy-shape router, chat streamed as `items`
/// with `delay_ms` before every piece (and before `data: [DONE]`).
pub fn chat_scenario(items: &[&str], delay_ms: u64) -> Scenario {
    Scenario::default()
        .with_shape(ModelsShape::Legacy)
        .with_model(MODEL, true)
        .with_fault(
            "POST /v1/chat/completions",
            Fault::Chunks {
                items: items.iter().map(|s| s.to_string()).collect(),
                delay_ms,
            },
        )
}

pub fn chat_body(stream: bool) -> serde_json::Value {
    serde_json::json!({
        "model": MODEL,
        "messages": [{"role": "user", "content": "hi"}],
        "stream": stream,
    })
}

/// Bytes captured from the pty, shared with the reader thread.
type Captured = Arc<Mutex<Vec<u8>>>;

/// The binary running in a pty against a throw-away `$HOME`.
pub struct Daemon {
    child: Box<dyn Child + Send + Sync>,
    _master: Box<dyn MasterPty>,
    captured: Captured,
    pub control_port: u16,
    _home: TempDir,
}

impl Daemon {
    /// Write `config_toml` to `$HOME/.config/saltnitor/config.toml`, launch the binary in a
    /// pty, and wait until `GET /healthz` answers 200 (or fail with the captured screen).
    pub async fn spawn(config_toml: &str, control_port: u16) -> Daemon {
        let home = TempDir::new("home");
        home.write(".config/saltnitor/config.toml", config_toml);

        let pty = native_pty_system()
            .openpty(PtySize {
                rows: 40,
                cols: 120,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty");
        let mut cmd = CommandBuilder::new(binary());
        cmd.env("HOME", home.path());
        cmd.env_remove("XDG_CONFIG_HOME");
        cmd.env_remove("SUDO_USER");
        cmd.env("TERM", "xterm-256color");
        cmd.cwd(home.path());
        let child = pty
            .slave
            .spawn_command(cmd)
            .expect("spawn saltnitor in pty");
        drop(pty.slave);

        let captured: Captured = Arc::new(Mutex::new(Vec::new()));
        let mut reader = pty.master.try_clone_reader().expect("pty reader");
        let sink = captured.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                sink.lock()
                    .expect("captured lock")
                    .extend_from_slice(&buf[..n]);
            }
        });

        let mut daemon = Daemon {
            child,
            _master: pty.master,
            captured,
            control_port,
            _home: home,
        };
        daemon.wait_ready().await;
        daemon
    }

    async fn wait_ready(&mut self) {
        let client = reqwest::Client::new();
        let url = self.url("/healthz");
        let deadline = Instant::now() + START_TIMEOUT;
        loop {
            if let Ok(Some(status)) = self.child.try_wait() {
                panic!(
                    "saltnitor exited during startup with {status:?}\n--- pty output ---\n{}",
                    self.screen()
                );
            }
            if let Ok(resp) = client.get(&url).send().await
                && resp.status() == reqwest::StatusCode::OK
            {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "saltnitor did not answer /healthz on :{} within {START_TIMEOUT:?}\n--- pty output ---\n{}",
                self.control_port,
                self.screen()
            );
            tokio::time::sleep(POLL).await;
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{}", self.control_port, path)
    }

    /// Everything the binary wrote to the pty so far (lossy UTF-8).
    pub fn screen(&self) -> String {
        String::from_utf8_lossy(&self.captured.lock().expect("captured lock")).into_owned()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Fake runtime + the binary pointed at it, with `control_token = TOKEN`.
pub struct Stack {
    pub fake: Handle,
    pub daemon: Daemon,
    pub client: reqwest::Client,
}

impl Stack {
    pub async fn with_token(scenario: Scenario) -> Stack {
        Self::start(scenario, Some(TOKEN)).await
    }

    pub async fn start(scenario: Scenario, token: Option<&str>) -> Stack {
        let fake = fake_llama_server::spawn(scenario).await;
        let port = free_port();
        let daemon = Daemon::spawn(&v1_config(port, token, &fake.base_url()), port).await;
        Stack {
            fake,
            daemon,
            client: reqwest::Client::new(),
        }
    }

    pub fn url(&self, path: &str) -> String {
        self.daemon.url(path)
    }

    /// `POST /v1/chat/completions` through the proxy with the bearer.
    pub fn chat(&self, stream: bool) -> reqwest::RequestBuilder {
        self.client
            .post(self.url("/v1/chat/completions"))
            .bearer_auth(TOKEN)
            .json(&chat_body(stream))
    }

    /// The same request sent straight to the fake (the upstream truth).
    pub fn upstream_chat(&self, stream: bool) -> reqwest::RequestBuilder {
        self.client
            .post(format!("{}/v1/chat/completions", self.fake.base_url()))
            .json(&chat_body(stream))
    }

    /// Poll the fake's recorder for a `disconnect` event until `budget` elapses.
    pub async fn wait_for_disconnect(&self, budget: Duration) -> bool {
        let deadline = Instant::now() + budget;
        loop {
            if !self.fake.recorder.of_kind("disconnect").is_empty() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(POLL).await;
        }
    }
}

/// One body chunk as the client saw it, with its arrival time.
pub struct Arrival {
    pub at: Instant,
    pub bytes: Vec<u8>,
}

/// Read a response body chunk by chunk, timestamping every arrival.
pub async fn read_arrivals(mut resp: reqwest::Response) -> Vec<Arrival> {
    let mut out = Vec::new();
    while let Some(chunk) = resp.chunk().await.expect("read body chunk") {
        out.push(Arrival {
            at: Instant::now(),
            bytes: chunk.to_vec(),
        });
    }
    out
}

pub fn concat(arrivals: &[Arrival]) -> Vec<u8> {
    arrivals
        .iter()
        .flat_map(|a| a.bytes.iter().copied())
        .collect()
}

/// Run the binary without a pty, stdin closed, and wait for it to exit (or kill it at the
/// timeout). Returns the exit status with captured stdout and stderr.
pub fn run_to_exit(args: &[&str], home: &Path, timeout: Duration) -> (ExitStatus, String, String) {
    let mut child = Command::new(binary())
        .args(args)
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("SUDO_USER")
        .current_dir(home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn saltnitor");
    // Drain both pipes on threads so a chatty child can never block on a full pipe.
    let stdout = drain(child.stdout.take().expect("stdout pipe"));
    let stderr = drain(child.stderr.take().expect("stderr pipe"));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait().expect("try_wait") {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                break child.wait().expect("wait after kill");
            }
            None => std::thread::sleep(POLL),
        }
    };
    let stdout = stdout.join().expect("stdout thread");
    let stderr = stderr.join().expect("stderr thread");
    (status, stdout, stderr)
}

fn drain(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = pipe.read_to_string(&mut text);
        text
    })
}
