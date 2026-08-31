//! P77 tag-sync tests: pure classification join (the annotated no-false-stale
//! crux) + an end-to-end `list_tag_sync` against a local bare-repo remote.

use super::*;

// ---------------------------------------------------------------- pure join

fn oid(hex_seed: u8) -> git2::Oid {
    let mut bytes = [0u8; 20];
    bytes[0] = hex_seed;
    git2::Oid::from_bytes(&bytes).expect("20-byte oid")
}

/// The full classification table on pure inputs: in-sync / stale / local-only /
/// remote-only, plus the annotated-tag no-false-stale case.
#[test]
fn classify_covers_all_statuses() {
    let a = oid(1);
    let b = oid(2);

    let mut local = HashMap::new();
    local.insert("in-sync".to_string(), (a, false));
    local.insert("stale".to_string(), (a, false)); // local a, remote b
    local.insert("local-only".to_string(), (a, false));
    // annotated tag: local peels to committish `a`; remote advertises `a` too.
    local.insert("annot".to_string(), (a, true));

    let mut remote = HashMap::new();
    remote.insert("in-sync".to_string(), a);
    remote.insert("stale".to_string(), b);
    remote.insert("remote-only".to_string(), b);
    remote.insert("annot".to_string(), a);

    let mut remote_annotated = HashSet::new();
    remote_annotated.insert("annot".to_string());

    let entries = classify(&local, &remote, &remote_annotated);
    let by: HashMap<&str, &TagSyncEntry> =
        entries.iter().map(|e| (e.name.as_str(), e)).collect();

    assert_eq!(by["in-sync"].status, TagSyncStatus::InSync);
    assert_eq!(by["stale"].status, TagSyncStatus::Stale);
    assert_ne!(by["stale"].local_oid, by["stale"].remote_oid);
    assert_eq!(by["local-only"].status, TagSyncStatus::LocalOnly);
    assert_eq!(by["local-only"].remote_oid, None);
    assert_eq!(by["remote-only"].status, TagSyncStatus::RemoteOnly);
    assert_eq!(by["remote-only"].local_oid, None);

    // The crux: annotated tag whose committish matches is IN-SYNC, not stale.
    assert_eq!(by["annot"].status, TagSyncStatus::InSync);
    assert!(by["annot"].annotated);
}

/// `parse_remote_tags` must let the peeled `X^{}` committish win over the tag
/// object oid `X`, regardless of emit order, and mark the tag annotated. A naive
/// tag-object-vs-committish compare would falsely report Stale.
#[test]
fn parse_remote_prefers_peeled_committish() {
    let tag_obj = oid(9);
    let committish = oid(3);

    // Normal order: "X" then "X^{}".
    let pairs = vec![
        ("refs/tags/v1".to_string(), tag_obj),
        ("refs/tags/v1^{}".to_string(), committish),
    ];
    let (map, annotated) = parse_remote_tags(&pairs);
    assert_eq!(map["v1"], committish);
    assert!(annotated.contains("v1"));

    // Reversed order: "X^{}" then "X" must NOT clobber the committish.
    let pairs_rev = vec![
        ("refs/tags/v1^{}".to_string(), committish),
        ("refs/tags/v1".to_string(), tag_obj),
    ];
    let (map_rev, _) = parse_remote_tags(&pairs_rev);
    assert_eq!(map_rev["v1"], committish);

    // Lightweight tag (no ^{}) keeps its commit oid and is not annotated.
    let light = vec![("refs/tags/lw".to_string(), committish)];
    let (lmap, lann) = parse_remote_tags(&light);
    assert_eq!(lmap["lw"], committish);
    assert!(!lann.contains("lw"));
}

