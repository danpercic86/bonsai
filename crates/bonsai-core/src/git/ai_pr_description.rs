//! Local AI pull-request description generation (P64 Part B). Given a base and a
//! head ref, walks the commits unique to `head` vs `base` (merge-base range) plus
//! the net diffstat — REUSING the shipped digest resolver (`ai_explain`), the
//! payload renderers (`ai::payload`), and the byte-cap (`cap_review_payload`),
//! exactly like `ai_changelog` — and asks the local `claude` CLI for a PR
//! title + Markdown body (why-not-what). Human-in-the-loop: the proposal fills the
//! create-PR form fields; the user reviews/edits and still clicks Create. Provider
//! AGNOSTIC (pure local git + `crate::ai`); WRITES NOTHING; never posts anywhere.

use std::path::Path;

use crate::ai::payload;
use crate::ai::{self, RunOpts};
use crate::error::AppError;
use crate::git::ai_explain::{self, AiDigestRange};
use crate::git::ai_summary::AI_SUMMARY_MAX_COMMITS;
use crate::git::diff::{apply_find_similar, build_diff_options, collect_headers};
use crate::git::stage::open_workdir_repo;

/// System prompt (via `--append-system-prompt`) for the PR draft: role + the
/// title-then-body output contract + the why-not-what grouping style (mirrors the
/// P56 changelog philosophy; OQ-B1). SINGLE line — on Windows the `claude` CLI is
/// a `.cmd` shim and Rust's `Command` REFUSES an argv argument containing a
/// newline. Multi-line content only ever flows through the stdin payload.
const PR_SYSTEM_PROMPT: &str = "You are drafting a pull-request title and description for a teammate reviewer from a list of commits and a net diffstat on standard input. Output whose FIRST line is a concise imperative PR title (<=72 chars, no trailing period, no 'PR:' prefix), then a blank line, then a Markdown body that explains WHY the change exists and WHAT it does at a high level: a one-paragraph summary, then a `## Changes` section with grouped bullets of the notable changes, then a `## Notes` section ONLY if something is risky/incomplete/needs reviewer attention. Prefer intent over a commit-by-commit list. Do NOT wrap the output in a code fence.";

/// The `-p` positional prompt (§4b, verbatim single line).
const PR_PROMPT: &str =
    "Draft a pull-request title and description for the branch described on standard input.";

/// A generated PR proposal: a title + Markdown body, plus the echoed requested
/// range and cost for the UI header. Serialize camelCase (mirrored in TS as
/// `PrDescription`). Generating WRITES NOTHING. (P64)
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrDescription {
    /// First output line, trimmed; imperative, no trailing period.
    pub title: String,
    /// Markdown body (why-not-what). May be `""` for a single-line reply.
    pub body: String,
    /// Echoed requested base ref (the argument, verbatim).
    pub base: String,
    /// Echoed requested head ref (the argument, verbatim).
    pub head: String,
    /// Commits listed (capped at [`AI_SUMMARY_MAX_COMMITS`]).
    pub commit_count: u32,
    pub cost_usd: Option<f64>,
}

