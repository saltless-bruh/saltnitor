//! Gate acceptance suite (REQ-TST-012, REQ-TST-014). One cargo target, `test = false`, so
//! plain `cargo test` never runs it. Run with `cargo test --test acceptance`.
//!
//! Every test here is black-box: it drives the built `saltnitor` binary in a pseudo-terminal
//! against `fake-llama-server`, and never imports `src/`. The one exception is `g1_process`,
//! which calls the process-control module API named in `README.md` (CR-7: the library crate
//! arrives with T1.8). Until that API exists, build the rest of the suite with
//! `RUSTFLAGS="--cfg acceptance_without_process_api --check-cfg cfg(acceptance_without_process_api)"`.

#![allow(
    unexpected_cfgs,
    reason = "`acceptance_without_process_api` is passed via RUSTFLAGS until the process API exists (README)"
)]

mod harness;

mod g1_auth;
mod g1_byte_exact;
mod g1_cancel;
mod g1_compositional;
mod g1_config;
#[cfg(not(acceptance_without_process_api))]
mod g1_process;
mod g1_streaming;
