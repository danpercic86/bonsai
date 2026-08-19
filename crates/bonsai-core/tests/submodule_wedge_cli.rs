//! P73 — end-to-end coverage for the WEDGE → RECONNECT path of
//! `update_submodule` (contract `docs/contracts/P73-submodule-reconnect.md`
//! §8.1, acceptance criteria §9.2-§9.7).
//!
//! The wedged state: the submodule worktree is an empty directory with no `.git`
//! gitlink while `.git/modules/<key>` is a complete healthy gitdir. libgit2 dies
//! there with `attempt to reinitialize`; P73 reattaches the existing gitdir and
//! checks out with `recreate_missing(true)`, reusing the cached objects (⇒ zero
//! network).
//!
//! Lives in its own file (not appended to `submodule_cli_2.rs`) to stay under the
//! ~500-line limit; the small fixture helpers are duplicated the same way
//! `submodule_reconnect_cli.rs` duplicates them (integration tests are separate
//! binaries and cannot share a `tests/*.rs` module).
//!
//! All submodules use a LOCAL `file://` URL (no creds, no network). Scratch on
//! D:. Skips (passes with a note) w/o `git`.

mod common;

use std::path::{Path, PathBuf};

use bonsai_core::error::AppError;
use bonsai_core::git::search::SpawnGitRunner;
use bonsai_core::git::submodule::{
    add_submodule, deinit_submodule, list_submodules, update_submodule, SubmoduleInfo,
    SubmoduleStatus,
};
use common::{commit_fixed, file_url, git, init_repo, scratch_dir};

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

const SUB_PATH: &str = "vendor/sub";
const SENTINEL: &str = "bonsai-sentinel";
const SENTINEL_BODY: &str = "keep me";

// ------------------------------------------------------------------ fixtures

/// Upstream sub-repo with two commits (v1, v2 on `lib.txt`). Returns
/// (dir, url, v1_oid, v2_oid); HEAD is at v2.
fn build_sub() -> (tempfile::TempDir, String, String, String) {
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("lib.txt"), "sub v1\n").unwrap();
    git(p, &["add", "-A"]);
    commit_fixed(p, "sub v1");
    let v1 = git(p, &["rev-parse", "HEAD"]);
    std::fs::write(p.join("lib.txt"), "sub v2\n").unwrap();
    git(p, &["add", "-A"]);
    commit_fixed(p, "sub v2");
    let v2 = git(p, &["rev-parse", "HEAD"]);
    let url = file_url(p);
    (dir, url, v1, v2)
}

/// Superproject with one commit and a submodule at `SUB_PATH` added + committed
/// (added through Bonsai, so `name == path`).
fn build_super_with_sub(url: &str) -> tempfile::TempDir {
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("top.txt"), "super\n").unwrap();
    git(p, &["add", "-A"]);
    commit_fixed(p, "super: initial");
    add_submodule(p, url, SUB_PATH).expect("add_submodule");
    git(p, &["add", "-A"]);
    commit_fixed(p, "super: add submodule");
    dir
}

/// Superproject whose `.gitmodules` section NAME differs from the checked-out
/// PATH (`git submodule add --name`). git keys the module gitdir on the NAME, so
/// the cached dir is `.git/modules/<name>` while libgit2's clone would key on
/// `<path>` — the divergence contract OPEN-1 resolves in favour of `name`.
fn build_super_with_renamed_sub(url: &str, name: &str, path: &str) -> tempfile::TempDir {
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("top.txt"), "super\n").unwrap();
    git(p, &["add", "-A"]);
    commit_fixed(p, "super: initial");
    git(p, &["-c", "protocol.file.allow=always", "submodule", "add", "--name", name, url, path]);
    git(p, &["add", "-A"]);
    commit_fixed(p, "super: add renamed submodule");
    dir
}

/// Leading status char from `git submodule status <path>` (` `/`+`/`-`). Uses
/// RAW output — the trimming `git()` helper would strip the space (UpToDate).
fn cli_status_char(super_dir: &Path, path: &str) -> char {
    let raw = common::git_raw(super_dir, &["submodule", "status", "--", path], &[]);
    raw.first().map(|&b| b as char).unwrap_or('?')
}

fn only(super_dir: &Path) -> SubmoduleInfo {
    let mut v = list_submodules(super_dir).expect("list");
    assert_eq!(v.len(), 1, "exactly one submodule: {v:?}");
    v.pop().unwrap()
}

