# P87 — Git activity observability (workstreams C + D)

ONE git-activity event stream, TWO views:
- **View C (live in-flight):** the toolbar shows phase-aware progress ("Running pre-push
  hook…" vs "Pushing…") instead of a bare "Pushing…" spinner.
- **View D (session log):** a persistent, reviewable log of every git command + hook run this
  session (incl. passing ones), with per-hook exit status + timestamps.

Pattern mirrors the AI stream (`crates/bonsai-core/src/ai/stream.rs`, `src-tauri/src/commands/ai_stream.rs`,
`src/components/repoWorkspace/useAiRuns.ts`, `src/ipc/mock/handlers/aiStream.ts`).

**Non-goals / unchanged:** buffered `GitExec::exec`, `AppError::HookRejected`, `useHookGate`,
`HookOutputDialog`, all command *return* types, the `skip_hooks` semantics. The activity stream is
**fire-and-forget observability** layered beside the existing request/response — it never gates or
changes an operation's success/error.

---

## 1. Module map

| File | Responsibility | Status |
|---|---|---|
| `crates/bonsai-core/src/git/activity.rs` | **NEW.** `GitActivityEvent`+kinds/phases/category, `GitStream`, `GitActivityRecorder` trait, `ActivityEmitter`, `new_activity_id()`, `activity_line()` (cap+strip). | new (~180 lines) |
| `crates/bonsai-core/src/git/exec.rs` | Add `LineSink`, `GitStream` re-export, `GitExec::exec_streaming` (default = buffered). Buffered `exec` + `GitOutput` unchanged. | edit |
| `crates/bonsai-core/src/git/exec_stream.rs` | **NEW.** `SpawnGitExec::exec_streaming` impl (reader threads + line splitter). | new — see §6 split |
| `crates/bonsai-core/src/git/hooks.rs` | Add `run_hook_streaming` / `run_hook_nonblocking_streaming`; keep old names as `None` wrappers. | edit |
| `crates/bonsai-core/src/git/commit.rs`, `remote.rs` | Cores gain a trailing `activity: Option<&dyn GitActivityRecorder>`; fire phase transitions. | edit |
| `src-tauri/src/state.rs` (AppState) | Hold a `GitActivityHub`. | edit |
| `src-tauri/src/commands/activity.rs` | **NEW.** `git_activity_subscribe` command + `with_activity` bracket helper. | new (~120) |
| `src-tauri/src/commands/{remotes,staging,merge}.rs` | Command inners wrap the core call in `with_activity`. **Public command signatures UNCHANGED.** | edit |
| `src/ipc/types/activity.ts` | **NEW.** TS mirror of the event + store record types. | new |
| `src/ipc/tauri/activity.ts`, `src/ipc/types/ipc-api.ts` | `gitActivitySubscribe` wrapper + `IpcApi` entry. | edit |
| `src/ipc/mock/gitActivity.ts`, `src/ipc/mock/handlers/activity.ts` | **NEW.** subscribe + `runMockActivity` emitter; query seams. | new |
| `src/ipc/mock/handlers/{remotesSync,status,merge}.ts` | Wrap bodies in `runMockActivity`. | edit |
| `src/components/repoWorkspace/useGitActivity.ts` (+ `gitActivityState.ts`, `gitActivityLog.ts`) | **NEW.** store for both views. Split like `useAiRuns` (state/transforms + log-append in siblings). | new |

---

## 2. Event model — `git/activity.rs`

Flat struct + `kind` discriminant (house `AiRunEvent` shape), camelCase serde. Optionals use
`skip_serializing_if = "Option::is_none"` so a line event stays tiny on the wire.

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitActivityEvent {
    pub id: String,                 // stable per activity; first delivered on Started
    pub seq: u64,                   // monotonic from 0 per activity; drop seq <= last-seen
    pub kind: GitActivityKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<GitActivityCategory>, // Started only
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<GitPhase>,    // Started + Phase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,       // StdoutLine / StderrLine only; capped + control-stripped
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook: Option<String>,       // HookDone only ("pre-commit"…"pre-push")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,          // HookDone + Finished (None = killed / no exit code)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,      // HookDone + Finished
    pub elapsed_ms: u64,            // since the Started event
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitActivityKind { Started, Phase, StdoutLine, StderrLine, HookDone, Finished }

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitActivityCategory { Commit, Amend, MergeCommit, Push, ForcePush, Fetch, Pull }

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPhase { pub kind: GitPhaseKind, #[serde(skip_serializing_if="Option::is_none")] pub hook: Option<String> }

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitPhaseKind { Preparing, RunningHook, Network, Finalizing }

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitStream { Stdout, Stderr }
```

**Bounds (backend):** `pub const MAX_ACTIVITY_LINE_CHARS: usize = 2000;` — `activity_line(raw)`
truncates to that (char boundary, trailing `…`) and strips C0/C1 + bidi controls so one hook line
can never forge extra log rows (reuse the `strip_control_chars` rule from `ai/stream.rs` — lift a
shared helper or duplicate the ~6-line fn). Total output is still capped by exec's existing 64 MiB
combined counter; per-run line *count* is bounded on the frontend (§8).

### Recorder + emitter (runtime-free)

```rust
/// Object-safe sink the callers thread through core (None = buffered/no-op path).
pub trait GitActivityRecorder: Send + Sync {
    fn phase(&self, kind: GitPhaseKind, hook: Option<&str>);
    fn line(&self, stream: GitStream, line: &str);
    fn hook_done(&self, hook: &str, code: Option<i32>, success: bool);
}

/// Owns id + monotonic seq + start Instant; forwards each event to `emit`. All
/// methods take &self (AtomicU64 seq) so it can be shared as `Arc<ActivityEmitter>`.
pub struct ActivityEmitter { /* id, start: Instant, seq: AtomicU64, emit: Box<dyn Fn(GitActivityEvent)+Send+Sync> */ }
impl ActivityEmitter {
    pub fn new(id: String, emit: Box<dyn Fn(GitActivityEvent) + Send + Sync>) -> Self;
    pub fn started(&self, category: GitActivityCategory, phase: GitPhaseKind); // seq 0
    pub fn finished(&self, code: Option<i32>, success: bool);
}
impl GitActivityRecorder for ActivityEmitter { /* phase/line/hook_done */ }

pub fn new_activity_id() -> String; // e.g. format!("git-{}-{}", process::id(), NEXT.fetch_add(1))
```

`started`/`finished` are emitted by the command bracket (§7); `phase`/`line`/`hook_done` by core.
Human copy for labels/phases is derived by the UI (Rust emits structured data only) — see §11.

---

## 3. Streaming exec seam — `exec.rs` + `exec_stream.rs`

Additive. The buffered `exec` and `GitOutput` are untouched.

```rust
/// Called once per output line (already \n-split), on the caller's thread.
pub trait LineSink { fn line(&self, stream: GitStream, line: &str); }

pub trait GitExec {
    fn exec(&self, args: &[&str], cwd: &Path, stdin: Option<&[u8]>, env: &[(&str,&str)]) -> Result<GitOutput, AppError>;

    /// Streams lines to `sink` as they arrive AND still returns the full buffered
    /// GitOutput (so the failure path keeps the complete combined output for
    /// HookOutputDialog). DEFAULT = ignore the sink and delegate to `exec`, so
    /// every existing fake/mock that only impls `exec` keeps compiling and behaves
    /// exactly as before (no lines, same GitOutput).
    fn exec_streaming(&self, args: &[&str], cwd: &Path, stdin: Option<&[u8]>,
                      env: &[(&str,&str)], sink: &dyn LineSink) -> Result<GitOutput, AppError> {
        let _ = sink; self.exec(args, cwd, stdin, env)
    }
}
```

`SpawnGitExec::exec_streaming` (in `exec_stream.rs`) — reuses `build_command` + the 64 MiB shared
counter; adds incremental line delivery:

```
spawn child (same hardening as exec); write+close stdin as today
counter = shared AtomicUsize
(tx, rx) = mpsc channel of (GitStream, Vec<u8> capped-buffer-chunk?)   // see note
spawn stdout reader thread: read_capped-style loop, but split on b'\n';
    for each complete line: send LineMsg{Stdout, bytes}; also append to local capped Vec
spawn stderr reader thread: same with GitStream::Stderr
DRAIN on caller thread: for msg in rx { sink.line(msg.stream, &activity_line(utf8_lossy(msg.bytes))) }
join both threads -> (stdout_bytes, of1), (stderr_bytes, of2)   // full buffers for GitOutput
child.wait(); overflow -> AppError::Git (unchanged); else build GitOutput exactly as `exec`
```

Note: readers own the byte accumulation + a `\n` splitter (flush trailing partial line at EOF);
they hand *complete lines* to the caller thread via the channel, so `sink.line` runs only on the
caller thread (LineSink need not be `Sync`). The returned `GitOutput` is **byte-identical** to what
buffered `exec` would produce for the same child.

**Adapter** (so a recorder can drive the sink) — in `hooks.rs`:
```rust
struct RecorderSink<'a>(&'a dyn GitActivityRecorder);
impl LineSink for RecorderSink<'_> {
    fn line(&self, s: GitStream, l: &str) { self.0.line(s, l); }
}
```

---

## 4. Hook engine threading — `hooks.rs`

Keep `run_hook` / `run_hook_nonblocking` as thin `None` wrappers (all existing tests unchanged);
add streaming variants:

```rust
pub fn run_hook(exec, workdir, hook, args, stdin) -> Result<(), AppError>          // = run_hook_streaming(.., None)
pub fn run_hook_streaming(exec: &dyn GitExec, workdir: &Path, hook: HookName,
    args: &[String], stdin: Option<&[u8]>, activity: Option<&dyn GitActivityRecorder>) -> Result<(), AppError>;

pub fn run_hook_nonblocking(exec, workdir, hook, args) -> HookRunInfo               // = _streaming(.., None)
pub fn run_hook_nonblocking_streaming(exec, workdir, hook, args, activity: Option<&dyn GitActivityRecorder>) -> HookRunInfo;
```

`run_hook_streaming` body:
```
if plan_hook == Skip -> return Ok(())                       // no phase, no events (unchanged)
if let Some(a) = activity { a.phase(RunningHook, Some(hook.as_str())) }
out = match activity {
    Some(a) => exec.exec_streaming(argv, workdir, stdin, &[], &RecorderSink(a))?,
    None    => exec.exec(argv, workdir, stdin, &[])?,       // exact current path
};
if let Some(a) = activity { a.hook_done(hook.as_str(), out.code, out.success); }
// classification below is 100% UNCHANGED: out.success -> Ok; is_unknown_subcommand ->
// Git; is_git_infra_failure -> Git; else -> AppError::HookRejected(combined_output).
```
`run_hook_nonblocking_streaming`: same streaming of lines + `hook_done`; the returned `HookRunInfo`
(and its `warning`) is unchanged. The combined output for `HookRejected` / `HookRunInfo` still comes
from `out.stdout`/`out.stderr`, independent of whether lines were streamed.

---

## 5. Caller phase map — `commit.rs`, `remote.rs`

Each core gains a trailing `activity: Option<&dyn GitActivityRecorder>` (direct test callers pass
`None`). Phase transitions:

- **`create_commit`** (category `Commit`; `Amend` variant if applicable): `Preparing` (default)
  → pre-commit is a `RunningHook` phase (via `run_hook_streaming`) → `Finalizing` for the git2
  write → commit-msg `RunningHook` → post-commit `RunningHook` (nonblocking) after the ref moves.
  Wire the two `run_hook` sites (`commit.rs:124`, `:269`) and the post-commit sites (`:202`,`:398`,
  via `run_hook_nonblocking_streaming`).
- **`push_current`** (`Push`): up-to-date short-circuit emits nothing extra. Else: pre-push is a
  `RunningHook` phase (`remote.rs:388` → `run_hook_streaming`), then `activity.phase(Network, None)`
  immediately before `remote.push(...)` (`remote.rs:~426`). ← **This is the C fix**: the UI shows
  "Running pre-push hook…" during the hook and "Pushing…" once `Network` arrives.
- **`force_push_with_lease`** (`ForcePush`): same, `remote.rs:587` hook site → `Network` before the
  git-binary push.
- **`fetch_all`** (`Fetch`) / **`pull_ff`** (`Pull`): no hooks; emit `phase(Network)` at entry.
  These use libgit2 (no exec lines). **RECOMMENDED (low cost, directly fixes "feels hung"):** wire
  git2 `RemoteCallbacks::sideband_progress` → `activity.line(Stderr, ..)` and
  `transfer_progress` → a coarse `StdoutLine` (throttled, e.g. every ~1%/250ms). If deferred, the
  op still shows as a `Network`-phase running row with start/end + Finished. Flagged §12.

Normal `push` (libgit2 network) likewise: only the pre-push hook streams lines; the `Network` phase
has no lines unless sideband is wired.

---

## 6. exec.rs file-size split

`exec.rs` is 386 lines; adding the trait method + `LineSink` keeps it, but the `SpawnGitExec::exec_streaming`
impl + reader/splitter + its tests would push it past the ~500 house limit. **Split:** the streaming
impl and its tests live in **`crates/bonsai-core/src/git/exec_stream.rs`** (`impl GitExec for SpawnGitExec`
can be split across files, or move both `exec`+`exec_streaming` there — recommend: keep trait +
`GitOutput` + `build_command` + `read_capped` in `exec.rs`, put the streaming reader in `exec_stream.rs`).

---

## 7. IPC surface

**One long-lived subscription, not per-command** (a session log must span many ops and future ops
appear automatically). All ops emit onto registered channels via the AppState hub.

### Command
```rust
/// Registers a long-lived channel that receives GitActivityEvents for EVERY git op
/// this session. Called once by the frontend on app/repo mount; re-invoked after an
/// HMR/reload (stale channels are pruned on send failure). Returns immediately.
#[tauri::command]
pub fn git_activity_subscribe(state: State<AppState>, on_event: tauri::ipc::Channel<GitActivityEvent>) -> ();
```

### AppState hub
```rust
#[derive(Clone, Default)]
pub struct GitActivityHub { subs: Arc<Mutex<Vec<tauri::ipc::Channel<GitActivityEvent>>>> }
impl GitActivityHub {
    pub fn subscribe(&self, ch: Channel<GitActivityEvent>);
    pub fn emit(&self, ev: GitActivityEvent);   // fan-out; drop a channel whose send() errs
    pub fn is_active(&self) -> bool;             // cheap skip when nobody is listening
}
```

### Command bracket (used by every op inner; public command signatures UNCHANGED)
```rust
async fn with_activity<T, F, Fut>(hub: GitActivityHub, category: GitActivityCategory, run: F) -> Result<T, AppError>
where F: FnOnce(Arc<ActivityEmitter>) -> Fut, Fut: Future<Output = Result<T, AppError>>
{
    let hub2 = hub.clone();
    let emitter = Arc::new(ActivityEmitter::new(new_activity_id(), Box::new(move |ev| hub2.emit(ev))));
    emitter.started(category, GitPhaseKind::Preparing);
    let res = run(emitter.clone()).await;
    match &res {
        Ok(_)  => emitter.finished(Some(0), true),
        Err(e) => emitter.finished(activity_exit_code(e), false), // HookRejected/Git -> None/1; see below
    }
    res
}
```
`activity_exit_code` maps `AppError` → `Option<i32>` (best-effort; `None` when unknown). `run`
threads `Some(emitter.as_ref())` into the core call inside its `spawn_blocking`. Example — `push_inner`:
```rust
with_activity(state.git_activity_hub(), GitActivityCategory::Push, |emitter| async move {
    let path = repo_path(state, repo_id)?;
    spawn_blocking(move || push_current(&path, &SpawnGitExec, skip, Some(emitter.as_ref()))).await?
}).await
```
`ActivityEmitter` (and thus `Arc<ActivityEmitter>`) is `Send + Sync`, so it crosses into
`spawn_blocking`; core sees `&dyn GitActivityRecorder`.

### TS wrapper + IpcApi
```ts
// ipc/tauri/activity.ts
gitActivitySubscribe(onEvent: (e: GitActivityEvent) => void): Promise<void> {
  const channel = new Channel<GitActivityEvent>();
  channel.onmessage = onEvent;
  return invoke<void>('git_activity_subscribe', { onEvent: channel });
}
// ipc/types/ipc-api.ts
gitActivitySubscribe(onEvent: (e: GitActivityEvent) => void): Promise<void>;
```
No change to `push/forcePush/fetch/pull/commit/commitMerge` IPC signatures.

### TS event type — `ipc/types/activity.ts` (mirrors §2, camelCase, absent = optional)
```ts
export type GitActivityKind = 'started'|'phase'|'stdoutLine'|'stderrLine'|'hookDone'|'finished';
export type GitActivityCategory = 'commit'|'amend'|'mergeCommit'|'push'|'forcePush'|'fetch'|'pull';
export type GitPhaseKind = 'preparing'|'runningHook'|'network'|'finalizing';
export interface GitPhase { kind: GitPhaseKind; hook?: string }
export interface GitActivityEvent {
  id: string; seq: number; kind: GitActivityKind; elapsedMs: number;
  category?: GitActivityCategory; phase?: GitPhase; line?: string;
  hook?: string; code?: number; success?: boolean;
}
```

### Mock contract — `ipc/mock/gitActivity.ts` + `handlers/activity.ts`
- `gitActivitySubscribe` stores the callback(s) in module state (like the events bus).
- `runMockActivity(category, label?, opts, async fn)` wraps a handler body: emits `started` →
  optional `phase`/`stdoutLine`/`hookDone` → runs `fn` → `finished` (success from resolve, failure
  from throw, with a matching `code`). A shared `sequencer` (copy `aiStream.ts`'s) gives monotonic
  `seq` + real `elapsedMs`.
- `handlers/remotesSync.ts` push/forcePush, `handlers/status.ts` commit, `handlers/merge.ts`
  commitMerge wrap their bodies in `runMockActivity`.
- **Query seams** (mirror `?aiSlow`/`?aiFail`): `?pushSlow` (long `Network` phase), `?prePushHook`
  (emit a `runningHook` phase + a few `MOCK_PRE_PUSH_OUTPUT` lines + `hookDone{success:true}` before
  Network), `?prePushFail` (emit those lines + `hookDone{success:false}`, then throw the existing
  `hookRejected` from `hooksGate.ts` — exercises **both** the HookOutputDialog **and** a failed log
  row from one seam), `?fetchSlow`. This makes every kind/phase/terminal reachable in a plain browser.

---

## 8. Frontend store — `useGitActivity.ts` (+ siblings)

Subscribes ONCE (effect on mount) via `ipc.gitActivitySubscribe`. Same D5 discipline as `useAiRuns`:
authoritative `runsRef`, render mirror committed on a 50 ms flush; log lines buffered per-id; a
status/phase change flushes immediately; drop any event whose `seq <= last-seen` for its id.

```ts
export interface GitActivityLine { seq: number; stream: 'stdout'|'stderr'; text: string }
export interface GitHookRecord { hook: string; code: number|null; success: boolean; at: number }
export interface GitActivityRun {
  id: string;
  category: GitActivityCategory;
  phase: GitPhase;                       // current phase — drives View C label
  status: 'running'|'success'|'failed';
  code: number|null;
  startedAt: number; endedAt: number|null;   // wall-clock (event-arrival anchored; elapsedMs for duration)
  lines: GitActivityLine[];              // bounded ring
  linesDropped: number;                  // for the "N earlier lines hidden" chip
  hooks: GitHookRecord[];                // one per hookDone (View D per-hook status)
}
export interface GitActivityApi {
  runs: GitActivityRun[];                // newest-first (View D log)
  activeRun: GitActivityRun | null;      // newest status==='running' (View C)
  clear(): void;                         // clears terminal runs from the log
  tick: number;                          // for live elapsed, interval only while something runs
}
```

**Bounds / retention:**
- `GIT_ACTIVITY_LINES_MAX = 500` per run (reuse `appendCapped` from `aiRunLog.ts`); overflow bumps
  `linesDropped`.
- `GIT_ACTIVITY_RUNS_MAX = 200` runs, newest-first; evict oldest **terminal** runs on overflow;
  never evict a `running` run. Session-scoped only (no persistence across app restart).
- `AI_LOG_FLUSH_MS` (50 ms) batching reused; a 1 s `tick` interval exists only while
  `activeRun !== null`.

**Event → state:** `started` creates a run (status `running`); `phase` sets `phase`; `stdoutLine`/
`stderrLine` append a buffered line; `hookDone` pushes a `GitHookRecord`; `finished` sets
`status`/`code`/`endedAt` and flushes. Pure transforms live in `gitActivityState.ts`; the log-append
+ cap in `gitActivityLog.ts` (file-size discipline — new files, all under 500).

**Wiring:** instantiate once in the workspace container (sibling to `useAiRuns`); View C reads
`activeRun` to label the toolbar spinner (`WorkspaceToolbar.tsx:182/261`); View D renders `runs` in
the panel the ui-designer specs.

---

## 9. Failure integration (§6 of brief)

The two paths are **independent and both fire**:
1. **Command Result (authoritative UX, unchanged):** a blocking hook non-zero exit → core returns
   `AppError::HookRejected(full combined output)` → `ipc.push()` rejects → `useHookGate` catches
   `hookRejected` → `HookOutputDialog` with the verbatim output + "Push anyway (skip hooks)".
2. **Activity stream (observability, new):** the same run already emitted `phase(RunningHook)` +
   the hook's `stdoutLine/stderrLine`s + `hookDone{success:false}`, and `with_activity` emits
   `finished{success:false}` from the `Err`. So the session log shows a failed row with the hook
   output and per-hook status — **no coupling** between the two.

A **passing** blocking hook: `hookDone{success:true}` then the phase advances; the command still
resolves `Ok`. A **post-commit** failure: `hookDone{success:false}` recorded, but `finished{success:true}`
(commit landed) — matching current non-blocking semantics.

---

## 10. Backward-compat guarantees (must hold)

- `GitExec::exec`, `GitOutput`, the 64 MiB cap, `build_command` hardening: **unchanged**.
- `run_hook`/`run_hook_nonblocking` keep their current signatures (as `None` wrappers) → all
  `hooks.rs` oracle tests pass untouched.
- `AppError::HookRejected` message shape, `useHookGate`, `HookOutputDialog`, `skip_hooks`: unchanged.
- Public command signatures (`push`/`force_push`/`fetch`/`pull`/`commit`/`commit_merge`): unchanged.
- With no subscriber, `GitActivityHub::emit` is a no-op and every core still runs the buffered path.

---

## 11. Needs a ui-designer decision (data/states only — no visuals here)

- **Phase copy:** map `GitPhaseKind`×`hook` → user strings ("Running pre-push hook…", "Pushing…",
  "Finalizing…", generic "Working…"). Backend emits structured data only.
- **View C surface:** does the phase-aware label replace the toolbar spinner text only, or also add
  a cancel affordance? (Cancel is out of scope for P87 — flag if wanted.)
- **View D panel:** states to cover — running (spinner + live phase), success, failed (with hook
  output), per-hook rows (pass/fail/exit code), the `linesDropped` truncation chip, empty log,
  `clear`, and the relationship to the existing HookOutputDialog (log row is the persistent record;
  dialog is the modal). Category iconography + timestamp/duration format.
- **a11y/theming:** both themes, line-wrapping vs monospace log, copy-to-clipboard of a run's output.

---

## 12. Open questions for the orchestrator

1. **IPC shape — Option B (chosen) vs A.** Chose a single long-lived `git_activity_subscribe`
   channel + AppState hub (one subscription, future ops auto-appear, zero op-command signature
   churn). Alternative A = a per-command `Channel<GitActivityEvent>` param on each op (exact AI
   parity, but 6 signature changes + the store correlating N channels). **Recommend B.** Confirm.
2. **fetch/pull sideband streaming.** Recommend wiring git2 `sideband_progress`/`transfer_progress`
   → activity lines now (cheap, directly fixes "feels hung"). If deferred, fetch/pull still log as a
   `Network` running→finished row with no lines. Confirm include vs defer.
3. **Per-hook exit granularity.** `HookDone` gives explicit per-hook `{code,success}`. Passing
   blocking hooks are exit-0 by construction. OK, or do you also want a `killed` distinction for a
   hook the user aborts? (No cancel path exists yet — tied to Q in §11.)
4. **Log persistence.** Session-scoped only (cleared on app restart). Confirm no on-disk retention
   for v1.

---

## 13. Acceptance criteria

- **AI-gate (browser harness, `VITE_MOCK_IPC=1`):**
  - `?prePushHook` → toolbar shows a "Running pre-push hook…" phase then "Pushing…"; the session log
    gets one `push` run: `runningHook` phase, hook lines, `hookDone{success:true}`, `finished{success:true}`.
  - `?prePushFail` → **HookOutputDialog opens** (verbatim `MOCK_PRE_PUSH_OUTPUT`) **and** the log has a
    failed `push` run with `hookDone{success:false}` + `finished{success:false}`. Both, from one action.
  - `?pushSlow`/`?fetchSlow` → a `running` row with a live `Network` phase and elapsed ticking; ends
    `success`. A passing commit records pre-commit/commit-msg/post-commit `hookDone` rows.
  - `?aiFlood`-style flood → per-run line cap at 500 with a `linesDropped` chip; runs cap at 200
    (oldest terminal evicted, running preserved).
  - `tsc` + `pnpm build` clean; mock satisfies `IpcApi`.
- **Rust:** `exec_streaming` on `SpawnGitExec` yields a `GitOutput` byte-identical to `exec` for the
  same child, plus the correct per-line sink calls (oracle test w/ a multi-line hook). `run_hook`
  classification (Ok / Git / HookRejected) is unchanged with a `None` and with a recording recorder.
  With no subscriber, no events emitted and the buffered path is taken. `cargo test`/`clippy` clean.
- **USER CHECKPOINT (native `pnpm tauri dev`):** real push with a real pre-push hook shows the phase
  transition live; a real failing hook shows the dialog AND a log entry; fetch/pull no longer feel
  silently hung.
