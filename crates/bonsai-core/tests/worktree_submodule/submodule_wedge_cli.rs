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

use std::path::{Path, PathBuf};

use crate::common;
use crate::common::{commit_fixed, file_url, git, init_repo};
use bonsai_core::git::search::SpawnGitRunner;
use bonsai_core::git::submodule::{
    add_submodule, deinit_submodule, list_submodules, update_submodule, SubmoduleInfo,
    SubmoduleStatus,
};

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
    assert_eq!(
        cli_status_char(super_dir, path),
        '-',
        "git reports the wedge as '-'"
    );
    let row = list_submodules(super_dir)
        .expect("list")
        .into_iter()
        .find(|s| s.path == path)
        .expect("the submodule row survives the wedge");
    assert_eq!(
        row.status,
        SubmoduleStatus::Uninitialized,
        "Bonsai reports Uninitialized"
    );

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
    assert_eq!(
        row.status,
        SubmoduleStatus::UpToDate,
        "row after reconnect: {row:?}"
    );
    assert_eq!(
        row.wt_oid.as_deref(),
        Some(v2.as_str()),
        "workdir at the pinned v2"
    );
    assert_eq!(
        row.index_oid.as_deref(),
        Some(v2.as_str()),
        "index still pins v2"
    );
    assert_eq!(
        cli_status_char(p, SUB_PATH),
        ' ',
        "git status char is a space"
    );
    assert_eq!(
        git(&sub_wd, &["rev-parse", "HEAD"]),
        v2,
        "submodule HEAD is v2"
    );

    // 5 — `<sub>/.git` is a FILE holding a RELATIVE, forward-slash gitlink.
    let gitlink = sub_wd.join(".git");
    let md = std::fs::symlink_metadata(&gitlink).expect("stat gitlink");
    assert!(
        md.is_file(),
        "the gitlink must be a regular file, not a directory"
    );
    let body = std::fs::read_to_string(&gitlink).expect("read gitlink");
    assert!(
        body.starts_with("gitdir: .."),
        "gitlink must be relative, got: {body:?}"
    );
    assert!(
        body.ends_with('\n'),
        "gitlink must end with a newline, got: {body:?}"
    );
    assert_eq!(body.matches('\n').count(), 1, "exactly one line: {body:?}");
    assert!(!body.contains('\\'), "forward slashes only, got: {body:?}");
    assert!(
        !body.contains("//?/"),
        "no Windows verbatim prefix, got: {body:?}"
    );
    let target = body.trim_start_matches("gitdir: ").trim_end();
    assert!(
        !target.contains(':'),
        "no absolute drive letter, got: {body:?}"
    );
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
    assert!(
        !sub.path().exists(),
        "the file:// url now points at nothing"
    );

    update_submodule(p, SUB_PATH).expect("reconnect must not need the network");

    assert_sentinel_intact(&sentinel);
    assert_eq!(
        read_lf(&p.join(SUB_PATH).join("lib.txt")),
        "sub v2\n",
        "repopulated offline from the cached objects"
    );
    let row = only(p);
    assert_eq!(
        row.status,
        SubmoduleStatus::UpToDate,
        "row after offline reconnect"
    );
    assert_eq!(
        row.wt_oid.as_deref(),
        Some(v2.as_str()),
        "workdir at the pinned v2"
    );
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

    deinit_submodule(p, &SpawnGitRunner, SUB_PATH, true).expect("deinit");
    assert_eq!(
        cli_status_char(p, SUB_PATH),
        '-',
        "deinit leaves the '-' (wedged) row"
    );
    assert_eq!(
        only(p).status,
        SubmoduleStatus::Uninitialized,
        "row after deinit"
    );
    assert!(
        sentinel.exists(),
        "deinit keeps the cached gitdir (that is the wedge)"
    );

    update_submodule(p, SUB_PATH).expect("update after deinit must reinitialize");

    assert_sentinel_intact(&sentinel);
    assert_eq!(read_lf(&p.join(SUB_PATH).join("lib.txt")), "sub v2\n");
    let row = only(p);
    assert_eq!(
        row.status,
        SubmoduleStatus::UpToDate,
        "row after the repair"
    );
    assert_eq!(
        row.wt_oid.as_deref(),
        Some(v2.as_str()),
        "workdir at the pinned v2"
    );
}

// The refusal paths and the renamed-submodule (name != path) cases live in their
// own files (~500-line limit), declared here as child modules so they share
// `require_git!`, the constants and the wedge fixtures above.
#[path = "submodule_wedge_refusals_cli.rs"]
mod submodule_wedge_refusals_cli;
#[path = "submodule_wedge_renamed_cli.rs"]
mod submodule_wedge_renamed_cli;
