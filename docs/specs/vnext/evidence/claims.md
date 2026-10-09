# README claims ledger (REQ-DOC-001)

Every feature bullet and load-bearing sentence in `README.md` at the T1.16 commit, with the evidence that backs it. Status `kept` = the claim is true as written; `reworded` = the claim was changed to match the evidence; `removed` = the claim was false or unverifiable and was deleted. `(I)` = verified by inspection, `(new in v0.2)` = documents behaviour added in Phase 1.

| # | README claim (line) | Evidence | Status |
|---|---|---|---|
| 1 | Hardware Telemetry (L12) | `src/snapshots/saltnitor__ui__tests__dashboard.snap`; "hardware-agnostic" removed (NVIDIA + Linux only, per the Prerequisites bullets) | reworded |
| 2 | Hardware Inspectors (L14) | container bullet; rows below | kept |
| 3 | GPU Inspector (`g`) (L15) | `src/snapshots/saltnitor__ui__tests__gpu_inspector.snap`; PID/user/RAM/VRAM columns: `tests/process_control.rs::nvidia_smi_compute_apps_parse_with_null_memory_and_a_reason` | kept |
| 4 | CPU/System Inspector (`c`) (L17) | `src/snapshots/saltnitor__ui__tests__cpu_inspector.snap`; old "60/40 split" and "btop-style equalizer" wording removed (panels are 50/50 since T1.13) | reworded |
| 5 | Process control (`x` / `X`) (L19) | `tests/process_control.rs::terminating_one_of_two_same_named_processes_leaves_the_other_alive`, `::sigterm_ignorer_is_still_running_then_kill_ends_it`, `::changed_identity_is_refused_and_nothing_is_signalled`, `::protected_targets_are_refused_with_a_reason`, `::pidfd_is_taken_at_selection_and_the_fallback_also_works`; confirm prompt: `src/snapshots/saltnitor__ui__tests__cpu_inspector_confirm.snap`; BD-05, BD-26 | kept (new in v0.2; replaces "process sniper kills by name" which is removed) |
| 6 | Live Model Orchestration (Dual-Mode Bottom Deck) (L21) | container bullet; rows below | kept |
| 7 | Hot-Swap (L22) | `src/control_api.rs` tests `chat_hot_swaps_a_non_resident_model_first`, `ensure_loads_with_a_one_token_warm_request`; `src/snapshots/saltnitor__ui__tests__deck_hot_swap.snap` (oracle warning in the deck). "Warns of potential OOM" reworded to "estimate" and the external-VRAM gap (BD-13) stated | reworded |
| 8 | Router tuner (`t`) (L24) | `src/snapshots/saltnitor__ui__tests__tuner_page_{1,2,3}.snap` (title now `router.ini Tuner`, BD-30); key list = `kv` in the Enter handler of `src/main.rs`. "generates router.env" and "bash-wrapper translation" removed (BD-22); `router_ini` required: `src/main.rs` refusal log (REQ-SEC-013). Unwritten controls stated as open BD-17 | reworded |
| 9 | API Interrogator (`i`) (L29) | `src/interrogate.rs::tests::strike_posts_to_the_given_port_with_bearer_and_timings_flag` (goes through the control port with the client key) | reworded |
| 10 | Metrics (L30) | `src/interrogate.rs::tests::timings_in_the_final_chunk_give_measured_pp_and_tg`, `::no_timings_means_estimated_or_not_available_never_zero` | reworded |
| 11 | Client key (L31) | `src/interrogate.rs::tests::strike_posts_to_the_given_port_with_bearer_and_timings_flag`; `client_key_env` in `src/config_v1.rs`. "Immune to self-lockout" removed | reworded |
| 12 | History (L32) | `src/interrogate.rs::tests::history_lives_under_xdg_state_home` (BD-18, BD-21); old `.saltnitor_history` in the CWD removed | reworded |
| 13 | Incident Response (L34) | container bullet; rows below | kept |
| 14 | Crash Dump (`Ctrl+D`) (L35) | `tests/secrets.rs::logs_and_crash_dumps_never_contain_tokens`; path and error reporting: `src/main.rs` Ctrl+D handler (inspection) | kept |
| 15 | Kill-Switch (`Ctrl+K`) (L36) | `src/main.rs` Ctrl+K handler runs `sudo -n systemctl stop <service>` (inspection; no automated test - needs systemd and sudo, G1 demonstration row) | kept (I) |
| 16 | OS (L40) | inspection: `src/main.rs` uses `/proc`, `journalctl`, `systemctl` | kept (I) |
| 17 | Systemd (L41) | inspection: `journalctl -u <svc> -f` poller, `systemctl` actions in `src/main.rs` | kept (I) |
| 18 | NVIDIA Drivers (L42) | inspection: `nvidia-smi` poller in `src/main.rs`; `tests/process_control.rs::nvidia_smi_compute_apps_parse_with_null_memory_and_a_reason` | kept |
| 19 | llama.cpp (L43) | `examples/external-mode/launch_router.sh` exists | kept (I) |
| 20 | Permission model (L174) | inspection: only `sudo -n systemctl start|stop|restart <svc>` calls in `src/main.rs` | kept (I) |
| 21 | Model ids are your contract (L175) | `src/control_api.rs::tests::ensure_unknown_profile_is_404_envelope`; `tests/proxy_streaming.rs::the_model_value_is_the_profile_id_and_reaches_the_runtime_unchanged` | kept |
| 22 | First call to a cold model is slower (L176) | `src/control_api.rs::tests::ensure_loads_with_a_one_token_warm_request`, `::ensure_already_resident_skips_the_warm_load`. "instant" reworded | reworded |
| 23 | Terminal Sizing Guardrails (L177) | `src/snapshots/saltnitor__ui__tests__too_small_79x16.snap`, `..._too_small_80x15.snap` | kept |
| 24 | Prose: "OpenAI-compatible control endpoint ... automatic model hot-swaps" | `src/control_api.rs::tests::chat_hot_swaps_a_non_resident_model_first`; REQ-MIG-007/AC1 | kept |
| 25 | Prose: responses "streamed through chunk by chunk" (endpoint table) | `tests/proxy_streaming.rs::first_chunk_is_forwarded_before_the_second_is_sent`, `::downstream_bytes_equal_upstream_bytes_for_any_chunking`; G1 D1 (BD-02) | kept |
| 26 | Prose: "Oracle-gated (refuses loads it estimates will not fit)" | `src/control_api.rs::tests::ensure_oracle_reject_is_507_envelope`, `::chat_oracle_reject_is_507_envelope` | kept |
| 27 | Prose: "every `/v1` route requires a Bearer when a token is configured" | `tests/auth_policy.rs::every_route_gets_the_policy_outcome`, `::every_handler_has_a_policy_and_every_policy_a_handler` (BD-03) | kept (new in v0.2) |
| 28 | Prose: `allow_query_token` default off, GET /v1/ensure/stream only | `tests/auth_policy.rs::query_token_is_ignored_by_default`, `::query_token_in_compat_mode_works_only_on_get_ensure_stream_and_is_redacted` (BD-04) | kept (new in v0.2) |
| 29 | Prose: credentials never forwarded to llama-server | `tests/proxy_streaming.rs::client_credentials_are_stripped_and_the_body_is_forwarded_verbatim` | kept (new in v0.2) |
| 30 | Prose: strict config, exit 2, `--config` | `tests/config_strict.rs::type_error_exits_2_with_the_diagnostic_and_leaves_the_port_free`, `::unknown_key_exits_2_with_a_hint`, `::missing_config_flag_file_exits_2` (BD-01) | kept (new in v0.2) |
| 31 | Prose: `control_token_env` / `control_token_file` (0600, owner) | `tests/secrets.rs::token_from_env_and_from_a_private_file`, `::unsafe_key_files_are_config_invalid` | kept (new in v0.2) |
| 32 | Prose: `[timeouts]` and `max_body_bytes` defaults | `tests/proxy_failures.rs::hang_before_headers_is_504`, `::thirty_three_mib_is_413_and_thirty_one_mib_passes`; defaults in `src/config_v1.rs` (REQ params §6) | kept (new in v0.2) |
| 33 | Prose: `[process] term_grace_ms` | `tests/process_control.rs::sigterm_ignorer_is_still_running_then_kill_ends_it`; default in `src/config_v1.rs` | kept (new in v0.2) |
| 34 | Prose: setup 1, `router.ini` sections are model ids | `src/control_api.rs::tests::ensure_unknown_profile_is_404_envelope`; REQ-MIG-007 | kept |
| 35 | Prose: `--models-max 1` keeps one model resident | llama.cpp router behaviour (not a Saltnitor claim); serialized loads: `ensure_lock` in `src/control_api.rs` | kept (I) |
| 36 | Prose: "see integrations/INTEGRATION.md", Pi/OpenCode configs | directory does not exist (BD-22) | removed |
| 37 | Prose: "generates a native Linux `router.env`" | tuner edits `router_ini` sections (see Router tuner row) (BD-22) | removed |
| 38 | Prose: "process sniper kills by name" / `killall` | `tests/process_control.rs` (kill by exact PID); invariants script ratchet (BD-05) | removed |
| 39 | Prose: "dynamic API Key authorization lock-downs" / "Speculative Decoding targets" applied by the tuner | keys are not written to `router.ini` (BD-17 still open) | removed (reworded as displayed-only controls) |
