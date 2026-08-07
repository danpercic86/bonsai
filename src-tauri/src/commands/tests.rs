    use super::*;

    const MISSING_ID: &str = "missing";

    fn path_string(p: &std::path::Path) -> String {
        p.to_string_lossy().into_owned()
    }

    /// Opens `path` runtime-free with a no-op watcher factory (P3e contract
    /// §9.1: `open_repo_inner(state, path, |_id| Box::new(|| {}))`).
    fn open(state: &AppState, path: &std::path::Path) -> Result<OpenRepoResult, AppError> {
        tauri::async_runtime::block_on(open_repo_inner(
            state,
            path_string(path),
            |_id| Box::new(|| {}),
        ))
    }

    /// git2-init a repo with a committable identity; returns the temp dir.
    fn init_repo_with_identity() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        let mut cfg = repo.config().expect("open config");
        cfg.set_str("user.name", "Test User").expect("set user.name");
        cfg.set_str("user.email", "test@example.com")
            .expect("set user.email");
        dir
    }

    /// Writes `rel` under the workdir, stages it, and commits — via the command
    /// inners, so the whole round-trip is keyed by `repo_id`.
    fn write_stage_commit(
        state: &AppState,
        repo_id: &str,
        workdir: &std::path::Path,
        rel: &str,
        contents: &str,
        message: &str,
    ) -> CommitResult {
        std::fs::write(workdir.join(rel), contents).expect("write file");
        tauri::async_runtime::block_on(stage_inner(state, repo_id, vec![rel.to_string()]))
            .expect("stage");
        tauri::async_runtime::block_on(commit_inner(state, repo_id, message.to_string()))
            .expect("commit")
    }

    fn repo_count(state: &AppState) -> usize {
        state.repos.lock().expect("repos lock").len()
    }

    /// Opening a non-repo path inserts NO entry and touches no other open tab
    /// (P3e contract §4.2 — there is no single "current repo" to clear).
    #[test]
    fn failed_open_leaves_other_entries_untouched() {
        let state = AppState::default();

        // Open a real (empty, unborn-HEAD) repo first.
        let repo_dir = tempfile::TempDir::new().expect("create temp dir");
        git2::Repository::init(repo_dir.path()).expect("init repo");
        let a = open(&state, repo_dir.path()).expect("open repo A");
        assert!(a.info.is_repo && !a.info.bare);
        tauri::async_runtime::block_on(get_status_inner(&state, &a.repo_id))
            .expect("status of repo A");

        // Now open a plain directory: not a repo. No entry is created for it…
        let non_repo_dir = tempfile::TempDir::new().expect("create temp dir");
        let n = open(&state, non_repo_dir.path()).expect("open non-repo dir");
        assert!(!n.info.is_repo);
        let err = tauri::async_runtime::block_on(get_status_inner(&state, &n.repo_id))
            .expect_err("a non-repo id must not be open");
        assert!(matches!(err, AppError::NoRepo));

        // …and repo A is still open and usable.
        tauri::async_runtime::block_on(get_status_inner(&state, &a.repo_id))
            .expect("repo A still open after a failed open");
        assert_eq!(repo_count(&state), 1);
    }

    /// `get_repo_health` errors only for an unknown id (`NoRepo`, P29 §D4);
    /// on an open repo it resolves with all four sections carrying data —
    /// section-level failures never reject the command.
    #[test]
    fn get_repo_health_requires_open_repo() {
        let state = AppState::default();
        let err = tauri::async_runtime::block_on(get_repo_health_inner(&state, MISSING_ID))
            .expect_err("unknown id must be NoRepo");
        assert!(matches!(err, AppError::NoRepo));

        let dir = init_repo_with_identity();
        let opened = open(&state, dir.path()).expect("open repo");
        write_stage_commit(&state, &opened.repo_id, dir.path(), "a.txt", "a\n", "C0");
        let health =
            tauri::async_runtime::block_on(get_repo_health_inner(&state, &opened.repo_id))
                .expect("health never errors for an open repo");
        assert!(health.stats.data.is_some(), "{:?}", health.stats.error);
        assert!(health.branches.data.is_some(), "{:?}", health.branches.error);
        assert!(
            health.working_state.data.is_some(),
            "{:?}",
            health.working_state.error
        );
        assert!(health.structure.data.is_some(), "{:?}", health.structure.error);
        assert_eq!(
            health.stats.data.as_ref().map(|s| s.commit_count),
            Some(1)
        );
        assert!(health.generated_at > 0);
    }

    /// `get_graph` with an unknown id returns `NoRepo`; after opening an
    /// unborn-HEAD repo it returns an empty layout (not an error).
    #[test]
    fn get_graph_no_repo_and_unborn() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(get_graph_inner(&state, MISSING_ID))
            .expect_err("no repo open must be NoRepo");
        assert!(matches!(err, AppError::NoRepo));

        let repo_dir = tempfile::TempDir::new().expect("create temp dir");
        git2::Repository::init(repo_dir.path()).expect("init repo");
        let id = open(&state, repo_dir.path()).expect("open unborn repo").repo_id;

        let layout = tauri::async_runtime::block_on(get_graph_inner(&state, &id))
            .expect("empty layout for unborn repo");
        assert!(layout.nodes.is_empty());
        assert_eq!(layout.head_index, None);
    }

    /// Bare repos are reported but not kept open; other entries are untouched.
    #[test]
    fn bare_open_leaves_other_entries_untouched() {
        let state = AppState::default();

        let repo_dir = tempfile::TempDir::new().expect("create temp dir");
        git2::Repository::init(repo_dir.path()).expect("init repo");
        let a = open(&state, repo_dir.path()).expect("open repo A");

        let bare_dir = tempfile::TempDir::new().expect("create temp dir");
        git2::Repository::init_bare(bare_dir.path()).expect("init bare repo");
        let b = open(&state, bare_dir.path()).expect("open bare repo");
        assert!(b.info.is_repo && b.info.bare);

        let err = tauri::async_runtime::block_on(get_status_inner(&state, &b.repo_id))
            .expect_err("bare repo must not be open");
        assert!(matches!(err, AppError::NoRepo));

        tauri::async_runtime::block_on(get_status_inner(&state, &a.repo_id))
            .expect("repo A still open after opening a bare repo");
        assert_eq!(repo_count(&state), 1);
    }

    /// The M3 mutation commands all return `NoRepo` for an unknown id
    /// (empty map + dummy id).
    #[test]
    fn mutation_commands_require_an_open_repo() {
        let state = AppState::default();
        let paths = vec!["file.txt".to_string()];

        let err = tauri::async_runtime::block_on(stage_inner(&state, MISSING_ID, paths.clone()))
            .expect_err("stage with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(unstage_inner(&state, MISSING_ID, paths))
            .expect_err("unstage with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err =
            tauri::async_runtime::block_on(commit_inner(&state, MISSING_ID, "msg".to_string()))
                .expect_err("commit with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P22 tag commands all return `NoRepo` for an unknown id (§8.4).
    #[test]
    fn tag_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(create_tag_inner(
            &state,
            MISSING_ID,
            "v1".to_string(),
            "0".repeat(40),
            None,
            false,
        ))
        .expect_err("create_tag with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(delete_tag_inner(
            &state,
            MISSING_ID,
            "v1".to_string(),
        ))
        .expect_err("delete_tag with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(push_tag_inner(
            &state,
            MISSING_ID,
            "origin".to_string(),
            "v1".to_string(),
            false,
        ))
        .expect_err("push_tag with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P22 remote-management commands all return `NoRepo` for an unknown
    /// id (§8.4).
    #[test]
    fn remote_mgmt_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(list_remotes_inner(&state, MISSING_ID))
            .expect_err("list_remotes with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(add_remote_inner(
            &state,
            MISSING_ID,
            "backup".to_string(),
            "https://example.com/repo.git".to_string(),
        ))
        .expect_err("add_remote with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(remove_remote_inner(
            &state,
            MISSING_ID,
            "origin".to_string(),
        ))
        .expect_err("remove_remote with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(rename_remote_inner(
            &state,
            MISSING_ID,
            "origin".to_string(),
            "upstream".to_string(),
        ))
        .expect_err("rename_remote with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(set_remote_url_inner(
            &state,
            MISSING_ID,
            "origin".to_string(),
            "https://example.com/other.git".to_string(),
        ))
        .expect_err("set_remote_url with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The M4 diff commands all return `NoRepo` for an unknown id
    /// (contract §6.2 scenario 17).
    #[test]
    fn diff_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(get_workdir_file_diff_inner(
            &state,
            MISSING_ID,
            "file.txt".to_string(),
            None,
            false,
            false,
        ))
        .expect_err("get_workdir_file_diff with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let oid = "0123456789abcdef0123456789abcdef01234567".to_string();
        let err =
            tauri::async_runtime::block_on(get_commit_diff_inner(&state, MISSING_ID, oid.clone()))
                .expect_err("get_commit_diff with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(get_commit_file_diff_inner(
            &state,
            MISSING_ID,
            oid,
            "file.txt".to_string(),
            None,
            false,
        ))
        .expect_err("get_commit_file_diff with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P23c blame + file-history commands return `NoRepo` for an unknown id.
    #[test]
    fn blame_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(blame_file_inner(
            &state,
            MISSING_ID,
            "src/app.ts".to_string(),
            None,
        ))
        .expect_err("blame_file with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(file_history_inner(
            &state,
            MISSING_ID,
            "src/app.ts".to_string(),
            200,
        ))
        .expect_err("file_history with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P38 reflog command returns `NoRepo` for an unknown id.
    #[test]
    fn read_reflog_requires_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(read_reflog_inner(
            &state,
            MISSING_ID,
            "HEAD".to_string(),
        ))
        .expect_err("read_reflog with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P40 config commands return `NoRepo` for an unknown id (the gate is
    /// `repo_path` before any git2 — never touches global config).
    #[test]
    fn config_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(get_config_inner(
            &state,
            MISSING_ID,
            ConfigLevelArg::Local,
        ))
        .expect_err("get_config with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(set_config_inner(
            &state,
            MISSING_ID,
            ConfigLevelArg::Global,
            "user.name".to_string(),
            "Nobody".to_string(),
        ))
        .expect_err("set_config with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(unset_config_inner(
            &state,
            MISSING_ID,
            ConfigLevelArg::Global,
            "user.name".to_string(),
        ))
        .expect_err("unset_config with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P44 `apply_identity_profile` command returns `NoRepo` for an unknown
    /// id. Its only pre-`spawn_blocking` gate is `repo_path` (it has no `_inner`
    /// split, and the tauri "test" feature is avoided on this machine — see
    /// `config_commands_require_an_open_repo`), so the NoRepo path is exercised
    /// at that gate. The gate fails before any identity field reaches git2, so
    /// global config stays untouched even for an unknown repo.
    #[test]
    fn apply_identity_profile_unknown_repo_errors() {
        let state = AppState::default();
        let err = repo_path(&state, MISSING_ID).expect_err("apply with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P17 partial-staging commands return `NoRepo` for an unknown id
    /// (contract §6.2 scenario 15) — the gate is `repo_path` before any git2.
    #[test]
    fn partial_staging_commands_require_an_open_repo() {
        let state = AppState::default();
        let selection = vec![LineSelection {
            kind: bonsai_core::git::diff::LineKind::Add,
            old_no: None,
            new_no: Some(1),
        }];

        let err = tauri::async_runtime::block_on(stage_partial_inner(
            &state,
            MISSING_ID,
            "file.txt".to_string(),
            None,
            selection.clone(),
        ))
        .expect_err("stage_partial with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(unstage_partial_inner(
            &state,
            MISSING_ID,
            "file.txt".to_string(),
            None,
            selection,
        ))
        .expect_err("unstage_partial with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P28 partial-discard command returns `NoRepo` for an unknown id —
    /// the gate is `repo_path` before any git2 work.
    #[test]
    fn discard_partial_command_requires_an_open_repo() {
        let state = AppState::default();
        let selection = vec![LineSelection {
            kind: bonsai_core::git::diff::LineKind::Add,
            old_no: None,
            new_no: Some(1),
        }];
        let err = tauri::async_runtime::block_on(discard_partial_inner(
            &state,
            MISSING_ID,
            "file.txt".to_string(),
            None,
            selection,
        ))
        .expect_err("discard_partial with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P5 compare commands also return `NoRepo` for an unknown id
    /// (contract §6.2).
    #[test]
    fn compare_commands_require_an_open_repo() {
        let state = AppState::default();
        let oid = "0123456789abcdef0123456789abcdef01234567".to_string();

        let err = tauri::async_runtime::block_on(compare_with_head_inner(
            &state,
            MISSING_ID,
            oid.clone(),
        ))
        .expect_err("compare_with_head with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(compare_with_head_file_diff_inner(
            &state,
            MISSING_ID,
            oid,
            "file.txt".to_string(),
            None,
            false,
        ))
        .expect_err("compare_with_head_file_diff with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The M5 branch commands all return `NoRepo` for an unknown id
    /// (contract §6.5).
    #[test]
    fn branch_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(list_branches_inner(&state, MISSING_ID))
            .expect_err("list_branches with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(create_branch_inner(
            &state,
            MISSING_ID,
            "topic".to_string(),
        ))
        .expect_err("create_branch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(checkout_branch_inner(
            &state,
            MISSING_ID,
            "topic".to_string(),
        ))
        .expect_err("checkout_branch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(delete_branch_inner(
            &state,
            MISSING_ID,
            "topic".to_string(),
        ))
        .expect_err("delete_branch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(checkout_remote_inner(
            &state,
            MISSING_ID,
            "origin/topic".to_string(),
        ))
        .expect_err("checkout_remote with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(delete_remote_tracking_inner(
            &state,
            MISSING_ID,
            "origin/topic".to_string(),
        ))
        .expect_err("delete_remote_tracking with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The M6 remote commands all return `NoRepo` for an unknown id
    /// (contract §6.7).
    #[test]
    fn remote_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(fetch_inner(&state, MISSING_ID))
            .expect_err("fetch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(pull_inner(&state, MISSING_ID))
            .expect_err("pull with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(push_inner(&state, MISSING_ID))
            .expect_err("push with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(force_push_inner(&state, MISSING_ID))
            .expect_err("force_push with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P3c merge/conflict commands all return `NoRepo` for an unknown id
    /// (contract §6).
    #[test]
    fn merge_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(get_op_state_inner(&state, MISSING_ID))
            .expect_err("get_op_state with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(merge_branch_inner(
            &state,
            MISSING_ID,
            "topic".to_string(),
        ))
        .expect_err("merge_branch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(commit_merge_inner(
            &state,
            MISSING_ID,
            "msg".to_string(),
        ))
        .expect_err("commit_merge with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(abort_merge_inner(&state, MISSING_ID))
            .expect_err("abort_merge with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(list_conflicts_inner(&state, MISSING_ID))
            .expect_err("list_conflicts with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(get_conflict_inner(
            &state,
            MISSING_ID,
            "file.txt".to_string(),
        ))
        .expect_err("get_conflict with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(resolve_conflict_inner(
            &state,
            MISSING_ID,
            "file.txt".to_string(),
            ConflictResolution::Ours,
        ))
        .expect_err("resolve_conflict with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(resolve_conflict_text_inner(
            &state,
            MISSING_ID,
            "file.txt".to_string(),
            "resolved\n".to_string(),
        ))
        .expect_err("resolve_conflict_text with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P3d rebase commands all return `NoRepo` for an unknown id
    /// (contract §4).
    #[test]
    fn rebase_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(rebase_branch_inner(
            &state,
            MISSING_ID,
            "main".to_string(),
        ))
        .expect_err("rebase_branch with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(rebase_continue_inner(&state, MISSING_ID))
            .expect_err("rebase_continue with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(rebase_skip_inner(&state, MISSING_ID))
            .expect_err("rebase_skip with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(rebase_abort_inner(&state, MISSING_ID))
            .expect_err("rebase_abort with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P23 interactive-rebase commands return `NoRepo` for an unknown id.
    #[test]
    fn interactive_rebase_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(get_interactive_plan_inner(
            &state,
            MISSING_ID,
            "a".repeat(40),
        ))
        .expect_err("get_interactive_plan with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(start_interactive_rebase_inner(
            &state,
            MISSING_ID,
            "a".repeat(40),
            Vec::new(),
        ))
        .expect_err("start_interactive_rebase with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    /// The P39 bisect commands return `NoRepo` for an unknown id.
    #[test]
    fn bisect_commands_require_an_open_repo() {
        let state = AppState::default();

        let err = tauri::async_runtime::block_on(start_bisect_inner(
            &state,
            MISSING_ID,
            "a".repeat(40),
            vec!["b".repeat(40)],
        ))
        .expect_err("start_bisect with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(bisect_mark_inner(&state, MISSING_ID, true))
            .expect_err("bisect_mark with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(bisect_skip_inner(&state, MISSING_ID))
            .expect_err("bisect_skip with no repo");
        assert!(matches!(err, AppError::NoRepo));

        let err = tauri::async_runtime::block_on(bisect_reset_inner(&state, MISSING_ID))
            .expect_err("bisect_reset with no repo");
        assert!(matches!(err, AppError::NoRepo));
    }

    // ---- P3e-a two-repo isolation (contract §9.1) --------------------------

    /// Committing in A leaves B's status/graph unaffected and A reflects the
    /// change.
    #[test]
    fn isolation_independent_status_and_commit() {
        let state = AppState::default();

        let dir_a = init_repo_with_identity();
        let dir_b = init_repo_with_identity();
        let id_a = open(&state, dir_a.path()).expect("open A").repo_id;
        let id_b = open(&state, dir_b.path()).expect("open B").repo_id;
        assert_ne!(id_a, id_b);

        write_stage_commit(&state, &id_a, dir_a.path(), "a.txt", "hello", "first in A");

        // A now has one commit; its status is clean.
        let graph_a = tauri::async_runtime::block_on(get_graph_inner(&state, &id_a))
            .expect("graph A");
        assert_eq!(graph_a.nodes.len(), 1, "A should have exactly one commit");
        let status_a = tauri::async_runtime::block_on(get_status_inner(&state, &id_a))
            .expect("status A");
        assert!(status_a.staged.is_empty() && status_a.unstaged.is_empty());

        // B is untouched: still unborn, empty graph, no files.
        let graph_b = tauri::async_runtime::block_on(get_graph_inner(&state, &id_b))
            .expect("graph B");
        assert!(graph_b.nodes.is_empty(), "B must be unaffected by a commit in A");
        let status_b = tauri::async_runtime::block_on(get_status_inner(&state, &id_b))
            .expect("status B");
        assert!(
            status_b.staged.is_empty()
                && status_b.unstaged.is_empty()
                && status_b.untracked.is_empty(),
            "B working dir must be empty"
        );
    }

    /// A branch created in A does not appear in B; B's op-state stays `None`.
    #[test]
    fn isolation_independent_branches_and_op_state() {
        let state = AppState::default();

        let dir_a = init_repo_with_identity();
        let dir_b = init_repo_with_identity();
        let id_a = open(&state, dir_a.path()).expect("open A").repo_id;
        let id_b = open(&state, dir_b.path()).expect("open B").repo_id;

        // Need a commit before a branch can be created at HEAD.
        write_stage_commit(&state, &id_a, dir_a.path(), "a.txt", "hello", "first in A");
        tauri::async_runtime::block_on(create_branch_inner(&state, &id_a, "x".to_string()))
            .expect("create branch x in A");

        let branches_a = tauri::async_runtime::block_on(list_branches_inner(&state, &id_a))
            .expect("branches A");
        assert!(
            branches_a.local.iter().any(|b| b.name == "x"),
            "A must have branch x"
        );

        let branches_b = tauri::async_runtime::block_on(list_branches_inner(&state, &id_b))
            .expect("branches B");
        assert!(
            !branches_b.local.iter().any(|b| b.name == "x"),
            "B must NOT have branch x"
        );

        let op_b = tauri::async_runtime::block_on(get_op_state_inner(&state, &id_b))
            .expect("op-state B");
        assert_eq!(op_b, RepoOpState::None, "B op-state must stay None");
    }

    /// Closing A makes A's commands `NoRepo` while B keeps working; the map
    /// then holds exactly one entry.
    #[test]
    fn isolation_close_only_affects_target() {
        let state = AppState::default();

        let dir_a = init_repo_with_identity();
        let dir_b = init_repo_with_identity();
        let id_a = open(&state, dir_a.path()).expect("open A").repo_id;
        let id_b = open(&state, dir_b.path()).expect("open B").repo_id;
        assert_eq!(repo_count(&state), 2);

        tauri::async_runtime::block_on(close_repo_inner(&state, &id_a)).expect("close A");

        let err = tauri::async_runtime::block_on(get_status_inner(&state, &id_a))
            .expect_err("A must be closed");
        assert!(matches!(err, AppError::NoRepo));

        tauri::async_runtime::block_on(get_status_inner(&state, &id_b)).expect("B still open");
        assert_eq!(repo_count(&state), 1, "exactly one entry after closing A");
    }

    /// Opening A's path twice (including a case-variant) focuses the same
    /// entry: same `repo_id`, one map entry.
    #[test]
    fn isolation_focus_dedupe_on_reopen() {
        let state = AppState::default();

        let dir_a = init_repo_with_identity();
        let first = open(&state, dir_a.path()).expect("open A").repo_id;
        assert_eq!(repo_count(&state), 1);

        // Re-open the exact same path.
        let again = open(&state, dir_a.path()).expect("re-open A").repo_id;
        assert_eq!(first, again, "re-opening the same path must reuse the id");
        assert_eq!(repo_count(&state), 1, "no duplicate entry on re-open");

        // Re-open via an ASCII case-variant of the path (Windows is
        // case-insensitive): still the same entry.
        let variant = path_string(dir_a.path()).to_uppercase();
        let cased = tauri::async_runtime::block_on(open_repo_inner(
            &state,
            variant,
            |_id| Box::new(|| {}),
        ))
        .expect("re-open A (case-variant)")
        .repo_id;
        assert_eq!(
            first, cased,
            "a case-variant path must dedupe to the same id"
        );
        assert_eq!(repo_count(&state), 1, "case-variant must not add an entry");
    }

    /// Closing an unknown id is a no-op `Ok(())` (idempotent).
    #[test]
    fn isolation_idempotent_close_of_unknown_id() {
        let state = AppState::default();
        tauri::async_runtime::block_on(close_repo_inner(&state, "does-not-exist"))
            .expect("closing an unknown id must be Ok(())");
        assert_eq!(repo_count(&state), 0);
    }

    /// Drives the repo `id` (workdir `dir`) into a PAUSED merge with a conflict
    /// on `a.txt`, entirely through the command inners + git2 checkout, and
    /// returns the base branch name. Post-condition: `merge_branch_inner`
    /// returned `Conflicts` and the repo is in `RepoOpState::Merge`.
    ///
    /// Recipe mirrors `merge_cli::script_conflict`: same middle line edited on
    /// both the base branch and `topic`, so the true merge is guaranteed to
    /// conflict.
    fn start_conflicting_merge(state: &AppState, id: &str, dir: &std::path::Path) -> String {
        // Base commit on the default branch.
        write_stage_commit(state, id, dir, "a.txt", "line1\nbase\nline3\n", "base");
        let base_branch = tauri::async_runtime::block_on(list_branches_inner(state, id))
            .expect("branches after base commit")
            .head
            .branch_name
            .expect("HEAD has a branch name after the first commit");

        // topic diverges: edits the middle line differently.
        tauri::async_runtime::block_on(create_branch_inner(state, id, "topic".to_string()))
            .expect("create topic");
        tauri::async_runtime::block_on(checkout_branch_inner(state, id, "topic".to_string()))
            .expect("checkout topic");
        write_stage_commit(state, id, dir, "a.txt", "line1\ntopic\nline3\n", "topic change");

        // Back on the base branch: a conflicting edit to the same line.
        tauri::async_runtime::block_on(checkout_branch_inner(state, id, base_branch.clone()))
            .expect("checkout base branch");
        write_stage_commit(state, id, dir, "a.txt", "line1\nmain\nline3\n", "main change");

        // Merge topic → guaranteed conflict, repo pauses in Merge state.
        let outcome =
            tauri::async_runtime::block_on(merge_branch_inner(state, id, "topic".to_string()))
                .expect("merge_branch");
        match outcome {
            MergeOutcome::Conflicts { paths, .. } => {
                assert!(
                    paths.iter().any(|p| p == "a.txt"),
                    "expected a.txt to be conflicted, got {paths:?}"
                );
            }
            other => panic!("expected a conflicting merge, got {other:?}"),
        }
        base_branch
    }

    /// An in-progress MERGE in repo A must NOT leak into repo B: B's op-state
    /// stays `None`, and B's status/branches are untouched, while A genuinely
    /// reflects the paused merge. This strengthens
    /// `isolation_independent_branches_and_op_state` (whose op-state half was
    /// tautological — it never started an operation). Contract §9.1.
    #[test]
    fn isolation_in_progress_merge_does_not_leak() {
        let state = AppState::default();

        let dir_a = init_repo_with_identity();
        let dir_b = init_repo_with_identity();
        let id_a = open(&state, dir_a.path()).expect("open A").repo_id;
        let id_b = open(&state, dir_b.path()).expect("open B").repo_id;
        assert_ne!(id_a, id_b);

        // Give B a real (but independent) history so "unaffected" is a
        // meaningful assertion rather than "both are empty".
        write_stage_commit(&state, &id_b, dir_b.path(), "b.txt", "b-only\n", "b base");
        tauri::async_runtime::block_on(create_branch_inner(&state, &id_b, "keep".to_string()))
            .expect("create branch keep in B");

        // Snapshot B before the merge storm in A.
        let branches_b_before = tauri::async_runtime::block_on(list_branches_inner(&state, &id_b))
            .expect("branches B before");
        let status_b_before = tauri::async_runtime::block_on(get_status_inner(&state, &id_b))
            .expect("status B before");
        let graph_b_before = tauri::async_runtime::block_on(get_graph_inner(&state, &id_b))
            .expect("graph B before");

        // Now drive A into a paused merge.
        start_conflicting_merge(&state, &id_a, dir_a.path());

        // A genuinely reflects the in-progress merge.
        let op_a = tauri::async_runtime::block_on(get_op_state_inner(&state, &id_a))
            .expect("op-state A");
        assert!(
            matches!(op_a, RepoOpState::Merge { .. }),
            "A must be paused in a merge, got {op_a:?}"
        );
        let conflicts_a = tauri::async_runtime::block_on(list_conflicts_inner(&state, &id_a))
            .expect("conflicts A");
        assert!(
            conflicts_a.iter().any(|c| c.path == "a.txt"),
            "A must list a.txt as conflicted, got {conflicts_a:?}"
        );

        // B is entirely unaffected: op-state None, and its branches/status/graph
        // are byte-identical to the pre-merge snapshot.
        let op_b = tauri::async_runtime::block_on(get_op_state_inner(&state, &id_b))
            .expect("op-state B");
        assert_eq!(op_b, RepoOpState::None, "B op-state must stay None");

        let branches_b_after = tauri::async_runtime::block_on(list_branches_inner(&state, &id_b))
            .expect("branches B after");
        assert_eq!(
            branches_b_after.local.iter().map(|b| b.name.clone()).collect::<Vec<_>>(),
            branches_b_before.local.iter().map(|b| b.name.clone()).collect::<Vec<_>>(),
            "B's branch set must be unchanged"
        );
        assert!(
            !branches_b_after.local.iter().any(|b| b.name == "topic"),
            "B must not have gained A's 'topic' branch"
        );

        let status_b_after = tauri::async_runtime::block_on(get_status_inner(&state, &id_b))
            .expect("status B after");
        assert_eq!(
            (status_b_after.staged.len(), status_b_after.unstaged.len(), status_b_after.untracked.len()),
            (status_b_before.staged.len(), status_b_before.unstaged.len(), status_b_before.untracked.len()),
            "B's working-dir status must be unchanged"
        );

        let graph_b_after = tauri::async_runtime::block_on(get_graph_inner(&state, &id_b))
            .expect("graph B after");
        assert_eq!(
            graph_b_after.nodes.len(),
            graph_b_before.nodes.len(),
            "B's commit graph must be unchanged"
        );
    }

    /// Closing repo B while repo A has a PAUSED merge must not disturb A's
    /// on-disk operation: A's op-state is still `Merge` and its conflicts are
    /// still readable, and the map holds exactly A. Contract §9.1 (close edge).
    #[test]
    fn isolation_close_preserves_other_repos_in_progress_op() {
        let state = AppState::default();

        let dir_a = init_repo_with_identity();
        let dir_b = init_repo_with_identity();
        let id_a = open(&state, dir_a.path()).expect("open A").repo_id;
        let id_b = open(&state, dir_b.path()).expect("open B").repo_id;

        start_conflicting_merge(&state, &id_a, dir_a.path());

        // Close B while A is mid-merge.
        tauri::async_runtime::block_on(close_repo_inner(&state, &id_b)).expect("close B");
        assert_eq!(repo_count(&state), 1, "only A remains open");

        // A's in-progress merge survives the close of B, fully readable.
        let op_a = tauri::async_runtime::block_on(get_op_state_inner(&state, &id_a))
            .expect("op-state A after closing B");
        assert!(
            matches!(op_a, RepoOpState::Merge { .. }),
            "A must still be paused in a merge after B is closed, got {op_a:?}"
        );
        let conflicts_a = tauri::async_runtime::block_on(list_conflicts_inner(&state, &id_a))
            .expect("conflicts A after closing B");
        assert!(
            conflicts_a.iter().any(|c| c.path == "a.txt"),
            "A must still list a.txt as conflicted after closing B, got {conflicts_a:?}"
        );
    }

    /// Patching only `theme` leaves `pane_widths`/`list_view` untouched, and
    /// each other single-field patch is equally partial (P2a contract §3.4.3;
    /// P3b contract §2.1).
    #[test]
    fn set_ui_settings_patch_is_partial() {
        let mut s = settings::Settings::default();
        let original_widths = s.pane_widths;

        apply_patch(
            &mut s,
            UiSettingsPatch {
                theme: Some(ThemeChoice::Light),
                pane_widths: None,
                list_view: None,
                ..Default::default()
            },
        );
        assert_eq!(s.theme, ThemeChoice::Light);
        assert_eq!(s.pane_widths, original_widths);
        assert_eq!(s.list_view, settings::ListView::Tree);

        apply_patch(
            &mut s,
            UiSettingsPatch {
                theme: None,
                pane_widths: Some(PaneWidths {
                    sidebar: 300,
                    right_panel: 400,
                }),
                list_view: None,
                ..Default::default()
            },
        );
        assert_eq!(s.theme, ThemeChoice::Light); // untouched by the second patch
        assert_eq!(s.list_view, settings::ListView::Tree);
        assert_eq!(
            s.pane_widths,
            PaneWidths {
                sidebar: 300,
                right_panel: 400,
            }
        );

        // Patching only `list_view` leaves theme + pane widths untouched.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                theme: None,
                pane_widths: None,
                list_view: Some(settings::ListView::Flat),
                ..Default::default()
            },
        );
        assert_eq!(s.list_view, settings::ListView::Flat);
        assert_eq!(s.theme, ThemeChoice::Light);
        assert_eq!(
            s.pane_widths,
            PaneWidths {
                sidebar: 300,
                right_panel: 400,
            }
        );

        // Out-of-range pane widths in a patch get clamped on write.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                theme: None,
                pane_widths: Some(PaneWidths {
                    sidebar: 5,
                    right_panel: 5000,
                }),
                list_view: None,
                ..Default::default()
            },
        );
        assert_eq!(s.pane_widths.sidebar, settings::SIDEBAR_MIN);
        assert_eq!(s.pane_widths.right_panel, settings::RIGHT_PANEL_MAX);
    }

    /// `auto_fetch` and `graph` patch independently, leave the other fields
    /// unchanged when `None`, and are clamped on write (P11 §2.4).
    #[test]
    fn set_ui_settings_patch_auto_fetch_and_graph() {
        let mut s = settings::Settings::default();
        let original_af = s.auto_fetch;
        let original_graph = s.graph;

        // Only `auto_fetch` changes auto-fetch; everything else untouched.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                auto_fetch: Some(AutoFetch {
                    enabled: true,
                    interval_minutes: 20,
                }),
                ..Default::default()
            },
        );
        assert_eq!(
            s.auto_fetch,
            AutoFetch {
                enabled: true,
                interval_minutes: 20,
            }
        );
        assert_eq!(s.graph, original_graph);
        assert_eq!(s.theme, ThemeChoice::default());

        // Only `graph` changes graph; auto-fetch preserved from the prior patch.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                graph: Some(GraphPrefs {
                    avatar_radius: 12,
                    row_height: 36,
                    lane_width: 20,
                    ..GraphPrefs::default()
                }),
                ..Default::default()
            },
        );
        assert_eq!(
            s.graph,
            GraphPrefs {
                avatar_radius: 12,
                row_height: 36,
                lane_width: 20,
                ..GraphPrefs::default()
            }
        );
        assert_eq!(
            s.auto_fetch,
            AutoFetch {
                enabled: true,
                interval_minutes: 20,
            }
        );

        // An empty patch leaves both new fields unchanged.
        apply_patch(&mut s, UiSettingsPatch::default());
        assert_eq!(
            s.auto_fetch,
            AutoFetch {
                enabled: true,
                interval_minutes: 20,
            }
        );
        assert_eq!(
            s.graph,
            GraphPrefs {
                avatar_radius: 12,
                row_height: 36,
                lane_width: 20,
                ..GraphPrefs::default()
            }
        );

        // Out-of-range interval (0) clamps to the min on write.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                auto_fetch: Some(AutoFetch {
                    enabled: true,
                    interval_minutes: 0,
                }),
                ..Default::default()
            },
        );
        assert_eq!(s.auto_fetch.interval_minutes, settings::AUTO_FETCH_INTERVAL_MIN);

        // Out-of-range interval (999) clamps to the max on write.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                auto_fetch: Some(AutoFetch {
                    enabled: false,
                    interval_minutes: 999,
                }),
                ..Default::default()
            },
        );
        assert_eq!(s.auto_fetch.interval_minutes, settings::AUTO_FETCH_INTERVAL_MAX);

        // Below-min / above-max graph knobs clamp to their bounds on write.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                graph: Some(GraphPrefs {
                    avatar_radius: 9999,
                    row_height: 0,
                    lane_width: 9999,
                    ..GraphPrefs::default()
                }),
                ..Default::default()
            },
        );
        assert_eq!(s.graph.avatar_radius, settings::AVATAR_RADIUS_MAX);
        assert_eq!(s.graph.row_height, settings::ROW_HEIGHT_MIN);
        assert_eq!(s.graph.lane_width, settings::LANE_WIDTH_MAX);

        // Sanity: the `original_*` snapshots were genuinely the defaults.
        assert_eq!(original_af, AutoFetch::default());
        assert_eq!(original_graph, GraphPrefs::default());
    }

    /// The three AI fields patch independently: patching only `ai_enabled`
    /// leaves autonomy + consent untouched (and vice versa), and an empty
    /// patch mutates nothing (P13 §4.2).
    #[test]
    fn set_ui_settings_patch_ai_is_partial() {
        let mut s = settings::Settings::default();
        // Defaults sanity: enabled true, ProposeReview, not consented.
        assert!(s.ai_enabled);
        assert_eq!(s.ai_conflict_autonomy, AiAutonomy::ProposeReview);
        assert!(!s.ai_consented);

        // Only `ai_enabled` changes; autonomy + consent untouched.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                ai_enabled: Some(false),
                ..Default::default()
            },
        );
        assert!(!s.ai_enabled);
        assert_eq!(s.ai_conflict_autonomy, AiAutonomy::ProposeReview);
        assert!(!s.ai_consented);
        // Unrelated fields untouched too.
        assert_eq!(s.theme, ThemeChoice::default());

        // Only `ai_consented` changes; enabled + autonomy preserved.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                ai_consented: Some(true),
                ..Default::default()
            },
        );
        assert!(s.ai_consented);
        assert!(!s.ai_enabled);
        assert_eq!(s.ai_conflict_autonomy, AiAutonomy::ProposeReview);

        // Only `ai_conflict_autonomy` changes; enabled + consent preserved.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                ai_conflict_autonomy: Some(AiAutonomy::AutoResolve),
                ..Default::default()
            },
        );
        assert_eq!(s.ai_conflict_autonomy, AiAutonomy::AutoResolve);
        assert!(!s.ai_enabled);
        assert!(s.ai_consented);

        // An empty patch leaves all three AI fields unchanged.
        apply_patch(&mut s, UiSettingsPatch::default());
        assert!(!s.ai_enabled);
        assert_eq!(s.ai_conflict_autonomy, AiAutonomy::AutoResolve);
        assert!(s.ai_consented);
    }

    /// `onboarding_seen` patches partially like every other field (P43 §6):
    /// the default is `false`; a `Some(true)` patch flips it while leaving
    /// unrelated fields untouched; and a subsequent empty patch (the common
    /// case where the frontend saves an unrelated pref) does NOT reset it back
    /// to `false` — pinning the "apply only when Some" property for the field
    /// the AI harness can't verify (the mock store resets per browser load).
    #[test]
    fn set_ui_settings_patch_onboarding_seen_is_partial() {
        let mut s = settings::Settings::default();
        // Default: onboarding not yet seen (⇒ show once).
        assert!(!s.onboarding_seen);

        // Only `onboarding_seen` changes; unrelated fields untouched.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                onboarding_seen: Some(true),
                ..Default::default()
            },
        );
        assert!(s.onboarding_seen);
        assert_eq!(s.theme, ThemeChoice::default());
        assert!(s.ai_enabled);

        // An empty patch (frontend saving some other pref) must NOT clear the
        // persisted flag — this is what keeps onboarding from reappearing.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                theme: Some(ThemeChoice::Light),
                ..Default::default()
            },
        );
        assert!(s.onboarding_seen);
        assert_eq!(s.theme, ThemeChoice::Light);

        // A totally empty patch is equally non-destructive.
        apply_patch(&mut s, UiSettingsPatch::default());
        assert!(s.onboarding_seen);
    }

    /// `auto_check_updates` (P42 D4/INV-4) patches partially like every other
    /// bool field: the default is `false`; a `Some(true)` patch flips it while
    /// leaving unrelated fields untouched; and a subsequent unrelated patch (or
    /// an empty one) does NOT reset it — pinning the "apply only when Some"
    /// property for the auto-check-on-launch flag the AI harness can't verify
    /// (the mock settings store resets per browser load). Mirrors
    /// `set_ui_settings_patch_onboarding_seen_is_partial`.
    #[test]
    fn set_ui_settings_patch_auto_check_updates_is_partial() {
        let mut s = settings::Settings::default();
        // Default: auto-check OFF (D4 — no surprise outbound call on launch).
        assert!(!s.auto_check_updates);

        // Only `auto_check_updates` changes; unrelated fields untouched.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                auto_check_updates: Some(true),
                ..Default::default()
            },
        );
        assert!(s.auto_check_updates);
        assert_eq!(s.theme, ThemeChoice::default());
        assert!(!s.onboarding_seen);

        // An unrelated patch (frontend saving some other pref) must NOT clear
        // the persisted flag.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                theme: Some(ThemeChoice::Light),
                ..Default::default()
            },
        );
        assert!(s.auto_check_updates);
        assert_eq!(s.theme, ThemeChoice::Light);

        // A totally empty patch is equally non-destructive.
        apply_patch(&mut s, UiSettingsPatch::default());
        assert!(s.auto_check_updates);

        // And it can be explicitly turned back off via `Some(false)`.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                auto_check_updates: Some(false),
                ..Default::default()
            },
        );
        assert!(!s.auto_check_updates);
    }

    /// P49: `terminal_command`/`editor_command` patch independently — a `Some`
    /// overwrites, a `None` (including an empty/unrelated patch) leaves the
    /// stored value untouched, and `Some("")` explicitly resets to auto-detect.
    #[test]
    fn set_ui_settings_patch_external_commands_is_partial() {
        let mut s = settings::Settings::default();
        assert_eq!(s.terminal_command, "");
        assert_eq!(s.editor_command, "");

        // Only `terminal_command` changes; the editor + unrelated fields stay.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                terminal_command: Some("wt -d {path}".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(s.terminal_command, "wt -d {path}");
        assert_eq!(s.editor_command, "");
        assert_eq!(s.theme, ThemeChoice::default());

        // Only `editor_command` changes; the terminal value is preserved.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                editor_command: Some("code {path}".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(s.terminal_command, "wt -d {path}");
        assert_eq!(s.editor_command, "code {path}");

        // An unrelated patch does NOT clear either command.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                theme: Some(ThemeChoice::Light),
                ..Default::default()
            },
        );
        assert_eq!(s.terminal_command, "wt -d {path}");
        assert_eq!(s.editor_command, "code {path}");

        // An empty patch is equally non-destructive.
        apply_patch(&mut s, UiSettingsPatch::default());
        assert_eq!(s.terminal_command, "wt -d {path}");
        assert_eq!(s.editor_command, "code {path}");

        // `Some("")` explicitly resets a command back to auto-detect.
        apply_patch(
            &mut s,
            UiSettingsPatch {
                terminal_command: Some(String::new()),
                ..Default::default()
            },
        );
        assert_eq!(s.terminal_command, "");
        assert_eq!(s.editor_command, "code {path}");
    }

    /// P49 (reviewer gap): the shared `launch_inner` missing-path precheck
    /// returns `AppError::Io` — and launches **nothing** — when the target path
    /// no longer exists. `reveal_in_file_manager` is the runtime-free command (it
    /// takes neither an `AppHandle` nor state), so it drives the exact precheck
    /// (`commands/external.rs` `launch_inner`, the `!p.exists()` guard) directly.
    /// `open_in_terminal`/`open_in_editor` funnel through the *same* precheck but
    /// first need an `AppHandle` to resolve the settings template, so they cannot
    /// be driven runtime-free here (the tauri "test" feature is avoided on this
    /// machine — see `config_commands_require_an_open_repo`); the shared seam is
    /// proven via reveal. The `p.exists()` check is the first statement in the
    /// spawn_blocking body — before any `SpawnRunner` is constructed — so an `Io`
    /// result proves no file-manager / terminal / editor process was spawned.
    #[test]
    fn external_launch_rejects_missing_path_before_spawning() {
        // Parent dir exists, leaf never created ⇒ guaranteed-missing on every OS,
        // so the precheck short-circuits deterministically.
        let dir = tempfile::TempDir::new().expect("temp dir");
        let missing = dir.path().join("does-not-exist-p49");
        let missing_str = missing.to_string_lossy().into_owned();
        assert!(!missing.exists(), "precondition: the path must not exist");

        let err = tauri::async_runtime::block_on(reveal_in_file_manager(missing_str.clone()))
            .expect_err("a nonexistent path must be rejected by the precheck");
        assert!(
            matches!(err, AppError::Io(_)),
            "missing path must surface as AppError::Io, got {err:?}"
        );
        // The precheck echoes the offending path — confirms this is *our* Io
        // guard (not some incidental filesystem error) and that we never spawned.
        assert!(
            err.to_string().contains(&missing_str),
            "the precheck error must name the offending path: {err}"
        );
    }

    /// `ai_resolve_conflict` enforces the backend consent gate (§9.6) BEFORE
    /// touching the repo: default settings (`ai_consented=false`) → `AiUnavailable`
    /// even with no repo open; once enabled+consented, an unknown repo id →
    /// `NoRepo` (the gate passed, `repo_path` then fails). Covers the
    /// AppHandle-free part of the command via its inner (P13 §6).
    #[test]
    fn ai_resolve_conflict_enforces_consent_gate_then_no_repo() {
        let state = AppState::default();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let file = dir.path().join("settings.json");

        // No settings file → defaults → not consented → the gate refuses.
        let err = tauri::async_runtime::block_on(ai_resolve_conflict_inner(
            &state,
            &file,
            MISSING_ID,
            "a.txt".to_string(),
        ))
        .expect_err("disabled gate must refuse");
        assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

        // P13 tester: the gate is `ai_enabled && ai_consented` — the OTHER OR-half.
        // Consented but DISABLED must still refuse (proves it is AND, not OR).
        let s = settings::Settings {
            ai_enabled: false,
            ai_consented: true,
            ..settings::Settings::default()
        };
        settings::save_to(&file, &s).expect("save settings");
        let err = tauri::async_runtime::block_on(ai_resolve_conflict_inner(
            &state,
            &file,
            MISSING_ID,
            "a.txt".to_string(),
        ))
        .expect_err("enabled=false must refuse even when consented");
        assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

        // Enable + consent; now the gate passes and the missing repo → NoRepo.
        let s = settings::Settings {
            ai_enabled: true,
            ai_consented: true,
            ..settings::Settings::default()
        };
        settings::save_to(&file, &s).expect("save settings");
        let err = tauri::async_runtime::block_on(ai_resolve_conflict_inner(
            &state,
            &file,
            MISSING_ID,
            "a.txt".to_string(),
        ))
        .expect_err("no repo open must be NoRepo");
        assert!(matches!(err, AppError::NoRepo), "got {err:?}");
    }

    /// P15a §8.5: `generate_commit_message` enforces the same backend consent
    /// gate BEFORE touching the repo: default settings (`ai_consented=false`) →
    /// `AiUnavailable`; once enabled+consented, an unknown repo id → `NoRepo`
    /// (the gate passed, `repo_path` then fails). No CLI needed.
    #[test]
    fn generate_commit_message_enforces_consent_gate_then_no_repo() {
        let state = AppState::default();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let file = dir.path().join("settings.json");

        // No settings file → defaults → not consented → the gate refuses.
        let err = tauri::async_runtime::block_on(generate_commit_message_inner(
            &state, &file, MISSING_ID,
        ))
        .expect_err("disabled gate must refuse");
        assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

        // Enable + consent; now the gate passes and the missing repo → NoRepo.
        let s = settings::Settings {
            ai_enabled: true,
            ai_consented: true,
            ..settings::Settings::default()
        };
        settings::save_to(&file, &s).expect("save settings");
        let err = tauri::async_runtime::block_on(generate_commit_message_inner(
            &state, &file, MISSING_ID,
        ))
        .expect_err("no repo open must be NoRepo");
        assert!(matches!(err, AppError::NoRepo), "got {err:?}");
    }

    /// P28 §5: `ai_digest` enforces the same backend consent gate BEFORE
    /// touching the repo: default settings (`ai_consented=false`) →
    /// `AiUnavailable`; once enabled+consented, an unknown repo id → `NoRepo`
    /// (the gate passed, `repo_path` then fails). No CLI needed.
    #[test]
    fn ai_digest_enforces_consent_gate_then_no_repo() {
        let state = AppState::default();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let file = dir.path().join("settings.json");
        let range = || AiDigestRange::LastDays { days: 7 };

        // No settings file → defaults → not consented → the gate refuses.
        let err =
            tauri::async_runtime::block_on(ai_digest_inner(&state, &file, MISSING_ID, range()))
                .expect_err("disabled gate must refuse");
        assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

        // Enable + consent; now the gate passes and the missing repo → NoRepo.
        let s = settings::Settings {
            ai_enabled: true,
            ai_consented: true,
            ..settings::Settings::default()
        };
        settings::save_to(&file, &s).expect("save settings");
        let err =
            tauri::async_runtime::block_on(ai_digest_inner(&state, &file, MISSING_ID, range()))
                .expect_err("no repo open must be NoRepo");
        assert!(matches!(err, AppError::NoRepo), "got {err:?}");
    }

    /// P15b §5/§8.5: `ai_analyze_diff` enforces the same backend consent gate
    /// BEFORE touching the repo: default settings (`ai_consented=false`) →
    /// `AiUnavailable`; once enabled+consented, an unknown repo id → `NoRepo`
    /// (the gate passed, `repo_path` then fails). No CLI needed.
    #[test]
    fn ai_analyze_diff_enforces_consent_gate_then_no_repo() {
        let state = AppState::default();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let file = dir.path().join("settings.json");

        // No settings file → defaults → not consented → the gate refuses.
        let err = tauri::async_runtime::block_on(ai_analyze_diff_inner(
            &state,
            &file,
            MISSING_ID,
            AiDiffTarget::Staged,
            AiAnalysisMode::Review,
        ))
        .expect_err("disabled gate must refuse");
        assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

        // Enable + consent; now the gate passes and the missing repo → NoRepo.
        let s = settings::Settings {
            ai_enabled: true,
            ai_consented: true,
            ..settings::Settings::default()
        };
        settings::save_to(&file, &s).expect("save settings");
        let err = tauri::async_runtime::block_on(ai_analyze_diff_inner(
            &state,
            &file,
            MISSING_ID,
            AiDiffTarget::Commit {
                oid: "0123456789abcdef0123456789abcdef01234567".to_string(),
            },
            AiAnalysisMode::Explain,
        ))
        .expect_err("no repo open must be NoRepo");
        assert!(matches!(err, AppError::NoRepo), "got {err:?}");
    }

    /// P15c §5/§8.5: `ai_summarize_range` enforces the same backend consent gate
    /// BEFORE touching the repo: default settings (`ai_consented=false`) →
    /// `AiUnavailable`; once enabled+consented, an unknown repo id → `NoRepo`
    /// (the gate passed, `repo_path` then fails). No CLI needed.
    #[test]
    fn ai_summarize_range_enforces_consent_gate_then_no_repo() {
        let state = AppState::default();
        let dir = tempfile::TempDir::new().expect("temp dir");
        let file = dir.path().join("settings.json");

        // No settings file → defaults → not consented → the gate refuses.
        let err = tauri::async_runtime::block_on(ai_summarize_range_inner(
            &state,
            &file,
            MISSING_ID,
            "main".to_string(),
            "feature".to_string(),
        ))
        .expect_err("disabled gate must refuse");
        assert!(matches!(err, AppError::AiUnavailable(_)), "got {err:?}");

        // Enable + consent; now the gate passes and the missing repo → NoRepo.
        let s = settings::Settings {
            ai_enabled: true,
            ai_consented: true,
            ..settings::Settings::default()
        };
        settings::save_to(&file, &s).expect("save settings");
        let err = tauri::async_runtime::block_on(ai_summarize_range_inner(
            &state,
            &file,
            MISSING_ID,
            "main".to_string(),
            "feature".to_string(),
        ))
        .expect_err("no repo open must be NoRepo");
        assert!(matches!(err, AppError::NoRepo), "got {err:?}");
    }

    /// P31 §5: the three worktree-context commands resolve the repo by id
    /// (`NoRepo` for unknown ids) and round-trip against the core: the matrix
    /// carries the `@main` row, preview is read-only, and activation writes
    /// the target file + records the activation.
    #[test]
    fn worktree_context_commands_round_trip() {
        let state = AppState::default();

        for res in [
            tauri::async_runtime::block_on(list_worktree_contexts_inner(&state, MISSING_ID))
                .map(|_| ()),
            tauri::async_runtime::block_on(preview_worktree_profile_inner(
                &state,
                MISSING_ID,
                "@main".to_string(),
                "p".to_string(),
            ))
            .map(|_| ()),
            tauri::async_runtime::block_on(activate_worktree_profile_inner(
                &state,
                MISSING_ID,
                "@main".to_string(),
                "p".to_string(),
            ))
            .map(|_| ()),
        ] {
            assert!(matches!(res.expect_err("unknown id"), AppError::NoRepo));
        }

        let dir = init_repo_with_identity();
        let opened = open(&state, dir.path()).expect("open repo");
        let id = &opened.repo_id;
        write_stage_commit(&state, id, dir.path(), "a.txt", "a\n", "C0");
        bonsai_core::assets::save_profile(
            dir.path(),
            bonsai_core::assets::ContextProfile {
                name: "p".to_string(),
                description: None,
                model: None,
                targets: vec![bonsai_core::assets::ProfileTarget {
                    asset_id: "claude".to_string(),
                    content: "# from command\n".to_string(),
                }],
            },
        )
        .expect("save profile");

        // Matrix: single main row, keyed "@main", activatable, no activation yet.
        let rows = tauri::async_runtime::block_on(list_worktree_contexts_inner(&state, id))
            .expect("matrix");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].worktree_key, "@main");
        assert!(rows[0].is_main && rows[0].activatable);
        assert_eq!(rows[0].active_profile, None);

        // Preview writes nothing.
        let preview = tauri::async_runtime::block_on(preview_worktree_profile_inner(
            &state,
            id,
            "@main".to_string(),
            "p".to_string(),
        ))
        .expect("preview");
        assert_eq!(preview.len(), 1);
        assert!(preview[0].changed);
        assert!(!dir.path().join("CLAUDE.md").exists());

        // Activate writes the target + records the "@main" activation.
        let act = tauri::async_runtime::block_on(activate_worktree_profile_inner(
            &state,
            id,
            "@main".to_string(),
            "p".to_string(),
        ))
        .expect("activate");
        assert_eq!(act.profile, "p");
        assert_eq!(
            std::fs::read(dir.path().join("CLAUDE.md")).expect("read CLAUDE.md"),
            b"# from command\n"
        );
        let rows = tauri::async_runtime::block_on(list_worktree_contexts_inner(&state, id))
            .expect("matrix after activation");
        assert_eq!(rows[0].active_profile.as_deref(), Some("p"));

        // Unknown worktree key surfaces the core's Git error.
        let err = tauri::async_runtime::block_on(activate_worktree_profile_inner(
            &state,
            id,
            "nope".to_string(),
            "p".to_string(),
        ))
        .expect_err("unknown worktree key");
        assert!(matches!(err, AppError::Git(m) if m.contains("not found")));
    }
