//! Submodule support (P19 contract §2). Read + common ops: list with
//! classified status, init, update (fetch + checkout the pinned commit), sync.
//!
//! Pure git2 logic, no Tauri types (runtime-free core → unit/CLI-testable
//! without the Tauri "test" feature, same rule as stash/remote). `update`
//! fetches, so it reuses the M6 credential chain (`remote::acquire_cred`)
//! verbatim — never prompts, never stores passwords.
//!
//! `status.rs` stays AS-IS (`.exclude_submodules(true)`, §7): submodule state
//! surfaces ONLY here, never mixed into the working-dir file-status lists.

use std::cell::RefCell;
use std::path::Path;

use crate::error::AppError;
use crate::git::remote::{acquire_cred, map_remote_err, CredAttempts};
use crate::git::search::GitRunner;
use crate::git::stage::{open_workdir_repo, validate_rel_path};

/// Consolidated state of one submodule. Wire: a camelCase string enum (no
/// data). Derived from git2's `Repository::submodule_status` bitflags (§2.4),
/// evaluated in PRIORITY order (first match wins):
/// Uninitialized > OutOfSync > ModifiedWorkdir > UpToDate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SubmoduleStatus {
    /// Registered in .gitmodules/index but not checked out (`WD_UNINITIALIZED`).
    /// Maps to `git submodule status` leading `-`.
    Uninitialized,
    /// Checked out and matching the recorded commit, clean workdir.
    /// Maps to `git submodule status` leading ` ` (space).
    UpToDate,
    /// The checked-out commit differs from the commit recorded in the
    /// superproject (index or HEAD). Maps to `git submodule status` leading `+`.
    OutOfSync,
    /// Checked-out commit matches, but the submodule's OWN worktree/index is
    /// dirty (staged, unstaged, or untracked changes inside it).
    ModifiedWorkdir,
}

/// One submodule row. Wire: camelCase. All oids are full 40-hex or null.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleInfo {
    /// Submodule NAME (stable key for init/update/sync). `Submodule::name()`.
    pub name: String,
    /// Repo-relative path, forward slashes on the wire. `Submodule::path()`.
    pub path: String,
    /// ABSOLUTE workdir path for open-in-tab (§OPEN-1): superproject workdir
    /// joined with `path`. Fed verbatim to the existing open-repo/tab flow.
    pub abs_path: String,
    /// Configured URL from .gitmodules/.git config. `Submodule::url()`.
    pub url: Option<String>,
    /// Commit recorded in the superproject HEAD tree. `Submodule::head_id()`.
    pub head_oid: Option<String>,
    /// Commit recorded in the superproject index. `Submodule::index_id()`.
    pub index_oid: Option<String>,
    /// Commit currently checked out in the submodule worktree.
    /// `Submodule::workdir_id()`. None when uninitialized.
    pub wt_oid: Option<String>,
    pub status: SubmoduleStatus,
}

/// Maps git2's `SubmoduleStatus` bitflags to our single enum in PRIORITY order
/// (first match wins). A submodule that is simultaneously out-of-sync AND dirty
/// classifies as `OutOfSync` (higher priority) — so the UI badge is
/// deterministic (§2.4).
fn classify_status(f: git2::SubmoduleStatus) -> SubmoduleStatus {
    use git2::SubmoduleStatus as S;
    // 1. Not checked out at all.
    if f.contains(S::WD_UNINITIALIZED) {
        return SubmoduleStatus::Uninitialized;
    }
    // 2. Recorded-commit mismatch: superproject index/HEAD pointer changed, OR
    //    the checked-out commit differs from the index pointer.
    if f.intersects(S::INDEX_ADDED | S::INDEX_DELETED | S::INDEX_MODIFIED | S::WD_MODIFIED) {
        return SubmoduleStatus::OutOfSync;
    }
    // 3. Submodule's own index/worktree is dirty (but the pinned commit matches).
    if f.intersects(S::WD_INDEX_MODIFIED | S::WD_WD_MODIFIED | S::WD_UNTRACKED) {
        return SubmoduleStatus::ModifiedWorkdir;
    }
    // 4. Checked out, clean, matching.
    SubmoduleStatus::UpToDate
}

