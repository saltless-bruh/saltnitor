# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow SemVer.

## [Unreleased]

### Removed
- `legacy.zip` (pre-control-API sources from May 2026): every file matched a committed
  revision of `src/` (checked with `git log --all -p -- src/`), so the archive was dropped
  without a tag.
- Tracked local state (`.saltnitor_history`, a crash dump, `.vscode/`) — now ignored.

### Changed
- `Contributing_Guidelines.md` → `CONTRIBUTING.md`.

## [0.2.0] — unreleased

Phase 1 hardening. Breaking changes are under **Changed**. Defect ids refer to
`docs/specs/vnext/baseline/DEFECTS.md`; only defects that landed with a test or a CI gate are listed.

### Security
- Bearer authentication is enforced on every `/v1` route whenever a control token is configured (BD-03). Errors are JSON envelopes.
- `?token=` is rejected by default; `allow_query_token = true` re-enables it for `GET /v1/ensure/stream` only, and the value is redacted from logs (BD-04).
- Secrets come from the environment or a private file: `control_token_env`, `control_token_file` (mode 0600, owned by you), `client_key_env`. A literal `control_token` still works with a deprecation warning. Configured tokens are redacted from logs and crash dumps.
- Client credentials are never forwarded to llama-server; `infer_bearer` is the only upstream credential.
- The hardcoded bearer `sk-saltnitor-2026` and the `/home/laz/...` paths are gone from the sources (BD-06, BD-07); `scripts/check-invariants.sh` guards against their return.

### Changed
- **Breaking:** configuration is strict. A syntax error, wrong type or unknown key prints file, line and key and exits with status 2 instead of falling back to defaults (BD-01). New `--config <path>` flag.
- **Breaking:** the `SUDO_USER` config-directory switching is removed; the config file is `--config`, `$XDG_CONFIG_HOME/saltnitor/config.toml` or `~/.config/saltnitor/config.toml`.
- **Breaking:** `router_ini` must be set for the tuner to write anything; there is no default path (BD-06).
- **Breaking:** the inspector `x` key no longer kills by name. `x` terminates and `X` kills the selected PID, with a `y/N` confirmation for the kill (BD-05).
- The proxy streams responses chunk by chunk instead of buffering them (BD-02). Request bodies up to 32 MiB (`max_body_bytes`) are accepted (BD-24). New `[timeouts]` and `[process]` sections.
- Errors from the control API use one JSON envelope with documented codes and statuses; oracle rejections are 507 on every route (BD-29).
- The interrogator sends its request through Saltnitor's own endpoint with the client key and reports measured rates from the runtime's `timings`, else `est.`/`n/a` (BD-18).
- The interrogator history moved from `./.saltnitor_history` to `$XDG_STATE_HOME/saltnitor/history` (default `~/.local/state/saltnitor/history`) (BD-21).
- The tuner window is titled `router.ini Tuner` (BD-30).
- `Cargo.lock` is tracked and CI builds with `--locked` (BD-08); CI runs the test suite and the invariant scan (BD-10); the release workflow no longer uses the archived upload action (BD-11).
- README, CHANGELOG and SECURITY rewritten against a claims ledger (`docs/specs/vnext/evidence/claims.md`) (BD-22).

### Fixed
- BD-01, BD-02, BD-03, BD-04, BD-05, BD-06, BD-07, BD-08, BD-09, BD-10, BD-11, BD-15 (control-API bind failures are reported), BD-16 (no panicking `unwrap`/`expect` on fallible paths), BD-18, BD-21, BD-22, BD-24, BD-26 (preflight no longer requires `killall`), BD-29, BD-30.
- Not fixed in this release: BD-12 (`main()` size), BD-13, BD-14, BD-17 (draft model, context shift, metrics and API Key tuner controls are still not written), BD-19, BD-20, BD-23, BD-25, BD-27, BD-28, BD-31, BD-32, BD-33, BD-34.

### Removed
- `killall`, and every kill-by-name path (BD-05).
- The literal API key and home-directory paths in the sources and scripts.
- README claims that the tuner "generates `router.env`" and links to `integrations/INTEGRATION.md`, which never existed in the tree (BD-22).

## [0.1.0] — 2026-05-23
Baseline (`master` @ `c89f278`). See `docs/specs/vnext/baseline/`.
