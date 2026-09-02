//! M4 CLI-oracle diff tests — commit-diff scenarios (contract §6.2,
//! scenarios 11–15): commit details/headers, per-file commit hunks, root
//! commits, merge commits (first parent only) and unborn HEAD.
//!
//! Moved verbatim out of `diff_cli.rs`; see that module for the oracle rules
//! and `diff_oracle` for the shared parser and fixtures.

use bonsai_core::git::diff::{commit_diff, commit_file_diff, workdir_file_diff, LineKind};
use bonsai_core::git::status::FileStatus;
use crate::common;
use crate::common::{commit_fixed, git, init_repo};
use crate::diff_oracle::{
    assert_matches_oracle, commit_fixture, edit_line, numbered_lines, numstat,
};


macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping: `git` CLI not found on PATH");
            return;
        }
    };
}

// Scenario 11: commit details + headers vs `git log` / `rev-parse` /
// `--numstat` oracles; files sorted by path.
#[test]
fn commit_diff_simple() {
    require_git!();
    let (dir, tip) = commit_fixture();
    let p = dir.path();

    let cd = commit_diff(p, &tip).expect("commit_diff");
    assert_eq!(cd.details.oid, tip);
    assert_eq!(cd.details.summary, "feat: subject line");

    let full_message = git(p, &["log", "-1", "--format=%B", &tip]);
    assert_eq!(cd.details.message, full_message.trim_end());
    assert!(cd.details.message.contains("body second line"));

    assert_eq!(cd.details.author_name, "Test User");
    assert_eq!(cd.details.author_email, "test@example.com");
    let at: i64 = git(p, &["log", "-1", "--format=%at", &tip])
        .parse()
        .expect("author ts");
    assert_eq!(cd.details.author_ts, at);
    let ct: i64 = git(p, &["log", "-1", "--format=%ct", &tip])
        .parse()
        .expect("committer ts");
    assert_eq!(cd.details.committer_ts, ct);

    let parents: Vec<String> = git(p, &["rev-parse", &format!("{tip}^@")])
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(cd.details.parents, parents);
    assert_eq!(cd.details.parents.len(), 1);

    // Headers vs numstat (the counts oracle), sorted by path.
    let ours: Vec<(String, u32, u32)> = cd
        .files
        .iter()
        .map(|f| (f.path.clone(), f.additions, f.deletions))
        .collect();
    assert_eq!(ours, numstat(p, &format!("{tip}^1"), &tip));
    let paths: Vec<&str> = cd.files.iter().map(|f| f.path.as_str()).collect();
    let mut sorted = paths.clone();
    sorted.sort_unstable();
    assert_eq!(paths, sorted, "files must be sorted by path");
    assert!(cd.files.iter().all(|f| !f.binary));
    let m = cd.files.iter().find(|f| f.path == "m.txt").expect("m.txt");
    assert_eq!(m.status, FileStatus::Added);
}

// Scenario 12: per-file commit hunks vs parsed `git diff tip^1 tip -- path`.
#[test]
fn commit_file_diff_matches_show() {
    require_git!();
    let (dir, tip) = commit_fixture();

    for path in ["a.txt", "m.txt", "z.txt"] {
        let fd = commit_file_diff(dir.path(), &tip, path, None, false, false)
            .unwrap_or_else(|e| panic!("commit_file_diff({path}): {e:?}"));
        assert_matches_oracle(
            &fd,
            dir.path(),
            &[
                "diff",
                "--no-color",
                "-U3",
                "-M",
                &format!("{tip}^1"),
                &tip,
                "--",
                path,
            ],
        );
    }
}