/// WEDGE the submodule (contract §8.1/§9 fixture recipe): keep
/// `.git/modules/<key>` intact, plant a sentinel file inside it, then delete the
/// worktree gitlink and every entry in the submodule workdir while KEEPING the
/// (now empty) workdir directory itself.
///
/// Asserts the wedge really took (`git submodule status` prints `-`, Bonsai says
/// `Uninitialized`) — that precondition is what makes each test meaningful.
/// Returns (module_gitdir, sentinel_path) so a later success can prove the gitdir
/// was REUSED rather than re-cloned.
fn wedge(super_dir: &Path, key: &str, path: &str) -> (PathBuf, PathBuf) {
    let module_dir = super_dir.join(".git").join("modules").join(key);
    assert!(
        module_dir.join("HEAD").exists(),
        "precondition: a complete cached gitdir at {}",
        module_dir.display()
    );
    let sentinel = module_dir.join(SENTINEL);
    std::fs::write(&sentinel, SENTINEL_BODY).expect("plant sentinel");

    let sub_wd = super_dir.join(path);
    std::fs::remove_file(sub_wd.join(".git")).expect("remove gitlink");
    for entry in std::fs::read_dir(&sub_wd).expect("read submodule workdir") {
        let entry = entry.expect("dir entry");
        let md = std::fs::symlink_metadata(entry.path()).expect("stat entry");
        if md.is_dir() {
            std::fs::remove_dir_all(entry.path()).expect("rm dir");
        } else {
            std::fs::remove_file(entry.path()).expect("rm file");
        }
    }
    assert!(sub_wd.is_dir(), "the empty workdir dir itself stays");
    assert_eq!(
        std::fs::read_dir(&sub_wd).unwrap().count(),
        0,
        "the wedged workdir is empty"
    );

    // The wedge is real, from both readers.
    assert_eq!(cli_status_char(super_dir, path), '-', "git reports the wedge as '-'");
    let row = list_submodules(super_dir)
        .expect("list")
        .into_iter()
        .find(|s| s.path == path)
        .expect("the submodule row survives the wedge");
    assert_eq!(row.status, SubmoduleStatus::Uninitialized, "Bonsai reports Uninitialized");

    (module_dir, sentinel)
}

/// Read a file, normalizing CRLF → LF. The module gitdir is created by libgit2's
/// clone, which inherits the DEVELOPER's global `core.autocrlf`, so a checked-out
/// text file may legitimately arrive with CRLF endings. Line endings are not what
/// these tests are about.
fn read_lf(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        .replace("\r\n", "\n")
}

fn assert_sentinel_intact(sentinel: &Path) {
    assert_eq!(
        std::fs::read_to_string(sentinel).expect("sentinel survives"),
        SENTINEL_BODY,
        "the cached gitdir was REUSED, not re-cloned (sentinel content intact)"
    );
}

// -------------------------------------------------- criteria 2, 3, 5

/// Criteria 2 (reconnect works), 3 (reuse not re-clone) and 5 (RELATIVE gitlink,
/// end-to-end) in one pass over the wedged fixture.
#[test]
fn update_reconnects_orphaned_module_gitdir() {
    require_git!();
    let (_sub, url, _v1, v2) = build_sub();
    let dir = build_super_with_sub(&url);
    let p = dir.path();
    let (_module_dir, sentinel) = wedge(p, SUB_PATH, SUB_PATH);

    update_submodule(p, SUB_PATH).expect("update must reconnect the orphaned gitdir");

    // 3 — REUSE, not re-clone.
    assert_sentinel_intact(&sentinel);

    // 2 — the worktree is repopulated at the pinned commit and reads clean.
    let sub_wd = p.join(SUB_PATH);
    assert_eq!(
        read_lf(&sub_wd.join("lib.txt")),
        "sub v2\n",
        "the pinned content is back on disk"
    );
    let row = only(p);
    assert_eq!(row.status, SubmoduleStatus::UpToDate, "row after reconnect: {row:?}");
    assert_eq!(row.wt_oid.as_deref(), Some(v2.as_str()), "workdir at the pinned v2");
    assert_eq!(row.index_oid.as_deref(), Some(v2.as_str()), "index still pins v2");
    assert_eq!(cli_status_char(p, SUB_PATH), ' ', "git status char is a space");
    assert_eq!(git(&sub_wd, &["rev-parse", "HEAD"]), v2, "submodule HEAD is v2");

    // 5 — `<sub>/.git` is a FILE holding a RELATIVE, forward-slash gitlink.
    let gitlink = sub_wd.join(".git");
    let md = std::fs::symlink_metadata(&gitlink).expect("stat gitlink");
    assert!(md.is_file(), "the gitlink must be a regular file, not a directory");
    let body = std::fs::read_to_string(&gitlink).expect("read gitlink");
    assert!(body.starts_with("gitdir: .."), "gitlink must be relative, got: {body:?}");
    assert!(body.ends_with('\n'), "gitlink must end with a newline, got: {body:?}");
    assert_eq!(body.matches('\n').count(), 1, "exactly one line: {body:?}");
    assert!(!body.contains('\\'), "forward slashes only, got: {body:?}");
    assert!(!body.contains("//?/"), "no Windows verbatim prefix, got: {body:?}");
    let target = body.trim_start_matches("gitdir: ").trim_end();
    assert!(!target.contains(':'), "no absolute drive letter, got: {body:?}");
    // ...and git itself resolves it back inside `<super>/.git/modules`.
    let resolved = git(&sub_wd, &["rev-parse", "--absolute-git-dir"]).replace('\\', "/");
    assert!(
        resolved.ends_with(&format!("/.git/modules/{SUB_PATH}")),
        "git must resolve the gitlink into the superproject's modules dir, got: {resolved}"
    );
    // No temp residue from the atomic write.
    assert!(
        !sub_wd.join(".git.bonsai-tmp").exists(),
        "the atomic-write temp file must not be left behind"
    );
}

