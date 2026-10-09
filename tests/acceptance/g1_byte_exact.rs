//! G1 row 2: downstream body bytes equal upstream body bytes (REQ-PRX-003, INV-01).
//!
//! Two upstreams: the fake runtime (its chat route emits real SSE deltas and `data: [DONE]`)
//! and a raw TCP upstream (`raw_upstream`) that can emit anything at all — SSE comments,
//! keep-alives, tool-call and reasoning deltas, non-UTF-8 bytes — with arbitrary chunk
//! boundaries. The raw upstream answers `/v1/models` so the proxy treats `A` as resident.

use std::sync::{Arc, Mutex};

use proptest::prelude::*;
use proptest::test_runner::{Config as PropConfig, TestRunner};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::harness::{
    Daemon, MODEL, Stack, TOKEN, chat_body, chat_scenario, concat, free_port, read_arrivals,
    v1_config,
};

/// Content deltas that stress JSON escaping and look like the awkward frames of INV-01.
const AWKWARD_DELTAS: &[&str] = &[
    "Hello",
    " wörld 🌍",
    "",
    "line\nbreak\ttab \"quoted\" back\\slash",
    r#"{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"f","arguments":"{\"q\":"}}]}"#,
    ": this is an SSE comment line",
    "data: [DONE]",
    "<think>reasoning</think>",
];

async fn body_via(req: reqwest::RequestBuilder) -> (reqwest::StatusCode, Vec<u8>) {
    let resp = req.send().await.expect("request");
    let status = resp.status();
    (status, concat(&read_arrivals(resp).await))
}

/// Verifies: REQ-PRX-003/AC1
///
/// Streaming: the fake's SSE body (awkward deltas, then `data: [DONE]`) fetched through the
/// proxy equals the same body fetched straight from the fake.
#[tokio::test]
async fn streaming_body_through_the_proxy_equals_the_upstream_body() {
    let stack = Stack::with_token(chat_scenario(AWKWARD_DELTAS, 0)).await;
    let (direct_status, direct) = body_via(stack.upstream_chat(true)).await;
    let (proxied_status, proxied) = body_via(stack.chat(true)).await;
    assert_eq!(direct_status, reqwest::StatusCode::OK);
    assert_eq!(
        proxied_status,
        reqwest::StatusCode::OK,
        "screen:\n{}",
        stack.daemon.screen()
    );
    assert!(
        String::from_utf8_lossy(&direct).ends_with("data: [DONE]\n\n"),
        "upstream body is not a terminated SSE stream"
    );
    assert_eq!(
        proxied,
        direct,
        "proxied != upstream\nproxied: {:?}\nupstream: {:?}",
        String::from_utf8_lossy(&proxied),
        String::from_utf8_lossy(&direct)
    );
}

/// Verifies: REQ-PRX-003/AC1
///
/// Non-streaming: the JSON completion fetched through the proxy equals the upstream bytes.
#[tokio::test]
async fn non_streaming_body_through_the_proxy_equals_the_upstream_body() {
    let stack = Stack::with_token(chat_scenario(AWKWARD_DELTAS, 0)).await;
    let (direct_status, direct) = body_via(stack.upstream_chat(false)).await;
    let (proxied_status, proxied) = body_via(stack.chat(false)).await;
    assert_eq!(direct_status, reqwest::StatusCode::OK);
    assert_eq!(
        proxied_status,
        reqwest::StatusCode::OK,
        "screen:\n{}",
        stack.daemon.screen()
    );
    assert_eq!(
        proxied,
        direct,
        "proxied != upstream\nproxied: {:?}\nupstream: {:?}",
        String::from_utf8_lossy(&proxied),
        String::from_utf8_lossy(&direct)
    );
}

/// A full SSE stream as a real llama-server might send it: comments, a keep-alive, tool-call
/// deltas, a reasoning field, and the terminal frame.
const REALISTIC_SSE: &str = concat!(
    ": ping\n\n",
    "data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"},\"finish_reason\":null}]}\n\n",
    "data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"let me think\"},\"finish_reason\":null}]}\n\n",
    ": keep-alive\n\n",
    "data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"read_file\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\n",
    "data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"path\\\":\\\"src/main.rs\\\"}\"}}]},\"finish_reason\":null}]}\n\n",
    "data: {\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":7,\"total_tokens\":10}}\n\n",
    "data: [DONE]\n\n",
);

/// Verifies: REQ-PRX-003/AC1
///
/// A raw upstream sends `REALISTIC_SSE` split at awkward chunk boundaries (mid-line, inside
/// the `[DONE]` frame). The proxy must deliver exactly those bytes.
#[tokio::test]
async fn realistic_sse_with_comments_and_tool_calls_is_forwarded_byte_exact() {
    let bytes = REALISTIC_SSE.as_bytes().to_vec();
    let cuts = vec![1, 7, 40, 41, bytes.len() - 5, bytes.len() - 1];
    let upstream = RawUpstream::start(bytes.clone(), cuts).await;
    let (daemon, client) = daemon_for(&upstream).await;

    let resp = client
        .post(daemon.url("/v1/chat/completions"))
        .bearer_auth(TOKEN)
        .json(&chat_body(true))
        .send()
        .await
        .expect("proxy answered");
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::OK,
        "screen:\n{}",
        daemon.screen()
    );
    let got = concat(&read_arrivals(resp).await);
    assert_eq!(
        String::from_utf8_lossy(&got),
        REALISTIC_SSE,
        "proxied bytes differ from the upstream bytes"
    );
}

