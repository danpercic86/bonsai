//! M4 CLI-oracle diff tests — adversarial probes (BOM/non-ASCII, tree-to-tree
//! renames, zero-line diffs) plus contract §6.2 scenario 16 (error paths).
//!
//! Moved verbatim out of `diff_cli.rs`; see that module for the oracle rules
//! and `diff_oracle` for the shared parser and fixtures.

use bonsai_core::error::AppError;
use bonsai_core::git::diff::{commit_diff, commit_file_diff, workdir_file_diff, LineKind};
use bonsai_core::git::status::FileStatus;
use crate::common;
use crate::common::{commit_fixed, git, init_repo};
use crate::diff_oracle::{assert_matches_oracle, commit_fixture, edit_line, numbered_lines};


macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// ---------------------------------------------------------------------------
// Adversarial probes (tester, beyond §6.2)
// ---------------------------------------------------------------------------

// Probe A: UTF-8 BOM + non-ASCII content. The BOM lives INSIDE line 1's
// bytes; lossy UTF-8 must carry it (and the CJK/accented text) through both
// our engine and the parsed CLI output byte-for-byte, on add AND del sides.
#[test]
fn utf8_bom_non_ascii_modified() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    let base = "\u{feff}café header\n日本語のテキスト\nplain ascii\nüber alles\n";
    std::fs::write(p.join("bom.txt"), base).expect("write bom.txt");
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");
    // Edit line 1 (the BOM-bearing line) and line 2 (pure non-ASCII).
    let edited = "\u{feff}café HEADER v2\n日本語のテキスト（更新）\nplain ascii\nüber alles\n";
    std::fs::write(p.join("bom.txt"), edited).expect("modify bom.txt");

    let fd = workdir_file_diff(p, "bom.txt", None, false, false, false).expect("bom diff");
    assert_eq!(fd.status, FileStatus::Modified);
    assert!(!fd.binary, "BOM + valid UTF-8 must not be treated as binary");
    // The deleted and added first lines must both retain the BOM.
    let firsts: Vec<&str> = fd.hunks[0]
        .lines
        .iter()
        .filter(|l| l.content.contains("café"))
        .map(|l| l.content.as_str())
        .collect();
    assert_eq!(firsts.len(), 2);
    assert!(
        firsts.iter().all(|c| c.starts_with('\u{feff}')),
        "BOM must survive in line content: {firsts:?}"
    );
    assert_matches_oracle(
        &fd,
        p,
        &["diff", "--no-color", "-U3", "-M", "--", "bom.txt"],
    );
}

// Probe B: a COMMIT that renames + modifies a file. commit_diff must surface
// one Renamed header with orig_path; commit_file_diff(orig_path: Some) must
// match `git diff -M tip^1 tip -- old new` (rename detection tree-to-tree,
// not just in the workdir/index paths §6.2 covers).
#[test]
fn commit_renamed_modified_file() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("old.txt"), numbered_lines(20)).expect("write old.txt");
    std::fs::write(p.join("other.txt"), "untouched\n").expect("write other.txt");
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");

    git(p, &["mv", "old.txt", "renamed.txt"]);
    edit_line(p, "renamed.txt", 10, "line 10 RENAME-EDIT");
    git(p, &["add", "-A"]);
    commit_fixed(p, "rename and tweak");
    let tip = git(p, &["rev-parse", "HEAD"]);

    // Header list: exactly one file, Renamed, orig_path set, counts 1/1.
    let cd = commit_diff(p, &tip).expect("commit_diff");
    assert_eq!(cd.files.len(), 1, "only the rename delta: {:?}", cd.files);
    let h = &cd.files[0];
    assert_eq!(h.path, "renamed.txt");
    assert_eq!(h.orig_path.as_deref(), Some("old.txt"));
    assert_eq!(h.status, FileStatus::Renamed);
    assert_eq!((h.additions, h.deletions), (1, 1));

    // Hunks: parity with the CLI including rename from/to capture.
    let fd = commit_file_diff(p, &tip, "renamed.txt", Some("old.txt"), false, false)
        .expect("commit rename file diff");
    assert_eq!(fd.status, FileStatus::Renamed);
    assert_eq!(fd.orig_path.as_deref(), Some("old.txt"));
    assert_matches_oracle(
        &fd,
        p,
        &[
            "diff",
            "--no-color",
            "-U3",
            "-M",
            &format!("{tip}^1"),
            &tip,
            "--",
            "old.txt",
            "renamed.txt",
        ],
    );
}