/// Build one [`SubmoduleInfo`] row from an opened submodule handle and its
/// already-resolved (UTF-8) `name`. `sm_workdir` is the superproject workdir
/// (used for the absolute `abs_path`). Shared by [`list_submodules`] and
/// [`add_submodule`] so the wire shape is produced in exactly one place.
fn submodule_info(
    repo: &git2::Repository,
    sm: &git2::Submodule,
    name: String,
    sm_workdir: &Path,
) -> Result<SubmoduleInfo, AppError> {
    let flags = repo.submodule_status(&name, git2::SubmoduleIgnore::None)?; // §OPEN-2
    Ok(SubmoduleInfo {
        name,
        path: sm.path().to_string_lossy().replace('\\', "/"), // forward slashes on the wire
        abs_path: sm_workdir.join(sm.path()).to_string_lossy().into_owned(),
        url: sm.url().ok().flatten().map(str::to_string),
        head_oid: sm.head_id().map(|o| o.to_string()),
        index_oid: sm.index_id().map(|o| o.to_string()),
        wt_oid: sm.workdir_id().map(|o| o.to_string()),
        status: classify_status(flags),
    })
}

/// Blocking. List every submodule with its classified status. No submodules →
/// Ok(vec![]). Order: `Repository::submodules()` order (stable).
pub fn list_submodules(workdir: &Path) -> Result<Vec<SubmoduleInfo>, AppError> {
    let repo = open_workdir_repo(workdir)?;
    let sm_workdir = repo
        .workdir()
        .ok_or_else(|| AppError::Git("repository has no working directory".to_string()))?
        .to_path_buf();

    let mut out = Vec::new();
    for sm in repo.submodules()? {
        // Skip non-UTF-8 names (cannot key status; log + skip, like fetch_all).
        let name = match sm.name().ok() {
            Some(n) => n.to_string(),
            None => {
                eprintln!("bonsai: skipping submodule with non-UTF-8 name");
                continue;
            }
        };
        out.push(submodule_info(&repo, &sm, name, &sm_workdir)?);
    }
    Ok(out)
}

/// Shared open + name-validate + `find_submodule` prologue. NotFound →
/// `AppError::Git` (§OPEN-3); blank name → `AppError::InvalidName`.
fn open_submodule<'r>(
    repo: &'r git2::Repository,
    name: &str,
) -> Result<git2::Submodule<'r>, AppError> {
    if name.trim().is_empty() {
        return Err(AppError::InvalidName("submodule name is empty".to_string()));
    }
    repo.find_submodule(name).map_err(|e| match e.code() {
        git2::ErrorCode::NotFound => AppError::Git(format!("submodule '{name}' not found")),
        _ => e.into(),
    })
}

/// Blocking. Register submodule `name` into .git/config (copies .gitmodules
/// url/config). git2: `Submodule::init(false)` (no overwrite). No worktree change.
pub fn init_submodule(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    let mut sm = open_submodule(&repo, name)?;
    sm.init(false)?;
    Ok(())
}

/// Blocking. Init-if-needed + fetch (shared M6 credential chain) + checkout the
/// pinned commit. git2: `Submodule::update(true, Some(&mut opts))` with the
/// fetch callbacks wired to the credential chain (§2.5). MODIFIES the submodule
/// worktree (safe checkout default; never force).
pub fn update_submodule(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    let mut sm = open_submodule(&repo, name)?;

    let attempts = RefCell::new(CredAttempts::default());
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|url, username_from_url, allowed| {
        acquire_cred(repo.workdir(), &attempts, url, username_from_url, allowed)
    });

    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(callbacks);
    let mut opts = git2::SubmoduleUpdateOptions::new();
    opts.fetch(fo);

    // init=true → init-then-update in one call (§OPEN-4). SAFE checkout default.
    sm.update(true, Some(&mut opts))
        .map_err(|e| map_remote_err(e, name))?;
    Ok(())
}