/// Verifies: REQ-PRX-003/AC1
///
/// `[PROP]`: for random bodies (any bytes, 0–4 KiB) and random chunk boundaries, the proxied
/// body equals the upstream body. One daemon serves every case; the raw upstream swaps its
/// response per request.
#[test]
fn any_upstream_body_with_any_chunking_is_forwarded_byte_exact() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let upstream = rt.block_on(RawUpstream::start(Vec::new(), Vec::new()));
    let (daemon, client) = rt.block_on(daemon_for(&upstream));

    let strategy = (
        prop::collection::vec(any::<u8>(), 0..4096),
        prop::collection::vec(any::<u16>(), 0..8),
    );
    let mut runner = TestRunner::new(PropConfig {
        cases: 24,
        ..PropConfig::default()
    });
    runner
        .run(&strategy, |(body, raw_cuts)| {
            let cuts = raw_cuts
                .iter()
                .map(|c| (*c as usize) % (body.len() + 1))
                .collect();
            upstream.set(body.clone(), cuts);
            let got = rt.block_on(async {
                let resp = client
                    .post(daemon.url("/v1/chat/completions"))
                    .bearer_auth(TOKEN)
                    .json(&chat_body(true))
                    .send()
                    .await
                    .expect("proxy answered");
                prop_assert_eq!(resp.status(), reqwest::StatusCode::OK);
                Ok(concat(&read_arrivals(resp).await))
            })?;
            prop_assert_eq!(got, body);
            Ok(())
        })
        .expect("proxied body equals upstream body for every case");
}

async fn daemon_for(upstream: &RawUpstream) -> (Daemon, reqwest::Client) {
    let port = free_port();
    let cfg = v1_config(port, Some(TOKEN), &upstream.base_url());
    (Daemon::spawn(&cfg, port).await, reqwest::Client::new())
}

/// Minimal HTTP/1.1 upstream: `GET /v1/models` reports `A` loaded (legacy shape);
/// `POST /v1/chat/completions` streams the configured body with chunked transfer encoding,
/// one HTTP chunk per configured cut, then closes the connection.
struct RawUpstream {
    addr: std::net::SocketAddr,
    response: Arc<Mutex<(Vec<u8>, Vec<usize>)>>,
}

impl RawUpstream {
    async fn start(body: Vec<u8>, cuts: Vec<usize>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind raw upstream");
        let addr = listener.local_addr().expect("addr");
        let response = Arc::new(Mutex::new((body, cuts)));
        let shared = response.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let shared = shared.clone();
                tokio::spawn(async move {
                    let _ = serve_one(stream, shared).await;
                });
            }
        });
        Self { addr, response }
    }

    fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn set(&self, body: Vec<u8>, cuts: Vec<usize>) {
        *self.response.lock().expect("response lock") = (body, cuts);
    }
}

async fn serve_one(
    mut stream: TcpStream,
    shared: Arc<Mutex<(Vec<u8>, Vec<usize>)>>,
) -> std::io::Result<()> {
    // Read the head, then the Content-Length body, so the request is fully consumed.
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).await? == 0 {
            return Ok(());
        }
        head.push(byte[0]);
    }
    let head_text = String::from_utf8_lossy(&head).into_owned();
    let request_line = head_text.lines().next().unwrap_or("").to_string();
    let content_length: usize = head_text
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse().unwrap_or(0))
        })
        .unwrap_or(0);
    let mut body = vec![0u8; content_length];
    stream.read_exact(&mut body).await?;

    if request_line.starts_with("GET /v1/models") {
        let json = format!(
            "{{\"object\":\"list\",\"data\":[{{\"id\":\"{MODEL}\",\"object\":\"model\",\"status\":\"loaded\"}}]}}"
        );
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
            json.len()
        );
        stream.write_all(resp.as_bytes()).await?;
    } else if request_line.starts_with("POST /v1/chat/completions") {
        let (bytes, cuts) = shared.lock().expect("response lock").clone();
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
            )
            .await?;
        let mut bounds: Vec<usize> = cuts.into_iter().filter(|c| *c < bytes.len()).collect();
        bounds.push(bytes.len());
        bounds.sort_unstable();
        bounds.dedup();
        let mut start = 0;
        for end in bounds {
            if end > start {
                let piece = &bytes[start..end];
                stream
                    .write_all(format!("{:x}\r\n", piece.len()).as_bytes())
                    .await?;
                stream.write_all(piece).await?;
                stream.write_all(b"\r\n").await?;
                stream.flush().await?;
                start = end;
            }
        }
        stream.write_all(b"0\r\n\r\n").await?;
    } else {
        stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await?;
    }
    stream.shutdown().await
}
