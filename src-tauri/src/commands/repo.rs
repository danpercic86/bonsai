//! `repo` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Payload of the `"repo-changed"` event. `reason` is `"fs"` in M1; future
/// reasons (e.g. `"op"` after a commit) reuse this event. `repo_id` identifies
/// which open repo's watcher fired so the frontend can route it to the right
/// tab (P3e contract §4.1).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoChangedPayload {
    pub repo_id: String,
    pub reason: String,
}

/// Result of `open_repo`: the resolved `repoId` plus the repo info (P3e
/// contract §4.2).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRepoResult {
    /// Canonical workdir path (P3e contract §2). Meaningful (a map entry
    /// exists) only when `info` is a usable repo; still returned for
    /// non-usable opens so the frontend can key its error UI, but no
    /// entry/watcher is created.
    pub repo_id: String,
    pub info: RepoInfo,
}

/// Opens the folder at `path` as a repository and reports its state.
///
/// Bare repositories are reported (`bare: true`) but NOT stored in state and
/// get no watcher — Bonsai v1 is a working-copy client (M1 contract §3.3).
/// The frontend treats `bare: true` like `isRepo: false`.
///
/// For usable (non-bare) repos this inserts a `RepoEntry` keyed by the
/// canonical workdir path (`repoId`, P3e contract §2) and (re)arms that
/// entry's file watcher: re-invoking on the same path is idempotent and
/// self-heals a dead watcher (this is what the refresh button relies on).
/// Opening an already-open path (same-directory match via canonicalization,
/// see `same_repo_path`) FOCUSES the existing entry — its id is returned, no
/// duplicate is created.
///
/// A non-usable open (non-repo or bare) inserts NO entry and touches no other
/// entry — there is no single "current repo" anymore, so other tabs are
/// unaffected.
#[tauri::command]
pub async fn open_repo(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<OpenRepoResult, AppError> {
    let emit_app = app.clone();
    let result = open_repo_inner(
        state.inner(),
        path,
        move |repo_id: String| {
            let emit_app = emit_app.clone();
            Box::new(move || {
                let _ = emit_app.emit(
                    "repo-changed",
                    RepoChangedPayload {
                        repo_id: repo_id.clone(),
                        reason: "fs".to_string(),
                    },
                );
            })
        },
    )
    .await?;
    let info = &result.info;

    // Recents hook (P1 contract §3.2): record every successful usable open.
    // Uses `info.path` (canonical workdir root), not the raw argument, so
    // "repo root" vs "subfolder" opens dedupe. Save failure is NON-FATAL —
    // the open itself succeeded.
    if info.is_repo && !info.bare {
        match settings::settings_file(&app) {
            Ok(file) => {
                let repo_path = info.path.clone();
                let saved = tauri::async_runtime::spawn_blocking(move || {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
                        .unwrap_or(0);
                    // Serialized load→mutate→save (audit §2.3).
                    settings::update(&file, |s| settings::record_recent(s, &repo_path, now))
                        .map(|_| ())
                })
                .await;
                match saved {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        eprintln!("bonsai: failed to save recent repos (non-fatal): {e}");
                    }
                    Err(e) => {
                        eprintln!("bonsai: recent-repos task join error (non-fatal): {e}");
                    }
                }
            }
            Err(e) => {
                eprintln!("bonsai: cannot resolve settings file (non-fatal): {e}");
            }
        }

        // Warm-on-open (P35 §16): pre-fill the in-process HTTPS credential
        // cache for this repo's remotes so the first fetch/pull/push does not
        // pay the cold `git credential fill` (e.g. GCM) spawn cost. This is a
        // fire-and-forget, best-effort step: it runs `list_remotes` (blocking
        // git2, hence off the async path), warms only http(s) remotes, is NEVER
        // awaited, and its failure is silently ignored — the open must stay fast
        // and never block on (or fail because of) credential resolution.
        let warm_workdir = info.path.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let workdir = std::path::Path::new(&warm_workdir);
            let Ok(remotes) = bonsai_core::git::remote::list_remotes(workdir) else {
                return;
            };
            for remote in remotes {
                let Some(url) = remote.url else { continue };
                let lower = url.to_ascii_lowercase();
                if lower.starts_with("https://") || lower.starts_with("http://") {
                    bonsai_core::git::cred_cache::warm(Some(workdir), &url);
                }
            }
        });

        // P52: (re)write the commit-graph so libgit2's revwalk / merge-base skip
        // re-parsing commit objects. Fire-and-forget, best-effort, off the UI
        // path (same shape/rationale as warm-on-open above). No error path:
        // `write_commit_graph_best_effort` never Errs — git absent / a non-zero
        // exit is a clean skip, so the open never blocks on or fails because of
        // it. The file lands under `.git/objects` (watcher-filtered, D5) ⇒ no
        // spurious repo-changed.
        let cg_workdir = info.path.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let _ = bonsai_core::git::maintenance::write_commit_graph_best_effort(
                std::path::Path::new(&cg_workdir),
            );
        });
    }
    Ok(result)
}

