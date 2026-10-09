//! Saltnitor library surface (CR-7, REQ-ARCH-002). Integration tests and the acceptance suite
//! import from here; `src/main.rs` owns the TUI loop and imports these same modules.
pub mod app;
pub mod auth;
pub mod config_v1;
pub mod control_api;
pub mod error;
pub mod events;
pub mod interrogate;
pub mod process;
pub mod proxy_stream;
pub mod ui;
