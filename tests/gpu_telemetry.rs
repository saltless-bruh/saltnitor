#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! GPU telemetry never turns "could not measure" into a number (INV-18, REQ-TUI-006/AC1).
use saltnitor::gpu::{parse_device, parse_sample};

const SAMPLE: &str = "6144, 55, 120.5, 170.0, 40, 12, 30, 1800, 7000\n";

/// Verifies: REQ-TUI-006/AC1 — a well-formed nvidia-smi line is parsed field by field
#[test]
fn a_good_sample_is_parsed() {
    let s = parse_sample(SAMPLE).unwrap();
    assert!((s.vram_used_gb - 6.0).abs() < 1e-9);
    assert_eq!(s.temp_c, 55);
    assert_eq!(s.power, "120.5W / 170.0W");
    assert_eq!(s.util, "40");
    assert_eq!(s.clocks, "1800 MHz / 7000 MHz");
}

/// Verifies: REQ-TUI-006/AC1 — too few fields, empty output and non-numbers are errors, not zeros
#[test]
fn a_bad_sample_is_an_error_with_a_reason_never_zero() {
    for bad in [
        "",
        "garbage",
        "1, 2, 3",
        "n/a, 55, 1, 1, 1, 1, 1, 1, 1",
        "1, hot, 1, 1, 1, 1, 1, 1, 1",
    ] {
        let e = parse_sample(bad).unwrap_err();
        assert!(!e.is_empty(), "{bad:?} must come with a reason");
    }
}

/// Verifies: REQ-TUI-006/AC1 — an unparsable total is an error, never a fabricated 1 MiB card
#[test]
fn the_device_probe_rejects_what_it_cannot_parse() {
    let (name, total_gb) = parse_device("NVIDIA GeForce RTX 3060, 12288\n").unwrap();
    assert_eq!(name, "NVIDIA GeForce RTX 3060");
    assert!((total_gb - 12.0).abs() < 1e-9);
    assert!(parse_device("NVIDIA GeForce RTX 3060, [N/A]").is_err());
    assert!(parse_device("").is_err());
    assert!(parse_device("one field only").is_err());
}

/// Verifies: REQ-TUI-006/AC1 — a failed poll marks the GPU numbers unmeasured; the next good one restores them
#[test]
fn a_failed_poll_marks_the_gpu_values_unmeasured() {
    let mut app = saltnitor::app::App::new(
        "cpu".into(),
        1,
        1.0,
        "gpu".into(),
        12.0,
        true,
        "127.0.0.1".into(),
        8080,
        "svc".into(),
        1,
        1,
    );
    assert!(
        !app.gpu_measured,
        "nothing is measured before the first sample"
    );
    app.apply_gpu_sample(Some(parse_sample(SAMPLE).unwrap()));
    assert!(app.gpu_measured);
    assert!((app.vram_used - 6.0).abs() < 1e-9);
    app.apply_gpu_sample(None);
    assert!(
        !app.gpu_measured,
        "a failing query must not leave the old numbers looking live"
    );
}