/// Blocking, READ-ONLY. Grounds a PR title+body in the commits unique to `head`
/// vs `base` (merge-base range) + the net diffstat, then calls the CLI. WRITES
/// NOTHING; never posts to a forge. Empty range / `base == head` (no unique
/// commits AND no changed files) => `AiFailed` BEFORE any CLI call (mirrors
/// `ai_changelog`'s empty-range bail). Errors: `aiFailed` (empty range / no
/// usable title / CLI failure) | `git` (bad ref) | (`aiUnavailable` via the
/// command-layer gate).
pub fn generate_pr_description(
    workdir: &Path,
    base: &str,
    head: &str,
    opts: RunOpts,
) -> Result<PrDescription, AppError> {
    let repo = open_workdir_repo(workdir)?;

    // 1. Reuse the shipped digest resolver (D2/OQ6): revwalk + merge-base + trees
    //    in ONE place — exactly the reuse `ai_changelog` does. A bad ref => Git
    //    (via revparse/peel inside the resolver).
    let (_header, commits, old_tree, new_tree) = ai_explain::resolve_digest_range(
        &repo,
        &AiDigestRange::BetweenRefs {
            from: base.to_string(),
            to: head.to_string(),
        },
    )?;

    // 2. Net diffstat over the whole range (merge-base tree -> `head` tree),
    //    headers only — the same aggregate `ai_changelog` renders.
    let mut diff_opts = build_diff_options(&[], false);
    let mut diff =
        repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut diff_opts))?;
    apply_find_similar(&mut diff)?;
    let headers = collect_headers(&diff)?;

    // 3. Empty range (no commits AND no changed files) => nothing to describe, no
    //    CLI call (§4a step 2, mirrors `ai_changelog`'s empty-range bail).
    if commits.is_empty() && headers.is_empty() {
        return Err(AppError::AiFailed(format!(
            "nothing to describe: {head} has no commits beyond {base}"
        )));
    }

    // 4. Commit list (newest first), capped at AI_SUMMARY_MAX_COMMITS.
    let total = commits.len();
    let commit_lines: Vec<payload::CommitLine> = commits
        .iter()
        .take(AI_SUMMARY_MAX_COMMITS)
        .map(|c| {
            let short_oid: String = c.id().to_string().chars().take(7).collect();
            let summary = c
                .summary_bytes()
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default();
            let author = String::from_utf8_lossy(c.author().name_bytes()).into_owned();
            payload::CommitLine {
                short_oid,
                summary,
                author,
            }
        })
        .collect();
    let commit_count = u32::try_from(commit_lines.len()).unwrap_or(u32::MAX);

    // 5. Assemble the labeled grounding payload (multi-line => stdin ONLY), then
    //    ONE combined byte-cap over the whole string (§4a step 3).
    let mut commits_section = payload::render_commit_list(&commit_lines);
    if total > commit_lines.len() {
        commits_section.push_str(&format!("(+{} more commits)\n", total - commit_lines.len()));
    }
    let diffstat = payload::render_headers(&headers);
    let payload_text = ai_explain::cap_review_payload(format!(
        "COMMITS (head since base):\n{commits_section}\nNET CHANGES (diffstat):\n{}",
        diffstat.text
    ));

    // 6. Ask the CLI for a title + body (system prompt set here; caller's opts
    //    carry model/timeout). A CLI hard-failure propagates as AiFailed.
    let result = ai::run_claude(
        workdir,
        PR_PROMPT,
        Some(&payload_text),
        RunOpts {
            system_prompt: Some(PR_SYSTEM_PROMPT.to_string()),
            ..opts
        },
    )?;

    // 7. Split the reply into (title, body). A reply with no usable title (all
    //    whitespace) => AiFailed (§4a).
    let (title, body) = split_title_body(&result.text);
    if title.is_empty() {
        return Err(AppError::AiFailed(
            "Claude returned no usable title".to_string(),
        ));
    }

    Ok(PrDescription {
        title,
        body,
        base: base.to_string(),
        head: head.to_string(),
        commit_count,
        cost_usd: result.cost_usd,
    })
}

/// Splits the model's reply into a PR `(title, body)`: the first non-empty line
/// is the title (a leading `# ` heading marker or `PR:` / `Title:` label stripped
/// defensively — the prompt already forbids them), and the body is everything
/// after that line with ONE leading blank line skipped and trailing whitespace
/// trimmed. A single-line reply => empty body. CRLF-tolerant: `str::lines()`
/// strips the `\r` and the body is re-joined with `\n`. Pure — unit-tested.
fn split_title_body(text: &str) -> (String, String) {
    let trimmed = text.trim();
    let lines: Vec<&str> = trimmed.lines().collect();

    // First non-empty line => title.
    let Some(title_idx) = lines.iter().position(|l| !l.trim().is_empty()) else {
        return (String::new(), String::new());
    };
    let title = strip_title_marker(lines[title_idx].trim());

    // Body = the lines after the title, skipping ONE leading blank line.
    let mut rest = &lines[title_idx + 1..];
    if rest.first().is_some_and(|l| l.trim().is_empty()) {
        rest = &rest[1..];
    }
    let body = rest.join("\n").trim_end().to_string();

    (title, body)
}