// -------------------------------------------------------- criterion 4

/// Criterion 4 — the salvage path performs ZERO network I/O: with the upstream
/// `file://` source DELETED (the url is dead, a clone or fetch could not
/// possibly succeed), the reconnect still repopulates from the cached objects.
#[test]
fn reconnect_works_offline() {
    require_git!();
    let (sub, url, _v1, v2) = build_sub();
    let dir = build_super_with_sub(&url);
    let p = dir.path();
    let (_module_dir, sentinel) = wedge(p, SUB_PATH, SUB_PATH);

    // Kill the upstream. `TempDir::drop` ignores an already-removed path.
    std::fs::remove_dir_all(sub.path()).expect("delete the upstream source");
    assert!(!sub.path().exists(), "the file:// url now points at nothing");

    update_submodule(p, SUB_PATH).expect("reconnect must not need the network");

    assert_sentinel_intact(&sentinel);
    assert_eq!(
        read_lf(&p.join(SUB_PATH).join("lib.txt")),
        "sub v2\n",
        "repopulated offline from the cached objects"
    );
    let row = only(p);
    assert_eq!(row.status, SubmoduleStatus::UpToDate, "row after offline reconnect");
    assert_eq!(row.wt_oid.as_deref(), Some(v2.as_str()), "workdir at the pinned v2");
}

// -------------------------------------------------------- criterion 6

/// Criterion 6 — refusal A: a workdir holding files but no `.git` link is NEVER
/// clobbered. The refusal is fail-closed: the stray file is byte-identical, no
/// gitlink was written, and the row is still `Uninitialized`.
///
/// NOTE the message asserted here is the §7 AMENDED user-facing copy ("The
/// folder already has files in it..."), not §9.6's original `no .git link`
/// wording — the amendment is the implemented contract.
#[test]
fn reconnect_refuses_non_empty_workdir() {
    require_git!();
    let (_sub, url, _v1, _v2) = build_sub();
    let dir = build_super_with_sub(&url);
    let p = dir.path();
    let (_module_dir, sentinel) = wedge(p, SUB_PATH, SUB_PATH);

    let sub_wd = p.join(SUB_PATH);
    let stray = sub_wd.join("keepme.txt");
    std::fs::write(&stray, "USER DATA - DO NOT DELETE\n").expect("write stray file");
    let before = std::fs::read(&stray).expect("read stray file");

    match update_submodule(p, SUB_PATH) {
        Err(AppError::Git(m)) => {
            assert!(m.contains("The folder already has files in it."), "got: {m}");
            assert!(m.contains(SUB_PATH), "the message must name the path, got: {m}");
            assert!(
                !m.to_lowercase().contains("reinitialize"),
                "no raw libgit2 prose, got: {m}"
            );
        }
        other => panic!("a non-empty workdir must be refused, got {other:?}"),
    }

    assert_eq!(
        std::fs::read(&stray).expect("stray survives"),
        before,
        "the user's file must be byte-identical after the refusal"
    );
    assert!(!sub_wd.join(".git").exists(), "no gitlink may be written on a refusal");
    assert!(
        !sub_wd.join(".git.bonsai-tmp").exists(),
        "no atomic-write residue on a refusal"
    );
    assert_sentinel_intact(&sentinel);
    assert_eq!(only(p).status, SubmoduleStatus::Uninitialized, "row unchanged after refusal");
    assert_eq!(cli_status_char(p, SUB_PATH), '-', "git still reports the wedge");
}

