//! M4 CLI-oracle diff tests (contract §6.1–§6.2).
//!
//! The oracle is the `git` CLI (`git diff` / `git diff --cached` /
//! `git diff <oid>^1 <oid>` / `git show` / `git diff --numstat`), compared as
//! PARSED STRUCTURES, never raw text: `parse_cli_diff` reduces CLI output to
//! files/hunks/lines under the same normalization our engine applies
//! (contract §2.4: strip one `\n` then one `\r`; function-context tail
//! dropped; `\ No newline` folded into a flag on the preceding line).
//!
//! HARD RULE: all scratch repos live on D: via `common::scratch_dir()`
//! (through `init_repo`). Fixture repos pin `core.autocrlf=false` so CLI and
//! git2 see identical bytes.
//!
//! Each test skips (passes with a note) if `git` is not on PATH.

use bonsai_core::git::diff::{workdir_file_diff, LineKind, MAX_FILE_DIFF_LINES};
use bonsai_core::git::status::FileStatus;
use crate::common;
use crate::common::{commit_fixed, git, git_raw, init_repo};
use crate::diff_oracle::{
    assert_line_numbers, assert_matches_oracle, edit_line, numbered_lines, parse_cli_diff,
    repo_with_f40,
};


macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// ---------------------------------------------------------------------------
// §6.2 scenarios
// ---------------------------------------------------------------------------

// Scenario 1: two separated edits -> two hunks, all numbers/kinds/contents
// match the CLI.
#[test]
fn unstaged_modified_multi_hunk() {
    require_git!();
    let dir = repo_with_f40();
    edit_line(dir.path(), "f.txt", 3, "line 3 CHANGED");
    edit_line(dir.path(), "f.txt", 30, "line 30 CHANGED");

    let fd = workdir_file_diff(dir.path(), "f.txt", None, false, false, false).expect("unstaged diff");
    assert_eq!(fd.status, FileStatus::Modified);
    assert_eq!(fd.hunks.len(), 2, "edits at lines 3 and 30 must be 2 hunks");
    assert_matches_oracle(
        &fd,
        dir.path(),
        &["diff", "--no-color", "-U3", "-M", "--", "f.txt"],
    );
}

// Scenario 2: staged edit matches `git diff --cached`; the unstaged diff of
// the same file is the benign-race empty FileDiff.
#[test]
fn staged_modified() {
    require_git!();
    let dir = repo_with_f40();
    edit_line(dir.path(), "f.txt", 3, "line 3 STAGED");
    git(dir.path(), &["add", "--", "f.txt"]);

    let staged = workdir_file_diff(dir.path(), "f.txt", None, true, false, false).expect("staged diff");
    assert_eq!(staged.status, FileStatus::Modified);
    assert_matches_oracle(
        &staged,
        dir.path(),
        &["diff", "--cached", "--no-color", "-U3", "-M", "--", "f.txt"],
    );

    let unstaged = workdir_file_diff(dir.path(), "f.txt", None, false, false, false).expect("unstaged diff");
    assert!(unstaged.hunks.is_empty(), "workdir == index -> no hunks");
    assert!(!unstaged.binary && !unstaged.too_large);
}

// Scenario 3: staged edit + further workdir edit -> the two diffs differ and
// each matches its own oracle.
#[test]
fn staged_vs_unstaged_split() {
    require_git!();
    let dir = repo_with_f40();
    edit_line(dir.path(), "f.txt", 3, "line 3 STAGED");
    git(dir.path(), &["add", "--", "f.txt"]);
    edit_line(dir.path(), "f.txt", 30, "line 30 WORKDIR");

    let staged = workdir_file_diff(dir.path(), "f.txt", None, true, false, false).expect("staged diff");
    let unstaged = workdir_file_diff(dir.path(), "f.txt", None, false, false, false).expect("unstaged diff");
    assert_ne!(staged, unstaged);
    assert_matches_oracle(
        &staged,
        dir.path(),
        &["diff", "--cached", "--no-color", "-U3", "-M", "--", "f.txt"],
    );
    assert_matches_oracle(
        &unstaged,
        dir.path(),
        &["diff", "--no-color", "-U3", "-M", "--", "f.txt"],
    );
}

