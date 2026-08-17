//! `ai_stream` commands (P68 §F): the STREAMING conflict resolver plus its cancel
//! and reply siblings. A separate module from `ai.rs` (543 lines) on purpose — the
//! file-size discipline, and these three share a registry that nothing else uses.
//!
//! Why three commands for one feature: a Tauri command cannot be aborted from JS
//! (the `ai_resolve_conflict_stream` promise settles only when the run ends), so
//! cancelling and answering a question have to arrive as SEPARATE calls keyed by a
//! run id — and that id therefore has to reach the UI on the FIRST channel event,
//! not as a return value (D8).
//!
//! `ai_resolve_conflict` (`ai.rs`) is untouched and stays registered as the
//! non-streaming fallback (D6/§8.1).

use super::shared::*;

/// Streaming AI resolution for 1..n conflicted paths (P68 §D). A single file is
/// literally `paths.len() == 1` (A1) — one command keeps the bulk split and the
/// per-file attribution in Rust (D1).
///
/// Pushes `AiRunEvent`s over `on_event` (first event = `started`, carrying the
/// `runId` the UI needs for `ai_cancel_run` / `ai_reply_run` — D8) and resolves
/// with the batch outcome. Loads settings and REFUSES with `AiUnavailable` unless
/// `ai_enabled && ai_consented` (the authoritative backend gate, mirroring
/// `ai_resolve_conflict`).
///
/// WRITES NOTHING (D4): applying a proposal stays the separate explicit
/// `resolve_conflict_text` command, and the CLI itself gets a read-only tool
/// allowlist (D10).
///
/// Errors: `aiUnavailable` | `aiFailed` | `aiCancelled` | `git` | `invalidName` |
/// `noRepo`.
#[tauri::command]
pub async fn ai_resolve_conflict_stream(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    registry: tauri::State<'_, AiRunRegistry>,
    repo_id: String,
    paths: Vec<String>,
    on_event: tauri::ipc::Channel<AiRunEvent>,
) -> Result<AiResolveBatch, AppError> {
    // Resolve the settings-file path at the AppHandle boundary so the inner stays
    // runtime-free and unit-testable (mirrors `ai_resolve_conflict`), then delegate
    // with the Channel abstracted to a plain callback (mirrors
    // `history_index_build`).
    let file = settings::settings_file(&app)?;
    ai_resolve_conflict_stream_inner(
        state.inner(),
        registry.inner(),
        &file,
        &repo_id,
        paths,
        move |ev| {
            // A send failure means the frontend dropped the channel — ignore it;
            // the run completes and the batch still resolves (same rule as
            // `history_index_build`).
            let _ = on_event.send(ev);
        },
    )
    .await
}

