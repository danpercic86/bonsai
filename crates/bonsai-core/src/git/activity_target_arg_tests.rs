//! P119 T-R2: the `TargetArg` funnel table and `RunSubject::many`. Pure — no
//! repo, no I/O.

use super::*;
use crate::git::activity::MAX_ACTIVITY_TARGET_CHARS;

fn arg(a: TargetArg<'_>) -> Option<String> {
    arg_activity_target(a).map(ActivityTarget::into_string)
}

#[test]
fn target_arg_table() {
    let oid40 = "0123456789abcdef0123456789abcdef01234567";
    let cases: Vec<(TargetArg<'_>, Option<&str>)> = vec![
        // Prefix strips.
        (TargetArg::Branch("refs/heads/feature/x"), Some("feature/x")),
        (TargetArg::Branch("main"), Some("main")),
        (TargetArg::Ref("refs/heads/main"), Some("main")),
        (
            TargetArg::Ref("refs/remotes/origin/main"),
            Some("origin/main"),
        ),
        (TargetArg::Ref("refs/tags/v1.0"), Some("v1.0")),
        (TargetArg::Ref("origin/main"), Some("origin/main")),
        // Exactly ONE prefix is stripped, never two in sequence.
        (
            TargetArg::Ref("refs/heads/refs/tags/x"),
            Some("refs/tags/x"),
        ),
        (TargetArg::Remote("origin"), Some("origin")),
        (TargetArg::Tag("refs/tags/v2"), Some("v2")),
        (TargetArg::Tag("v2"), Some("v2")),
        // Commit: 40-hex → 7; a revspec or a too-short/non-hex string → None.
        (TargetArg::Commit(oid40), Some("0123456")),
        (TargetArg::Commit("ABCDEF1"), Some("ABCDEF1")),
        (TargetArg::Commit("HEAD~2"), None),
        (TargetArg::Commit("abc"), None),
        (TargetArg::Commit("abcdefg"), None),
        (TargetArg::Commit(""), None),
        (TargetArg::Stash(2), Some("stash@{2}")),
        (TargetArg::Stash(0), Some("stash@{0}")),
        // Name may contain spaces.
        (TargetArg::Name("my wt"), Some("my wt")),
        (TargetArg::Name("   "), None),
        // PathLeaf: either separator, trailing separators ignored.
        (TargetArg::PathLeaf("C:\\a\\repo\\"), Some("repo")),
        (TargetArg::PathLeaf("/a/b/"), Some("b")),
        (TargetArg::PathLeaf("/a/b"), Some("b")),
        (TargetArg::PathLeaf("C:/mixed\\sep/leaf"), Some("leaf")),
        (TargetArg::PathLeaf("single"), Some("single")),
        (TargetArg::PathLeaf(""), None),
        (TargetArg::PathLeaf("///"), None),
        // A URL-shaped path yields only its last component — never the token.
        (TargetArg::PathLeaf("https://tok@host/r.git"), Some("r.git")),
        // Bidi / controls are stripped by the one funnel.
        (TargetArg::Branch("ma\u{202e}in"), Some("main")),
        (TargetArg::Name("a\u{200b}b\nc"), Some("abc")),
    ];
    for (input, want) in cases {
        assert_eq!(arg(input).as_deref(), want, "{input:?}");
    }
}

#[test]
fn over_long_argument_is_capped_to_255_chars_ending_in_ellipsis() {
    let long = "x".repeat(400);
    for a in [
        TargetArg::Branch(&long),
        TargetArg::Name(&long),
        TargetArg::PathLeaf(&long),
    ] {
        let t = arg(a).expect("target");
        assert_eq!(t.chars().count(), MAX_ACTIVITY_TARGET_CHARS, "{a:?}");
        assert!(t.ends_with('…'), "{a:?}");
    }
}

#[test]
fn run_subject_many() {
    let none: [TargetArg<'_>; 0] = [];
    let empty = RunSubject::many(none.into_iter());
    assert_eq!(empty, RunSubject::default());
    assert_eq!(empty.target(), None);
    assert_eq!(empty.count(), None);

    let one = RunSubject::many([TargetArg::Branch("refs/heads/topic")].into_iter());
    assert_eq!(one.target().map(ActivityTarget::as_str), Some("topic"));
    assert_eq!(one.count(), None);

    let two = RunSubject::many([TargetArg::Branch("a"), TargetArg::Branch("b")].into_iter());
    assert_eq!(two.count(), Some(2));
    assert_eq!(two.target(), None, "a count is never paired with a target");

    let paths = ["a.txt", "b.txt", "c d.txt"];
    let three = RunSubject::many(paths.iter().map(|p| TargetArg::Name(p)));
    assert_eq!(three.count(), Some(3));
    assert_eq!(three.target(), None);
}

#[test]
fn run_subject_from_target_has_no_count() {
    let s = RunSubject::from(arg_activity_target(TargetArg::Tag("v1")));
    assert_eq!(s.target().map(ActivityTarget::as_str), Some("v1"));
    assert_eq!(s.count(), None);
    assert_eq!(RunSubject::from(None), RunSubject::default());
}