// -------------------------------------------------------- criterion 7

/// Criterion 7 — refusal B: the cached gitdir's `origin` points somewhere else,
/// so ownership cannot be proven. Both urls are quoted and nothing is written.
#[test]
fn reconnect_refuses_url_mismatch() {
    require_git!();
    let (_sub, url, _v1, _v2) = build_sub();
    let (_other, other_url, _o1, _o2) = build_sub();
    let dir = build_super_with_sub(&url);
    let p = dir.path();
    let (module_dir, sentinel) = wedge(p, SUB_PATH, SUB_PATH);

    git(&module_dir, &["remote", "set-url", "origin", &other_url]);

    match update_submodule(p, SUB_PATH) {
        Err(AppError::Git(m)) => {
            assert!(
                m.contains(&other_url) && m.contains(&url),
                "the refusal must quote BOTH urls, got: {m}"
            );
            assert!(
                m.contains("Bonsai has cached data for a different remote URL"),
                "got: {m}"
            );
        }
        other => panic!("a url mismatch must be refused, got {other:?}"),
    }

    assert!(!p.join(SUB_PATH).join(".git").exists(), "no gitlink on a refusal");
    assert_eq!(
        std::fs::read_dir(p.join(SUB_PATH)).unwrap().count(),
        0,
        "the workdir is still empty"
    );
    assert_sentinel_intact(&sentinel);
    assert_eq!(only(p).status, SubmoduleStatus::Uninitialized, "row unchanged after refusal");
}

// ------------------------------------------ cosmetic-url tolerance (§8.1)

/// `urls_equivalent` normalization: a trailing `/` plus a `.git` suffix on the
/// CONFIGURED url must NOT be read as a mismatch — the reconnect still happens.
#[test]
fn reconnect_tolerates_url_cosmetic_difference() {
    require_git!();
    let (_sub, url, _v1, v2) = build_sub();
    let dir = build_super_with_sub(&url);
    let p = dir.path();
    let (_module_dir, sentinel) = wedge(p, SUB_PATH, SUB_PATH);

    // Cosmetically different, semantically identical: `<url>.git/`.
    let cosmetic = format!("{url}.git/");
    let key = format!("submodule.{SUB_PATH}.url");
    git(p, &["config", "--local", &key, &cosmetic]);
    git(p, &["config", "-f", ".gitmodules", &key, &cosmetic]);
    assert_ne!(cosmetic, url, "precondition: the strings really differ");

    update_submodule(p, SUB_PATH).expect("a cosmetic url difference must not block the reconnect");

    assert_sentinel_intact(&sentinel);
    let row = only(p);
    assert_eq!(row.status, SubmoduleStatus::UpToDate, "row after reconnect");
    assert_eq!(row.wt_oid.as_deref(), Some(v2.as_str()), "workdir at the pinned v2");
}

// ------------------------------------------ deinit → update (§8.1, real path)

/// The real-world route that produced the reported bug: `deinit` keeps
/// `.git/modules/<key>` and empties the worktree, so the very next `update` hits
/// the wedged state. It must repair itself (and reuse the cached gitdir).
#[test]
fn reconnect_after_deinit_reinitializes() {
    require_git!();
    let (_sub, url, _v1, v2) = build_sub();
    let dir = build_super_with_sub(&url);
    let p = dir.path();

    let sentinel = p.join(".git").join("modules").join(SUB_PATH).join(SENTINEL);
    std::fs::write(&sentinel, SENTINEL_BODY).expect("plant sentinel");

    deinit_submodule(p, &SpawnGitRunner, SUB_PATH).expect("deinit");
    assert_eq!(cli_status_char(p, SUB_PATH), '-', "deinit leaves the '-' (wedged) row");
    assert_eq!(only(p).status, SubmoduleStatus::Uninitialized, "row after deinit");
    assert!(sentinel.exists(), "deinit keeps the cached gitdir (that is the wedge)");

    update_submodule(p, SUB_PATH).expect("update after deinit must reinitialize");

    assert_sentinel_intact(&sentinel);
    assert_eq!(
        read_lf(&p.join(SUB_PATH).join("lib.txt")),
        "sub v2\n"
    );
    let row = only(p);
    assert_eq!(row.status, SubmoduleStatus::UpToDate, "row after the repair");
    assert_eq!(row.wt_oid.as_deref(), Some(v2.as_str()), "workdir at the pinned v2");
}