/// Lightweight-vs-annotated on BOTH sides that peel to the SAME committish must
/// be `in-sync` with `annotated=true` when either side is annotated (per §4:
/// "annotated somewhere" is the display flag). A naive same-name compare that
/// ignored the peel/annotated split could mis-flag these.
#[test]
fn classify_lightweight_vs_annotated_both_directions() {
    let a = oid(1);

    // Direction 1: lightweight LOCAL, annotated REMOTE, same committish.
    let mut local = HashMap::new();
    local.insert("mix".to_string(), (a, false)); // local is lightweight
    let mut remote = HashMap::new();
    remote.insert("mix".to_string(), a); // remote peeled committish
    let mut remote_annotated = HashSet::new();
    remote_annotated.insert("mix".to_string()); // remote advertised X^{}

    let e = &classify(&local, &remote, &remote_annotated)[0];
    assert_eq!(e.status, TagSyncStatus::InSync);
    assert!(e.annotated, "annotated-on-remote must surface annotated=true");

    // Direction 2: annotated LOCAL, lightweight REMOTE, same committish.
    let mut local2 = HashMap::new();
    local2.insert("mix".to_string(), (a, true)); // local annotated (peels to a)
    let mut remote2 = HashMap::new();
    remote2.insert("mix".to_string(), a); // remote lightweight commit oid
    let empty = HashSet::new();

    let e2 = &classify(&local2, &remote2, &empty)[0];
    assert_eq!(e2.status, TagSyncStatus::InSync);
    assert!(e2.annotated, "annotated-on-local must surface annotated=true");
}

/// `resolve_default_remote`: caller override wins; else `origin` is preferred
/// over any other; else the first configured remote; else `NoRemote`.
#[test]
fn resolve_default_remote_selection() {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init");

    // No remotes yet → NoRemote.
    match resolve_default_remote(&repo, None) {
        Err(AppError::NoRemote(_)) => {}
        other => panic!("expected NoRemote, got {other:?}"),
    }

    // A single non-origin remote → that remote is the default.
    repo.remote("upstream", "https://example.invalid/u.git")
        .expect("add upstream");
    assert_eq!(
        resolve_default_remote(&repo, None).expect("first-remote default"),
        "upstream"
    );

    // Add `origin` → origin is preferred over the pre-existing remote.
    repo.remote("origin", "https://example.invalid/o.git")
        .expect("add origin");
    assert_eq!(
        resolve_default_remote(&repo, None).expect("origin default"),
        "origin"
    );

    // A caller-supplied remote short-circuits the default resolution.
    assert_eq!(
        resolve_default_remote(&repo, Some("upstream")).expect("override"),
        "upstream"
    );
    // Blank/whitespace override falls through to the default.
    assert_eq!(
        resolve_default_remote(&repo, Some("   ")).expect("blank override → default"),
        "origin"
    );
}

// ------------------------------------------------- end-to-end (bare remote)

/// A signature usable without any git config (tests must not depend on the
/// host's user.name/email).
fn sig() -> git2::Signature<'static> {
    git2::Signature::now("T", "t@e.x").expect("sig")
}

/// Commit a single file with the given contents; returns the new commit oid.
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

