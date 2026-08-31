//! Wire-shape + stats tests for `health`. Extracted verbatim from the former
//! inline `mod tests`; shared fixtures live in `test_support`.

use super::test_support::*;
use super::*;
use std::process::Command;

#[test]
fn wire_shapes_serialize_camelcase() {
    let health = RepoHealth {
        stats: Section {
            data: Some(StatsSection {
                commit_count: 5,
                commit_count_capped: true,
                commits_last_30d: 2,
                authors_last_30d: 1,
                authors_total: 3,
                object_count: 42,
                object_scan_capped: false,
                largest_blobs: vec![BlobStat {
                    oid: "a".repeat(40),
                    size: 1234,
                }],
                workdir_file_count: 7,
                workdir_bytes: 999,
                workdir_scan_capped: false,
                largest_files: vec![FileStat {
                    path: "big/file.bin".to_string(),
                    size: 999,
                }],
                large_file_count: 1,
                git_dir_bytes: 100,
                git_dir_scan_capped: false,
            }),
            error: None,
            elapsed_ms: 12,
        },
        branches: Section {
            data: Some(BranchesSection {
                local_count: 2,
                remote_count: 1,
                tag_count: 0,
                current_branch: Some("main".to_string()),
                detached: false,
                unborn: false,
                ahead: Some(2),
                behind: Some(5),
                upstream: Some("origin/main".to_string()),
                stale: Some(StaleRollup {
                    base: "main".to_string(),
                    merged_count: 3,
                    gone_upstream_count: 1,
                }),
                stale_error: None,
            }),
            error: None,
            elapsed_ms: 3,
        },
        working_state: Section {
            data: Some(WorkingStateSection {
                staged: 1,
                unstaged: 2,
                untracked: 3,
                conflicted: 0,
                op_state: RepoOpState::Merge {
                    incoming: "feature/x".to_string(),
                    message: "Merge branch 'feature/x'".to_string(),
                },
                stash_count: 2,
                has_gitignore: true,
            }),
            error: None,
            elapsed_ms: 1,
        },
        structure: Section {
            data: None,
            error: Some("boom".to_string()),
            elapsed_ms: 0,
        },
        generated_at: 1_700_000_000,
    };
    let v = serde_json::to_value(&health).expect("serialize");
    assert_eq!(v["generatedAt"], 1_700_000_000_i64);
    let s = &v["stats"];
    assert_eq!(s["elapsedMs"], 12);
    assert_eq!(s["data"]["commitCountCapped"], true);
    assert_eq!(s["data"]["commitsLast30d"], 2);
    assert_eq!(s["data"]["authorsLast30d"], 1);
    assert_eq!(s["data"]["objectScanCapped"], false);
    assert_eq!(s["data"]["largestBlobs"][0]["oid"], "a".repeat(40));
    assert_eq!(s["data"]["largestFiles"][0]["path"], "big/file.bin");
    assert_eq!(s["data"]["largeFileCount"], 1);
    assert_eq!(s["data"]["gitDirBytes"], 100);
    assert_eq!(s["data"]["workdirScanCapped"], false);
    let b = &v["branches"]["data"];
    assert_eq!(b["localCount"], 2);
    assert_eq!(b["currentBranch"], "main");
    assert_eq!(b["stale"]["mergedCount"], 3);
    assert_eq!(b["stale"]["goneUpstreamCount"], 1);
    assert_eq!(b["staleError"], serde_json::Value::Null);
    let w = &v["workingState"]["data"];
    assert_eq!(w["opState"]["kind"], "merge");
    assert_eq!(w["opState"]["incoming"], "feature/x");
    assert_eq!(w["stashCount"], 2);
    assert_eq!(w["hasGitignore"], true);
    // Error envelope: data null, error set.
    assert_eq!(v["structure"]["data"], serde_json::Value::Null);
    assert_eq!(v["structure"]["error"], "boom");
}

// ------------------------------------------------------- stats

/// N commits → commitCount == N == `git rev-list --count HEAD`; a >10 MiB
/// file shows in largestFiles + largeFileCount; objectCount ≥ commits.
#[test]
fn stats_counts_and_large_files() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    init(d);
    commit(d, "C0", &[("a.txt", "a\n")]);
    commit(d, "C1", &[("b.txt", "b\n")]);
    commit(d, "C2", &[("c.txt", "c\n")]);

    // One file over the 10 MiB threshold (untracked, non-ignored → counts).
    let big = vec![0u8; (LARGE_FILE_BYTES + 1) as usize];
    std::fs::write(d.join("big.bin"), &big).expect("write big file");

    let stats = collect_stats_with_caps(d, DEFAULT_CAPS).expect("stats");
    assert_eq!(stats.commit_count, 3);
    assert!(!stats.commit_count_capped);
    assert_eq!(stats.commits_last_30d, 3, "fresh commits are within 30d");
    assert_eq!(stats.authors_last_30d, 1);
    assert_eq!(stats.authors_total, 1);
    assert!(
        stats.object_count >= 3,
        "at least the commit objects: {}",
        stats.object_count
    );
    assert!(!stats.largest_blobs.is_empty(), "blobs exist in the odb");

    assert_eq!(stats.large_file_count, 1);
    assert_eq!(stats.largest_files[0].path, "big.bin");
    assert_eq!(stats.largest_files[0].size, LARGE_FILE_BYTES + 1);
    assert!(stats.workdir_file_count >= 4, "a,b,c + big.bin");
    assert!(stats.workdir_bytes > LARGE_FILE_BYTES);
    assert!(stats.git_dir_bytes > 0);
    assert!(!stats.workdir_scan_capped);
    assert!(!stats.git_dir_scan_capped);

    // CLI oracle (skip when git absent).
    if have_git() {
        let out = Command::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .current_dir(d)
            .output()
            .expect("git rev-list");
        assert!(out.status.success());
        let cli: u32 = String::from_utf8_lossy(&out.stdout).trim().parse().expect("count");
        assert_eq!(stats.commit_count, cli, "matches git rev-list --count");
    }
}

