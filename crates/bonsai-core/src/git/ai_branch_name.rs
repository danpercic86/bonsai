//! AI branch naming (P53c). Proposes a RANKED list of valid, kebab-case git
//! branch-name candidates from a grounding source (the working-tree change set,
//! or a commit range), so the user can pick/edit one in the branch-create
//! dialog. Read-only prose in, a small list out; WRITES NOTHING — the branch is
//! created by the existing confirmed create path (contract §0 D3). Each returned
//! candidate is SANITIZED to a valid git ref component and the invalid dropped,
//! so we never surface an uncreatable name. Pure git2 + crate::ai; the CLI system
//! prompt asks for the INTENT of the change ("WHY, not WHAT" — phase2 overview C1).

use std::path::Path;

use crate::ai::payload;
use crate::ai::{self, RunOpts};
use crate::error::AppError;
use crate::git::ai_explain::{cap_review_payload, gather_worktree, has_analyzable_content};
use crate::git::diff::{apply_find_similar, build_diff_options, collect_headers};
use crate::git::stage::open_workdir_repo;

/// Hard cap on returned candidates (§0 D3 / OQ4). The prompt asks for ~3; we
/// accept up to this many after sanitizing so a slightly over-eager model still
/// yields a bounded, deduped list.
pub const MAX_BRANCH_NAME_SUGGESTIONS: usize = 5;

/// Commits sampled into the `CommitRange` grounding payload. A branch name only
/// needs a representative sample of the range's intent, not the whole history;
/// beyond this the list is truncated with a "(+N more commits)" note (the whole
/// payload is byte-capped separately by `cap_review_payload`).
const BRANCH_NAME_MAX_COMMITS: usize = 50;

/// System prompt (via `--append-system-prompt`) for branch naming (§3.3,
/// verbatim). SINGLE line — on Windows the `claude` CLI is a `.cmd` shim and
/// Rust's `Command` REFUSES an argv arg containing a newline (same rule as the
/// P15 prompts). Multi-line grounding only ever flows through the stdin payload.
const BRANCH_NAME_SYSTEM_PROMPT: &str = "You are naming a git branch from a description of code changes on standard input. Propose three short, descriptive branch names in kebab-case, most fitting first, reflecting the INTENT of the change. Use an optional single type prefix (feat/, fix/, chore/, refactor/) then a hyphenated slug. Names must be valid git refs: lowercase, hyphen-separated, at most one '/', no spaces or special characters. Output ONLY the names, one per line — no numbering, no explanation, no code fences.";

/// The `-p` positional prompt for branch naming (§3.3, verbatim single line).
const BRANCH_NAME_PROMPT: &str = "Suggest branch names for the changes described on standard input.";

/// Where to draw the branch-name grounding from. COMMAND INPUT (Deserialize);
/// the TS mirror is a discriminated union (§2.2).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BranchNameSource {
    /// Index-aware working-tree change set (HEAD tree vs workdir, incl.
    /// untracked) — the common "about to start work" case. Clean tree =>
    /// `AiFailed` (§0 D6), before any CLI call.
    Working,
    /// Name a branch that will carry `from..to`. Both revparse-able. Empty range
    /// => `AiFailed`.
    CommitRange { from: String, to: String },
}

/// Ranked branch-name candidates (best first), each a VALID git ref component
/// (already sanitized; never empty). Serialize camelCase (mirrored in TS).
/// Only `PartialEq` (not `Eq`): `Option<f64>` is not `Eq` — same as the sibling
/// `AiAnalysis`/`AiSummary` result types.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchNameProposal {
    pub names: Vec<String>,
    pub cost_usd: Option<f64>,
}

