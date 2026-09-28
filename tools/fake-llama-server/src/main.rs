//! `fake-llama-server` binary: behaves like llama-server for process-level tests.
use std::io::Write;
use std::path::Path;

use fake_llama_server::{CrashMode, Recorder, Scenario, record, serve_on};
use serde_json::{Map, Value, json};

const DEFAULT_HELP: &str = include_str!("../fixtures/help-default.txt");
const DEFAULT_VERSION: &str = include_str!("../fixtures/version-default.txt");
/// Only these environment variables are recorded, so secrets never reach the JSONL.
const ENV_PREFIXES: &[&str] = &["LLAMA_", "GGML_", "CUDA_", "HIP_", "FAKE_"];

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("record") {
        std::process::exit(record::cli(&args[2..]).await);
    }
    if args.iter().skip(1).any(|a| a == "--help" || a == "-h") {
        return print_fixture("FAKE_HELP_FIXTURE", DEFAULT_HELP);
    }
    if args.iter().skip(1).any(|a| a == "--version") {
        return print_fixture("FAKE_VERSION_FIXTURE", DEFAULT_VERSION);
    }
    let scenario = match std::env::var_os("FAKE_SCENARIO") {
        Some(p) => Scenario::from_file(Path::new(&p))
            .unwrap_or_else(|e| fail(2, &format!("bad scenario: {e}"))),
        None => Scenario::default(),
    };
    let recorder = match std::env::var_os("FAKE_RECORD") {
        Some(p) => Recorder::to_file(Path::new(&p))
            .unwrap_or_else(|e| fail(2, &format!("cannot open record file: {e}"))),
        None => Recorder::default(),
    };
    recorder.record("argv", json!({ "argv": args }));
    let env: Map<String, Value> = std::env::vars()
        .filter(|(k, _)| ENV_PREFIXES.iter().any(|p| k.starts_with(p)))
        .map(|(k, v)| (k, json!(v)))
        .collect();
    recorder.record("env", json!({ "env": env }));

    if let Some(rule) = &scenario.oom {
        if let Some(v) = arg_value(&args, &rule.arg).and_then(|v| v.parse::<u64>().ok()) {
            if v > rule.gt {
                recorder.record("oom", json!({ "arg": rule.arg, "value": v }));
                eprintln!(
                    "ggml_backend_cuda_buffer_type_alloc_buffer: allocating {v} MiB on device 0: cudaMalloc failed: out of memory"
                );
                eprintln!(
                    "llama_init_from_model: failed to initialize the context: failed to allocate buffer"
                );
                std::process::exit(1);
            }
        }
    }

    let host = arg_value(&args, "--host").unwrap_or_else(|| "127.0.0.1".into());
    let port = arg_value(&args, "--port").unwrap_or_else(|| "8080".into());
    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}"))
        .await
        .unwrap_or_else(|e| fail(1, &format!("bind {host}:{port}: {e}")));
    println!(
        "FAKE_LISTENING {}",
        listener.local_addr().expect("local addr")
    );
    let _ = std::io::stdout().flush();
    if let Err(e) = serve_on(listener, scenario, recorder, CrashMode::ExitProcess).await {
        fail(1, &e.to_string());
    }
}

/// Value of `--name V` or `--name=V`.
fn arg_value(args: &[String], name: &str) -> Option<String> {
    let eq = format!("{name}=");
    args.iter().enumerate().find_map(|(i, a)| {
        if a == name {
            args.get(i + 1).cloned()
        } else {
            a.strip_prefix(&eq).map(str::to_string)
        }
    })
}

fn print_fixture(var: &str, default: &str) {
    match std::env::var_os(var).map(std::fs::read_to_string) {
        Some(Ok(text)) => print!("{text}"),
        Some(Err(e)) => fail(2, &format!("{var}: {e}")),
        None => print!("{default}"),
    }
}

fn fail(code: i32, msg: &str) -> ! {
    eprintln!("fake-llama-server: {msg}");
    std::process::exit(code)
}

#[cfg(test)]
mod tests {
    use super::arg_value;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Verifies: REQ-TST-011/AC1
    #[test]
    fn arg_value_reads_space_and_equals_forms_only_for_the_exact_flag() {
        assert_eq!(
            arg_value(&a(&["x", "--port", "9"]), "--port").as_deref(),
            Some("9")
        );
        assert_eq!(
            arg_value(&a(&["x", "--port=9"]), "--port").as_deref(),
            Some("9")
        );
        assert_eq!(arg_value(&a(&["x", "--port"]), "--port"), None);
        assert_eq!(arg_value(&a(&["x", "--portal", "9"]), "--port"), None);
    }
}
