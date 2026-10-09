# fake-llama-server

Test-only stand-in for `llama-server` (REQ-TST-011). Never released (`publish = false`).

## In-process (fast HTTP tests)

    let fake = fake_llama_server::spawn(Scenario::default().with_model("A", true)).await;
    // point the code under test at fake.base_url(); inspect fake.recorder / fake.loaded()

## As a process (argv, env, --help, crashes, OOM)

Inside this crate's tests: `env!("CARGO_BIN_EXE_fake-llama-server")`.
From other crates' tests: run `cargo build -p fake-llama-server` first, then use
`<target-dir>/debug/fake-llama-server` (target dir = `$CARGO_TARGET_DIR` or `<workspace>/target`).

    FAKE_SCENARIO=path/to/scenario.toml   scenario (see src/scenario.rs for every field)
    FAKE_RECORD=path/to/events.jsonl      append argv/env/request/disconnect/crash events
    FAKE_HELP_FIXTURE / FAKE_VERSION_FIXTURE   text printed for --help / --version

Prints `FAKE_LISTENING <addr>` on stdout once bound (`--port 0` picks a free port).
Exit codes: 1 OOM rule or bind failure · 2 bad scenario/record/fixture file · 101 `crash_after`.
Only env vars starting with LLAMA_, GGML_, CUDA_, HIP_, FAKE_ are recorded; the
Authorization header value is never recorded.

## Scenario TOML

    models_shape = "status_object"   # or "legacy"
    max_loaded = 1
    [[models]]
    id = "A"
    loaded = true
    [routes."POST /v1/chat/completions"]
    kind = "chunks"            # status | chunks | raw_chunks | hang_before_headers | crash_after | malformed
    items = ["a", "b"]
    delay_ms = 200

`raw_chunks` streams the `items` byte pieces verbatim (no SSE wrapping): the first at once, then
`delay_ms` before each later one. Optional `code` (default 200), `content_type` (default
`text/event-stream`) and `headers` (list of `[name, value]` pairs, e.g. hop-by-hop ones the proxy
must drop):

    [routes."POST /v1/chat/completions"]
    kind = "raw_chunks"
    items = ["data: A\n\n", "data: B\n\n"]
    delay_ms = 2000
    headers = [["x-fake-upstream", "1"]]

Each piece records `chunk_sent {path, index, t_unix_ms}` (wall-clock, for measuring proxy latency).
A client that goes away mid-stream records `disconnect`. In-process, `Handle::in_flight()` counts
raw-chunk streams still open.

## Record / replay (real-world check, CR-2)

    fake-llama-server record --upstream http://127.0.0.1:8080 --out DIR [--baseline DIR] [--bearer-env VAR]

Read-only GETs only. Saves sanitized bodies + manifest.json and prints a LIVE/DOWN/DEGRADED
and SHAPE-OK/SHAPE-DRIFT table. A scenario with `replay_from = "DIR"` serves those bodies.
