# Context-pollution audit — oversized source files

> Snapshot: 2026-08-06. Measured with `wc -l`, excluding `node_modules/`, `target/`,
> `dist/`, lockfiles, and generated output. Test files (`*.test.ts`) and the already-isolated
> `src/ipc/fixtures/*` are exempt per the CLAUDE.md invariant (tests and fixtures are meant to
> be their own files).

## Why this exists

CLAUDE.md sets a **~500-line soft limit per file** (architecture invariants — "File-size /
single-responsibility discipline"). Files far over that limit force every session to read a
god-file in full to touch anything inside it, which is the single biggest avoidable drain on the
AI context budget. This doc ranks the offenders and records the recommended split for each, so
the work survives session restarts and can be picked up incrementally.

## Ranked offenders

| Lines | File | Over? | Problem | Recommended split |
|------:|------|:-----:|---------|-------------------|
| 5123 | `src-tauri/src/commands.rs` | 10× | 125 unrelated `#[tauri::command]`s in one flat file (repo, status, graph, staging, diffs, branches, remotes, tags, stashes, fetch/pull/push, merge, rebase, bisect, cherry-pick, revert, submodules, worktrees, blame, reflog, config, AI, MCP, profiles, health) | `commands/` submodules by domain (`repo.rs`, `branches.rs`, `remotes.rs`, `stash.rs`, `rebase.rs`, `worktree.rs`, `diff.rs`, `ai.rs`, `config.rs`, `mcp.rs`, `health.rs`, …), re-exported from `commands/mod.rs`. `invoke_handler!` in `lib.rs` keeps referencing the same fns via re-export → no call-site churn. |
| 4681 | `src/ipc/mock.ts` | 9× | Mock IPC handlers interleaved with large inline fixture/seed data (status snapshots, stash/blame/file-history seeds, MCP + scheduler mock state) | Move seed/fixture data into `src/ipc/fixtures/*`; split handlers by domain to mirror the commands split. Preserve `VITE_MOCK_IPC=1` behavior. |
| 3412 | `src/components/RepoWorkspace.tsx` | 7× | 171 hook calls / ~3000 lines of state+effects+handlers before the render body; render already delegates to child panes | Extract cohesive handler groups into custom hooks: `useReflogOverlay`, `useBranchActions`, `useStashActions`, `useRebaseFlow`, `useConflictFlow`, `useKeyboardShortcuts` (the two large keydown handlers). |
| 1709 | `src/ipc/types.ts` | 3× | 137 exported IPC types in one file (cohesive but oversized; read whole every session) | Domain split: `types/graph.ts`, `types/diff.ts`, `types/status.ts`, `types/ai.ts`, `types/settings.ts`, re-exported from an index. |
| 1480 | `src-tauri/src/mcp.rs` | 3× | Transport (TCP bind), auth middleware, server lifecycle, and tool dispatch mixed | `mcp/server.rs` (bind/start/stop), `mcp/auth.rs` (token + auth layer), `mcp/tools.rs` (handlers). |
| 1416 | `src-tauri/src/scheduler.rs` | 3× | Scheduling engine mixed with concrete job bodies (`execute_job` ~388 lines, `run_scheduler` ~535) | Extract job bodies into `scheduler/jobs.rs`; keep engine/planning/backoff in the module core. |
| 1209 | `src-tauri/src/settings.rs` | 2× | Many config structs + clamp helpers (mostly cohesive) | Lower priority; split only if it keeps growing. |
| 1027 | `src/App.tsx` | 2× | Top-level container with accreted utility + theme helpers (`folderName`, `isUsableRepo`, `clampLive`, `applyTheme`) | Move pure helpers to `src/utils/`; theme logic to a `useTheme` hook; keep App as thin composition. |
| 895 | `src/graph/GraphCanvas.tsx` | 1.8× | Component + hit-testing/tooltip interaction plumbing (`hitTest`, pointer/scroll handling) | Extract hit-testing/tooltip logic into `graph/interaction.ts`. |
| 835 | `src/ipc/tauri.ts` | 1.7× | ~130 `invoke<>`/channel wrappers (cohesive transport; grows lockstep with commands.rs) | Split alongside the commands refactor, mirroring the by-domain grouping. |
| 831 | `src/components/WorkspaceDialogs.tsx` | 1.7× | ~20 confirmation/action dialogs + a ~50-prop firehose in one component | Per-family files: `dialogs/DestructiveDialogs.tsx`, `dialogs/StashDialogs.tsx`, `dialogs/BranchTagDialogs.tsx`; group props into objects. |
| 825 | `src/graph/draw.ts` | 1.7× | Many canvas primitives (reasonably cohesive) | Optional: `draw/edges.ts`, `draw/refs.ts`, `draw/icons.ts`. |
| 809 | `src/components/Sidebar.tsx` | 1.6× | `Sidebar` + ~10 inline row sub-components (BranchRow, RemoteRow, TagRow, StashRow, SubmoduleRow, WorktreeRow…) | Move each row into `sidebar/*`. |

Just over the line, watch/split opportunistically: `StatusPanel.tsx` 698, `workspaceMenus.ts`
621, `ConflictEditor.tsx` 517, `AiAssetsPanel.tsx` 510. Just under: `AgentAssetEditor.tsx` 495,
`SettingsPanel.tsx` 488, `DiffView.tsx` 475, `ProfileManager.tsx` 454, `RepoHealthPanel.tsx`
448, `SettingsGitConfigSection.tsx` 432, `DiffBrowser.tsx` 432.

## Execution status

Splitting the **top 4** offenders now (highest read-cost relief), each as its own compiling,
reviewed, separately-committed increment:

1. [x] `commands.rs` → `commands/` submodules (5123 → 28 files, largest non-test 310) — `505cab7`
2. [x] `mock.ts` → handlers + fixtures (4681 → 43-line composer + 35 files) — `8f93d98`
3. [x] `RepoWorkspace.tsx` → handler hooks (3428 → 2086 + 14 hooks) — `733c254`
4. [x] `WorkspaceDialogs.tsx` → per-family dialog files (882 → 311 + 6 files) — `d0fd8f0`

Remaining rows (`types.ts`, `mcp.rs`, `scheduler.rs`, `App.tsx`, `GraphCanvas.tsx`, `tauri.ts`,
`draw.ts`, `Sidebar.tsx`) are lower-urgency follow-ups tracked here.