/// Runtime-free core of `ai_resolve_conflict_stream` (unit-testable without a
/// Tauri app — the `test` feature is unusable on this machine). Order of
/// operations is BINDING: load settings → consent gate → path check → `repo_path`
/// → register → run → `finish`. The consent gate runs BEFORE any repo work so a
/// refusal never touches the repository (§9.6 of P13, kept).
pub(crate) async fn ai_resolve_conflict_stream_inner(
    state: &AppState,
    registry: &AiRunRegistry,
    settings_file: &std::path::Path,
    repo_id: &str,
    paths: Vec<String>,
    on_event: impl Fn(AiRunEvent) + Send + Sync + 'static,
) -> Result<AiResolveBatch, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable(
            "AI features are disabled or not yet consented to".to_string(),
        ));
    }
    if paths.is_empty() {
        return Err(AppError::AiFailed("no conflicted paths given".to_string()));
    }
    let workdir = repo_path(state, repo_id)?;
    let cfg = stream_opts(&s);

    // Concurrency guard (OQ1), enforced HERE rather than only in the frontend: this
    // is the trust boundary. Each run is a `claude` process tree with no wall-clock
    // deadline (D3/D7) and no default spend cap, so a frontend regression, a second
    // window, a retried IPC call or a double-fired dock action would otherwise fan
    // out unbounded runs against a metered subscription. REJECT, never queue — a
    // queued run with no visible state is worse than an error. The message is
    // deliberately distinct (and stable) so the UI can say "too many AI runs"
    // instead of showing a generic `aiFailed`.
    let active = registry.active();
    if active >= ai::AI_MAX_CONCURRENT_RUNS {
        return Err(AppError::AiFailed(format!(
            "too many AI runs in progress ({active} of {} allowed) — cancel one and try again",
            ai::AI_MAX_CONCURRENT_RUNS
        )));
    }

    // The registry entry must go away on EVERY exit path — including a panic
    // inside the blocking task — or `ai_cancel_run` would keep accepting a dead id
    // and the app-exit hook would try to kill a stale pid. A drop guard is the only
    // shape that survives an early `?`.
    let (run_id, ctl) = registry.register();
    let _guard = FinishGuard { registry: registry.clone(), run_id };

    tauri::async_runtime::spawn_blocking(move || {
        // `ctl` is MOVED here and borrowed inside: a bulk run drives several
        // sequential children from this one control (same cancel flag, same reply
        // channel, one run id — §6.3).
        ai_resolve_stream::resolve_conflicts_streaming(&workdir, &paths, cfg, &ctl, &on_event)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Cancels a streaming AI run (P68 §B/D7). IDEMPOTENT: an unknown or
/// already-finished id resolves `Ok` — a cancel racing a completion is normal, and
/// the UI must not show an error for clicking Cancel a moment too late.
#[tauri::command]
pub async fn ai_cancel_run(
    registry: tauri::State<'_, AiRunRegistry>,
    run_id: String,
) -> Result<(), AppError> {
    ai_cancel_run_inner(registry.inner(), &run_id)
}

/// Runtime-free core of `ai_cancel_run`. Not `spawn_blocking`: flipping an
/// `AtomicBool` behind a mutex cannot block meaningfully, and the whole point is
/// that the flag lands FAST (the session polls it every loop iteration).
pub(crate) fn ai_cancel_run_inner(
    registry: &AiRunRegistry,
    run_id: &str,
) -> Result<(), AppError> {
    let _known = registry.cancel(run_id);
    Ok(())
}

/// Answers a mid-run question (P68 §B/D9). Rejects with `AiFailed` when the run is
/// unknown or is NOT awaiting input, so a stray reply is never silently swallowed
/// into a channel nobody reads.
#[tauri::command]
pub async fn ai_reply_run(
    registry: tauri::State<'_, AiRunRegistry>,
    run_id: String,
    text: String,
) -> Result<(), AppError> {
    ai_reply_run_inner(registry.inner(), &run_id, text)
}

/// Runtime-free core of `ai_reply_run`. `AiRunRegistry::reply` enforces the
/// awaiting-input rule itself (`is_awaiting` is the read-only query for a UI that
/// wants to ask first), and the text reaches the child through STDIN, never argv
/// (D13).
pub(crate) fn ai_reply_run_inner(
    registry: &AiRunRegistry,
    run_id: &str,
    text: String,
) -> Result<(), AppError> {
    registry.reply(run_id, text)
}

/// Maps the persisted settings onto the run's limits (P68 §8.3). The two LOCKED
/// user decisions live here: `ai_hard_cap_secs == 0` means NO absolute cap (the
/// user cancels instead), and `ai_max_budget_usd == 0.0` means the
/// `--max-budget-usd` flag is not passed AT ALL rather than passed as `0.0000`
/// (which the CLI would read as "spend nothing").
fn stream_opts(s: &settings::Settings) -> StreamResolveOpts {
    use std::time::Duration;
    StreamResolveOpts {
        opts: RunOpts::default(),
        limits: ai::RunLimits {
            idle_timeout: Duration::from_secs(u64::from(s.ai_idle_timeout_secs)),
            hard_cap: (s.ai_hard_cap_secs > 0)
                .then(|| Duration::from_secs(u64::from(s.ai_hard_cap_secs))),
            max_turns: s.ai_max_turns,
            tools: match s.ai_conflict_tools {
                AiConflictTools::ReadOnly => ai::ToolPolicy::ReadOnly,
                AiConflictTools::None => ai::ToolPolicy::None,
            },
            max_budget_usd: (s.ai_max_budget_usd > 0.0).then_some(s.ai_max_budget_usd),
            include_partial_messages: s.ai_include_partial_messages,
            // Always interactive: stdin stays open so the model CAN ask (D9), and
            // that is the whole point of the milestone.
            interactive: true,
        },
        bulk_max_bytes: s.ai_bulk_max_bytes as usize,
        stream_log: s.ai_stream_log,
    }
}

/// Removes the run from the registry when the command returns, however it returns
/// (D7: "`finish` on every exit path — use a guard so a panic cannot leak an
/// entry"). Holds a CLONE of the registry handle rather than a borrow, so it is
/// free of the `tauri::State` lifetime.
struct FinishGuard {
    registry: AiRunRegistry,
    run_id: String,
}

impl Drop for FinishGuard {
    fn drop(&mut self) {
        self.registry.finish(&self.run_id);
    }
}
