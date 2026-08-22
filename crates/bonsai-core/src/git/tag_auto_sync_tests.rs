//! P84 auto-sync tests against local bare-repo remotes: adopt / move-FF /
//! skip-diverged / skip-siblings / no-op / annotated adoption / no-remote /
//! best-effort-fetch-failure, each asserting the private `refs/bonsai-tagsync/*`
//! namespace is cleaned up afterward.

use super::*;

/// A signature usable without any git config.
fn sig() -> git2::Signature<'static> {
    git2::Signature::now("T", "t@e.x").expect("sig")
}

/// Commit a single file; returns the new commit oid.
fn commit_file(repo: &git2::Repository, name: &str, body: &str, msg: &str) -> git2::Oid {
    let root = repo.workdir().expect("workdir");
    std::fs::write(root.join(name), body).expect("write file");
    let mut index = repo.index().expect("index");
    index.add_path(std::path::Path::new(name)).expect("add");
    index.write().expect("index write");
    let tree_oid = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_oid).expect("tree");
    let parents: Vec<git2::Commit> = match repo.head().ok().and_then(|h| h.target()) {
        Some(t) => vec![repo.find_commit(t).expect("parent")],
        None => vec![],
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    repo.commit(Some("HEAD"), &sig(), &sig(), msg, &tree, &parent_refs)
        .expect("commit")
}

/// Number of refs currently under the private tag-sync namespace (must be 0
/// after every `auto_sync_tags` call).
fn temp_ref_count(workdir: &std::path::Path) -> usize {
    let repo = git2::Repository::open(workdir).expect("open");
    repo.references_glob(TAGSYNC_GLOB).expect("glob").count()
}

/// End-to-end auto-sync: adopt remote-only, move an FF-able stale tag, skip a
/// diverged/local-ahead stale tag, leave in-sync + local-only untouched, and
/// clean the temp namespace afterward.
#[test]
fn auto_sync_adopts_moves_and_skips() {
    let work_dir = crate::testutil::scratch_dir();
    let bare_dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(work_dir.path()).expect("init work");
    git2::Repository::init_bare(bare_dir.path()).expect("init bare");

    // Linear history c0 -> c1 -> c2.
    let c0 = commit_file(&repo, "a.txt", "0", "c0");
    let c1 = commit_file(&repo, "a.txt", "1", "c1");
    let c2 = commit_file(&repo, "a.txt", "2", "c2");
    let obj0 = repo.find_object(c0, None).expect("obj0");
    let obj1 = repo.find_object(c1, None).expect("obj1");
    let obj2 = repo.find_object(c2, None).expect("obj2");

    let url = bare_dir.path().to_str().expect("utf8");
    let mut remote = repo.remote("origin", url).expect("remote");

    // Remote tags: adoptme@c1 (remote-only), ffme@c2, insync@c0, ahead@c0.
    repo.tag_lightweight("adoptme", &obj1, false).expect("adoptme");
    repo.tag_lightweight("ffme", &obj2, false).expect("ffme");
    repo.tag_lightweight("insync", &obj0, false).expect("insync");
    repo.tag_lightweight("ahead", &obj0, false).expect("ahead");
    remote
        .push(
            &[
                "refs/tags/adoptme:refs/tags/adoptme",
                "refs/tags/ffme:refs/tags/ffme",
                "refs/tags/insync:refs/tags/insync",
                "refs/tags/ahead:refs/tags/ahead",
            ],
            None,
        )
        .expect("push");

    // Diverge locally:
    // - delete adoptme (now remote-only)
    // - move ffme back to c0 (remote c2 strictly descends => FF-able)
    // - keep insync at c0
    // - move ahead forward to c2 (local ahead of remote c0 => skip)
    // - add localonly@c1 (never on remote)
    repo.tag_delete("adoptme").expect("del adoptme");
    repo.tag_lightweight("ffme", &obj0, true).expect("ffme back");
    repo.tag_lightweight("ahead", &obj2, true).expect("ahead fwd");
    repo.tag_lightweight("localonly", &obj1, false)
        .expect("localonly");

    let report = auto_sync_tags(work_dir.path(), None).expect("auto-sync");
    assert_eq!(report.remote, "origin");
    assert_eq!(report.adopted, vec!["adoptme".to_string()]);
    assert_eq!(report.moved, vec!["ffme".to_string()]);
    assert_eq!(report.skipped_diverged, vec!["ahead".to_string()]);

    // Post-conditions on the actual refs.
    let peel = |n: &str| {
        repo.find_reference(&format!("refs/tags/{n}"))
            .expect("ref")
            .peel(git2::ObjectType::Any)
            .expect("peel")
            .id()
    };
    assert_eq!(peel("adoptme"), c1, "adopted at remote committish");
    assert_eq!(peel("ffme"), c2, "fast-forwarded to remote");
    assert_eq!(peel("ahead"), c2, "local-ahead tag left untouched");
    assert_eq!(peel("insync"), c0, "in-sync untouched");
    assert_eq!(peel("localonly"), c1, "local-only untouched");

    assert_eq!(temp_ref_count(work_dir.path()), 0, "temp namespace cleaned");
}

