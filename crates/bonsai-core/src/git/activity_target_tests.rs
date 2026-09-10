//! P87b FU-1 §9.1-9.3 tests for [`super`] (`git::activity_target`): one fixture
//! repo per row of the §4 table, the anti-drift pin against `push_current`'s own
//! upstream resolution, and the never-panics-on-a-broken-repo guarantee.

use super::*;
use crate::git::activity::GitActivityCategory as Cat;

/// A signature usable without any git config.
fn sig() -> git2::Signature<'static> {
    git2::Signature::now("T", "t@e.x").expect("sig")
}

/// A repo whose initial branch is ALWAYS `main` — `Repository::init` would
/// otherwise honour the machine's `init.defaultBranch` and make the table
/// assertions host-dependent.
fn init_main(path: &Path) -> git2::Repository {
    let mut opts = git2::RepositoryInitOptions::new();
    opts.initial_head("main");
    git2::Repository::init_opts(path, &opts).expect("init repo")
}

/// Commit a single file onto HEAD; returns the new commit oid.
fn commit_file(repo: &git2::Repository, name: &str, body: &str) -> git2::Oid {
    let root = repo.workdir().expect("workdir");
    std::fs::write(root.join(name), body).expect("write");
    let mut index = repo.index().expect("index");
    index.add_path(Path::new(name)).expect("add");
    index.write().expect("index write");
    let tree_oid = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_oid).expect("tree");
    let parents: Vec<git2::Commit> = match repo.head().ok().and_then(|h| h.target()) {
        Some(t) => vec![repo.find_commit(t).expect("parent")],
        None => vec![],
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    repo.commit(Some("HEAD"), &sig(), &sig(), "c", &tree, &parent_refs)
        .expect("commit")
}

/// Point `branch.<branch>.remote`/`.merge` at `<remote>/<remote_branch>`.
/// Config-only (no tracking ref needed) — exactly what `configured_upstream`
/// reads, and what `git branch --set-upstream-to` writes.
fn set_upstream_config(repo: &git2::Repository, branch: &str, remote: &str, remote_branch: &str) {
    let mut cfg = repo.config().expect("config");
    cfg.set_str(&format!("branch.{branch}.remote"), remote)
        .expect("set remote");
    cfg.set_str(
        &format!("branch.{branch}.merge"),
        &format!("refs/heads/{remote_branch}"),
    )
    .expect("set merge");
}

fn target(workdir: &Path, category: Cat) -> Option<String> {
    resolve_activity_target(workdir, category).map(ActivityTarget::into_string)
}

