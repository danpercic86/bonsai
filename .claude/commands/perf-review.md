---
description: Review the latest (or a given) Bonsai dev-mode session log for perf and render-stability problems
argument-hint: [path/to/bonsai-<ts>-<sessionId>.jsonl]
---

Run a perf review of a Bonsai dev-mode session log. Log path argument (optional): $ARGUMENTS

## 1. Locate the log
The app writes logs to `<app_config_dir>/logs` and metrics to `<app_config_dir>/metrics/usage.json`
(`src-tauri/src/obs/mod.rs`, `logs_dir` / `metrics_dir`). `app_config_dir` for identifier
`com.bonsai.app` is:

| OS | Directory |
|---|---|
| Windows | `%APPDATA%\com.bonsai.app` |
| macOS | `~/Library/Application Support/com.bonsai.app` |
| Linux | `~/.config/com.bonsai.app` |

- **With an argument:** use that file. Its session is the `<sessionId>` in the name.
- **Without one:** pick the newest `bonsai-*.jsonl` by modification time.
- **Either way,** collect **every part of that session**: `bonsai-<ts>-<sessionId>.jsonl` plus any
  `…-<sessionId>-<N>.jsonl`, ordered by `N`.
- **If there is no log** (no directory, or no `bonsai-*.jsonl`), stop and tell the user: turn on
  Dev mode in Settings → Developer, use the app for a while in `pnpm tauri dev`, then rerun
  `/perf-review`. Don't spawn anything.

## 2. Review
Spawn the `perf-reviewer` agent. Pass it:
- the part file **paths** in order, never their contents
- the `usage.json` path, if it exists
- the report path: `D:\Data\Temp\bonsai-perf-review\<sessionId>.md` on Windows, or the session
  scratchpad elsewhere.

## 3. Present
- **Relay the ranked summary.** Mark each suspected source location as a hypothesis until you
  have checked it against the repo yourself; check the top ones before presenting them.
- **Link the report file.**
- **Do not file anything in `TODO.md`.** Ask the user which findings to take on. Board items are
  filed only after the user triages, per the spec-008 ruling.
