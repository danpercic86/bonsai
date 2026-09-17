//! Ref-pill decoration tests. Exercises `decorate.rs`: pill order and
//! `is_head` across local branches, remote-tracking branches, lightweight and
//! annotated tags, plus detached HEAD. Sibling of `tests_lane.rs`; shared
//! fixture builders live in `tests.rs`.

use super::{branch, commit, ids, init_repo, set_head};
use crate::graph::*;

/// One commit that is simultaneously local branch tip (HEAD attached),
/// remote-tracking `origin/main`, lightweight tag, and annotated tag.
/// Label order per §2.2 pill_order; `is_head` only on the local branch.
#[test]
fn ref_pills_stacking() {
    let (dir, repo) = init_repo();
    let c = commit(&repo, "C", &[], 1);
    branch(&repo, "main", c);
    set_head(&repo, "main");
    repo.reference("refs/remotes/origin/main", c, true, "test")
        .expect("create remote ref");
    let obj = repo.find_object(c, None).expect("find object");
    repo.tag_lightweight("v1.0", &obj, true)
        .expect("lightweight tag");
    let sig = git2::Signature::new("Test User", "test@example.com", &git2::Time::new(2, 0))
        .expect("signature");
    repo.tag("v1.1-notes", &obj, &sig, "annotated notes tag", true)
        .expect("annotated tag");

    let l = compute_graph(dir.path()).expect("compute_graph");
    assert_eq!(l.nodes.len(), 1);
    assert_eq!(l.head_index, Some(0));
    assert_eq!(
        l.nodes[0].refs,
        vec![
            RefLabel {
                name: "main".to_string(),
                kind: RefKind::LocalBranch,
                is_head: true,
            },
            RefLabel {
                name: "origin/main".to_string(),
                kind: RefKind::RemoteBranch,
                is_head: false,
            },
            RefLabel {
                name: "v1.0".to_string(),
                kind: RefKind::Tag,
                is_head: false,
            },
            RefLabel {
                name: "v1.1-notes".to_string(),
                kind: RefKind::Tag,
                is_head: false,
            },
        ]
    );
}

/// Detached HEAD on a mid-history commit gets a Head label; `head_index`
/// points at it; no Head label anywhere else; the branch loses `is_head`.
#[test]
fn detached_head() {
    let (dir, repo) = init_repo();
    let c0 = commit(&repo, "C0", &[], 1);
    let c1 = commit(&repo, "C1", &[c0], 2);
    let c2 = commit(&repo, "C2", &[c1], 3);
    branch(&repo, "main", c2);
    repo.set_head_detached(c1).expect("detach HEAD");

    let l = compute_graph(dir.path()).expect("compute_graph");
    assert_eq!(
        ids(&l),
        [c2, c1, c0]
            .iter()
            .map(|o| o.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(l.head_index, Some(1));
    assert_eq!(
        l.nodes[1].refs,
        vec![RefLabel {
            name: "HEAD".to_string(),
            kind: RefKind::Head,
            is_head: true,
        }]
    );
    // Branch pill still present on the tip, but not marked HEAD.
    assert_eq!(
        l.nodes[0].refs,
        vec![RefLabel {
            name: "main".to_string(),
            kind: RefKind::LocalBranch,
            is_head: false,
        }]
    );
    // No Head label anywhere else.
    let head_labels = l
        .nodes
        .iter()
        .flat_map(|n| n.refs.iter())
        .filter(|r| r.kind == RefKind::Head)
        .count();
    assert_eq!(head_labels, 1);
}

/// A tag object pointing at a blob is ignored — no label, no panic.
#[test]
fn annotated_tag_to_blob_skipped() {
    let (dir, repo) = init_repo();
    let c = commit(&repo, "C", &[], 1);
    branch(&repo, "main", c);
    set_head(&repo, "main");
    let blob = repo.blob(b"just a blob").expect("blob");
    let obj = repo.find_object(blob, None).expect("find blob object");
    let sig = git2::Signature::new("Test User", "test@example.com", &git2::Time::new(2, 0))
        .expect("signature");
    repo.tag("blob-tag", &obj, &sig, "tag on a blob", false)
        .expect("tag blob");

    let l = compute_graph(dir.path()).expect("compute_graph");
    assert_eq!(l.nodes.len(), 1);
    assert!(l.nodes[0].refs.iter().all(|r| r.kind != RefKind::Tag));
}