/// End-to-end reconciliation against a local bare remote covering in-sync
/// (lightweight + annotated), stale/moved, local-only and remote-only.
#[test]
fn list_tag_sync_against_bare_remote() {
    let work_dir = crate::testutil::scratch_dir();
    let bare_dir = crate::testutil::scratch_dir();

    let repo = git2::Repository::init(work_dir.path()).expect("init work");
    git2::Repository::init_bare(bare_dir.path()).expect("init bare");

    let c0 = commit_file(&repo, "a.txt", "0", "c0");
    let c1 = commit_file(&repo, "a.txt", "1", "c1");
    let obj0 = repo.find_object(c0, None).expect("obj0");
    let obj1 = repo.find_object(c1, None).expect("obj1");

    // Remote url = the bare repo path (git2 local transport, no creds).
    let url = bare_dir.path().to_str().expect("bare path utf8");
    let mut remote = repo.remote("origin", url).expect("add remote");

    // Tags: lightweight in-sync, annotated in-sync, stale (moved), local-only.
    repo.tag_lightweight("light", &obj0, false).expect("light");
    repo.tag("annot", &obj0, &sig(), "annotated", false)
        .expect("annot");
    repo.tag_lightweight("moved", &obj0, false).expect("moved");
    repo.tag_lightweight("localonly", &obj1, false).expect("localonly");
    repo.tag_lightweight("remoteonly", &obj1, false)
        .expect("remoteonly");

    // Push everything to the remote.
    let specs = [
        "refs/tags/light:refs/tags/light",
        "refs/tags/annot:refs/tags/annot",
        "refs/tags/moved:refs/tags/moved",
        "refs/tags/remoteonly:refs/tags/remoteonly",
    ];
    remote.push(&specs, None).expect("push tags");

    // Now diverge: move "moved" locally to c1 (remote still c0 => stale), and
    // delete "remoteonly" locally (present only on the remote now).
    repo.tag_lightweight("moved", &obj1, true).expect("re-move");
    repo.tag_delete("remoteonly").expect("delete local");

    let report = list_tag_sync(work_dir.path(), None).expect("reconcile");
    assert_eq!(report.remote, "origin");
    let by: HashMap<&str, &TagSyncEntry> =
        report.entries.iter().map(|e| (e.name.as_str(), e)).collect();

    assert_eq!(by["light"].status, TagSyncStatus::InSync);
    // The crux: annotated tag with matching committish is IN-SYNC, not stale.
    assert_eq!(by["annot"].status, TagSyncStatus::InSync);
    assert!(by["annot"].annotated);
    assert_eq!(by["moved"].status, TagSyncStatus::Stale);
    assert_eq!(by["localonly"].status, TagSyncStatus::LocalOnly);
    assert_eq!(by["remoteonly"].status, TagSyncStatus::RemoteOnly);
    assert_eq!(by["remoteonly"].local_oid, None);
}

/// The flagship v1.1.0 regression, end-to-end with an ANNOTATED tag: push an
/// annotated tag, then force-move it on the remote to a new committish while the
/// local annotated tag keeps the OLD target. It MUST classify `stale` (comparing
/// the PEELED committish, never the tag-object oid), then `force_refresh_tag`
/// MUST correct it to `in-sync`. This is the moved-tag bug P77 exists to kill.
#[test]
fn annotated_moved_tag_is_stale_then_refresh_fixes() {
    let work_dir = crate::testutil::scratch_dir();
    let bare_dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(work_dir.path()).expect("init work");
    git2::Repository::init_bare(bare_dir.path()).expect("init bare");

    let c0 = commit_file(&repo, "a.txt", "0", "c0");
    let c1 = commit_file(&repo, "a.txt", "1", "c1");
    let obj0 = repo.find_object(c0, None).expect("obj0");
    let obj1 = repo.find_object(c1, None).expect("obj1");

    let url = bare_dir.path().to_str().expect("utf8");
    let mut remote = repo.remote("origin", url).expect("remote");

    // Local ANNOTATED v1.1.0 at c0; push it (remote gets the tag object + peel).
    repo.tag("v1.1.0", &obj0, &sig(), "release 1.1.0", false)
        .expect("annot tag");
    remote
        .push(&["refs/tags/v1.1.0:refs/tags/v1.1.0"], None)
        .expect("push v1.1.0");

    // Simulate a "second machine" force-moving the tag on the remote to c1:
    // retag annotated at c1 locally and force-push over the remote ref.
    repo.tag("v1.1.0", &obj1, &sig(), "release 1.1.0 (moved)", true)
        .expect("retag");
    remote
        .push(&["+refs/tags/v1.1.0:refs/tags/v1.1.0"], None)
        .expect("force-push moved");
    // Restore THIS machine's stale local view: annotated at the OLD target c0.
    repo.tag("v1.1.0", &obj0, &sig(), "release 1.1.0", true)
        .expect("stale local");

    // Reproduce the bug: local peels to c0, remote peels to c1 → STALE, not a
    // false in-sync and not a false-stale-from-comparing-tag-objects.
    let before = list_tag_sync(work_dir.path(), None).expect("before");
    let v = &before.entries[0];
    assert_eq!(v.name, "v1.1.0");
    assert_eq!(v.status, TagSyncStatus::Stale);
    assert!(v.annotated);
    assert_eq!(v.local_oid.as_deref(), Some(c0.to_string().as_str()));
    assert_eq!(v.remote_oid.as_deref(), Some(c1.to_string().as_str()));

    // Force-refresh corrects the local ref to the remote's target → in-sync.
    force_refresh_tag(work_dir.path(), "origin", "v1.1.0").expect("refresh");
    let after = list_tag_sync(work_dir.path(), None).expect("after");
    assert_eq!(after.entries[0].status, TagSyncStatus::InSync);
    assert_eq!(after.entries[0].local_oid, after.entries[0].remote_oid);
}