// Scenario 4: untracked file -> structural all-Add assertion (the CLI has no
// direct oracle; contract §6.1 sanctions the structural check).
#[test]
fn untracked_file() {
    require_git!();
    let dir = repo_with_f40();
    let content = ["alpha", "beta", "gamma"];
    std::fs::write(
        dir.path().join("u.txt"),
        format!("{}\n", content.join("\n")),
    )
    .expect("write u.txt");

    let fd = workdir_file_diff(dir.path(), "u.txt", None, false, false, false).expect("untracked diff");
    assert_eq!(fd.status, FileStatus::Untracked);
    assert!(!fd.binary && !fd.too_large);
    assert_eq!(fd.hunks.len(), 1);
    let h = &fd.hunks[0];
    assert_eq!((h.old_start, h.old_lines), (0, 0));
    assert_eq!((h.new_start, h.new_lines), (1, content.len() as u32));
    for (i, line) in h.lines.iter().enumerate() {
        assert_eq!(line.kind, LineKind::Add);
        assert_eq!(line.content, content[i]);
        assert_eq!(line.new_no, Some(i as u32 + 1));
        assert_eq!(line.old_no, None);
    }
    assert_line_numbers(&fd);
}

// Scenario 5: fs-deleted tracked file -> unstaged all-Del; staged after
// `git add -A` -> staged all-Del. Both vs their oracles.
#[test]
fn deleted_file() {
    require_git!();
    let dir = repo_with_f40();
    std::fs::remove_file(dir.path().join("f.txt")).expect("delete f.txt");

    let fd = workdir_file_diff(dir.path(), "f.txt", None, false, false, false).expect("unstaged diff");
    assert_eq!(fd.status, FileStatus::Deleted);
    assert!(fd.hunks[0].lines.iter().all(|l| l.kind == LineKind::Del));
    assert_matches_oracle(
        &fd,
        dir.path(),
        &["diff", "--no-color", "-U3", "-M", "--", "f.txt"],
    );

    git(dir.path(), &["add", "-A", "--", "f.txt"]);
    let staged = workdir_file_diff(dir.path(), "f.txt", None, true, false, false).expect("staged diff");
    assert_eq!(staged.status, FileStatus::Deleted);
    assert_matches_oracle(
        &staged,
        dir.path(),
        &["diff", "--cached", "--no-color", "-U3", "-M", "--", "f.txt"],
    );
}

// Scenario 6: `git mv` + edit + stage -> one Renamed delta with orig_path,
// hunks matching `git diff --cached -M -- old new`.
#[test]
fn renamed_modified_staged() {
    require_git!();
    let dir = init_repo();
    std::fs::write(dir.path().join("old.txt"), numbered_lines(20)).expect("write old.txt");
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "base");

    git(dir.path(), &["mv", "old.txt", "new.txt"]);
    edit_line(dir.path(), "new.txt", 10, "line 10 TWEAKED");
    git(dir.path(), &["add", "--", "new.txt"]);

    let fd = workdir_file_diff(dir.path(), "new.txt", Some("old.txt"), true, false, false)
        .expect("staged rename diff");
    assert_eq!(fd.status, FileStatus::Renamed);
    assert_eq!(fd.orig_path.as_deref(), Some("old.txt"));
    assert_eq!(fd.path, "new.txt");
    assert_matches_oracle(
        &fd,
        dir.path(),
        &[
            "diff", "--cached", "--no-color", "-U3", "-M", "--", "old.txt", "new.txt",
        ],
    );
}

// Scenario 7: missing trailing newline -> `no_newline` flags on exactly the
// lines where the CLI prints its marker (del AND add side).
#[test]
fn no_trailing_newline() {
    require_git!();
    let dir = init_repo();
    std::fs::write(dir.path().join("n.txt"), "alpha\nbeta\ngamma").expect("write n.txt");
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "base");
    std::fs::write(dir.path().join("n.txt"), "alpha\nbeta\ndelta").expect("modify n.txt");

    let fd = workdir_file_diff(dir.path(), "n.txt", None, false, false, false).expect("diff");
    assert_matches_oracle(
        &fd,
        dir.path(),
        &["diff", "--no-color", "-U3", "-M", "--", "n.txt"],
    );
    // Both the removed and the added final line lack the newline.
    let flagged: Vec<&str> = fd.hunks[0]
        .lines
        .iter()
        .filter(|l| l.no_newline)
        .map(|l| l.content.as_str())
        .collect();
    assert_eq!(flagged, vec!["gamma", "delta"]);
}