/// Map a raw model line to a VALID git branch-name component, or `None` if it
/// can't be salvaged (§3.3). Pure; the highest-value unit-test surface (§7.4).
///
/// Algorithm: lowercase; keep `[a-z0-9]` and `/` (git allows nested names); map
/// EVERY other char — spaces, punctuation, `.`/`_`, control chars — to a single
/// `-`, collapsing runs; drop a `-` sitting next to a `/` and never emit a
/// leading/`//` slash; finally trim leading/trailing `-` and `/`. Because `.` is
/// mapped away and `//` cannot form, the git check-ref-format hazards (`..`,
/// `.lock`, leading `.`, empty component) are structurally impossible EXCEPT the
/// all-junk case, which collapses to the empty string and returns `None`.
fn sanitize_branch_name(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut prev_dash = false;
    for ch in raw.trim().chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_dash = false;
        } else if c == '/' {
            // A slash supersedes a pending dash; never emit a leading or double
            // slash (both are invalid ref components).
            while out.ends_with('-') {
                out.pop();
            }
            if !out.is_empty() && !out.ends_with('/') {
                out.push('/');
            }
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            // Any other char collapses to a single interior dash.
            out.push('-');
            prev_dash = true;
        }
    }
    // Trim trailing separators (a leading one can never be pushed above).
    while out.ends_with('-') || out.ends_with('/') {
        out.pop();
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Parse the model's raw multi-line output into ranked, valid, deduped branch
/// names capped at [`MAX_BRANCH_NAME_SUGGESTIONS`] (§3.3 step 3). Each line ->
/// [`sanitize_branch_name`] -> keep `Some` -> stable-dedup -> stop at the cap.
/// Pure; unit-tested (§7.8).
fn parse_branch_names(raw: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in raw.lines() {
        let Some(name) = sanitize_branch_name(line) else {
            continue;
        };
        if !out.contains(&name) {
            out.push(name);
            if out.len() >= MAX_BRANCH_NAME_SUGGESTIONS {
                break;
            }
        }
    }
    out
}

/// Assembles the `CommitRange` grounding payload: the commits unique to `to`
/// (merge-base `from..to`, or `from..to` directly for unrelated histories) plus
/// the net diffstat, mirroring the `ai_summary::summarize_range` walk. Empty
/// range => `AiFailed` before any CLI call.
fn build_range_payload(workdir: &Path, from: &str, to: &str) -> Result<String, AppError> {
    let repo = open_workdir_repo(workdir)?;
    let from_commit = repo.revparse_single(from)?.peel_to_commit()?;
    let to_commit = repo.revparse_single(to)?.peel_to_commit()?;
    let to_tree = to_commit.tree()?;

    // Merge base of the two. None => unrelated histories: diff vs the empty tree
    // and hide `from` directly.
    let mb = repo.merge_base(from_commit.id(), to_commit.id()).ok();
    let base_tree = match mb {
        Some(oid) => repo.find_commit(oid)?.tree()?,
        None => {
            let empty = repo.treebuilder(None)?.write()?;
            repo.find_tree(empty)?
        }
    };

    // Commits unique to `to`; collect up to the cap, track the pre-truncation total.
    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TIME)?;
    walk.push(to_commit.id())?;
    match mb {
        Some(oid) => walk.hide(oid)?,
        None => walk.hide(from_commit.id())?,
    }
    let mut commit_lines: Vec<payload::CommitLine> = Vec::new();
    let mut total = 0usize;
    for oid_res in walk {
        let oid = oid_res?;
        total += 1;
        if commit_lines.len() < BRANCH_NAME_MAX_COMMITS {
            let commit = repo.find_commit(oid)?;
            let short_oid: String = oid.to_string().chars().take(7).collect();
            let summary = commit
                .summary_bytes()
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default();
            let author = String::from_utf8_lossy(commit.author().name_bytes()).into_owned();
            commit_lines.push(payload::CommitLine {
                short_oid,
                summary,
                author,
            });
        }
    }
    if total == 0 {
        return Err(AppError::AiFailed(
            "no commits in the selected range to name a branch from".to_string(),
        ));
    }

    // Net diffstat: base_tree (or empty) vs to_tree, headers only.
    let mut diff_opts = build_diff_options(&[], false);
    let mut diff = repo.diff_tree_to_tree(Some(&base_tree), Some(&to_tree), Some(&mut diff_opts))?;
    apply_find_similar(&mut diff)?;
    let headers = collect_headers(&diff)?;
    let diffstat = payload::render_headers(&headers);

    let mut commits_section = payload::render_commit_list(&commit_lines);
    if total > commit_lines.len() {
        commits_section.push_str(&format!("(+{} more commits)\n", total - commit_lines.len()));
    }
    Ok(cap_review_payload(format!(
        "COMMITS TO NAME A BRANCH FOR:\n{commits_section}\n\nNET CHANGES:\n{}",
        diffstat.text
    )))
}