/// Lightweight LOCAL vs annotated REMOTE that peel to the SAME commit ⇒ in-sync,
/// annotated=true — end-to-end through a real ls-remote (the peeled `^{}` entry
/// is what makes the compare correct). Guards the crux across the wire, not just
/// in the pure `classify`.
#[test]
fn lightweight_local_vs_annotated_remote_same_commit_in_sync() {
    let work_dir = crate::testutil::scratch_dir();
    let bare_dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(work_dir.path()).expect("init work");
    git2::Repository::init_bare(bare_dir.path()).expect("init bare");

    let c0 = commit_file(&repo, "a.txt", "0", "c0");
    let obj0 = repo.find_object(c0, None).expect("obj0");
    let url = bare_dir.path().to_str().expect("utf8");
    let mut remote = repo.remote("origin", url).expect("remote");

    // Push an ANNOTATED tag, then locally replace it with a LIGHTWEIGHT tag at
    // the same commit — remote stays annotated, local is lightweight.
    repo.tag("mix", &obj0, &sig(), "annotated", false)
        .expect("annot");
    remote
        .push(&["refs/tags/mix:refs/tags/mix"], None)
        .expect("push");
    repo.tag_delete("mix").expect("del local annot");
    repo.tag_lightweight("mix", &obj0, false).expect("light local");

    let report = list_tag_sync(work_dir.path(), None).expect("reconcile");
    let e = &report.entries[0];
    assert_eq!(e.name, "mix");
    assert_eq!(e.status, TagSyncStatus::InSync);
    assert!(e.annotated, "remote is annotated → annotated=true");
}

/// A tag that peels to a NON-commit (a tree) classifies by the peeled object oid
/// with no special-casing (§4 note). Same tree object on both sides ⇒ in-sync.
#[test]
fn remote_tag_peeling_to_non_commit() {
    let work_dir = crate::testutil::scratch_dir();
    let bare_dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(work_dir.path()).expect("init work");
    git2::Repository::init_bare(bare_dir.path()).expect("init bare");

    let c0 = commit_file(&repo, "a.txt", "0", "c0");
    let tree_oid = repo.find_commit(c0).expect("commit").tree_id();
    let tree_obj = repo.find_object(tree_oid, None).expect("tree obj");

    let url = bare_dir.path().to_str().expect("utf8");
    let mut remote = repo.remote("origin", url).expect("remote");

    // Lightweight tag pointing directly at a TREE (not a commit) on both sides.
    repo.tag_lightweight("treetag", &tree_obj, false)
        .expect("tag tree");
    remote
        .push(&["refs/tags/treetag:refs/tags/treetag"], None)
        .expect("push tree tag");

    let report = list_tag_sync(work_dir.path(), None).expect("reconcile");
    let e = &report.entries[0];
    assert_eq!(e.name, "treetag");
    assert_eq!(e.status, TagSyncStatus::InSync);
    assert_eq!(e.local_oid.as_deref(), Some(tree_oid.to_string().as_str()));
}