/// Blocking. Copy the URL from .gitmodules into .git/config and the submodule's
/// remote. git2: `Submodule::sync()`. No worktree change, no fetch/credentials.
pub fn sync_submodule(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    let mut sm = open_submodule(&repo, name)?;
    sm.sync()?;
    Ok(())
}

/// Blocking. Adds a submodule at repo-relative `path` from `url` (D4): git2
/// `Repository::submodule(url, Path::new(path), /*use_gitlink*/ true)` →
/// clone the subrepo with the shared M6 credential callback (`acquire_cred`,
/// exactly as [`update_submodule`]) → `Submodule::init(false)` (write
/// `submodule.<name>.*` into .git/config so it is "initialized", matching
/// `git submodule add`) → `Submodule::add_finalize()` (stage .gitmodules + the
/// new gitlink). `path` is validated with `validate_rel_path`; a blank url/path
/// → InvalidName. Errors: `invalidName` | `git` (incl. network/auth via
/// `map_remote_err`) | `noRepo`.
pub fn add_submodule(workdir: &Path, url: &str, path: &str) -> Result<SubmoduleInfo, AppError> {
    if url.trim().is_empty() {
        return Err(AppError::InvalidName("submodule url is empty".to_string()));
    }
    if path.trim().is_empty() {
        return Err(AppError::InvalidName("submodule path is empty".to_string()));
    }
    validate_rel_path(path)?; // reject absolute / `..` / backslash traversal

    let repo = open_workdir_repo(workdir)?;
    let sm_workdir = repo
        .workdir()
        .ok_or_else(|| AppError::Git("repository has no working directory".to_string()))?
        .to_path_buf();

    // add-setup: write .gitmodules + register the submodule (use_gitlink = true).
    let mut sm = repo
        .submodule(url, Path::new(path), true)
        .map_err(|e| map_remote_err(e, path))?;

    // The name is path-derived at add-setup; grab it up front so a failure
    // below can be rolled back by name (F-A7-10).
    let name = match sm.name().ok() {
        Some(n) => n.to_string(),
        None => {
            rollback_partial_add(&repo, path, path);
            return Err(AppError::Git("submodule has a non-UTF-8 name".to_string()));
        }
    };

    // Clone + register + finalize. On ANY failure, best-effort rollback of the
    // add-setup residue (.gitmodules entries, .git/config registration, the
    // partial checkout dir, the cached .git/modules dir) so a retry does not
    // hit "submodule already exists" (F-A7-10); the ORIGINAL error is returned.
    let finalize = (|| -> Result<(), AppError> {
        // Clone the subrepo with the shared M6 credential chain (uniform with
        // `update_submodule`; never prompts, never stores passwords).
        {
            let attempts = RefCell::new(CredAttempts::default());
            let mut callbacks = git2::RemoteCallbacks::new();
            callbacks.credentials(|url, username_from_url, allowed| {
                acquire_cred(repo.workdir(), &attempts, url, username_from_url, allowed)
            });
            let mut fo = git2::FetchOptions::new();
            fo.remote_callbacks(callbacks);
            let mut opts = git2::SubmoduleUpdateOptions::new();
            opts.fetch(fo);
            sm.clone(Some(&mut opts))
                .map_err(|e| map_remote_err(e, url))?;
        }

        // Register in .git/config (like `git submodule add`) then stage
        // .gitmodules + the gitlink for the next commit.
        sm.init(false)?;
        sm.add_finalize()?;
        Ok(())
    })();
    if let Err(e) = finalize {
        rollback_partial_add(&repo, &name, path);
        return Err(e);
    }

    submodule_info(&repo, &sm, name, &sm_workdir)
}