/// Blocking. Builds the grounding payload for `source`, asks the CLI for
/// kebab-case candidates, then sanitizes + dedups + caps them at
/// [`MAX_BRANCH_NAME_SUGGESTIONS`]. WRITES NOTHING (contract §0 D3). Empty
/// grounding (clean worktree / empty range) => `AiFailed` BEFORE any CLI call
/// (§0 D6). A model reply that yields no salvageable name => `AiFailed`. Errors:
/// `aiFailed` | `git` (bad ref) | (`aiUnavailable` via the command-layer gate).
pub fn suggest_branch_name(
    workdir: &Path,
    source: &BranchNameSource,
    opts: RunOpts,
) -> Result<BranchNameProposal, AppError> {
    // 1. Build the labeled grounding payload (empty grounding fails here, no CLI).
    let payload_text = match source {
        BranchNameSource::Working => {
            let files = gather_worktree(workdir)?;
            if !has_analyzable_content(&files) {
                return Err(AppError::AiFailed(
                    "no changes to name a branch from".to_string(),
                ));
            }
            let rendered = payload::render_file_diffs(&files);
            cap_review_payload(format!("WORKING CHANGES:\n{}", rendered.text))
        }
        BranchNameSource::CommitRange { from, to } => build_range_payload(workdir, from, to)?,
    };

    // 2. Ask the CLI (default model = sonnet, OQ3; caller's `opts` carry timeout).
    let result = ai::run_claude(
        workdir,
        BRANCH_NAME_PROMPT,
        Some(&payload_text),
        RunOpts {
            system_prompt: Some(BRANCH_NAME_SYSTEM_PROMPT.to_string()),
            ..opts
        },
    )?;

    // 3. Parse -> sanitize -> dedup -> cap. Nothing salvageable => AiFailed.
    let names = parse_branch_names(&result.text);
    if names.is_empty() {
        return Err(AppError::AiFailed(
            "no usable branch name suggested".to_string(),
        ));
    }
    Ok(BranchNameProposal {
        names,
        cost_usd: result.cost_usd,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    /// Serialize env-mutating tests: `BONSAI_CLAUDE_BIN` is process-global, so
    /// parallel tests that touch it would race (mirrors `ai::mod` tests).
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// §7.4: `sanitize_branch_name` lowercases + kebab-ifies salvageable input
    /// and rejects the unsalvageable. Documented mapping: spaces/punctuation
    /// (incl. `:`) collapse to a single `-`; a single `/` is preserved (nested
    /// branch names are valid git refs); leading/trailing separators are trimmed.
    #[test]
    fn sanitize_branch_name_rules() {
        // Salvage: words -> lowercase kebab.
        assert_eq!(
            sanitize_branch_name("Add AI Why Layer").as_deref(),
            Some("add-ai-why-layer")
        );
        // A conventional-commit style "feat: X" -> "feat-x" (colon -> dash).
        assert_eq!(sanitize_branch_name("feat: X").as_deref(), Some("feat-x"));
        // A single slash is kept (a type prefix like feat/…); interior dash kept.
        assert_eq!(
            sanitize_branch_name("feat/add-thing").as_deref(),
            Some("feat/add-thing")
        );
        // Post-slash dash is PRESERVED — only a dash BEFORE a '/' is dropped, so a
        // separator immediately AFTER a '/' becomes a leading dash on that
        // component: `feat/ fix` and `feat/-fix` both -> `feat/-fix`. This is
        // intentional and creatable: `git check-ref-format refs/heads/feat/-fix`
        // accepts it, so we surface the name rather than mangling it. Locks the
        // current output (reviewer nit).
        assert_eq!(sanitize_branch_name("feat/ fix").as_deref(), Some("feat/-fix"));
        assert_eq!(sanitize_branch_name("feat/-fix").as_deref(), Some("feat/-fix"));
        // Leading/trailing junk trimmed; a dash next to a slash is dropped; a
        // double slash collapses.
        assert_eq!(
            sanitize_branch_name("  --Feat//Add--Thing--  ").as_deref(),
            Some("feat/add-thing")
        );
        // Runs of mixed separators collapse to one dash.
        assert_eq!(
            sanitize_branch_name("fix   ***   bug").as_deref(),
            Some("fix-bug")
        );

        // Reject the unsalvageable: empty, whitespace-only, all-dots, all-junk,
        // control chars -> None (never an uncreatable name).
        assert_eq!(sanitize_branch_name(""), None);
        assert_eq!(sanitize_branch_name("   "), None);
        assert_eq!(sanitize_branch_name(".."), None);
        assert_eq!(sanitize_branch_name("..."), None);
        assert_eq!(sanitize_branch_name("---"), None);
        assert_eq!(sanitize_branch_name("///"), None);
        assert_eq!(sanitize_branch_name("\u{0}\u{1}\t"), None);

        // Every produced name is a valid ref component: no leading/trailing
        // separator, no `..`, no `//`, non-empty.
        for raw in ["Add AI Why Layer", "feat: X", "feat/add-thing", "  --x--  "] {
            if let Some(name) = sanitize_branch_name(raw) {
                assert!(!name.is_empty());
                assert!(!name.starts_with('-') && !name.starts_with('/'));
                assert!(!name.ends_with('-') && !name.ends_with('/'));
                assert!(!name.contains(".."));
                assert!(!name.contains("//"));
            }
        }
    }

    /// §7.8: parse the model's raw multi-line output into ranked, deduped, capped
    /// candidates — invalid lines dropped, duplicates removed (stable order), and
    /// the result capped at [`MAX_BRANCH_NAME_SUGGESTIONS`].
    #[test]
    fn parse_branch_names_dedups_drops_invalid_and_caps() {
        // 8 non-blank lines: one all-junk (dropped), one duplicate-after-sanitize
        // ("Feat/One" == "feat/one"), and 7 distinct valid names — proving the
        // cap trims to MAX and the invalid/dup never appear.
        let raw = "\
feat/one
Feat/One
fix-two
chore/three
refactor-four
!!!___!!!
feat/five
topic/six
seven-branch
";
        let names = parse_branch_names(raw);
        assert_eq!(
            names.len(),
            MAX_BRANCH_NAME_SUGGESTIONS,
            "must cap at MAX_BRANCH_NAME_SUGGESTIONS; got {names:?}"
        );
        // Order preserved, duplicate collapsed, junk dropped.
        assert_eq!(
            names,
            vec!["feat/one", "fix-two", "chore/three", "refactor-four", "feat/five"]
        );
        assert!(
            !names.iter().any(|n| n.contains('!') || n.contains('_')),
            "the all-junk line must be dropped: {names:?}"
        );

        // Nothing salvageable => empty (caller maps to AiFailed).
        assert!(parse_branch_names("...\n---\n   \n").is_empty());
    }

    /// §7.5: `BranchNameSource` deserializes from the EXACT JSON the TS
    /// discriminated union sends for each variant — locking the IPC contract.
    #[test]
    fn branch_name_source_deserializes_each_variant() {
        let working: BranchNameSource =
            serde_json::from_str(r#"{"kind":"working"}"#).expect("working");
        assert!(matches!(working, BranchNameSource::Working));

        let range: BranchNameSource =
            serde_json::from_str(r#"{"kind":"commitRange","from":"main","to":"feature"}"#)
                .expect("commitRange");
        match range {
            BranchNameSource::CommitRange { from, to } => {
                assert_eq!(from, "main");
                assert_eq!(to, "feature");
            }
            other => panic!("expected CommitRange, got {other:?}"),
        }
    }

    /// §7.6: serde casing must match the TS `BranchNameProposal` type
    /// (`names` / `costUsd`); `None` cost serializes as `null`.
    #[test]
    fn branch_name_proposal_wire_shape_is_camel_case() {
        let v = serde_json::to_value(BranchNameProposal {
            names: vec!["feat/ai-why-layer".to_string(), "ai-why-layer".to_string()],
            cost_usd: Some(0.003),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "names": ["feat/ai-why-layer", "ai-why-layer"],
                "costUsd": 0.003
            })
        );

        let v = serde_json::to_value(BranchNameProposal {
            names: vec!["topic/x".to_string()],
            cost_usd: None,
        })
        .expect("json");
        assert_eq!(v, serde_json::json!({ "names": ["topic/x"], "costUsd": null }));
    }

    /// §7.9: the prompt/system-prompt consts MUST be single-line (Windows argv
    /// constraint) — a newline would make `claude.cmd` reject the argument.
    #[test]
    fn prompts_are_single_line() {
        for s in [BRANCH_NAME_SYSTEM_PROMPT, BRANCH_NAME_PROMPT] {
            assert!(!s.contains('\n'), "prompt must be single-line: {s:?}");
            assert!(!s.contains('\r'), "prompt must be single-line: {s:?}");
        }
    }

    // ---- scratch-repo edge: empty grounding fails before any CLI spawn --------

    use crate::git::commit::create_commit;
    use crate::git::stage::stage_paths;

    /// git2-init a scratch repo with identity + autocrlf off (mirrors `ai_explain`).
    fn init_scratch() -> tempfile::TempDir {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        dir
    }

    /// §7.7: a clean worktree (nothing staged/unstaged/untracked) => `AiFailed`
    /// with the specific "no changes to name a branch from" message BEFORE any
    /// CLI spawn. `BONSAI_CLAUDE_BIN` is pointed at a nonexistent path: if the
    /// code regressed and spawned, `run_claude` would return `AiUnavailable`
    /// (binary NotFound) — a DIFFERENT variant — so the precise `AiFailed`
    /// assertion proves no spawn happened.
    #[test]
    fn suggest_branch_name_working_empty_fails_before_cli() {
        let _g = env_lock();
        std::env::set_var(
            crate::ai::CLAUDE_BIN_ENV,
            "D:/nonexistent/claude-must-not-spawn.exe",
        );

        let dir = init_scratch();
        let p = dir.path();
        std::fs::write(p.join("base.txt"), "base\n").expect("write");
        stage_paths(p, &["base.txt".into()]).expect("stage");
        create_commit(p, "base").expect("commit");
        // Worktree is now clean => no grounding.

        let err = suggest_branch_name(p, &BranchNameSource::Working, RunOpts::default())
            .expect_err("clean worktree must fail before any CLI call");

        std::env::remove_var(crate::ai::CLAUDE_BIN_ENV);

        match err {
            AppError::AiFailed(m) => assert_eq!(
                m, "no changes to name a branch from",
                "empty-grounding message proves the pre-CLI bail; got: {m}"
            ),
            other => panic!("expected AiFailed (pre-CLI), got {other:?} — a spawn would be AiUnavailable"),
        }
    }

    /// Symmetric to the Working case (§7.7): an EMPTY commit range — here
    /// `from == to`, so the `from..to` revwalk hides exactly the commit it pushed
    /// and yields nothing — must fail with the specific "no commits in the
    /// selected range …" `AiFailed` inside `build_range_payload`, BEFORE any CLI
    /// spawn. `BONSAI_CLAUDE_BIN` points at a nonexistent path: a regressed spawn
    /// would surface as `AiUnavailable` (binary NotFound) — a DIFFERENT variant —
    /// so the precise `AiFailed` assertion proves the pre-CLI bail.
    #[test]
    fn suggest_branch_name_range_empty_fails_before_cli() {
        let _g = env_lock();
        std::env::set_var(
            crate::ai::CLAUDE_BIN_ENV,
            "D:/nonexistent/claude-must-not-spawn.exe",
        );

        let dir = init_scratch();
        let p = dir.path();
        std::fs::write(p.join("base.txt"), "base\n").expect("write");
        stage_paths(p, &["base.txt".into()]).expect("stage");
        create_commit(p, "base").expect("commit");

        // from == to => merge-base is that same commit, which the revwalk both
        // pushes and hides => zero commits in the range => empty grounding.
        let source = BranchNameSource::CommitRange {
            from: "HEAD".to_string(),
            to: "HEAD".to_string(),
        };
        let err = suggest_branch_name(p, &source, RunOpts::default())
            .expect_err("empty range must fail before any CLI call");

        std::env::remove_var(crate::ai::CLAUDE_BIN_ENV);

        match err {
            AppError::AiFailed(m) => assert_eq!(
                m, "no commits in the selected range to name a branch from",
                "empty-range message proves the pre-CLI bail; got: {m}"
            ),
            other => panic!("expected AiFailed (pre-CLI), got {other:?} — a spawn would be AiUnavailable"),
        }
    }
}