// Probe C: zero-line diffs. An empty file added (staged + committed) must
// yield hunks: [] WITHOUT tripping the benign-race empty (status must be
// Added, header counts 0/0) and commit_file_diff must NOT error ("path not
// changed" would be wrong — the delta exists, its content diff is empty).
// A tracked file truncated to 0 bytes is the all-Del mirror.
#[test]
fn empty_file_added_and_file_emptied() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("content.txt"), numbered_lines(3)).expect("write content.txt");
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");

    // Stage a brand-new EMPTY file; truncate the tracked one in the workdir.
    std::fs::write(p.join("empty.txt"), "").expect("write empty.txt");
    git(p, &["add", "--", "empty.txt"]);
    std::fs::write(p.join("content.txt"), "").expect("truncate content.txt");

    let staged = workdir_file_diff(p, "empty.txt", None, true, false, false).expect("staged empty diff");
    assert_eq!(staged.status, FileStatus::Added, "not the benign-race shape");
    assert!(!staged.binary && !staged.too_large);
    assert!(staged.hunks.is_empty());

    let emptied = workdir_file_diff(p, "content.txt", None, false, false, false).expect("emptied diff");
    assert_eq!(emptied.status, FileStatus::Modified);
    assert!(emptied.hunks[0].lines.iter().all(|l| l.kind == LineKind::Del));
    assert_matches_oracle(
        &emptied,
        p,
        &["diff", "--no-color", "-U3", "-M", "--", "content.txt"],
    );

    // Commit both; header counts must be 0/0 for the empty file and the
    // per-file diff must succeed with zero hunks.
    git(p, &["add", "-A"]);
    commit_fixed(p, "empty add + emptied");
    let tip = git(p, &["rev-parse", "HEAD"]);
    let cd = commit_diff(p, &tip).expect("commit_diff");
    let e = cd
        .files
        .iter()
        .find(|f| f.path == "empty.txt")
        .expect("empty.txt header");
    assert_eq!(e.status, FileStatus::Added);
    assert_eq!((e.additions, e.deletions), (0, 0));
    let c = cd
        .files
        .iter()
        .find(|f| f.path == "content.txt")
        .expect("content.txt header");
    assert_eq!((c.additions, c.deletions), (0, 3));

    let fd = commit_file_diff(p, &tip, "empty.txt", None, false, false)
        .expect("empty added file must be a delta, not 'path not changed'");
    assert_eq!(fd.status, FileStatus::Added);
    assert!(fd.hunks.is_empty());
    assert!(!fd.binary && !fd.too_large);
}

// Scenario 16: bad oids -> AppError::Git; escaping paths -> AppError::Other;
// commit_file_diff on an untouched path -> AppError::Git.
#[test]
fn bad_oid_and_path_validation() {
    require_git!();
    let (dir, tip) = commit_fixture();
    let p = dir.path();

    // Garbage oid: not hex.
    let err = commit_diff(p, "not-a-hex-oid").expect_err("garbage oid");
    assert!(matches!(err, AppError::Git(_)), "got: {err:?}");
    // Well-formed but unknown oid.
    let err = commit_diff(p, "0123456789abcdef0123456789abcdef01234567")
        .expect_err("unknown oid");
    assert!(matches!(err, AppError::Git(_)), "got: {err:?}");

    // Path validation (reused validate_rel_path).
    let err = workdir_file_diff(p, "../escape", None, false, false, false).expect_err("escaping path");
    assert!(
        matches!(&err, AppError::Other(m) if m.contains("invalid path")),
        "got: {err:?}"
    );
    let err = commit_file_diff(p, &tip, "../escape", None, false, false).expect_err("escaping path (commit)");
    assert!(
        matches!(&err, AppError::Other(m) if m.contains("invalid path")),
        "got: {err:?}"
    );

    // Untouched path in an immutable commit: an error, not an empty diff.
    let err = commit_file_diff(p, &tip, "b.txt", None, false, false).expect_err("untouched path");
    match err {
        AppError::Git(m) => assert!(m.contains("path not changed in commit"), "got: {m}"),
        other => panic!("expected AppError::Git, got: {other:?}"),
    }
}