/// Strips a leading marker a model sometimes prefixes onto the title line: a
/// Markdown heading (`# `, `## `, …) or a `PR:` / `Title:` label (case-insensitive
/// for the labels). Defensive; the system prompt already forbids these.
fn strip_title_marker(line: &str) -> String {
    let l = line.trim();
    // Markdown heading: strip leading '#'s + the following spaces.
    let without_hash = l.trim_start_matches('#');
    if without_hash.len() != l.len() {
        return without_hash.trim_start().to_string();
    }
    for prefix in ["PR:", "Title:"] {
        // Compare on BYTES: `l[..prefix.len()]` would panic if a multibyte char
        // straddled the boundary (e.g. an accented/emoji-prefixed title). An
        // ASCII-prefix match then guarantees `prefix.len()` IS a char boundary
        // for the tail slice.
        let pb = prefix.as_bytes();
        if l.as_bytes().get(..pb.len()).is_some_and(|b| b.eq_ignore_ascii_case(pb)) {
            return l[prefix.len()..].trim_start().to_string();
        }
    }
    l.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    /// Serialize env-mutating tests: `BONSAI_CLAUDE_BIN` is process-global, so
    /// parallel tests that touch it would race (mirrors `ai_changelog` / `ai::mod`).
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// git2-init a scratch repo with identity + autocrlf off (mirrors `ai_changelog`).
    fn init_scratch() -> tempfile::TempDir {
        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        dir
    }

    /// A per-message unique tree (single evolving blob) — no workdir writes.
    fn tree_with<'r>(repo: &'r git2::Repository, key: &str) -> git2::Tree<'r> {
        let blob = repo.blob(key.as_bytes()).expect("blob");
        let mut tb = repo.treebuilder(None).expect("treebuilder");
        tb.insert("f.txt", blob, 0o100644).expect("insert");
        let oid = tb.write().expect("tree write");
        repo.find_tree(oid).expect("find tree")
    }

    fn commit_at(
        repo: &git2::Repository,
        update_ref: Option<&str>,
        msg: &str,
        secs: i64,
        parents: &[&git2::Commit<'_>],
    ) -> git2::Oid {
        let sig = git2::Signature::new("Test User", "test@example.com", &git2::Time::new(secs, 0))
            .expect("signature");
        let tree = tree_with(repo, msg);
        repo.commit(update_ref, &sig, &sig, msg, &tree, parents)
            .expect("commit")
    }

    /// `main` = A->B, `feature` = B->C->D, HEAD on feature. Returns (dir, [a,b,c,d]).
    fn pr_fixture() -> (tempfile::TempDir, [git2::Oid; 4]) {
        let dir = init_scratch();
        let repo = git2::Repository::open(dir.path()).expect("open");
        let t = 1_700_000_000i64;
        let a = commit_at(&repo, None, "A", t, &[]);
        let a_c = repo.find_commit(a).expect("A");
        let b = commit_at(&repo, None, "B", t + 10, &[&a_c]);
        let b_c = repo.find_commit(b).expect("B");
        let c = commit_at(&repo, None, "feat: add C", t + 20, &[&b_c]);
        let c_c = repo.find_commit(c).expect("C");
        let d = commit_at(&repo, None, "fix: fix D", t + 30, &[&c_c]);
        let d_c = repo.find_commit(d).expect("D");
        repo.branch("main", &b_c, true).expect("main");
        repo.branch("feature", &d_c, true).expect("feature");
        repo.set_head("refs/heads/feature").expect("head");
        drop((a_c, b_c, c_c, d_c));
        (dir, [a, b, c, d])
    }

    /// §4b: the prompt consts MUST be single-line (Windows argv constraint).
    #[test]
    fn prompts_are_single_line() {
        for s in [PR_SYSTEM_PROMPT, PR_PROMPT] {
            assert!(!s.contains('\n'), "prompt must be single-line: {s:?}");
            assert!(!s.contains('\r'), "prompt must be single-line: {s:?}");
        }
    }

    /// Serde casing matches the TS `PrDescription`
    /// (`title`/`body`/`base`/`head`/`commitCount`/`costUsd`; `None` => `null`).
    #[test]
    fn pr_description_wire_shape_is_camel_case() {
        let v = serde_json::to_value(PrDescription {
            title: "Add PR descriptions".to_string(),
            body: "## Changes\n- thing".to_string(),
            base: "main".to_string(),
            head: "feature".to_string(),
            commit_count: 2,
            cost_usd: Some(0.012),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "title": "Add PR descriptions",
                "body": "## Changes\n- thing",
                "base": "main",
                "head": "feature",
                "commitCount": 2,
                "costUsd": 0.012
            })
        );

        let v = serde_json::to_value(PrDescription {
            title: "t".to_string(),
            body: String::new(),
            base: "a".to_string(),
            head: "b".to_string(),
            commit_count: 0,
            cost_usd: None,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "title": "t",
                "body": "",
                "base": "a",
                "head": "b",
                "commitCount": 0,
                "costUsd": null
            })
        );
    }

    /// `split_title_body`: title + blank line + Markdown body.
    #[test]
    fn split_title_body_title_and_body() {
        let (title, body) = split_title_body(
            "Add PR description generation\n\nThis grounds the draft in real commits.\n\n## Changes\n- a\n- b",
        );
        assert_eq!(title, "Add PR description generation");
        assert_eq!(
            body,
            "This grounds the draft in real commits.\n\n## Changes\n- a\n- b"
        );
    }

    /// `split_title_body`: a single-line reply => empty body.
    #[test]
    fn split_title_body_title_only() {
        let (title, body) = split_title_body("Just a title");
        assert_eq!(title, "Just a title");
        assert_eq!(body, "");
    }

    /// `split_title_body`: a leading `# ` heading marker on the title is stripped,
    /// and a single leading blank line before the body is consumed.
    #[test]
    fn split_title_body_strips_heading_marker() {
        let (title, body) = split_title_body("# Fix the widget\n\nBody text.");
        assert_eq!(title, "Fix the widget");
        assert_eq!(body, "Body text.");
        // `## ` too.
        let (title2, _) = split_title_body("## Fix\n\nx");
        assert_eq!(title2, "Fix");
    }

    /// `split_title_body`: `PR:` / `Title:` labels stripped (case-insensitive);
    /// leading blank lines before the title are skipped; trailing whitespace
    /// trimmed off the body.
    #[test]
    fn split_title_body_strips_labels_and_trims() {
        let (title, body) = split_title_body("PR: Add thing\n\nbody\n\n\n");
        assert_eq!(title, "Add thing");
        assert_eq!(body, "body");

        let (title2, body2) = split_title_body("\n\ntitle:  Fix bug\nline1\nline2");
        assert_eq!(title2, "Fix bug");
        // No blank line after the title => the body starts at the next line.
        assert_eq!(body2, "line1\nline2");
    }

    /// `strip_title_marker` must NOT panic when the title's leading bytes are a
    /// multibyte char straddling a label prefix's byte length (BYTE-slice hazard):
    /// an accented word and an emoji-prefixed title both pass through unchanged.
    #[test]
    fn strip_title_marker_handles_non_ascii_without_panic() {
        // A multibyte char ("ö" = 2 bytes) inside the first 3 bytes ("PR:".len())
        // — a naive `l[..3]` slice would panic on the non-char-boundary cut.
        assert_eq!(strip_title_marker("coördinate the retries"), "coördinate the retries");
        // Emoji-prefixed (4-byte char at the front).
        assert_eq!(strip_title_marker("🚀 Ship the thing"), "🚀 Ship the thing");
        // A genuine label prefix is still stripped even with non-ASCII in the tail.
        assert_eq!(strip_title_marker("PR: coördinate the retries"), "coördinate the retries");
        // And via the full splitter (end-to-end, no panic).
        let (title, body) = split_title_body("🚀 Ship it\n\nbody");
        assert_eq!(title, "🚀 Ship it");
        assert_eq!(body, "body");
    }

    /// `split_title_body`: whitespace-only reply => empty title (caller maps this
    /// to `AiFailed`).
    #[test]
    fn split_title_body_blank_reply_yields_empty_title() {
        let (title, body) = split_title_body("   \n\t\n  ");
        assert_eq!(title, "");
        assert_eq!(body, "");
    }

    /// §4a: `base == head` (empty range) => `AiFailed("nothing to describe: …")`
    /// BEFORE any CLI call. `BONSAI_CLAUDE_BIN` points at a nonexistent path: a
    /// regressed spawn would return `AiUnavailable` (a DIFFERENT variant), so the
    /// precise `AiFailed` assertion proves the pre-CLI bail.
    #[test]
    fn empty_range_fails_before_cli() {
        let _g = env_lock();
        std::env::set_var(ai::CLAUDE_BIN_ENV, "D:/nonexistent/claude-must-not-spawn.exe");

        let (dir, _) = pr_fixture();
        let err = generate_pr_description(dir.path(), "feature", "feature", RunOpts::default())
            .expect_err("empty range must fail");
        std::env::remove_var(ai::CLAUDE_BIN_ENV);

        match err {
            AppError::AiFailed(m) => {
                assert_eq!(m, "nothing to describe: feature has no commits beyond feature");
            }
            other => {
                panic!("expected AiFailed (pre-CLI), got {other:?} — a spawn would be AiUnavailable")
            }
        }
    }

    /// A bad base ref => `Git` (via the resolver's revparse), BEFORE any CLI call.
    #[test]
    fn bad_ref_maps_to_git_error() {
        let _g = env_lock();
        std::env::set_var(ai::CLAUDE_BIN_ENV, "D:/nonexistent/claude-must-not-spawn.exe");

        let (dir, _) = pr_fixture();
        let err = generate_pr_description(dir.path(), "no-such-ref", "feature", RunOpts::default())
            .expect_err("bad ref must fail");
        std::env::remove_var(ai::CLAUDE_BIN_ENV);

        assert!(matches!(err, AppError::Git(_)), "got {err:?}");
    }
}