// ---------------------------------------- renamed submodule (name != path)

/// OPEN-1: for a renamed submodule (`.gitmodules` section name != checked-out
/// path) the cached gitdir git itself writes is `<modules>/<name>`. With a DECOY
/// `<modules>/<path>` also present (a valid repo pointing at a DIFFERENT url),
/// `name` must win — picking `path` would surface a url-mismatch refusal
/// instead of repairing.
#[test]
fn reconnect_renamed_submodule_prefers_name_keyed_gitdir() {
    require_git!();
    let (_sub, url, _v1, v2) = build_sub();
    let (_other, other_url, _o1, _o2) = build_sub();
    let name = "renamed-sub";
    let dir = build_super_with_renamed_sub(&url, name, SUB_PATH);
    let p = dir.path();

    // Precondition: git keyed the cache on the NAME, not the path.
    assert!(
        p.join(".git").join("modules").join(name).join("HEAD").exists(),
        "precondition: the cached gitdir is name-keyed"
    );
    let (_module_dir, sentinel) = wedge(p, name, SUB_PATH);

    // Decoy path-keyed gitdir for a DIFFERENT remote.
    let decoy = p.join(".git").join("modules").join(SUB_PATH);
    std::fs::create_dir_all(&decoy).expect("mkdir decoy");
    git(&decoy, &["init", "-b", "main"]);
    git(&decoy, &["remote", "add", "origin", &other_url]);

    update_submodule(p, name).expect("the name-keyed gitdir must be the one reconnected");

    assert_sentinel_intact(&sentinel);
    assert_eq!(
        read_lf(&p.join(SUB_PATH).join("lib.txt")),
        "sub v2\n"
    );
    let row = only(p);
    assert_eq!(row.name, name, "the row is the renamed section");
    assert_eq!(row.path, SUB_PATH);
    assert_eq!(row.status, SubmoduleStatus::UpToDate, "row after reconnect");
    assert_eq!(row.wt_oid.as_deref(), Some(v2.as_str()), "workdir at the pinned v2");
    // The gitlink resolves into the NAME-keyed dir.
    let resolved = git(&p.join(SUB_PATH), &["rev-parse", "--absolute-git-dir"]).replace('\\', "/");
    assert!(
        resolved.ends_with(&format!("/.git/modules/{name}")),
        "must resolve into the name-keyed gitdir, got: {resolved}"
    );
}

/// The leftover-data refusal (§7 row 16) names the **path**-keyed folder, because
/// libgit2 keys the init it fails on `sm->path`. With `name != path` a name-keyed
/// message would send the user to a folder that does not exist.
#[test]
fn leftover_data_refusal_names_path_keyed_folder_for_renamed_submodule() {
    require_git!();
    let (_sub, url, _v1, _v2) = build_sub();
    let name = "renamed-sub";
    let super_dir = build_super_with_renamed_sub(&url, name, SUB_PATH);

    // Fresh clone: registered in `.gitmodules`, never cloned → no cached gitdir.
    let parent = scratch_dir();
    git(parent.path(), &["clone", &file_url(super_dir.path()), "work"]);
    let work = parent.path().join("work");
    assert!(
        !work.join(".git").join("modules").exists(),
        "precondition: nothing cached in the fresh clone"
    );

    // Forge a "looks like a repo, cannot be opened" dir under the PATH key —
    // exactly what an aborted libgit2 clone leaves behind.
    let garbage = work.join(".git").join("modules").join(SUB_PATH);
    std::fs::create_dir_all(garbage.join("objects")).expect("mkdir objects");
    std::fs::create_dir_all(garbage.join("refs")).expect("mkdir refs");
    std::fs::write(garbage.join("HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
    std::fs::write(garbage.join("config"), "[core\nnot valid ini\n").expect("write config");

    let err = match update_submodule(&work, name) {
        Err(e) => e.to_string(),
        Ok(()) => panic!("an unopenable module gitdir must not silently succeed"),
    };
    assert!(
        !err.to_lowercase().contains("reinitialize"),
        "the raw libgit2 message must never reach the UI, got: {err}"
    );
    assert!(
        err.contains(&format!("\".git/modules/{SUB_PATH}\"")),
        "the refusal must name the PATH-keyed folder, got: {err}"
    );
    assert!(
        !err.contains(&format!("\".git/modules/{name}\"")),
        "it must NOT name the (nonexistent) name-keyed folder, got: {err}"
    );
    assert!(garbage.exists(), "Bonsai must not delete the folder itself");
}
