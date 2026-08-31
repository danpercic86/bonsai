    use super::*;
    use crate::git::branches::{checkout_branch, create_branch};
    use crate::git::commit::create_commit;
    use crate::git::stage::stage_paths;
    use crate::git::status::FileStatus;

    fn init_scratch() -> tempfile::TempDir {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        dir
    }

    /// A feature branch diverges from base: base…head is the THREE-DOT diff, so
    /// a commit landed on base AFTER the fork must NOT appear in the PR diff.
    #[test]
    fn pr_diff_is_three_dot_from_merge_base() {
        let dir = init_scratch();
        let p = dir.path();

        // O: base commit with file1.
        std::fs::write(p.join("file1.txt"), "one\n").expect("write");
        stage_paths(p, &["file1.txt".into()]).expect("stage");
        let o = create_commit(p, "O", None, false).expect("commit O");
        let main_name = o.branch.expect("base branch");

        // feat forks at O and adds file_feat.
        create_branch(p, "feat").expect("create feat");
        checkout_branch(p, "feat").expect("checkout feat");
        std::fs::write(p.join("file_feat.txt"), "feat\n").expect("write");
        stage_paths(p, &["file_feat.txt".into()]).expect("stage");
        let head = create_commit(p, "H", None, false).expect("commit H").oid;

        // base advances past the fork with file_base (must NOT show in base…head).
        checkout_branch(p, &main_name).expect("checkout base");
        std::fs::write(p.join("file_base.txt"), "base\n").expect("write");
        stage_paths(p, &["file_base.txt".into()]).expect("stage");
        let base = create_commit(p, "B", None, false).expect("commit B").oid;

        let stats = pr_diff_headers(p, &base, &head).expect("pr diff");
        assert_eq!(stats.merge_base_oid, o.oid, "merge-base is the fork point");
        let paths: Vec<&str> = stats.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["file_feat.txt"], "only the PR's own change: {paths:?}");
        assert_eq!(stats.changed_files, 1);
        assert_eq!(stats.additions, 1);
        assert_eq!(stats.deletions, 0);
        assert_eq!(stats.files[0].status, FileStatus::Added);

        // Per-file hunks for the changed file.
        let fd = pr_file_diff(p, &stats.merge_base_oid, &head, "file_feat.txt", None, false, false)
            .expect("file diff");
        assert_eq!(fd.path, "file_feat.txt");
        assert_eq!(fd.hunks.len(), 1);

        // A path not in the diff errors.
        let err = pr_file_diff(p, &stats.merge_base_oid, &head, "file_base.txt", None, false, false)
            .expect_err("unchanged path must error");
        assert!(matches!(err, AppError::Git(_)), "got {err:?}");
    }

    /// Unrelated histories → empty merge-base string; everything in head shows
    /// as Added against the empty tree.
    #[test]
    fn pr_diff_unrelated_histories_empty_merge_base() {
        let dir = init_scratch();
        let p = dir.path();

        std::fs::write(p.join("a.txt"), "a\n").expect("write");
        stage_paths(p, &["a.txt".into()]).expect("stage");
        let base = create_commit(p, "base", None, false).expect("commit").oid;

        // An orphan commit with no shared ancestry, created directly via git2.
        let repo = git2::Repository::open(p).expect("open");
        let head = {
            let mut idx = repo.index().expect("index");
            std::fs::write(p.join("b.txt"), "b\n").expect("write");
            idx.add_path(std::path::Path::new("b.txt")).expect("add");
            let tree_oid = idx.write_tree().expect("write tree");
            let tree = repo.find_tree(tree_oid).expect("tree");
            let sig = git2::Signature::now("T", "t@e.com").expect("sig");
            repo.commit(None, &sig, &sig, "orphan", &tree, &[])
                .expect("orphan commit")
                .to_string()
        };

        let stats = pr_diff_headers(p, &base, &head).expect("pr diff");
        assert_eq!(stats.merge_base_oid, "", "unrelated ⇒ empty merge-base");
        // vs empty tree, head's b.txt is Added.
        assert!(stats.files.iter().any(|f| f.path == "b.txt" && f.status == FileStatus::Added));

        let fd = pr_file_diff(p, "", &head, "b.txt", None, false, false).expect("file diff");
        assert_eq!(fd.path, "b.txt");
    }

    // ---- ground-truth integration tests (vs the real `git` CLI) ----

    /// Run `git <args>` in `dir` and return trimmed stdout; panics on failure.
    fn git_out(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim_end().to_string()
    }

    /// Sum numeric additions/deletions from `git diff --numstat` (binary rows are
    /// "-\t-\t…" and contribute 0, matching the engine).
    fn numstat_totals(numstat: &str) -> (u32, u32) {
        let mut adds = 0u32;
        let mut dels = 0u32;
        for line in numstat.lines() {
            let mut f = line.split('\t');
            let a = f.next().unwrap_or("-");
            let d = f.next().unwrap_or("-");
            adds += a.parse::<u32>().unwrap_or(0);
            dels += d.parse::<u32>().unwrap_or(0);
        }
        (adds, dels)
    }

    /// KEY CORRECTNESS PROPERTY: `pr_diff_headers` matches `git diff base...head`
    /// (three-dot) exactly — additions/deletions/changed-files AND the file set —
    /// on a history where base advanced past the fork. A two-dot implementation
    /// would wrongly fold in the base-side changes and FAIL this test.
    #[test]
    fn pr_diff_matches_git_three_dot_ground_truth() {
        let dir = init_scratch();
        let p = dir.path();

        // O: base commit with several tracked files.
        std::fs::write(p.join("common.txt"), "line1\nline2\nline3\n").expect("w");
        std::fs::write(p.join("oldname.txt"), "rename me\nkeep\n").expect("w");
        std::fs::write(p.join("todelete.txt"), "bye\n").expect("w");
        stage_paths(
            p,
            &["common.txt".into(), "oldname.txt".into(), "todelete.txt".into()],
        )
        .expect("stage");
        let o = create_commit(p, "O", None, false).expect("commit O");
        let main_name = o.branch.expect("base branch");

        // feat forks at O: modify common, add a file, rename oldname, delete a file.
        create_branch(p, "feat").expect("create feat");
        checkout_branch(p, "feat").expect("checkout feat");
        std::fs::write(p.join("common.txt"), "line1\nCHANGED\nline3\nline4\n").expect("w");
        std::fs::write(p.join("file_feat.txt"), "brand new\n").expect("w");
        std::fs::rename(p.join("oldname.txt"), p.join("newname.txt")).expect("rename");
        std::fs::remove_file(p.join("todelete.txt")).expect("rm");
        // git add -A picks up the rename/delete/add/modify set.
        git_out(p, &["add", "-A"]);
        let head = create_commit(p, "H", None, false).expect("commit H").oid;

        // base advances PAST the fork — these must NOT appear in base...head.
        checkout_branch(p, &main_name).expect("checkout base");
        std::fs::write(p.join("common.txt"), "line1\nline2\nline3\nBASE_ONLY\n").expect("w");
        std::fs::write(p.join("file_base.txt"), "base side\n").expect("w");
        stage_paths(p, &["common.txt".into(), "file_base.txt".into()]).expect("stage");
        let base = create_commit(p, "B", None, false).expect("commit B").oid;

        // Ground truth from the git CLI (three-dot, with rename detection).
        let numstat = git_out(p, &["diff", "--numstat", "-M", &format!("{base}...{head}")]);
        let name_status = git_out(p, &["diff", "--name-status", "-M", &format!("{base}...{head}")]);
        let (git_adds, git_dels) = numstat_totals(&numstat);
        // name-status renders renames as `R100\told\tnew` (tab-separated), so the
        // last tab field is always the NEW path git attributes the change to.
        let git_files: std::collections::BTreeSet<&str> = name_status
            .lines()
            .map(|l| l.rsplit('\t').next().unwrap_or(""))
            .collect();

        // Engine.
        let stats = pr_diff_headers(p, &base, &head).expect("pr diff");
        assert_eq!(stats.merge_base_oid, o.oid, "merge-base is the fork point O");
        assert_eq!(stats.additions, git_adds, "additions vs git three-dot\n{numstat}");
        assert_eq!(stats.deletions, git_dels, "deletions vs git three-dot\n{numstat}");
        assert_eq!(
            stats.changed_files as usize,
            git_files.len(),
            "changed-files count vs git three-dot\n{name_status}"
        );

        // File set: engine reports new paths; must equal git's three-dot set and
        // must EXCLUDE base-side files (file_base.txt, base-only common change kept
        // via the same path but its content matches head only through merge-base).
        let engine_files: std::collections::BTreeSet<&str> =
            stats.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(engine_files, git_files, "file set mismatch");
        assert!(
            !engine_files.contains("file_base.txt"),
            "base-side file leaked into three-dot diff: {engine_files:?}"
        );
        assert!(engine_files.contains("newname.txt"), "rename target present");
        assert!(engine_files.contains("file_feat.txt"));

        // Rename is detected as Renamed with the original path preserved.
        let renamed = stats
            .files
            .iter()
            .find(|f| f.path == "newname.txt")
            .expect("renamed entry");
        assert_eq!(renamed.status, FileStatus::Renamed, "{name_status}");
        assert_eq!(renamed.orig_path.as_deref(), Some("oldname.txt"));

        // Deleted / added statuses.
        assert_eq!(
            stats.files.iter().find(|f| f.path == "todelete.txt").unwrap().status,
            FileStatus::Deleted
        );
        assert_eq!(
            stats.files.iter().find(|f| f.path == "file_feat.txt").unwrap().status,
            FileStatus::Added
        );

        // Per-file hunks for the modified file resolve against the merge-base.
        let fd = pr_file_diff(p, &stats.merge_base_oid, &head, "common.txt", None, false, false)
            .expect("file diff");
        assert_eq!(fd.path, "common.txt");
        assert!(!fd.hunks.is_empty(), "modified file has hunks");
    }

    /// A binary file added on the head branch is flagged binary with 0/0 line
    /// counts, matching git's "-" numstat rows.
    #[test]
    fn pr_diff_binary_file_is_flagged() {
        let dir = init_scratch();
        let p = dir.path();

        std::fs::write(p.join("seed.txt"), "seed\n").expect("w");
        stage_paths(p, &["seed.txt".into()]).expect("stage");
        let o = create_commit(p, "O", None, false).expect("commit O");
        let main_name = o.branch.expect("branch");

        create_branch(p, "feat").expect("create feat");
        checkout_branch(p, "feat").expect("checkout feat");
        // NUL bytes ⇒ git + libgit2 treat as binary.
        std::fs::write(p.join("blob.bin"), [0u8, 1, 2, 0, 255, 0, 42]).expect("w");
        stage_paths(p, &["blob.bin".into()]).expect("stage");
        let head = create_commit(p, "H", None, false).expect("commit H").oid;

        checkout_branch(p, &main_name).expect("checkout base");
        let base = o.oid.clone();

        let stats = pr_diff_headers(p, &base, &head).expect("pr diff");
        let bin = stats
            .files
            .iter()
            .find(|f| f.path == "blob.bin")
            .expect("binary entry");
        assert!(bin.binary, "binary flag set");
        assert_eq!(bin.additions, 0);
        assert_eq!(bin.deletions, 0);
        assert_eq!(bin.status, FileStatus::Added);

        // git agrees it is binary ("-\t-\tblob.bin").
        let numstat = git_out(p, &["diff", "--numstat", &format!("{base}...{head}")]);
        assert!(
            numstat.lines().any(|l| l.starts_with("-\t-\t") && l.ends_with("blob.bin")),
            "git numstat binary row: {numstat}"
        );
    }

    /// A bad head oid maps to `AppError::Git`.
    #[test]
    fn pr_diff_bad_oid_errors() {
        let dir = init_scratch();
        let p = dir.path();
        std::fs::write(p.join("a.txt"), "a\n").expect("write");
        stage_paths(p, &["a.txt".into()]).expect("stage");
        let base = create_commit(p, "base", None, false).expect("commit").oid;

        let err = pr_diff_headers(p, &base, "notahexoid").expect_err("bad oid");
        assert!(matches!(err, AppError::Git(_)), "got {err:?}");
    }