/// Best-effort rollback of a failed [`add_submodule`] (F-A7-10): removes the
/// `.gitmodules` entries written by add-setup, the `submodule.<name>.*`
/// registration `init` may have written to .git/config, the (partial) checkout
/// dir at `path` (already contained in the workdir via `validate_rel_path`),
/// and the cached `.git/modules/<name>` dir the clone may have created. Every
/// step is independent and its own failure ignored — the caller reports the
/// ORIGINAL error. An empty `.gitmodules` file may remain (harmless to git),
/// and anything `add_finalize` staged before failing stays staged; both
/// self-heal on the next successful add or a manual `git checkout .gitmodules`.
fn rollback_partial_add(repo: &git2::Repository, name: &str, path: &str) {
    if let Some(wd) = repo.workdir() {
        // .gitmodules entries (written by add-setup).
        if let Ok(mut cfg) = git2::Config::open(&wd.join(".gitmodules")) {
            let _ = cfg.remove(&format!("submodule.{name}.path"));
            let _ = cfg.remove(&format!("submodule.{name}.url"));
        }
        // The partial checkout dir at `path` (validated repo-relative).
        let _ = std::fs::remove_dir_all(wd.join(path));
    }
    // .git/config registration (written by `init`, when reached).
    if let Ok(mut cfg) = repo.config() {
        let _ = cfg.remove(&format!("submodule.{name}.url"));
        let _ = cfg.remove(&format!("submodule.{name}.update"));
        let _ = cfg.remove(&format!("submodule.{name}.active"));
    }
    // Cached git dir under .git/modules (created by the clone), with the same
    // traversal guard as `remove_submodule` (F-A7-2).
    if validate_modules_name(name).is_ok() {
        remove_cached_git_dir(repo, name);
    }
}

/// Resolve submodule `name` → its repo-relative path with forward slashes,
/// validating the name via [`open_submodule`] (blank → InvalidName, unknown →
/// Git). Shared by the deinit/remove shell-out ops so the pathspec fed to `git`
/// is always the tracked submodule path.
fn submodule_path(repo: &git2::Repository, name: &str) -> Result<String, AppError> {
    let sm = open_submodule(repo, name)?;
    Ok(sm.path().to_string_lossy().replace('\\', "/"))
}

/// Pure argv for `git submodule deinit -f -- <path>`. `path` is ALWAYS the
/// final token, after `--` — never interpolated into a flag — so a space/`;`
/// in it stays one token and can never become a second command.
fn deinit_args(path: &str) -> Vec<String> {
    vec![
        "submodule".to_string(),
        "deinit".to_string(),
        "-f".to_string(),
        "--".to_string(),
        path.to_string(),
    ]
}

/// Pure argv for `git rm -f -- <path>` (drops the gitlink + .gitmodules entry
/// and stages the removal). `path` is the final token, after `--`.
fn rm_args(path: &str) -> Vec<String> {
    vec![
        "rm".to_string(),
        "-f".to_string(),
        "--".to_string(),
        path.to_string(),
    ]
}

/// Blocking. `git submodule deinit -f -- <path>` via `runner` (no libgit2
/// primitive; D4). Clears `submodule.<name>` from .git/config and empties the
/// submodule worktree; KEEPS the .gitmodules entry (re-init-able). `name` is
/// resolved to its path via `find_submodule`. Errors: `invalidName` | `git`
/// (stderr tail) | `noRepo`.
pub fn deinit_submodule(
    workdir: &Path,
    runner: &dyn GitRunner,
    name: &str,
) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    let path = submodule_path(&repo, name)?;
    runner.run(&deinit_args(&path), workdir)?;
    Ok(())
}

/// Blocking. Full removal (git's documented sequence): `git submodule deinit -f
/// -- <path>` → `git rm -f -- <path>` → best-effort `remove_dir_all(.git/
/// modules/<name>)`. DESTRUCTIVE (deletes the worktree, edits the index, drops
/// the .gitmodules entry + gitlink). Errors: `invalidName` | `git` | `noRepo`.
pub fn remove_submodule(
    workdir: &Path,
    runner: &dyn GitRunner,
    name: &str,
) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    // F-A7-2: `name` comes from .gitmodules (attacker-controllable) and is
    // joined under .git/modules — refuse traversal BEFORE any destructive step.
    validate_modules_name(name)?;
    let path = submodule_path(&repo, name)?;
    runner.run(&deinit_args(&path), workdir)?;
    runner.run(&rm_args(&path), workdir)?;
    // Best-effort: drop the cached git dir (`git rm` leaves it; may be absent).
    remove_cached_git_dir(&repo, name);
    Ok(())
}

