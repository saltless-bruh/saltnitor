//! Key classification for the process inspectors (REQ-TUI-010/AC1): `x`/`Delete` terminate
//! (SIGTERM), `X` kills after confirmation (SIGKILL). Found at G1 D3: terminals may report
//! Shift+x either as the uppercase `X` or as `x` with a SHIFT modifier — both mean kill.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcKey {
    Up,
    Down,
    /// SIGTERM the selected PID (`x` or `Delete`).
    Terminate,
    /// Arm the SIGKILL confirmation (`X`, or `x` + SHIFT).
    Kill,
    /// Close the inspector (`Esc`, `q`, or the pane's own toggle letter).
    Close,
    Other,
}

/// `close_char` is the letter that toggled this pane open (`g` for GPU, `c` for CPU).
pub fn classify(key: &KeyEvent, close_char: char) -> ProcKey {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let ctrl_or_alt = key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    match key.code {
        KeyCode::Up => ProcKey::Up,
        KeyCode::Down => ProcKey::Down,
        KeyCode::Esc => ProcKey::Close,
        KeyCode::Char('X') if !ctrl_or_alt => ProcKey::Kill,
        KeyCode::Char('x') if shift && !ctrl_or_alt => ProcKey::Kill,
        KeyCode::Char('x') if !ctrl_or_alt => ProcKey::Terminate,
        KeyCode::Delete => ProcKey::Terminate,
        KeyCode::Char('q') => ProcKey::Close,
        KeyCode::Char(c) if c == close_char && !ctrl_or_alt => ProcKey::Close,
        _ => ProcKey::Other,
    }
}
