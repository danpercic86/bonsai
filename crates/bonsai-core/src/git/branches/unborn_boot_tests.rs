//! P99: the boot-time read path on a GENUINELY UNBORN repo (`git init`, zero
//! commits).
//!
//! Why this file exists: the frontend computes
//! `head = repo?.head ?? branches?.head ?? null` (`RepoWorkspace.tsx`) and
//! gates the "No commits yet" empty state on `head?.unborn`. In a production
//! bundle the `repo` state is null at boot, so the empty state depends
//! ENTIRELY on the branches snapshot carrying `unborn == true`. `list.rs`
//! DOCUMENTS that intent ("Unborn repo: ... `head.unborn == true` — `Ok`, not
//! `Err`") but nothing asserted it. If `list_refs` errored instead, `branches`
//! would stay null and the empty state would silently not render.
//!
//! Every slice `RepoWorkspace`'s mount effect calls is exercised, and each
//! `Ok`-ness is asserted at test top level via `.expect(...)` on the call
//! itself — never inside a helper that could skip the assertions.
//!
//! Fixture: git2 only, in a scratch `TempDir` under the crate scratch root
//! (`crate::testutil::scratch_dir`, i.e. `D:\Data\Temp\bonsai-scratch` on
//! Windows). `initial_head("main")` is pinned by the test so the expected
//! unborn branch name does not depend on the machine's `init.defaultBranch`.

use std::path::Path;

use crate::git::branches::list_refs;
use crate::git::repo::{read_head_info, read_repo_info};
use crate::git::status::read_status;
use crate::graph::{graph_seed, stream_graph_core, GraphChunk};
use crate::testutil::scratch_dir;

/// `git init` with NO commit and HEAD symbolic to `refs/heads/main`.
fn unborn_repo(dir: &Path) -> git2::Repository {
    let mut opts = git2::RepositoryInitOptions::new();
    opts.initial_head("main");
    let repo = git2::Repository::init_opts(dir, &opts).expect("init unborn repo");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Test User").expect("name");
    cfg.set_str("user.email", "test@example.com")
        .expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    drop(cfg);
    // Guard the fixture itself: this must really be unborn.
    assert_eq!(
        repo.head().err().map(|e| e.code()),
        Some(git2::ErrorCode::UnbornBranch),
        "fixture is not unborn"
    );
    repo
}

/// (1) The load-bearing case: `list_refs` must return `Ok` with `unborn == true`
/// on an unborn repo. Would catch any `Err` from the snapshot (which nulls the
/// frontend's `branches` state and kills the empty state) AND a snapshot that
/// succeeded but reported `unborn == false`.
#[test]
fn list_refs_on_unborn_repo_is_ok_and_unborn() {
    let dir = scratch_dir();
    let _repo = unborn_repo(dir.path());

    let snap = list_refs(dir.path()).expect("list_refs must return Ok on an unborn repo");

    assert!(snap.head.unborn, "head.unborn must be true on unborn repo");
    assert!(!snap.head.detached, "unborn HEAD is not detached");
    assert!(
        snap.head.oid.is_empty(),
        "unborn HEAD has no oid, got {:?}",
        snap.head.oid
    );
    assert_eq!(
        snap.head.branch_name.as_deref(),
        Some("main"),
        "unborn branch name comes from HEAD's symbolic target"
    );
    assert!(snap.local.is_empty(), "no local branches exist yet");
    assert!(
        snap.remote.is_empty(),
        "no remote-tracking branches exist yet"
    );
    assert!(snap.tags.is_empty(), "no tags exist yet");
}

/// (2) The shared builder both `read_repo_info` and `list_refs` use.
#[test]
fn read_head_info_on_unborn_repo_is_ok_and_unborn() {
    let dir = scratch_dir();
    let repo = unborn_repo(dir.path());

    let head = read_head_info(&repo).expect("read_head_info must return Ok on an unborn repo");

    assert!(head.unborn);
    assert!(!head.detached);
    assert!(head.oid.is_empty());
    assert_eq!(head.branch_name.as_deref(), Some("main"));
}

/// (4) `openRepo`-equivalent. The Rust-side counterpart of the frontend's
/// `isUsableRepo` (`is_repo && !bare`) — an unborn repo must be usable.
#[test]
fn read_repo_info_on_unborn_repo_is_usable_and_unborn() {
    let dir = scratch_dir();
    let _repo = unborn_repo(dir.path());

    let info = read_repo_info(dir.path()).expect("read_repo_info must return Ok on an unborn repo");

    assert!(info.is_repo, "unborn repo is a repo");
    assert!(!info.bare, "unborn repo is not bare");
    let head = info
        .head
        .expect("RepoInfo.head must be Some on an unborn repo");
    assert!(head.unborn);
    assert!(!head.detached);
    assert!(head.oid.is_empty());
    assert_eq!(head.branch_name.as_deref(), Some("main"));
}

/// (3a) Status must be `Ok` AND usable for the first commit — an untracked file
/// has to show up, otherwise the empty-state panel cannot stage anything.
#[test]
fn read_status_on_unborn_repo_is_ok_and_usable() {
    let dir = scratch_dir();
    let _repo = unborn_repo(dir.path());

    let empty = read_status(dir.path()).expect("read_status must return Ok on an unborn repo");
    assert!(empty.staged.is_empty());
    assert!(empty.unstaged.is_empty());
    assert!(empty.untracked.is_empty());
    assert!(empty.conflicted.is_empty());

    std::fs::write(dir.path().join("first.txt"), "hello\n").expect("write file");
    let snap =
        read_status(dir.path()).expect("read_status must return Ok on an unborn repo with a file");
    assert_eq!(
        snap.untracked.len(),
        1,
        "the first file must be visible for staging"
    );
    assert!(snap.staged.is_empty());
    assert!(snap.conflicted.is_empty());
}

/// (3b) Graph seed probe — no tips, no HEAD oid, and no error.
#[test]
fn graph_seed_on_unborn_repo_is_ok_and_empty() {
    let dir = scratch_dir();
    let _repo = unborn_repo(dir.path());

    let seed = graph_seed(dir.path()).expect("graph_seed must return Ok on an unborn repo");
    assert!(seed.tips.is_empty(), "unborn repo has no walk tips");
    assert!(seed.head.is_none(), "unborn HEAD resolves to no oid");
}

/// (3c) The commit-log stream must yield exactly `Meta` then `Done` with zero
/// rows — the empty graph the frontend expects — never an error.
#[test]
fn stream_graph_on_unborn_repo_emits_meta_then_empty_done() {
    let dir = scratch_dir();
    let _repo = unborn_repo(dir.path());

    let mut chunks = Vec::new();
    stream_graph_core(dir.path(), |c| {
        chunks.push(c);
        true
    })
    .expect("stream_graph_core must return Ok on an unborn repo");

    assert_eq!(
        chunks.len(),
        2,
        "expected exactly Meta + Done, got {chunks:?}"
    );
    match &chunks[0] {
        GraphChunk::Meta { head_oid, .. } => {
            assert!(head_oid.is_none(), "unborn HEAD has no oid in Meta");
        }
        other => panic!("first chunk must be Meta, got {other:?}"),
    }
    match &chunks[1] {
        GraphChunk::Done {
            total_rows,
            lane_count,
            head_index,
            truncated,
        } => {
            assert_eq!(*total_rows, 0);
            assert_eq!(*lane_count, 0);
            assert!(head_index.is_none());
            assert!(!*truncated);
        }
        other => panic!("second chunk must be Done, got {other:?}"),
    }
}
