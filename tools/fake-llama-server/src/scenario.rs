//! Scenario: the data a test (or `FAKE_SCENARIO` TOML) gives the fake runtime.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    #[serde(default)]
    pub models_shape: ModelsShape,
    /// Like llama-server `--models-max`: loading beyond this evicts the oldest.
    #[serde(default)]
    pub max_loaded: Option<usize>,
    /// Content pieces of the default chat reply (default: "Hello", " from", " fake").
    #[serde(default)]
    pub chat_chunks: Option<Vec<String>>,
    /// Directory of an R0 capture; GET endpoints found there are served from it.
    #[serde(default)]
    pub replay_from: Option<PathBuf>,
    #[serde(default)]
    pub oom: Option<OomRule>,
    #[serde(default)]
    pub models: Vec<ModelSpec>,
    /// Faults keyed by `"METHOD /path"`, e.g. `"POST /v1/chat/completions"`.
    #[serde(default)]
    pub routes: HashMap<String, Fault>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSpec {
    pub id: String,
    #[serde(default)]
    pub loaded: bool,
}

/// How `/models` and `/v1/models` report load state.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelsShape {
    /// Current upstream router: `"status": {"value": "loaded"}`.
    #[default]
    StatusObject,
    /// What Saltnitor's `router_loaded()` reads at c89f278: `"status": "loaded"`.
    Legacy,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Fault {
    /// Reply with this status and body (JSON content type when the body starts with `{`).
    Status {
        code: u16,
        #[serde(default)]
        body: String,
    },
    /// Stream these pieces with a delay before each (chat: content deltas as SSE).
    Chunks { items: Vec<String>, delay_ms: u64 },
    /// Accept the request and never send headers.
    HangBeforeHeaders,
    /// Send this many pieces, then crash (drop the connection, or exit the binary).
    CrashAfter { chunks: usize },
    /// 200 with a truncated JSON body or broken SSE framing.
    Malformed,
    /// Stream these byte pieces verbatim (no SSE wrapping): the first immediately, then `delay_ms`
    /// before each later piece. Records `chunk_sent {index, t_unix_ms}` per piece.
    RawChunks {
        items: Vec<String>,
        #[serde(default)]
        delay_ms: u64,
        #[serde(default = "default_200")]
        code: u16,
        #[serde(default = "default_sse")]
        content_type: String,
        /// Extra response headers (the proxy must drop the hop-by-hop ones among them).
        #[serde(default)]
        headers: Vec<(String, String)>,
    },
}

fn default_200() -> u16 {
    200
}

fn default_sse() -> String {
    "text/event-stream".into()
}

/// Binary only: exit at startup with llama.cpp-style OOM text when `arg`'s value > `gt`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OomRule {
    pub arg: String,
    pub gt: u64,
}

impl Scenario {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// Parse a scenario file; a relative `replay_from` is resolved against the file's directory.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let mut s = Self::from_toml(&text)?;
        if let Some(r) = s.replay_from.as_ref().filter(|r| r.is_relative()) {
            s.replay_from = Some(path.parent().unwrap_or(Path::new(".")).join(r));
        }
        Ok(s)
    }

    pub fn with_model(mut self, id: &str, loaded: bool) -> Self {
        self.models.push(ModelSpec {
            id: id.to_string(),
            loaded,
        });
        self
    }

    pub fn with_shape(mut self, shape: ModelsShape) -> Self {
        self.models_shape = shape;
        self
    }

    pub fn with_max_loaded(mut self, n: usize) -> Self {
        self.max_loaded = Some(n);
        self
    }

    pub fn with_fault(mut self, route: &str, fault: Fault) -> Self {
        self.routes.insert(route.to_string(), fault);
        self
    }

    pub fn with_chat_chunks(mut self, pieces: &[&str]) -> Self {
        self.chat_chunks = Some(pieces.iter().map(|p| p.to_string()).collect());
        self
    }

    pub fn with_replay(mut self, dir: impl Into<PathBuf>) -> Self {
        self.replay_from = Some(dir.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies: REQ-TST-011/AC1
    #[test]
    fn toml_scenario_parses_every_field() {
        let s = Scenario::from_toml(
            r#"
models_shape = "legacy"
max_loaded = 1
chat_chunks = ["a", "b"]
[oom]
arg = "--ctx-size"
gt = 65536
[[models]]
id = "A"
loaded = true
[routes."GET /health"]
kind = "status"
code = 500
body = "down"
[routes."POST /v1/chat/completions"]
kind = "chunks"
items = ["x"]
delay_ms = 10
"#,
        )
        .unwrap();
        assert_eq!(s.models_shape, ModelsShape::Legacy);
        assert_eq!(s.max_loaded, Some(1));
        assert_eq!(s.chat_chunks, Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(
            s.oom,
            Some(OomRule {
                arg: "--ctx-size".into(),
                gt: 65536
            })
        );
        assert_eq!(s.models[0].id, "A");
        assert!(s.models[0].loaded);
        assert_eq!(
            s.routes["GET /health"],
            Fault::Status {
                code: 500,
                body: "down".into()
            }
        );
        assert_eq!(
            s.routes["POST /v1/chat/completions"],
            Fault::Chunks {
                items: vec!["x".into()],
                delay_ms: 10
            }
        );
    }

    /// Verifies: REQ-TST-011/AC1
    #[test]
    fn unknown_keys_are_rejected() {
        assert!(Scenario::from_toml("modelz = []").is_err());
    }

    /// Verifies: REQ-TST-011/AC1
    #[test]
    fn relative_replay_dir_resolves_against_the_scenario_file() {
        let dir = std::env::temp_dir().join(format!("fake-scn-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("s.toml");
        std::fs::write(&file, "replay_from = \"cap\"\n").unwrap();
        assert_eq!(
            Scenario::from_file(&file).unwrap().replay_from,
            Some(dir.join("cap"))
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