/// F-A7-2: reject submodule names that could escape `.git/modules` when joined
/// (CVE-2018-11235 vector). Names MAY contain `/` (nested defaults such as
/// `vendor/libcore`), but never `..`/`.`/empty components (across `/` AND `\`)
/// or absolute paths.
fn validate_modules_name(name: &str) -> Result<(), AppError> {
    let bytes = name.as_bytes();
    let absolute = name.starts_with('/')
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':');
    let bad_component = name
        .split(['/', '\\'])
        .any(|c| c.is_empty() || c == "." || c == "..");
    if name.trim().is_empty() || absolute || bad_component {
        return Err(AppError::Git(format!(
            "refusing to touch .git/modules for submodule '{name}': unsafe name"
        )));
    }
    Ok(())
}

/// Best-effort removal of the cached `.git/modules/<name>` dir with
/// belt-and-braces containment (F-A7-2): even after name validation, delete
/// only when the canonicalized dir is strictly inside the canonicalized
/// modules root. An absent dir is a silent no-op (canonicalize fails).
fn remove_cached_git_dir(repo: &git2::Repository, name: &str) {
    let root = repo.path().join("modules");
    let dir = root.join(name);
    if let (Ok(canon_root), Ok(canon_dir)) = (root.canonicalize(), dir.canonicalize()) {
        if canon_dir.starts_with(&canon_root) && canon_dir != canon_root {
            let _ = std::fs::remove_dir_all(&canon_dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §8.2 #6: wire shapes must match the TS mirrors — the status enum
    /// serializes to camelCase strings and `SubmoduleInfo` to camelCase keys.
    #[test]
    fn status_enum_serializes_camel_case() {
        let cases = [
            (SubmoduleStatus::Uninitialized, "uninitialized"),
            (SubmoduleStatus::UpToDate, "upToDate"),
            (SubmoduleStatus::OutOfSync, "outOfSync"),
            (SubmoduleStatus::ModifiedWorkdir, "modifiedWorkdir"),
        ];
        for (variant, wire) in cases {
            let v = serde_json::to_value(variant).expect("json");
            assert_eq!(v, serde_json::json!(wire));
        }
    }

    #[test]
    fn info_serializes_camel_case_keys() {
        let info = SubmoduleInfo {
            name: "vendor/libcore".to_string(),
            path: "vendor/libcore".to_string(),
            abs_path: "/repo/vendor/libcore".to_string(),
            url: Some("https://example.com/libcore.git".to_string()),
            head_oid: Some("a".repeat(40)),
            index_oid: Some("b".repeat(40)),
            wt_oid: None,
            status: SubmoduleStatus::Uninitialized,
        };
        let v = serde_json::to_value(&info).expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "name": "vendor/libcore",
                "path": "vendor/libcore",
                "absPath": "/repo/vendor/libcore",
                "url": "https://example.com/libcore.git",
                "headOid": "a".repeat(40),
                "indexOid": "b".repeat(40),
                "wtOid": null,
                "status": "uninitialized"
            })
        );
    }

    /// §8.2 #7: `classify_status` truth table, including the priority tie-breaks.
    #[test]
    fn classify_status_priority_table() {
        use git2::SubmoduleStatus as S;

        // Uninitialized wins even when combined with anything else.
        assert_eq!(
            classify_status(S::WD_UNINITIALIZED),
            SubmoduleStatus::Uninitialized
        );
        assert_eq!(
            classify_status(S::WD_UNINITIALIZED | S::WD_MODIFIED | S::WD_WD_MODIFIED),
            SubmoduleStatus::Uninitialized
        );

        // Superproject-pointer / checked-out-commit mismatch → OutOfSync.
        assert_eq!(classify_status(S::WD_MODIFIED), SubmoduleStatus::OutOfSync);
        assert_eq!(classify_status(S::INDEX_ADDED), SubmoduleStatus::OutOfSync);
        assert_eq!(classify_status(S::INDEX_DELETED), SubmoduleStatus::OutOfSync);
        assert_eq!(classify_status(S::INDEX_MODIFIED), SubmoduleStatus::OutOfSync);

        // OutOfSync outranks internal dirtiness (documented tie-break).
        assert_eq!(
            classify_status(S::WD_MODIFIED | S::WD_WD_MODIFIED),
            SubmoduleStatus::OutOfSync
        );

        // Internal dirtiness only → ModifiedWorkdir.
        assert_eq!(
            classify_status(S::WD_INDEX_MODIFIED),
            SubmoduleStatus::ModifiedWorkdir
        );
        assert_eq!(
            classify_status(S::WD_WD_MODIFIED),
            SubmoduleStatus::ModifiedWorkdir
        );
        assert_eq!(
            classify_status(S::WD_UNTRACKED),
            SubmoduleStatus::ModifiedWorkdir
        );

        // Clean, checked-out, matching.
        assert_eq!(classify_status(S::IN_HEAD), SubmoduleStatus::UpToDate);
        assert_eq!(
            classify_status(S::IN_HEAD | S::IN_INDEX | S::IN_CONFIG | S::IN_WD),
            SubmoduleStatus::UpToDate
        );
    }

    /// Blank / whitespace names are rejected before touching the repo.
    #[test]
    fn blank_name_is_invalid() {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init");
        for name in ["", "   "] {
            match open_submodule(&repo, name).map(|_| ()) {
                Err(AppError::InvalidName(_)) => {}
                other => panic!("expected InvalidName for {name:?}, got {other:?}"),
            }
        }
    }

    /// An unknown submodule name maps NotFound → `AppError::Git` (§OPEN-3).
    #[test]
    fn unknown_name_maps_to_git_error() {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init");
        match open_submodule(&repo, "does/not/exist").map(|_| ()) {
            Err(AppError::Git(m)) => assert!(m.contains("does/not/exist"), "{m}"),
            other => panic!("expected Git error, got {other:?}"),
        }
    }

    /// P60d: the deinit/remove argv builders are byte-exact — `path` is the
    /// FINAL token, immediately after `--`.
    #[test]
    fn deinit_args_exact() {
        assert_eq!(
            deinit_args("vendor/sub"),
            ["submodule", "deinit", "-f", "--", "vendor/sub"]
                .map(String::from)
                .to_vec()
        );
    }

    #[test]
    fn rm_args_exact() {
        assert_eq!(
            rm_args("vendor/sub"),
            ["rm", "-f", "--", "vendor/sub"].map(String::from).to_vec()
        );
    }

    /// P60d injection-safety: a space/`;`-bearing path stays exactly ONE argv
    /// token, always the element AFTER `--` — never split, never a 2nd command.
    #[test]
    fn args_keep_metachar_path_as_single_token_after_dashdash() {
        let evil = "a b; rm -rf /";
        let d = deinit_args(evil);
        assert_eq!(d.last().unwrap(), evil);
        assert_eq!(d[d.len() - 2], "--");
        let r = rm_args(evil);
        assert_eq!(r.last().unwrap(), evil);
        assert_eq!(r[r.len() - 2], "--");
    }

    /// F-A7-2: the .git/modules name guard — traversal/absolute/dot components
    /// reject; ordinary (incl. nested) names pass.
    #[test]
    fn modules_name_validation_rejects_traversal() {
        for bad in [
            "..",
            "../x",
            "a/../b",
            "..\\evil",
            "a\\..\\b",
            "/abs",
            "C:/abs",
            "C:\\abs",
            "",
            "   ",
            "a//b",
            "./x",
            "a/.",
        ] {
            match validate_modules_name(bad) {
                Err(AppError::Git(_)) => {}
                other => panic!("name {bad:?} must be rejected, got {other:?}"),
            }
        }
        for good in ["sub", "vendor/libcore", "a.b", "with space", "..dots", "x.."] {
            validate_modules_name(good)
                .unwrap_or_else(|e| panic!("name {good:?} must pass: {e:?}"));
        }
    }

    /// F-A7-2: `remove_submodule` refuses a hostile name BEFORE running any
    /// git command (the runner panics if invoked).
    #[test]
    fn remove_submodule_rejects_hostile_name_before_running_git() {
        struct PanicRunner;
        impl GitRunner for PanicRunner {
            fn run(&self, _args: &[String], _cwd: &Path) -> Result<String, AppError> {
                panic!("runner must not be invoked for a hostile submodule name");
            }
        }
        let dir = crate::testutil::scratch_dir();
        git2::Repository::init(dir.path()).expect("init");
        match remove_submodule(dir.path(), &PanicRunner, "../../escape") {
            Err(AppError::Git(m)) => assert!(m.contains("unsafe name"), "{m}"),
            other => panic!("hostile name must be Git error, got {other:?}"),
        }
    }

    /// F-A7-10: a failed clone rolls back the add-setup residue (.gitmodules
    /// entry, config, dirs) so a retry with a good url succeeds instead of
    /// hitting "already exists".
    #[test]
    fn add_submodule_rolls_back_on_clone_failure() {
        let sup_dir = crate::testutil::scratch_dir();
        let d = sup_dir.path();
        let repo = git2::Repository::init(d).expect("init superproject");
        seed_commit(&repo);

        // A url that fails fast via the local transport (no network).
        let bad_url = d
            .join("definitely-missing-source")
            .to_string_lossy()
            .replace('\\', "/");
        let err = add_submodule(d, &bad_url, "vendor/sub");
        assert!(err.is_err(), "clone from a missing source must fail");

        // Residue is gone: no .gitmodules entry, no registered submodule.
        assert!(
            repo.find_submodule("vendor/sub").is_err(),
            ".gitmodules entry must be rolled back"
        );
        assert!(
            !d.join("vendor").join("sub").exists(),
            "partial checkout dir must be rolled back"
        );

        // Retry with a valid LOCAL source now succeeds (no Exists collision).
        let src_dir = crate::testutil::scratch_dir();
        let src = git2::Repository::init(src_dir.path()).expect("init source");
        seed_commit(&src);
        let src_url = src_dir.path().to_string_lossy().replace('\\', "/");
        let info = add_submodule(d, &src_url, "vendor/sub").expect("retry succeeds");
        assert_eq!(info.path, "vendor/sub");
    }

    /// Minimal deterministic commit so a repo has a HEAD (used by the rollback
    /// test's superproject + source repos).
    fn seed_commit(repo: &git2::Repository) {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        drop(cfg);
        let wd = repo.workdir().expect("workdir");
        std::fs::write(wd.join("seed.txt"), "seed\n").expect("write");
        let mut index = repo.index().expect("index");
        index.add_path(Path::new("seed.txt")).expect("add");
        index.write().expect("write index");
        let tree_oid = index.write_tree().expect("tree");
        let tree = repo.find_tree(tree_oid).expect("find tree");
        let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
        repo.commit(Some("HEAD"), &sig, &sig, "seed\n", &tree, &[])
            .expect("commit");
    }

    /// P60d: `add_submodule` rejects blank url/path and traversing paths BEFORE
    /// opening the repo (so no git/network is touched on obviously bad input).
    #[test]
    fn add_submodule_rejects_bad_url_and_path() {
        let dir = crate::testutil::scratch_dir();
        match add_submodule(dir.path(), "   ", "vendor/x") {
            Err(AppError::InvalidName(_)) => {}
            other => panic!("blank url ⇒ InvalidName, got {other:?}"),
        }
        match add_submodule(dir.path(), "https://example.com/x.git", "  ") {
            Err(AppError::InvalidName(_)) => {}
            other => panic!("blank path ⇒ InvalidName, got {other:?}"),
        }
        match add_submodule(dir.path(), "https://example.com/x.git", "../escape") {
            Err(AppError::Other(_)) => {}
            other => panic!("traversing path ⇒ Other (validate_rel_path), got {other:?}"),
        }
    }
}