/// Gitignored files/dirs are EXCLUDED from workdir stats (P44b): a >10 MiB
/// ignored `ignored.bin` and an ignored `node_modules/` subtree do NOT
/// appear in `largest_files`, do NOT bump `large_file_count`, and are not
/// counted in `workdir_file_count`/`workdir_bytes` — only the non-ignored
/// control file + the `.gitignore` itself count. This proves exclusion
/// (the `stats_counts_and_large_files` test only confirms a NON-ignored
/// large file still counts).
#[test]
fn stats_excludes_gitignored_files() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    init(d);
    // One committed, non-ignored control file.
    commit(d, "C0", &[("control.txt", "control\n")]);

    // .gitignore covering a single file and a whole directory.
    std::fs::write(d.join(".gitignore"), "ignored.bin\nnode_modules/\n")
        .expect("write .gitignore");

    // A >10 MiB IGNORED file (same construction as the large-file test):
    // must NOT count nor appear in largest_files.
    let big = vec![0u8; (LARGE_FILE_BYTES + 1) as usize];
    std::fs::write(d.join("ignored.bin"), &big).expect("write big ignored file");

    // An ignored directory with a file inside: the whole subtree is excluded.
    std::fs::create_dir(d.join("node_modules")).expect("mkdir node_modules");
    std::fs::write(d.join("node_modules").join("dep.js"), "module.exports={}\n")
        .expect("write node_modules file");

    let stats = collect_stats_with_caps(d, DEFAULT_CAPS).expect("stats");

    // The ignored large file is absent from largestFiles.
    assert!(
        !stats.largest_files.iter().any(|f| f.path == "ignored.bin"),
        "ignored.bin must be excluded from largestFiles: {:?}",
        stats.largest_files
    );
    // No node_modules/* path leaks into largestFiles either.
    assert!(
        !stats
            .largest_files
            .iter()
            .any(|f| f.path.starts_with("node_modules/")),
        "node_modules subtree must be excluded: {:?}",
        stats.largest_files
    );
    // The only >10 MiB file is ignored → large_file_count is not bumped.
    assert_eq!(stats.large_file_count, 0, "ignored large file must not count");
    // Only the two non-ignored files (control.txt + .gitignore) are counted;
    // ignored.bin and node_modules/dep.js are not.
    assert_eq!(
        stats.workdir_file_count, 2,
        "only control.txt + .gitignore count, got {}",
        stats.workdir_file_count
    );
    // workdir_bytes excludes the 10 MiB ignored blob (both counted files are
    // tiny text) — proves the ignored bytes are not summed in.
    assert!(
        stats.workdir_bytes < LARGE_FILE_BYTES,
        "workdir_bytes must exclude the ignored 10 MiB file: {}",
        stats.workdir_bytes
    );
    // Sanity: the non-ignored control file IS present.
    assert!(
        stats.largest_files.iter().any(|f| f.path == "control.txt"),
        "control.txt should be counted: {:?}",
        stats.largest_files
    );
}

/// Shadowed caps: revwalk stops at the cap with capped=true and
/// count == cap; the workdir walk sets its capped flag on overflow.
#[test]
fn stats_capped_flags() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    init(d);
    for i in 0..5 {
        commit(d, &format!("C{i}"), &[("a.txt", &format!("{i}\n"))]);
    }
    let stats = collect_stats_with_caps(d, caps(3, WORKDIR_WALK_CAP)).expect("stats");
    assert_eq!(stats.commit_count, 3, "count equals the cap");
    assert!(stats.commit_count_capped);

    let stats = collect_stats_with_caps(d, caps(REVWALK_CAP, 1)).expect("stats");
    assert!(stats.workdir_scan_capped, "workdir cap of 1 entry overflows");
}

/// Unborn repo: stats section still Ok with zero commits.
#[test]
fn stats_unborn_repo_ok() {
    let dir = crate::testutil::scratch_dir();
    init(dir.path());
    let stats = collect_stats_with_caps(dir.path(), DEFAULT_CAPS).expect("stats");
    assert_eq!(stats.commit_count, 0);
    assert!(!stats.commit_count_capped);
}