/// No remote configured => `NoRemote`, never a panic.
#[test]
fn list_tag_sync_no_remote() {
    let dir = crate::testutil::scratch_dir();
    git2::Repository::init(dir.path()).expect("init");
    match list_tag_sync(dir.path(), None) {
        Err(AppError::NoRemote(_)) => {}
        other => panic!("expected NoRemote, got {other:?}"),
    }
}

/// `force_refresh_tag` corrects a stale local tag to the remote's target.
#[test]
fn force_refresh_corrects_stale() {
    let work_dir = crate::testutil::scratch_dir();
    let bare_dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(work_dir.path()).expect("init work");
    git2::Repository::init_bare(bare_dir.path()).expect("init bare");

    let c0 = commit_file(&repo, "a.txt", "0", "c0");
    let c1 = commit_file(&repo, "a.txt", "1", "c1");
    let obj0 = repo.find_object(c0, None).expect("obj0");
    let obj1 = repo.find_object(c1, None).expect("obj1");

    let url = bare_dir.path().to_str().expect("utf8");
    let mut remote = repo.remote("origin", url).expect("remote");
    // Remote has "v1" at c1; local has a stale "v1" at c0.
    repo.tag_lightweight("v1", &obj1, false).expect("tag c1");
    remote
        .push(&["refs/tags/v1:refs/tags/v1"], None)
        .expect("push");
    repo.tag_lightweight("v1", &obj0, true).expect("stale local");

    // Pre-condition: stale.
    let before = list_tag_sync(work_dir.path(), None).expect("before");
    assert_eq!(before.entries[0].status, TagSyncStatus::Stale);

    force_refresh_tag(work_dir.path(), "origin", "v1").expect("refresh");

    // Post-condition: in-sync.
    let after = list_tag_sync(work_dir.path(), None).expect("after");
    assert_eq!(after.entries[0].status, TagSyncStatus::InSync);
}

/// `delete_remote_tag` removes the remote ref (post-condition: local-only).
#[test]
fn delete_remote_tag_removes_ref() {
    let work_dir = crate::testutil::scratch_dir();
    let bare_dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(work_dir.path()).expect("init work");
    git2::Repository::init_bare(bare_dir.path()).expect("init bare");

    let c0 = commit_file(&repo, "a.txt", "0", "c0");
    let obj0 = repo.find_object(c0, None).expect("obj0");
    let url = bare_dir.path().to_str().expect("utf8");
    let mut remote = repo.remote("origin", url).expect("remote");
    repo.tag_lightweight("v1", &obj0, false).expect("tag");
    remote
        .push(&["refs/tags/v1:refs/tags/v1"], None)
        .expect("push");

    // Pre-condition: in-sync (present both sides).
    let before = list_tag_sync(work_dir.path(), None).expect("before");
    assert_eq!(before.entries[0].status, TagSyncStatus::InSync);

    delete_remote_tag(work_dir.path(), "origin", "v1").expect("delete remote");

    // Post-condition: local-only (remote ref gone).
    let after = list_tag_sync(work_dir.path(), None).expect("after");
    assert_eq!(after.entries[0].status, TagSyncStatus::LocalOnly);
}

/// Name validation guards the resolve ops before any network work.
#[test]
fn resolve_ops_validate_name() {
    let dir = crate::testutil::scratch_dir();
    git2::Repository::init(dir.path()).expect("init");
    for bad in ["", "   ", "-x"] {
        match force_refresh_tag(dir.path(), "origin", bad) {
            Err(AppError::InvalidName(_)) => {}
            other => panic!("force_refresh {bad:?} => {other:?}"),
        }
        match delete_remote_tag(dir.path(), "origin", bad) {
            Err(AppError::InvalidName(_)) => {}
            other => panic!("delete_remote {bad:?} => {other:?}"),
        }
    }
}
