//! Analyze/wire tests for `ai_explain` (prompts, serde shapes, worktree gather,
//! commit-payload prefix, base resolution, byte-cap). Extracted verbatim from
//! the former inline `mod tests`; shared fixtures live in `test_support`.

use super::test_support::{commit_of, init_scratch};
use super::*;

/// The prompt/system-prompt consts MUST be single-line (Windows argv
/// constraint): a newline in any of them would make `claude.cmd` reject the
/// argument.
#[test]
fn prompts_are_single_line() {
    for s in [
        EXPLAIN_SYSTEM_PROMPT,
        REVIEW_SYSTEM_PROMPT,
        EXPLAIN_PROMPT,
        REVIEW_PROMPT,
        DIGEST_SYSTEM_PROMPT,
        DIGEST_PROMPT,
    ] {
        assert!(!s.contains('\n'), "prompt must be single-line: {s:?}");
        assert!(!s.contains('\r'), "prompt must be single-line: {s:?}");
    }
}

/// Serde casing must match the TS `AiAnalysis` type (`text` / `costUsd`);
/// `None` cost serializes as `null`.
#[test]
fn analysis_wire_shape_is_camel_case() {
    let v = serde_json::to_value(AiAnalysis {
        text: "does a thing".to_string(),
        cost_usd: Some(0.006),
    })
    .expect("json");
    assert_eq!(
        v,
        serde_json::json!({ "text": "does a thing", "costUsd": 0.006 })
    );

    let v = serde_json::to_value(AiAnalysis {
        text: "no cost".to_string(),
        cost_usd: None,
    })
    .expect("json");
    assert_eq!(v, serde_json::json!({ "text": "no cost", "costUsd": null }));
}