/// Sibling commits (no ancestry either way) => skipped as diverged, untouched.
#[test]
fn auto_sync_skips_siblings() {
    let work_dir = crate::testutil::scratch_dir();
    let bare_dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(work_dir.path()).expect("init work");
    git2::Repository::init_bare(bare_dir.path()).expect("init bare");

    let c0 = commit_file(&repo, "a.txt", "0", "c0");
    let obj0 = repo.find_object(c0, None).expect("obj0");
    // Two independent children of c0 (siblings).
    let root = repo.workdir().expect("workdir");
    std::fs::write(root.join("a.txt"), "x").expect("w");
    let mut idx = repo.index().expect("idx");
    idx.add_path(std::path::Path::new("a.txt")).expect("add");
    idx.write().expect("iw");
    let tree = repo.find_tree(idx.write_tree().expect("wt")).expect("t");
    let parent = repo.find_commit(c0).expect("parent");
    let sib_a = repo
        .commit(None, &sig(), &sig(), "sib-a", &tree, &[&parent])
        .expect("sib-a");
    std::fs::write(root.join("a.txt"), "y").expect("w");
    let mut idx = repo.index().expect("idx");
    idx.add_path(std::path::Path::new("a.txt")).expect("add");
    idx.write().expect("iw");
    let tree = repo.find_tree(idx.write_tree().expect("wt")).expect("t");
    let sib_b = repo
        .commit(None, &sig(), &sig(), "sib-b", &tree, &[&parent])
        .expect("sib-b");

    let url = bare_dir.path().to_str().expect("utf8");
    let mut remote = repo.remote("origin", url).expect("remote");
    let _ = obj0;
    // Remote tag at sib_a; push; then move local to sib_b (diverged).
    repo.tag_lightweight("t", &repo.find_object(sib_a, None).unwrap(), false)
        .expect("tag");
    remote.push(&["refs/tags/t:refs/tags/t"], None).expect("push");
    repo.tag_lightweight("t", &repo.find_object(sib_b, None).unwrap(), true)
        .expect("move local");

    let report = auto_sync_tags(work_dir.path(), None).expect("auto-sync");
    assert_eq!(report.skipped_diverged, vec!["t".to_string()]);
    assert!(report.adopted.is_empty() && report.moved.is_empty());
    // Local left at sib_b.
    let cur = repo
        .find_reference("refs/tags/t")
        .unwrap()
        .peel(git2::ObjectType::Any)
        .unwrap()
        .id();
    assert_eq!(cur, sib_b);
    assert_eq!(temp_ref_count(work_dir.path()), 0);
}

/// Everything already in sync => empty report, nothing touched.
#[test]
fn auto_sync_noop_when_in_sync() {
    let work_dir = crate::testutil::scratch_dir();
    let bare_dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(work_dir.path()).expect("init work");
    git2::Repository::init_bare(bare_dir.path()).expect("init bare");
    let c0 = commit_file(&repo, "a.txt", "0", "c0");
    let obj0 = repo.find_object(c0, None).expect("obj0");
    let url = bare_dir.path().to_str().expect("utf8");
    let mut remote = repo.remote("origin", url).expect("remote");
    repo.tag_lightweight("v1", &obj0, false).expect("tag");
    remote.push(&["refs/tags/v1:refs/tags/v1"], None).expect("push");

    let report = auto_sync_tags(work_dir.path(), None).expect("auto-sync");
    assert!(report.adopted.is_empty());
    assert!(report.moved.is_empty());
    assert!(report.skipped_diverged.is_empty());
    assert_eq!(temp_ref_count(work_dir.path()), 0);
}