/// Recent successfully-opened repos, most recent first, max 10. Never rejects
/// for a missing/corrupt settings file (`load_from` defaults); only
/// settings-path resolution can error (P1 contract §3.2).
#[tauri::command]
pub async fn get_recent_repos(app: tauri::AppHandle) -> Result<Vec<RecentRepo>, AppError> {
    let file = settings::settings_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || settings::load_from(&file).recent_repos)
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))
}

/// Removes one recents entry (same-directory match via `same_repo_path`) and
/// returns the updated list (P1 contract §3.2).
#[tauri::command]
pub async fn remove_recent_repo(
    app: tauri::AppHandle,
    path: String,
) -> Result<Vec<RecentRepo>, AppError> {
    let file = settings::settings_file(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        // Serialized load→mutate→save (audit §2.3).
        let s = settings::update(&file, |s| {
            s.recent_repos.retain(|r| !same_repo_path(&r.path, &path));
        })?;
        Ok(s.recent_repos)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Records the frontend's focused-tab repoId (or `None` when no repo is
/// focused). Lock-and-clone discipline like `repo_path`; poisoned lock →
/// recovered. This seeds a new embedded-MCP session's initial repo (P16 §5) — it
/// does NOT change any already-connected AI session's selection.
#[tauri::command]
pub async fn set_active_repo(
    state: tauri::State<'_, AppState>,
    repo_id: Option<String>,
) -> Result<(), AppError> {
    // Poison recovery (audit §3.8): the guarded Option<String> is structurally
    // valid at every point, so a past panic under the lock must not brick this.
    *state
        .active_repo
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = repo_id;
    Ok(())
}

/// Runtime-free core of `open_repo` (unit-testable without a Tauri app).
/// `make_on_change` is given the resolved `repo_id` and returns the watcher
/// callback for that repo; the command wires it to an app-wide
/// `"repo-changed"` emit carrying that id. Tests pass `|_id| Box::new(|| {})`
/// (no Tauri runtime).
pub(crate) async fn open_repo_inner<F>(
    state: &AppState,
    path: String,
    make_on_change: F,
) -> Result<OpenRepoResult, AppError>
where
    F: FnOnce(String) -> Box<dyn Fn() + Send + 'static>,
{
    let path_buf = std::path::PathBuf::from(&path);
    let info = tauri::async_runtime::spawn_blocking(move || read_repo_info(&path_buf))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))??;

    // repoId == canonical workdir path string (P3e contract §2).
    let mut repo_id = info.path.clone();

    if info.is_repo && !info.bare {
        let workdir = std::path::PathBuf::from(&info.path);

        // Dedupe scan (P3e contract §2): if the same directory is already open
        // (canonical-path match, `same_repo_path`), reuse its exact key so we
        // FOCUS it instead of inserting a duplicate. Only compute the callback
        // once we know the final id.
        //
        // Audit §3.4: `same_repo_path` canonicalizes (blocking fs I/O), so the
        // scan must not run under the map lock on the async executor. Snapshot
        // the keys under the lock, release it, then compare in
        // `spawn_blocking`. TOCTOU: a tab closed between snapshot and use just
        // means we insert a fresh entry under that (still-canonical) key below
        // — same behaviour as opening it anew, nothing to guard.
        let keys: Vec<String> = {
            // Poison recovery (audit §3.8): the guarded map is structurally
            // valid at every point (plain insert/remove), so recover instead
            // of bricking every later command.
            let repos = state
                .repos
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            repos.keys().cloned().collect()
        };
        let candidate = repo_id.clone();
        let existing = tauri::async_runtime::spawn_blocking(move || {
            keys.into_iter().find(|k| same_repo_path(k, &candidate))
        })
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?;
        if let Some(existing) = existing {
            repo_id = existing;
        }

        let on_change = make_on_change(repo_id.clone());

        // Watch failure is non-fatal (M1 contract §4): manual refresh + focus
        // rescan keep the app correct even without filesystem events. Build the
        // handle OUTSIDE the map lock (the initial watch registration is
        // synchronous), then insert/replace the entry under the lock. Replacing
        // an existing entry drops its old watcher here (self-heal); to keep
        // that drop-join off the map lock we take the old entry out first.
        let watcher = match spawn_watcher(&workdir, on_change) {
            Ok(handle) => Some(handle),
            Err(e) => {
                eprintln!("bonsai: file watcher failed to start (falling back to manual refresh): {e}");
                None
            }
        };

        let previous = {
            // Poison recovery — see the dedupe-scan comment above (audit §3.8).
            let mut repos = state
                .repos
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            repos.insert(
                repo_id.clone(),
                RepoEntry {
                    path: workdir,
                    watcher,
                },
            )
        };
        // Drop the replaced entry (its watcher's debounce thread joins) off the
        // map lock.
        drop(previous);
    }
    // Non-usable open (non-repo or bare): insert nothing, touch no other entry.

    Ok(OpenRepoResult { repo_id, info })
}

