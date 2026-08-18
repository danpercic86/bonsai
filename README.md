<div align="center">

<img src="src-tauri/icons/128x128.png" alt="Bonsai" width="96" height="96" />

# Bonsai

**A fast, native-feeling desktop Git client built around a rich, multi-lane commit graph.**

Cross-platform (Windows · macOS · Linux) · Built with Tauri, Rust, and React.

</div>

---

Bonsai is a local Git client built around a smooth, multi-lane commit graph. Rust owns all of
the Git logic and the graph-layout math via [libgit2](https://libgit2.org/); the UI only
renders — so the graph stays fast even over histories of 20,000+ commits.

> **Status: preparing for the first public release (`1.0.0`).** The app is feature-complete
> for everyday Git work. A set of recently added features is still finishing a final
> native-window verification pass — see the [CHANGELOG](CHANGELOG.md).

## Screenshots

|                                                                      |                                                                    |
| :--------------------------------------------------------------------: | :------------------------------------------------------------------: |
| ![Multi-lane commit graph with staging](docs/assets/screenshots/workspace-graph.png) | ![Commit diff view](docs/assets/screenshots/commit-diff.png) |
| **Commit graph & staging** — multi-colored branch lanes, ref pills, and file-level staging in the same view. | **Commit diffs** — select any commit to see it diffed against its first parent. |
| ![Pull requests panel](docs/assets/screenshots/pull-requests.png) | ![Light theme](docs/assets/screenshots/workspace-light.png) |
| **Pull requests** — connect GitHub, GitLab, Bitbucket, or Azure DevOps to list, read, and open PRs. | **Light theme** — every view adapts to a light or dark theme. |

_Captured against the mock-IPC browser harness with fixture data — see
[docs/assets/screenshots/README.md](docs/assets/screenshots/README.md) for what's shown and how
to regenerate these._

## Features

- **Rich commit graph** — multi-colored branch lanes, smooth curved fork/merge
  edges, and ref pills for local branches, `origin/*` remotes, tags, and HEAD. Virtualized
  on a canvas so scrolling stays smooth over very large histories.
- **Three-pane workspace** — branches / remotes / tags on the left, the graph in the center,
  and working-directory status, diffs, and commit details on the right.
- **Stage & commit** — file-level staging/unstaging and commit, with author/committer taken
  from your Git config.
- **Diffs** — for both working-directory changes and any commit you select (vs. its first
  parent), including per-line discard of unstaged changes.
- **Branches & remotes** — create, checkout, and delete branches; fetch, fast-forward pull,
  push, and force-push-with-lease.
- **History tools** — reflog viewer with restore, `git bisect`, merge, rebase, and stashes,
  all with in-app conflict resolution.
- **Repository management** — multiple repos open in tabs, named worktrees, background
  auto-fetch, first-run onboarding, and in-app Git config editing.
- **Search & command palette** — commit/content search, a `Ctrl`/`Cmd`-K command palette, and
  filtering for the sidebar lists.
- **Pull requests** — connect a GitHub, GitLab, Bitbucket or Azure DevOps repository with a
  personal access token to list, read and open PRs, and see PR/CI badges on the graph.
- **Auto-update** — checks a signed release manifest and updates in place (opt-in).
- **AI features (optional, local)** — everything AI runs through the
  [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code) installed on your own machine,
  under your own subscription; nothing goes to Bonsai servers. Merge-conflict resolution with a
  live streaming log, cancel, mid-run questions and a "resolve all conflicts" option; commit
  messages and a WIP→commits composer; explain-commit and blame-why; semantic history search;
  changelog and PR-description drafting. AI is off until you enable it and accept the consent
  dialog, which spells out what is sent.
- **AI-ready for other tools** — an embedded MCP server exposes structured Git data (graph,
  diffs, conflicts) to AI tools, plus an optional "what changed" digest.

## Install

Prebuilt installers are attached to each [GitHub Release](https://github.com/danpercic86/bonsai/releases).

> **Note — v1 builds are unsigned.** Bonsai does not yet use OS code signing, so the first
> launch shows a publisher warning. This is expected; here's how to proceed:
>
> - **Windows** — the SmartScreen dialog says "Windows protected your PC". Click
>   **More info → Run anyway**.
> - **macOS** — Gatekeeper may block the first launch. **Right-click the app → Open**, then
>   confirm; or allow it under **System Settings → Privacy & Security → Open Anyway**.
> - **Linux** — for the `.AppImage`, make it executable (`chmod +x Bonsai_*.AppImage`) and
>   run it; or install the `.deb`.

OS code signing (Authenticode / Apple notarization) is planned for a later release — see
[docs/code-signing.md](docs/code-signing.md).

## Build from source

**Prerequisites** (all platforms): [Rust](https://rustup.rs/) (via rustup), Node LTS +
[pnpm](https://pnpm.io/), and the Tauri CLI (installed by `pnpm install`). Plus a per-OS
toolchain for building libgit2 and the native webview:

- **Windows** — MSVC build tools; WebView2 (bundled on Windows 11).
- **macOS** — Xcode Command Line Tools (system WebKit is used).
- **Linux** — a C toolchain (e.g. `build-essential`) plus `webkit2gtk` dev packages.

```bash
pnpm install
pnpm tauri dev      # run the app in development
pnpm tauri build    # produce a release build + installers
```

Frontend-only development (no native window) runs against mock Git data in a plain browser:

```bash
pnpm dev:mock       # Vite in mock mode; open the printed URL (port 1420)
```

## Tech stack

- **Backend** — Rust, [Tauri v2](https://v2.tauri.app/), `git2` (libgit2), `notify`, `serde`.
- **Frontend** — React + Vite + TypeScript; the commit graph is drawn on `<canvas>`.
- **Package manager** — pnpm.

## License

[MIT](LICENSE) © 2026 Dan Percic
