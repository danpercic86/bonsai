# Changelog

All notable changes to Bonsai are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] — 2026-08-05

An interactive side-by-side diff view, plus continued release-readiness hardening.

### Added
- Interactive side-by-side (split) diff view with synchronized horizontal scrolling,
  copyable selection, and auto-advance to the next file after staging.
- MIT `LICENSE` and end-user `README.md`.
- Top-level and per-pane React error boundaries (commit graph, diff view, conflict editor)
  so a render error is contained to a pane instead of white-screening the whole app.

### Changed
- Release builds no longer emit internal self-test/perf logs to the console.
- A Content-Security-Policy is now enforced for the app webview.
- The release workflow publishes releases directly (not as drafts) so the auto-updater's
  `latest.json` resolves via the `releases/latest` URL.

### Fixed
- The macOS `universal-apple-darwin` release build now installs the extra Rust targets onto
  the toolchain pinned by `rust-toolchain.toml` (via `actions-rust-lang/setup-rust-toolchain`),
  fixing the "target x86_64-apple-darwin is not installed" build failure.

### Notes
- **Verification in progress.** A large set of already-built features (force-push-with-lease,
  reflog viewer, bisect, Git config editing, first-run onboarding, per-line discard, named
  worktrees, background auto-fetch, the AI what-changed digest, and the repo-health
  dashboard) has passed automated tests and the browser harness but is still completing a
  formal native-window verification pass ahead of a future 1.0.0 tag.
- Installers ship **unsigned**. See the README for the one-time Windows SmartScreen /
  macOS Gatekeeper approval steps. OS code signing is planned for a later release.

## [0.2.0] — 2026-08-05

The MVP and first productization phase. Highlights:

### Added
- GitKraken-style commit graph rendered on canvas — multi-colored branch lanes, curved
  fork/merge edges, ref pills (branches, `origin/*` remotes, tags, HEAD), virtualized for
  large histories (20k+ commits).
- Three-pane workspace: branches / remotes / tags sidebar, commit graph, and a status +
  diff + commit-details panel.
- Working-directory status with file-level staging/unstaging and commit.
- Diffs for both working-directory changes and any selected commit (vs. its first parent).
- Branch management: list, create, checkout, delete.
- Remotes: fetch, fast-forward-only pull, and push, with credential handling.
- Merge, rebase, and stash workflows with conflict resolution.
- Multiple repositories open in tabs; the last repo reopens on launch.
- Tauri v2 auto-update scaffolding (behind Bonsai IPC) and a first-run onboarding overlay.
- An embedded MCP server exposing structured Git data (graph, diffs, conflicts) to AI tools.

[Unreleased]: https://github.com/danpercic86/bonsai/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/danpercic86/bonsai/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/danpercic86/bonsai/releases/tag/v0.2.0
