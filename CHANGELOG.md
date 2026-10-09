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

## [0.1.0] — 2026-05-23
Baseline (`master` @ `c89f278`). See `docs/specs/vnext/baseline/`.