/// Closes the tab identified by `repo_id`, stopping just that repo's watcher.
/// Idempotent: closing an unknown/already-closed id is `Ok(())` (P3e contract
/// §4.3).
#[tauri::command]
pub async fn close_repo(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    close_repo_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `close_repo` (unit-testable without a Tauri app).
pub(crate) async fn close_repo_inner(state: &AppState, repo_id: &str) -> Result<(), AppError> {
    // Take the entry out UNDER the lock, then drop it OUTSIDE the lock so the
    // WatcherHandle's debounce-thread join (≤ ~300 ms) doesn't hold the map
    // lock.
    let entry = {
        // Poison recovery on AppState.repos (audit §3.8) — the map is
        // structurally valid at every point.
        let mut repos = state
            .repos
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        repos.remove(repo_id)
    };
    drop(entry); // watcher stops, debounce thread joins here
    Ok(())
}

/// Clones `url` into `dest`, streaming `CloneProgress` over `on_progress`.
/// Returns the absolute workdir path of the clone (frontend then calls
/// `open_repo`/openTab). NOT repo-scoped — it CREATES a repo (P21 §OPEN-2).
/// Rejects io | authFailed | networkError | git.
#[tauri::command]
pub async fn clone_repo(
    url: String,
    dest: String,
    on_progress: tauri::ipc::Channel<CloneProgress>,
) -> Result<String, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        clone_repo_core(&url, std::path::Path::new(&dest), move |p| {
            // Channel is Clone+Send+Sync+'static; a send failure means the
            // frontend dropped the channel — ignore it, the clone completes.
            let _ = on_progress.send(p);
        })
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Initializes (or opens, if already a repo) a repository at `path`. Returns
/// the absolute workdir path. NOT repo-scoped (P21 §OPEN-2/§OPEN-3).
/// Rejects io | git.
#[tauri::command]
pub async fn init_repo(path: String) -> Result<String, AppError> {
    tauri::async_runtime::spawn_blocking(move || init_repo_core(std::path::Path::new(&path)))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Same-directory test for the open-repo dedupe scan and recents removal
/// (T2.1 BUG-1). Compares `fs::canonicalize` results when BOTH sides resolve —
/// this folds filesystem case (including non-ASCII, e.g. `Übung`/`übung` on
/// NTFS), separators (`/` vs `\`), trailing slashes, and 8.3 short names,
/// none of which the previous `eq_ignore_ascii_case` handled. When either
/// side cannot be canonicalized (e.g. a recents entry whose directory was
/// deleted) it falls back to the previous ASCII-case-insensitive string
/// compare, so existing behavior is preserved for unresolvable paths.
pub(crate) fn same_repo_path(a: &str, b: &str) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a.eq_ignore_ascii_case(b),
    }
}