/// `AiDiffTarget` deserializes from the EXACT JSON the TS discriminated
/// union sends for each variant — locking the IPC contract without a CLI.
#[test]
fn diff_target_deserializes_each_variant() {
    let commit: AiDiffTarget =
        serde_json::from_str(r#"{"kind":"commit","oid":"deadbeef"}"#).expect("commit");
    match commit {
        AiDiffTarget::Commit { oid } => assert_eq!(oid, "deadbeef"),
        other => panic!("expected Commit, got {other:?}"),
    }

    let wf: AiDiffTarget = serde_json::from_str(
        r#"{"kind":"workdirFile","path":"src/a.rs","origPath":null,"staged":true}"#,
    )
    .expect("workdirFile");
    match wf {
        AiDiffTarget::WorkdirFile {
            path,
            orig_path,
            staged,
        } => {
            assert_eq!(path, "src/a.rs");
            assert_eq!(orig_path, None);
            assert!(staged);
        }
        other => panic!("expected WorkdirFile, got {other:?}"),
    }

    // origPath may also be a string, and (via #[serde(default)]) omitted.
    let wf_renamed: AiDiffTarget = serde_json::from_str(
        r#"{"kind":"workdirFile","path":"src/new.rs","origPath":"src/old.rs","staged":false}"#,
    )
    .expect("workdirFile renamed");
    match wf_renamed {
        AiDiffTarget::WorkdirFile {
            orig_path, staged, ..
        } => {
            assert_eq!(orig_path.as_deref(), Some("src/old.rs"));
            assert!(!staged);
        }
        other => panic!("expected WorkdirFile, got {other:?}"),
    }

    let staged: AiDiffTarget =
        serde_json::from_str(r#"{"kind":"staged"}"#).expect("staged");
    assert!(matches!(staged, AiDiffTarget::Staged));

    // P25 B1: the two new review scopes deserialize from the exact TS union.
    let worktree: AiDiffTarget =
        serde_json::from_str(r#"{"kind":"worktree"}"#).expect("worktree");
    assert!(matches!(worktree, AiDiffTarget::Worktree));

    // Branch: base may be null, omitted (via #[serde(default)]), or a string.
    let branch_null: AiDiffTarget =
        serde_json::from_str(r#"{"kind":"branch","name":"feature","base":null}"#)
            .expect("branch base null");
    match branch_null {
        AiDiffTarget::Branch { name, base } => {
            assert_eq!(name, "feature");
            assert_eq!(base, None);
        }
        other => panic!("expected Branch, got {other:?}"),
    }

    let branch_omitted: AiDiffTarget =
        serde_json::from_str(r#"{"kind":"branch","name":"feature"}"#)
            .expect("branch base omitted");
    match branch_omitted {
        AiDiffTarget::Branch { name, base } => {
            assert_eq!(name, "feature");
            assert_eq!(base, None);
        }
        other => panic!("expected Branch, got {other:?}"),
    }

    let branch_base: AiDiffTarget =
        serde_json::from_str(r#"{"kind":"branch","name":"f","base":"main"}"#)
            .expect("branch base string");
    match branch_base {
        AiDiffTarget::Branch { name, base } => {
            assert_eq!(name, "f");
            assert_eq!(base.as_deref(), Some("main"));
        }
        other => panic!("expected Branch, got {other:?}"),
    }
}

// ---- P25 B1 scratch-repo + cap tests -------------------------------------

use crate::git::commit::create_commit;
use crate::git::stage::stage_paths;
/// §9.1(3): `gather_worktree` covers staged + unstaged + untracked in one
/// pass; a clean worktree gathers empty (→ `AiFailed` in `analyze_diff`).
#[test]
fn gather_worktree_covers_all_change_kinds() {
    let dir = init_scratch();
    let p = dir.path();

    std::fs::write(p.join("staged.txt"), "s1\n").expect("write");
    std::fs::write(p.join("unstaged.txt"), "u1\n").expect("write");
    stage_paths(p, &["staged.txt".into(), "unstaged.txt".into()]).expect("stage");
    create_commit(p, "base", None, false).expect("commit");

    // Clean worktree => empty gather.
    assert!(
        gather_worktree(p).expect("gather clean").is_empty(),
        "a clean worktree must gather no files"
    );

    // Stage a modification to staged.txt.
    std::fs::write(p.join("staged.txt"), "s1 changed\n").expect("write");
    stage_paths(p, &["staged.txt".into()]).expect("stage");
    // Modify unstaged.txt WITHOUT staging.
    std::fs::write(p.join("unstaged.txt"), "u1 changed\n").expect("write");
    // Add an untracked file.
    std::fs::write(p.join("untracked.txt"), "new\n").expect("write");

    let files = gather_worktree(p).expect("gather dirty");
    let mut paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        vec!["staged.txt", "unstaged.txt", "untracked.txt"],
        "worktree gather must cover staged + unstaged + untracked"
    );
    assert!(has_analyzable_content(&files));
}

/// §7.10 (P53b/D2): the Commit-target grounding prefix now carries the FULL
/// commit MESSAGE (author intent = the strongest "why" signal), not just the
/// summary — inserted after AUTHOR and before the per-file blocks.
#[test]
fn commit_payload_prefix_carries_full_message() {
    let dir = init_scratch();
    let p = dir.path();

    std::fs::write(p.join("f.txt"), "hello\n").expect("write");
    stage_paths(p, &["f.txt".into()]).expect("stage");
    // Summary line + a body line whose text lives ONLY in the body — proving
    // the whole message body (not merely the summary) reaches the grounding.
    let msg = "Add greeting\n\nExplains WHY: users needed a friendly hello.";
    let oid = create_commit(p, msg, None, false).expect("commit").oid;

    let (prefix, files) =
        build_payload(p, &AiDiffTarget::Commit { oid }).expect("build payload");

    assert!(
        prefix.contains("MESSAGE:\n"),
        "prefix must carry a MESSAGE section: {prefix:?}"
    );
    assert!(
        prefix.contains("Explains WHY: users needed a friendly hello."),
        "prefix must carry the full message BODY, not just the summary: {prefix:?}"
    );
    // Ordering: COMMIT → AUTHOR → MESSAGE, before the (separately rendered)
    // per-file blocks.
    let author_idx = prefix.find("\nAUTHOR ").expect("AUTHOR line");
    let msg_idx = prefix.find("\nMESSAGE:").expect("MESSAGE line");
    assert!(msg_idx > author_idx, "MESSAGE must follow AUTHOR: {prefix:?}");
    assert!(!files.is_empty(), "the commit changed a file");
}

/// §9.1(4): `resolve_branch_base` — explicit base wins over everything.
#[test]
fn resolve_base_explicit_wins() {
    let dir = init_scratch();
    let p = dir.path();
    std::fs::write(p.join("a.txt"), "a\n").expect("write");
    stage_paths(p, &["a.txt".into()]).expect("stage");
    let head = create_commit(p, "A", None, false).expect("commit").oid;

    let repo = git2::Repository::open(p).expect("open");
    let head_commit = commit_of(&repo, &head);
    // A distinct base branch pointing at the same commit is enough to verify
    // the returned shorthand + commit come from the explicit ref.
    repo.branch("some-base", &head_commit, true).expect("branch");

    let (name, commit) = resolve_branch_base(&repo, "feature", Some("some-base")).expect("base");
    assert_eq!(name, "some-base");
    assert_eq!(commit.id().to_string(), head);
}

/// §9.1(4): configured upstream is used when no explicit base is given.
#[test]
fn resolve_base_uses_upstream() {
    let dir = init_scratch();
    let p = dir.path();
    std::fs::write(p.join("a.txt"), "a\n").expect("write");
    stage_paths(p, &["a.txt".into()]).expect("stage");
    let head = create_commit(p, "A", None, false).expect("commit").oid;

    let repo = git2::Repository::open(p).expect("open");
    let head_commit = commit_of(&repo, &head);
    repo.branch("feature", &head_commit, true).expect("local branch");
    repo.remote_with_fetch(
        "origin",
        "https://example.invalid/x.git",
        "+refs/heads/*:refs/remotes/origin/*",
    )
    .expect("remote");
    repo.reference(
        "refs/remotes/origin/feature",
        head_commit.id(),
        true,
        "test upstream",
    )
    .expect("remote-tracking ref");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("branch.feature.remote", "origin").expect("remote cfg");
        cfg.set_str("branch.feature.merge", "refs/heads/feature")
            .expect("merge cfg");
    }

    let (name, commit) = resolve_branch_base(&repo, "feature", None).expect("base");
    assert_eq!(name, "origin/feature");
    assert_eq!(commit.id().to_string(), head);
}

/// §9.1(4): with no upstream, `origin/HEAD`'s target is used (before local
/// `main`/`master`).
#[test]
fn resolve_base_uses_origin_head() {
    let dir = init_scratch();
    let p = dir.path();
    std::fs::write(p.join("a.txt"), "a\n").expect("write");
    stage_paths(p, &["a.txt".into()]).expect("stage");
    let head = create_commit(p, "A", None, false).expect("commit").oid;

    let repo = git2::Repository::open(p).expect("open");
    let head_commit = commit_of(&repo, &head);
    repo.reference("refs/remotes/origin/main", head_commit.id(), true, "test")
        .expect("origin/main");
    repo.reference_symbolic(
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/main",
        true,
        "test",
    )
    .expect("origin/HEAD symbolic");

    let (name, commit) = resolve_branch_base(&repo, "no-upstream", None).expect("base");
    assert_eq!(name, "origin/main");
    assert_eq!(commit.id().to_string(), head);
}

/// §9.1(4): with no upstream and no `origin/HEAD`, local `main` is preferred
/// over `master`; and with neither `main` nor any base, an error results.
#[test]
fn resolve_base_main_then_master_then_error() {
    let dir = init_scratch();
    let p = dir.path();
    std::fs::write(p.join("a.txt"), "a\n").expect("write");
    stage_paths(p, &["a.txt".into()]).expect("stage");
    let head = create_commit(p, "A", None, false).expect("commit").oid;

    let repo = git2::Repository::open(p).expect("open");
    let head_commit = commit_of(&repo, &head);

    // Ensure both main + master exist at head without force-updating whichever
    // one is the current HEAD (libgit2's default branch varies by host config).
    let head_name = repo
        .head()
        .expect("head")
        .shorthand()
        .expect("head shorthand")
        .to_string();
    for b in ["main", "master"] {
        if b != head_name {
            repo.branch(b, &head_commit, true).expect("branch");
        }
    }
    // main wins over master.
    let (name, _) = resolve_branch_base(&repo, "topic", None).expect("main base");
    assert_eq!(name, "main");

    // Move HEAD to a lone `topic`, then drop main → master wins.
    repo.branch("topic", &head_commit, true).expect("topic");
    repo.set_head("refs/heads/topic").expect("head->topic");
    repo.find_branch("main", git2::BranchType::Local)
        .expect("find main")
        .delete()
        .expect("delete main");
    let (name, _) = resolve_branch_base(&repo, "topic", None).expect("master base");
    assert_eq!(name, "master");

    // Drop master too → no base resolves.
    repo.find_branch("master", git2::BranchType::Local)
        .expect("find master")
        .delete()
        .expect("delete master");
    let err = resolve_branch_base(&repo, "topic", None).expect_err("no base");
    assert!(matches!(err, AppError::Git(_)), "got {err:?}");
}

/// §9.1(5): the byte-cap truncates an oversize payload with the marker, keeps
/// the result ≤ cap, and cuts on a valid char boundary.
#[test]
fn review_payload_byte_cap() {
    // Under the cap: returned unchanged.
    let small = "small payload".to_string();
    assert_eq!(cap_review_payload(small.clone()), small);

    // Multi-byte content well over the cap → truncated, still valid UTF-8.
    let big = "é".repeat(MAX_REVIEW_PAYLOAD_BYTES); // 2 bytes each
    let capped = cap_review_payload(big);
    assert!(
        capped.len() <= MAX_REVIEW_PAYLOAD_BYTES,
        "capped payload ({} bytes) must stay <= cap",
        capped.len()
    );
    assert!(
        capped.ends_with(TRUNCATION_NOTE),
        "capped payload must carry the truncation marker"
    );
    // Round-trips as UTF-8 (a bad char boundary would corrupt it) — the
    // string already IS valid UTF-8, so just assert no replacement char crept
    // in at the cut and the prefix is all 'é'.
    let body = &capped[..capped.len() - TRUNCATION_NOTE.len()];
    assert!(body.chars().all(|c| c == 'é'), "cut must be on a char boundary");
}

/// `AiAnalysisMode` deserializes from the exact `"explain"`/`"review"`
/// literals the TS `AiAnalysisMode` union sends.
#[test]
fn analysis_mode_deserializes_literals() {
    let explain: AiAnalysisMode = serde_json::from_str(r#""explain""#).expect("explain");
    assert!(matches!(explain, AiAnalysisMode::Explain));
    let review: AiAnalysisMode = serde_json::from_str(r#""review""#).expect("review");
    assert!(matches!(review, AiAnalysisMode::Review));
}
