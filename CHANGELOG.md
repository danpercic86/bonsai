# Changelog

All notable changes to Bonsai are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

AI merge-conflict resolution stops being a black box, and the right panel gives its space back
to the file list.

### Added
- **Live AI activity dock.** AI conflict resolution now streams its progress into a collapsible
  full-width dock at the bottom of the window: the model's output as it arrives, the tools it
  calls, elapsed time, and the run's cost. The dock is resizable and remembers its height and
  collapsed state.
- **Cancel.** An AI conflict run can be stopped at any time. Everything logged before the cancel
  stays on screen, and nothing is written or staged.
- **No hard time limit by default.** A conflict run is no longer cut off after 90 seconds. It is
  bounded instead by an idle watchdog (no output for 5 minutes), with an optional absolute cap you
  can enable yourself.
- **Answer Claude mid-run.** If the model needs a decision it cannot make alone, the run pauses,
  the question appears in the dock with a reply box, and your typed answer continues the same run
  through to a proposal. A run waiting on you is never timed out.
- **Per-file AI state.** Each conflicted file now tracks its own run and its own result, so a
  proposal is no longer lost when you click another file while a run is in flight — and a run on
  one file no longer disables the AI button on every other file.
- **"Resolve all with AI".** Available from the Conflicts section header and from the merge
  banner when there are at least two text conflicts. Claude sees the conflicts together, so a
  change split across several files can be resolved coherently. Each file gets its own outcome:
  a file the model fails on does not invalidate the rest.
- **Read-only repository access for AI conflict runs.** Claude may now use `Read`, `Grep` and
  `Glob` — and nothing else. It cannot write files, stage anything, or run commands. Being able
  to read the surrounding code is what lets it match your project's conventions, and it is the
  real fix for conflict runs that used to fail after 90 seconds with nothing to show: the run had
  previously been started with no tools at all, leaving the model blind to the repository.
- **Reads are fenced to the repository.** An attempt to read outside the repository folder is
  refused, and the refusal is shown as a line in the AI activity dock.
- **Eight AI run settings** under Settings → AI: repository access (Read-only or none), live log
  on/off, partial-message streaming, idle timeout, optional absolute time cap, maximum
  interactive turns, optional spend limit, and the bulk payload size cap. There is no
  write/edit/shell option, by design.
- **A HEAD guideline that stays put.** The dashed line connecting the working-directory row to
  the checked-out commit no longer vanishes after a few rows of scrolling, and when the
  checked-out commit is off-screen an edge marker shows which way it lies.
- **Cozy / Compact panel density** (Settings → Appearance) for the right panel, independent of
  the commit graph's compact rows.

### Changed
- The right panel's fixed controls were reorganised and tightened, giving about 115px — roughly
  five more file rows in the cozy default — back to the changes tree. Compact density frees a
  further ~30px. "Stash all" moved into an overflow menu; all three stash scopes are still there,
  and the sidebar keeps its one-click stash.

### Fixed
- The AI consent dialog described what happens inaccurately on two counts. It now states that
  Claude may read other files in the repository and that whatever it reads is sent to Anthropic
  with the request, that its tools are read-only and out-of-repository reads are refused, and
  that Bonsai writes to your files only when you apply a result — with the one exception spelled
  out below.

### Notes
- **`Resolve automatically` writes without a review step.** Under Settings → AI assistance, the
  `Resolve automatically` autonomy mode writes Claude's marker-free results to your files and
  stages them with no review; only results that still contain conflict markers open as proposals.
  Both autonomy modes now state their consequence next to the choice itself. `Propose & review`
  (the default) never writes until you apply a result.
- **A bulk resolve can cost more than one Claude run.** Bonsai splits the conflicts into as many
  sequential runs as the payload size requires, all against your Claude quota; `Cancel all` stops
  the remaining ones. No file is ever silently truncated — a single file too large to send is
  reported as failed for that file alone.
- AI conflict features still run entirely through the Claude Code CLI installed on your machine,
  under your own Claude subscription. Nothing goes to Bonsai servers.
- **Verification in progress.** Everything above has passed the automated suite and the browser
  harness; the native-window confirmation pass (real CLI, real conflicts, appearance) is still
  outstanding. See `docs/contracts/P67-user-checklist.md` and
  `docs/contracts/P68-user-checklist.md`.

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
- Rich commit graph rendered on canvas — multi-colored branch lanes, curved
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