/// §9.1 — one fixture repo per row of the §4 table, each asserting the exact
/// `Option<&str>`.
#[test]
fn resolves_table() {
    // ---- (a) a branch WITH a configured upstream: push / force-push / pull.
    let with_upstream = crate::testutil::scratch_dir();
    let repo = init_main(with_upstream.path());
    commit_file(&repo, "a.txt", "1");
    repo.remote("origin", "https://example.invalid/r.git")
        .expect("remote");
    set_upstream_config(&repo, "main", "origin", "main");
    let p = with_upstream.path();
    assert_eq!(target(p, Cat::Push).as_deref(), Some("origin/main"));
    assert_eq!(target(p, Cat::ForcePush).as_deref(), Some("origin/main"));
    assert_eq!(target(p, Cat::Pull).as_deref(), Some("origin/main"));
    // Fetch is fetch-ALL — never a single ref, even here.
    assert_eq!(target(p, Cat::Fetch), None);
    // Commit / amend / merge name the LOCAL branch.
    assert_eq!(target(p, Cat::Commit).as_deref(), Some("main"));
    assert_eq!(target(p, Cat::Amend).as_deref(), Some("main"));
    assert_eq!(target(p, Cat::MergeCommit).as_deref(), Some("main"));

    // A non-default upstream remote/branch pair is FOLLOWED, not assumed.
    let renamed = crate::testutil::scratch_dir();
    let repo = init_main(renamed.path());
    commit_file(&repo, "a.txt", "1");
    repo.remote("upstream", "https://example.invalid/r.git")
        .expect("remote");
    set_upstream_config(&repo, "main", "upstream", "trunk");
    assert_eq!(
        target(renamed.path(), Cat::Pull).as_deref(),
        Some("upstream/trunk")
    );
    // Push too, and this is the ONLY place it is pinned: the anti-drift test
    // below cannot witness a divergent upstream branch, because
    // `PushResult::UpToDate` carries the LOCAL branch name. Without this line the
    // Push arm could read `branch.<x>.remote` and ignore `branch.<x>.merge`
    // (yielding `upstream/main`) with every test still green.
    assert_eq!(
        target(renamed.path(), Cat::Push).as_deref(),
        Some("upstream/trunk")
    );

    // ---- (b) no upstream but an `origin` exists: push defaults to
    // `origin/<branch>`; force-push/pull do NOT (the op returns `NoUpstream`).
    let origin_only = crate::testutil::scratch_dir();
    let repo = init_main(origin_only.path());
    commit_file(&repo, "a.txt", "1");
    repo.remote("origin", "https://example.invalid/r.git")
        .expect("remote");
    let p = origin_only.path();
    assert_eq!(target(p, Cat::Push).as_deref(), Some("origin/main"));
    assert_eq!(target(p, Cat::ForcePush), None);
    assert_eq!(target(p, Cat::Pull), None);
    assert_eq!(target(p, Cat::Commit).as_deref(), Some("main"));

    // ---- (c) neither an upstream nor an `origin`: push has nothing to name.
    let no_remote = crate::testutil::scratch_dir();
    let repo = init_main(no_remote.path());
    commit_file(&repo, "a.txt", "1");
    let p = no_remote.path();
    assert_eq!(target(p, Cat::Push), None);
    assert_eq!(target(p, Cat::Commit).as_deref(), Some("main"));

    // ---- (d) detached HEAD: no ref the run is "on" ⇒ None for every category.
    let detached = crate::testutil::scratch_dir();
    let repo = init_main(detached.path());
    let oid = commit_file(&repo, "a.txt", "1");
    repo.remote("origin", "https://example.invalid/r.git")
        .expect("remote");
    set_upstream_config(&repo, "main", "origin", "main");
    repo.set_head_detached(oid).expect("detach");
    let p = detached.path();
    assert_eq!(target(p, Cat::Commit), None);
    assert_eq!(target(p, Cat::Amend), None);
    assert_eq!(target(p, Cat::MergeCommit), None);
    assert_eq!(target(p, Cat::Push), None);

    // ---- (e) unborn HEAD: FU-1 §3.6-1 wants `null` even though HEAD's symbolic
    // target does name the branch-to-be.
    let unborn = crate::testutil::scratch_dir();
    let repo = init_main(unborn.path());
    assert!(
        read_head_info(&repo).expect("head").unborn,
        "fixture must have an unborn HEAD"
    );
    assert_eq!(target(unborn.path(), Cat::Commit), None);
    assert_eq!(target(unborn.path(), Cat::Push), None);
}

/// §9.1 (guarantee 1, core side) — no §4 output is ever a human phrase: a target
/// is a raw identifier, so it can contain no space, arrow, or quote.
#[test]
fn no_target_contains_prose() {
    let dir = crate::testutil::scratch_dir();
    let repo = init_main(dir.path());
    commit_file(&repo, "a.txt", "1");
    repo.remote("origin", "https://example.invalid/r.git")
        .expect("remote");
    set_upstream_config(&repo, "main", "origin", "main");
    for cat in [
        Cat::Push,
        Cat::ForcePush,
        Cat::Pull,
        Cat::Fetch,
        Cat::Commit,
        Cat::Amend,
        Cat::MergeCommit,
    ] {
        let Some(t) = target(dir.path(), cat) else {
            continue;
        };
        assert!(
            !t.contains([' ', '\u{2192}', '\'', '"']),
            "target {t:?} for {cat:?} reads like prose"
        );
    }
}