// Scenario 8: NUL-bearing blob -> binary: true, hunks: []; CLI agrees.
#[test]
fn binary_file() {
    require_git!();
    let dir = init_repo();
    let blob: Vec<u8> = (0u8..=255).cycle().take(1024).collect();
    std::fs::write(dir.path().join("blob.bin"), &blob).expect("write blob.bin");
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "base");
    let mut modified = blob;
    modified[10] = 0xAA;
    modified.extend_from_slice(&[0, 1, 2, 3]);
    std::fs::write(dir.path().join("blob.bin"), &modified).expect("modify blob.bin");

    let fd = workdir_file_diff(dir.path(), "blob.bin", None, false, false, false).expect("binary diff");
    assert!(fd.binary);
    assert!(!fd.too_large);
    assert!(fd.hunks.is_empty());

    let out = git_raw(
        dir.path(),
        &["diff", "--no-color", "-U3", "-M", "--", "blob.bin"],
        &[],
    );
    let parsed = parse_cli_diff(&String::from_utf8_lossy(&out));
    assert_eq!(parsed.len(), 1);
    assert!(parsed[0].binary, "CLI must print Binary files ... differ");
    assert!(parsed[0].hunks.is_empty());
}

// Scenario 9: 6000-line deletion busts the 5000-line cap (too_large,
// all-or-nothing); a 100-line file stays under it.
#[test]
fn too_large_cap() {
    require_git!();
    let dir = init_repo();
    std::fs::write(dir.path().join("big.txt"), numbered_lines(6_000)).expect("write big.txt");
    std::fs::write(dir.path().join("small.txt"), numbered_lines(100)).expect("write small.txt");
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "base");
    std::fs::remove_file(dir.path().join("big.txt")).expect("delete big.txt");
    std::fs::remove_file(dir.path().join("small.txt")).expect("delete small.txt");

    let big = workdir_file_diff(dir.path(), "big.txt", None, false, false, false).expect("big diff");
    assert!(big.too_large, "6000 del lines > {MAX_FILE_DIFF_LINES}");
    assert!(!big.binary);
    assert!(big.hunks.is_empty(), "all-or-nothing: no partial hunks");

    let small = workdir_file_diff(dir.path(), "small.txt", None, false, false, false).expect("small diff");
    assert!(!small.too_large);
    assert_eq!(
        small.hunks.iter().map(|h| h.lines.len()).sum::<usize>(),
        100
    );
    assert_matches_oracle(
        &small,
        dir.path(),
        &["diff", "--no-color", "-U3", "-M", "--", "small.txt"],
    );
}

// Scenario 10: CRLF content (autocrlf=false) -> contents match the CLI after
// §2.4 stripping; no phantom whole-file churn.
#[test]
fn crlf_content() {
    require_git!();
    let dir = init_repo();
    std::fs::write(dir.path().join("c.txt"), "one\r\ntwo\r\nthree\r\nfour\r\nfive\r\n")
        .expect("write c.txt");
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "base");
    std::fs::write(
        dir.path().join("c.txt"),
        "one\r\ntwo CHANGED\r\nthree\r\nfour\r\nfive\r\n",
    )
    .expect("modify c.txt");

    let fd = workdir_file_diff(dir.path(), "c.txt", None, false, false, false).expect("crlf diff");
    assert_eq!(fd.hunks.len(), 1, "single edit must stay a single hunk");
    assert!(
        fd.hunks[0]
            .lines
            .iter()
            .all(|l| !l.content.contains('\r') && !l.content.contains('\n')),
        "line endings must be stripped from content"
    );
    assert_matches_oracle(
        &fd,
        dir.path(),
        &["diff", "--no-color", "-U3", "-M", "--", "c.txt"],
    );
}