// Scenario 13: root commit -> parents [], all files Added, file diff matches
// parsed `git show --format= root`.
#[test]
fn root_commit() {
    require_git!();
    let dir = init_repo();
    std::fs::write(dir.path().join("first.txt"), numbered_lines(4)).expect("write first.txt");
    git(dir.path(), &["add", "-A"]);
    commit_fixed(dir.path(), "root");
    let root = git(dir.path(), &["rev-parse", "HEAD"]);

    let cd = commit_diff(dir.path(), &root).expect("commit_diff(root)");
    assert!(cd.details.parents.is_empty());
    assert!(cd.files.iter().all(|f| f.status == FileStatus::Added));
    assert_eq!(cd.files.len(), 1);
    assert_eq!(cd.files[0].additions, 4);
    assert_eq!(cd.files[0].deletions, 0);

    let fd = commit_file_diff(dir.path(), &root, "first.txt", None, false, false).expect("root file diff");
    assert_eq!(fd.status, FileStatus::Added);
    assert_matches_oracle(
        &fd,
        dir.path(),
        &["show", "--format=", "--no-color", "-U3", "-M", &root],
    );
}

// Scenario 14: merge commit diffs against the FIRST parent only (never --cc);
// details.parents carries both oids in order.
#[test]
fn merge_commit_first_parent() {
    require_git!();
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("main.txt"), numbered_lines(10)).expect("write main.txt");
    std::fs::write(p.join("feat.txt"), numbered_lines(10)).expect("write feat.txt");
    git(p, &["add", "-A"]);
    commit_fixed(p, "base");

    git(p, &["checkout", "-b", "feat"]);
    edit_line(p, "feat.txt", 2, "line 2 FEATURE");
    git(p, &["add", "-A"]);
    commit_fixed(p, "feat work");
    let feat_tip = git(p, &["rev-parse", "HEAD"]);

    git(p, &["checkout", "main"]);
    edit_line(p, "main.txt", 7, "line 7 MAINLINE");
    git(p, &["add", "-A"]);
    commit_fixed(p, "main work");
    let main_tip = git(p, &["rev-parse", "HEAD"]);

    common::git_env(
        p,
        &["merge", "--no-ff", "-m", "merge feat", "feat"],
        &[
            ("GIT_AUTHOR_DATE", common::FIXED_DATE),
            ("GIT_COMMITTER_DATE", common::FIXED_DATE),
        ],
    );
    let merge = git(p, &["rev-parse", "HEAD"]);

    let cd = commit_diff(p, &merge).expect("commit_diff(merge)");
    assert_eq!(cd.details.parents, vec![main_tip, feat_tip]);

    // vs first parent: only the feature-branch file changed.
    let ours: Vec<(String, u32, u32)> = cd
        .files
        .iter()
        .map(|f| (f.path.clone(), f.additions, f.deletions))
        .collect();
    assert_eq!(ours, numstat(p, &format!("{merge}^1"), &merge));
    assert_eq!(cd.files.len(), 1);
    assert_eq!(cd.files[0].path, "feat.txt");

    let fd = commit_file_diff(p, &merge, "feat.txt", None, false, false).expect("merge file diff");
    assert_matches_oracle(
        &fd,
        p,
        &[
            "diff",
            "--no-color",
            "-U3",
            "-M",
            &format!("{merge}^1"),
            &merge,
            "--",
            "feat.txt",
        ],
    );
}

// Scenario 15: unborn HEAD -> staged diff against the empty tree (all Add).
#[test]
fn unborn_staged() {
    require_git!();
    let dir = init_repo();
    std::fs::write(dir.path().join("seed.txt"), numbered_lines(3)).expect("write seed.txt");
    git(dir.path(), &["add", "--", "seed.txt"]);

    let fd = workdir_file_diff(dir.path(), "seed.txt", None, true, false, false).expect("unborn staged diff");
    assert_eq!(fd.status, FileStatus::Added);
    assert!(fd.hunks[0].lines.iter().all(|l| l.kind == LineKind::Add));
    assert_matches_oracle(
        &fd,
        dir.path(),
        &["diff", "--cached", "--no-color", "-U3", "-M", "--", "seed.txt"],
    );
}
