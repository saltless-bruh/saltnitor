#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
use saltnitor::auth::Redactor;
use saltnitor::config_v1::{ConfigV1, resolve_control_token, validate_key_file};
use std::os::unix::fs::PermissionsExt;

fn env<'a>(map: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |k| {
        map.iter()
            .find(|(n, _)| *n == k)
            .map(|(_, v)| v.to_string())
    }
}

/// Verifies: REQ-SEC-003/AC1
#[test]
fn token_from_env_and_from_a_private_file() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("key");
    std::fs::write(&f, "file-secret\n").unwrap();
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut notes = vec![];
    let cfg = ConfigV1 {
        control_token_env: Some("SALT_T110".into()),
        ..Default::default()
    };
    assert_eq!(
        resolve_control_token(
            &cfg,
            std::path::Path::new("/cfg/d.toml"),
            &env(&[("SALT_T110", "env-secret")]),
            &mut notes
        )
        .unwrap()
        .as_deref(),
        Some("env-secret")
    );
    let cfg = ConfigV1 {
        control_token_file: Some(f.to_string_lossy().into_owned()),
        ..Default::default()
    };
    assert_eq!(
        resolve_control_token(
            &cfg,
            std::path::Path::new("/cfg/d.toml"),
            &env(&[]),
            &mut notes
        )
        .unwrap()
        .as_deref(),
        Some("file-secret")
    );
    assert!(
        notes.iter().all(|n| !n.contains("secret")),
        "notes must never carry the value: {notes:?}"
    );
}

/// Verifies: REQ-SEC-003/AC2 — [RF-3] group-readable file, directory, foreign owner
#[test]
fn unsafe_key_files_are_config_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("key");
    std::fs::write(&f, "s").unwrap();
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o640)).unwrap();
    let cfg = ConfigV1 {
        control_token_file: Some(f.to_string_lossy().into_owned()),
        ..Default::default()
    };
    let e = resolve_control_token(
        &cfg,
        std::path::Path::new("/cfg/d.toml"),
        &env(&[]),
        &mut vec![],
    )
    .unwrap_err();
    assert_eq!(e.key, "control_token_file");
    assert!(e.found.contains("0640"), "{e}");
    let cfg = ConfigV1 {
        control_token_file: Some(dir.path().to_string_lossy().into_owned()),
        ..Default::default()
    };
    assert!(
        resolve_control_token(
            &cfg,
            std::path::Path::new("/cfg/d.toml"),
            &env(&[]),
            &mut vec![]
        )
        .unwrap_err()
        .found
        .contains("not a regular file")
    );
    assert!(
        validate_key_file(true, 0o600, 1234, 1000)
            .unwrap_err()
            .contains("owned by uid 1234")
    );
    assert!(validate_key_file(true, 0o600, 1000, 1000).is_ok());
}

/// Verifies: REQ-SEC-003/AC4
#[test]
fn literal_token_still_works_with_a_deprecation_warning() {
    let cfg = ConfigV1 {
        control_token: Some("lit".into()),
        ..Default::default()
    };
    let mut notes = vec![];
    assert_eq!(
        resolve_control_token(
            &cfg,
            std::path::Path::new("/cfg/d.toml"),
            &env(&[]),
            &mut notes
        )
        .unwrap()
        .as_deref(),
        Some("lit")
    );
    assert!(notes.iter().any(|n| n.contains("deprecated")), "{notes:?}");
    let two = ConfigV1 {
        control_token: Some("a".into()),
        control_token_env: Some("B".into()),
        ..Default::default()
    };
    assert_eq!(
        resolve_control_token(
            &two,
            std::path::Path::new("/cfg/d.toml"),
            &env(&[("B", "b")]),
            &mut vec![]
        )
        .unwrap_err()
        .key,
        "control_token"
    );
}

/// Verifies: REQ-SEC-006/AC1
#[test]
fn logs_and_crash_dumps_never_contain_tokens() {
    let r = Redactor::new(vec!["s3cr3t-value".into()]);
    assert_eq!(
        r.redact("auth ok s3cr3t-value done"),
        "auth ok <redacted> done"
    );
    assert_eq!(
        r.redact("Authorization: Bearer abc.def-123"),
        "Authorization: Bearer <redacted>"
    );
    assert_eq!(
        r.redact("GET /v1/ensure/stream?profile=A&token=zzz9"),
        "GET /v1/ensure/stream?profile=A&token=<redacted>"
    );
    let mut app = saltnitor::app::App::new(
        "cpu".into(),
        1,
        1.0,
        "gpu".into(),
        1.0,
        false,
        "127.0.0.1".into(),
        8080,
        "svc".into(),
        1,
        1,
    );
    app.redactor = r;
    app.add_log("router said Bearer s3cr3t-value".into());
    let dump = app.crash_dump_text("20260101_000000");
    assert!(
        !dump.contains("s3cr3t-value") && dump.contains("<redacted>"),
        "{dump}"
    );
}

/// Verifies: REQ-CFG-003/AC2, REQ-SEC-003/AC2 — a token-source error names the config file that was loaded
#[test]
fn token_source_errors_name_the_loaded_config_file() {
    let cfg = ConfigV1 {
        control_token_file: Some("/nonexistent/key".into()),
        ..Default::default()
    };
    let e = resolve_control_token(
        &cfg,
        std::path::Path::new("/etc/elsewhere/d.toml"),
        &env(&[]),
        &mut vec![],
    )
    .unwrap_err();
    assert_eq!(e.file, std::path::PathBuf::from("/etc/elsewhere/d.toml"));
    assert!(e.to_string().starts_with("/etc/elsewhere/d.toml"), "{e}");
}