/// Adopting an ANNOTATED remote-only tag keeps it annotated locally (the local
/// ref points at the tag OBJECT, not the peeled committish).
#[test]
fn auto_sync_adopts_annotated_tag() {
    let work_dir = crate::testutil::scratch_dir();
    let bare_dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(work_dir.path()).expect("init work");
    git2::Repository::init_bare(bare_dir.path()).expect("init bare");
    let c0 = commit_file(&repo, "a.txt", "0", "c0");
    let obj0 = repo.find_object(c0, None).expect("obj0");
    let url = bare_dir.path().to_str().expect("utf8");
    let mut remote = repo.remote("origin", url).expect("remote");

    repo.tag("rel", &obj0, &sig(), "annotated release", false)
        .expect("annot");
    remote.push(&["refs/tags/rel:refs/tags/rel"], None).expect("push");
    repo.tag_delete("rel").expect("del local");

    let report = auto_sync_tags(work_dir.path(), None).expect("auto-sync");
    assert_eq!(report.adopted, vec!["rel".to_string()]);

    let r = repo.find_reference("refs/tags/rel").expect("ref");
    // Annotated => the ref peels to a Tag object AND its committish == c0.
    assert!(
        r.peel(git2::ObjectType::Tag).is_ok(),
        "adopted tag stays annotated"
    );
    assert_eq!(r.peel(git2::ObjectType::Any).expect("peel").id(), c0);
    assert_eq!(temp_ref_count(work_dir.path()), 0);
}

/// A stale `refs/bonsai-tagsync/*` ref left by a crashed prior run must be swept
/// BEFORE the fetch — otherwise a ghost whose remote tag was since deleted (so
/// the force refspec never overwrites it) would be spuriously re-adopted. Here
/// the remote has no tags at all, so the only way `ghost` could be adopted is
/// the stale temp ref surviving into the reconcile pass.
#[test]
fn auto_sync_sweeps_preexisting_stale_temp_ref() {
    let work_dir = crate::testutil::scratch_dir();
    let bare_dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(work_dir.path()).expect("init work");
    git2::Repository::init_bare(bare_dir.path()).expect("init bare");
    let c0 = commit_file(&repo, "a.txt", "0", "c0");
    let url = bare_dir.path().to_str().expect("utf8");
    let mut remote = repo.remote("origin", url).expect("remote");
    // Push HEAD so the bare remote is a valid fetch source (it has no tags).
    remote
        .push(&["refs/heads/master:refs/heads/master"], None)
        .or_else(|_| remote.push(&["refs/heads/main:refs/heads/main"], None))
        .expect("push head");

    // Plant a stale temp ref as if a previous run crashed before cleanup.
    repo.reference(
        "refs/bonsai-tagsync/ghost",
        c0,
        true,
        "planted stale temp ref",
    )
    .expect("plant ghost");

    let report = auto_sync_tags(work_dir.path(), None).expect("auto-sync");
    assert!(
        report.adopted.is_empty(),
        "stale temp ref must not be re-adopted"
    );
    assert!(
        repo.find_reference("refs/tags/ghost").is_err(),
        "no ghost tag should be created from a swept stale temp ref"
    );
    assert_eq!(temp_ref_count(work_dir.path()), 0, "temp namespace cleaned");
}

/// No remote configured => empty Ok report, never an error.
#[test]
fn auto_sync_no_remote_is_empty_ok() {
    let dir = crate::testutil::scratch_dir();
    git2::Repository::init(dir.path()).expect("init");
    let report = auto_sync_tags(dir.path(), None).expect("empty ok");
    assert_eq!(report.remote, "");
    assert!(report.adopted.is_empty());
    assert!(report.moved.is_empty());
    assert!(report.skipped_diverged.is_empty());
}

/// A fetch failure (bad remote URL) => empty Ok report naming the remote, temp
/// namespace clean, never propagated as an error.
#[test]
fn auto_sync_fetch_failure_is_best_effort() {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init");
    repo.remote("origin", "file:///nonexistent/bonsai-missing.git")
        .expect("remote");

    let report = auto_sync_tags(dir.path(), None).expect("best-effort ok");
    assert_eq!(report.remote, "origin");
    assert!(report.adopted.is_empty());
    assert_eq!(temp_ref_count(dir.path()), 0, "temp namespace cleaned on failure");
}
