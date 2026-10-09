#![allow(clippy::unwrap_used, clippy::expect_used)] // tests may unwrap: a panic is the failure signal
//! Process-inspector key classification (REQ-TUI-010/AC1). Found at G1 D3: a terminal that
//! reports Shift+x as `x` + SHIFT must still mean "kill (confirm)", never "terminate".
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use saltnitor::proc_keys::{ProcKey, classify};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

/// Verifies: REQ-TUI-010/AC1
#[test]
fn shift_x_is_kill_whether_the_terminal_reports_uppercase_or_a_shift_modifier() {
    assert_eq!(
        classify(&key(KeyCode::Char('X'), KeyModifiers::SHIFT), 'c'),
        ProcKey::Kill
    );
    assert_eq!(
        classify(&key(KeyCode::Char('X'), KeyModifiers::NONE), 'c'),
        ProcKey::Kill
    );
    assert_eq!(
        classify(&key(KeyCode::Char('x'), KeyModifiers::SHIFT), 'c'),
        ProcKey::Kill
    );
}

/// Verifies: REQ-TUI-010/AC1
#[test]
fn plain_x_and_delete_terminate_and_nothing_else_signals() {
    assert_eq!(
        classify(&key(KeyCode::Char('x'), KeyModifiers::NONE), 'c'),
        ProcKey::Terminate
    );
    assert_eq!(
        classify(&key(KeyCode::Delete, KeyModifiers::NONE), 'c'),
        ProcKey::Terminate
    );
    assert_eq!(
        classify(&key(KeyCode::Char('x'), KeyModifiers::CONTROL), 'c'),
        ProcKey::Other
    );
    assert_eq!(
        classify(&key(KeyCode::Char('z'), KeyModifiers::NONE), 'c'),
        ProcKey::Other
    );
}

#[test]
fn navigation_and_close_keys_classify_per_pane() {
    assert_eq!(
        classify(&key(KeyCode::Up, KeyModifiers::NONE), 'c'),
        ProcKey::Up
    );
    assert_eq!(
        classify(&key(KeyCode::Down, KeyModifiers::NONE), 'c'),
        ProcKey::Down
    );
    assert_eq!(
        classify(&key(KeyCode::Char('c'), KeyModifiers::NONE), 'c'),
        ProcKey::Close
    );
    assert_eq!(
        classify(&key(KeyCode::Char('g'), KeyModifiers::NONE), 'g'),
        ProcKey::Close
    );
    assert_eq!(
        classify(&key(KeyCode::Char('g'), KeyModifiers::NONE), 'c'),
        ProcKey::Other
    );
    assert_eq!(
        classify(&key(KeyCode::Char('q'), KeyModifiers::NONE), 'c'),
        ProcKey::Close
    );
    assert_eq!(
        classify(&key(KeyCode::Esc, KeyModifiers::NONE), 'c'),
        ProcKey::Close
    );
}
