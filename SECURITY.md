# Security

Saltnitor manages a local llama.cpp runtime and exposes an OpenAI-compatible control API
(default `127.0.0.1:8765`). This page states what it defends against in v0.2.0, what it does not,
and how to report a problem.

## Threat model

| Actor | What Saltnitor does | Evidence |
|---|---|---|
| **Tailnet / network peers** that can reach the control port | When a control token is configured, every `/v1` route requires `Authorization: Bearer <token>`; failures are JSON envelopes, compared in constant time. `?token=` is off by default (`allow_query_token`). The control listener binds 127.0.0.1 only (BD-31: the address is not configurable yet); the raw llama-server stays on loopback. Client credentials are never forwarded upstream. | `tests/auth_policy.rs`, `tests/proxy_streaming.rs` |
| **Other local users** on the machine | Token and key files must be mode 0600 and owned by you, otherwise the config is rejected. Tokens are redacted from logs and crash dumps. The process control (`x`/`X`) only signals processes you own, by exact PID, and refuses PID 1, Saltnitor and the runtime. | `tests/secrets.rs`, `tests/process_control.rs` |
| **A misbehaving or hung runtime** | Connect, first-byte and idle timeouts, a request body limit (32 MiB by default), and client-disconnect propagation. | `tests/proxy_failures.rs` |
| **Bad configuration** | Strict parsing: unknown keys and wrong types exit with status 2 and are never silently replaced by defaults. | `tests/config_strict.rs` |

Without a configured token the API is unauthenticated: it is safe only because it binds loopback.
Set `control_token_env` or `control_token_file` before exposing it to anything else.

## Out of scope in v0.2.0

- Separate admin credentials (an inference token never grants admin scope, but no admin token exists yet). Planned for a later phase.
- Remote (non-loopback) listeners, TLS and per-client rate limits. Planned for a later phase; use a tailnet or SSH tunnel and a token meanwhile.
- The VRAM oracle ignores memory held by other processes (BD-13); it is advice, not a security control.
- The tuner rewrites the `router.ini` named by `router_ini` in place and restarts the unit with `sudo -n systemctl` (BD-25). Grant sudo only for the exact `start`/`stop`/`restart` commands shown in the README.

## Reporting
Open a private security advisory on the repository, or email the maintainer named in
`Cargo.toml`. Do not file public issues for vulnerabilities.