/// §9.2 anti-drift, scoped to what `PushResult` can actually witness: the
/// **remote name**.
///
/// `PushResult::UpToDate { remote, branch }` carries the remote plus the
/// **local** branch name (`remote_push_activity.rs`), never the upstream branch.
/// So this pins two things and no more: that the resolver picked the same remote
/// `push_current_with_activity` picked, and — only because this fixture's
/// upstream branch happens to equal its local branch — the whole
/// `remote/branch` string. On a divergent upstream (`main` → `upstream/trunk`)
/// the two sides would differ while BOTH are correct, so the resolver's use of
/// `branch.<x>.merge` is pinned separately, by `resolves_table`'s `renamed`
/// fixture.
///
/// The fixture short-circuits as `UpToDate`, so there is no network and — per
/// the panicking exec — no hook spawn either.
#[test]
fn push_target_agrees_with_push_result_remote() {
    use crate::error::AppError;
    use crate::git::exec::{GitExec, GitOutput};
    use crate::git::remote::PushResult;
    use crate::git::remote_push_activity::push_current_with_activity;

    /// The `UpToDate` short-circuit precedes the pre-push hook, so spawning git
    /// at all would be a bug.
    struct PanicExec;
    impl GitExec for PanicExec {
        fn exec(
            &self,
            _a: &[&str],
            _c: &Path,
            _s: Option<&[u8]>,
            _e: &[(&str, &str)],
        ) -> Result<GitOutput, AppError> {
            panic!("the up-to-date short-circuit must not spawn git");
        }
    }

    let work = crate::testutil::scratch_dir();
    let bare = crate::testutil::scratch_dir();
    let repo = init_main(work.path());
    let tip = commit_file(&repo, "a.txt", "1");
    git2::Repository::init_bare(bare.path()).expect("init bare");
    let url = bare.path().to_str().expect("utf8");
    let mut remote = repo.remote("origin", url).expect("remote");
    remote
        .push(&["refs/heads/main:refs/heads/main"], None)
        .expect("push to bare");
    // Tracking ref at the local tip + upstream config ⇒ "already up to date".
    repo.reference("refs/remotes/origin/main", tip, true, "seed tracking")
        .expect("tracking ref");
    set_upstream_config(&repo, "main", "origin", "main");

    let result = push_current_with_activity(work.path(), &PanicExec, false, None).expect("push");
    let PushResult::UpToDate { remote, branch } = result else {
        panic!("fixture must short-circuit as up-to-date, got {result:?}");
    };
    assert_eq!(
        target(work.path(), Cat::Push),
        Some(format!("{remote}/{branch}")),
        "the resolver disagrees with push_current's own upstream resolution — which \
         pins the REMOTE, this fixture's local and upstream branch names being equal; \
         the upstream BRANCH is pinned by resolves_table's `renamed` fixture"
    );
}

/// §9.3 — infallible: a non-repo directory, a corrupt HEAD, and a path that does
/// not exist all yield `None` for every category, with no panic.
#[test]
fn resolver_never_panics_on_broken_repo() {
    let cats = [Cat::Push, Cat::Pull, Cat::ForcePush, Cat::Commit, Cat::Amend];

    let plain = crate::testutil::scratch_dir();
    for cat in cats {
        assert_eq!(target(plain.path(), cat), None, "non-repo dir, {cat:?}");
    }

    let corrupt = crate::testutil::scratch_dir();
    let repo = init_main(corrupt.path());
    commit_file(&repo, "a.txt", "1");
    let head_file = repo.path().join("HEAD");
    drop(repo);
    std::fs::write(&head_file, b"\x00\x01not a ref at all\n").expect("corrupt HEAD");
    for cat in cats {
        assert_eq!(target(corrupt.path(), cat), None, "corrupt HEAD, {cat:?}");
    }

    let missing = plain.path().join("no-such-dir").join("nested");
    for cat in cats {
        assert_eq!(target(&missing, cat), None, "missing path, {cat:?}");
    }
}
