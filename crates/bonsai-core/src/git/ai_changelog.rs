//! Local AI changelog / release-notes (P56). Given a tag/ref range
//! (`v1.2.0..v1.3.0`) or "since the last tag", walks the commit range Bonsai
//! already computes — REUSING the shipped digest resolver (D2/OQ6) — renders a
//! compact `COMMITS` + `NET CHANGES (diffstat)` payload, and asks the local
//! `claude` CLI to write GROUPED, categorized Markdown release notes. Grouping
//! is the model's job, guided by a conventional-commits hint (OQ1); Rust does
//! NOT parse prefixes. Read-only prose out; WRITES NOTHING. Pure git2 +
//! crate::ai.

use std::path::Path;

use crate::ai::payload;
use crate::ai::{self, RunOpts};
use crate::error::AppError;
use crate::git::ai_explain::{self, AiDigestRange};
use crate::git::diff::{apply_find_similar, build_diff_options, collect_headers};
use crate::git::stage::open_workdir_repo;

/// Cap on commits listed in the payload (keeps the call bounded); beyond it a
/// "(+N more commits)" note is appended (same idiom as `AI_SUMMARY_MAX_COMMITS`).
pub const MAX_CHANGELOG_COMMITS: usize = 300;

/// System prompt (via `--append-system-prompt`) for the changelog: role + the
/// fixed taxonomy + the conventional-commit grouping HINT (D4/D5, §3). SINGLE
/// line — on Windows the `claude` CLI is a `.cmd` shim and Rust's `Command`
/// REFUSES an argv argument containing a newline. Multi-line content only ever
/// flows through the stdin payload.
const CHANGELOG_SYSTEM_PROMPT: &str = "You are writing release notes from a commit list and a diffstat on standard input. Produce concise Markdown release notes grouped by change type. Begin with one short summary sentence, then use these level-3 headings IN THIS ORDER, omitting any that would be empty: `### Features`, `### Fixes`, `### Performance`, `### Refactoring`, `### Documentation`, `### Tests`, `### Other`. Classify each commit by its conventional-commit prefix when present (feat->Features, fix->Fixes, perf->Performance, refactor->Refactoring, docs->Documentation, test->Tests; build/ci/chore->Other) and by its subject/diff otherwise. Under each heading write one bullet per notable change: a short human-readable description followed by the short hash in parentheses, e.g. `- Add SSH commit signing (a1b2c3d)`. Omit merge commits and pure version bumps. Output Markdown only — do NOT wrap the whole document in a code fence.";

/// The `-p` positional prompt (§3, verbatim single line).
const CHANGELOG_PROMPT: &str =
    "Write grouped release notes for the commits and diffstat on standard input.";

/// Which range to write release notes for. Command INPUT (Deserialize); the TS
/// mirror is a discriminated union (§4). (P56)
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChangelogRange {
    /// Notes for commits in `to` but not `from` (merge-base range). Both accept
    /// any revparse-able ref/oid; tags (`v1.2.0`..`v1.3.0`) are the common case.
    BetweenRefs { from: String, to: String },
    /// Notes since the most recent tag reachable from `target` (default HEAD),
    /// EXCLUDING `target`'s own tip. `from` resolves to that previous tag.
    SinceLastTag {
        #[serde(default)]
        target: Option<String>,
    },
}

/// Grouped release notes (Markdown) + the RESOLVED range echoed for the UI
/// header. Serialize camelCase (mirrored in TS). (P56)
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChangelog {
    /// Grouped Markdown release notes.
    pub text: String,
    /// Resolved `from` (e.g. the previous-tag name for `SinceLastTag`).
    pub from_ref: String,
    /// Resolved `to`.
    pub to_ref: String,
    /// Commits listed (capped at [`MAX_CHANGELOG_COMMITS`]).
    pub commit_count: u32,
    pub cost_usd: Option<f64>,
}

/// Blocking, READ-ONLY. Resolves `range` (reusing `resolve_digest_range`, D2),
/// gathers the commit list + net diffstat, renders the payload, and asks the CLI
/// for grouped Markdown notes. WRITES NOTHING. Errors: `aiFailed` (empty range /
/// no previous tag / CLI failure) | `git` (bad ref) | (`aiUnavailable` via the
/// command-layer gate).
pub fn generate_changelog(
    workdir: &Path,
    range: ChangelogRange,
    opts: RunOpts,
) -> Result<AiChangelog, AppError> {
    let repo = open_workdir_repo(workdir)?;

    // 1. Resolve the range to a concrete (from_ref, to_ref) pair. For
    //    SinceLastTag we first find the previous tag reachable from the target;
    //    a missing previous tag is AiFailed BEFORE any CLI call (OQ7 / §3).
    let (from_ref, to_ref) = match range {
        ChangelogRange::BetweenRefs { from, to } => (from, to),
        ChangelogRange::SinceLastTag { target } => {
            let to_ref = target.unwrap_or_else(|| "HEAD".to_string());
            let to_oid = repo.revparse_single(&to_ref)?.peel_to_commit()?.id();
            match resolve_last_tag(&repo, to_oid)? {
                Some((tag, _)) => (tag, to_ref),
                None => {
                    return Err(AppError::AiFailed(format!(
                        "no earlier tag found before {to_ref}"
                    )));
                }
            }
        }
    };

    // 2. Reuse the shipped digest resolver (D2/OQ6): revwalk + merge-base + trees
    //    in ONE place. A bad ref => Git (via revparse/peel inside the resolver).
    let (_header, commits, old_tree, new_tree) = ai_explain::resolve_digest_range(
        &repo,
        &AiDigestRange::BetweenRefs {
            from: from_ref.clone(),
            to: to_ref.clone(),
        },
    )?;

    // 3. Net diffstat over the whole range (merge-base tree -> `to` tree),
    //    headers only — the same aggregate `ai_summary` renders.
    let mut diff_opts = build_diff_options(&[], false);
    let mut diff =
        repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), Some(&mut diff_opts))?;
    apply_find_similar(&mut diff)?;
    let headers = collect_headers(&diff)?;

    // 4. Empty range (no commits AND no changed files) => nothing to write, no
    //    CLI call (§3 step 2, mirrors `digest_changes`' empty-range bail).
    if commits.is_empty() && headers.is_empty() {
        return Err(AppError::AiFailed(format!(
            "no changes between {from_ref} and {to_ref}"
        )));
    }

    // 5. Commit list (newest first), capped at MAX_CHANGELOG_COMMITS.
    let total = commits.len();
    let commit_lines: Vec<payload::CommitLine> = commits
        .iter()
        .take(MAX_CHANGELOG_COMMITS)
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

    // 6. Assemble the labeled grounding payload (multi-line => stdin ONLY), then
    //    ONE combined byte-cap over the whole string (§3 step 3).
    let mut commits_section = payload::render_commit_list(&commit_lines);
    if total > commit_lines.len() {
        commits_section.push_str(&format!("(+{} more commits)\n", total - commit_lines.len()));
    }
    let diffstat = payload::render_headers(&headers);
    let payload_text = ai_explain::cap_review_payload(format!(
        "COMMITS:\n{commits_section}\nNET CHANGES (diffstat):\n{}",
        diffstat.text
    ));

    // 7. Ask the CLI for grouped Markdown (system prompt set here; caller's opts
    //    carry model/timeout). A CLI hard-failure propagates as AiFailed.
    let result = ai::run_claude(
        workdir,
        CHANGELOG_PROMPT,
        Some(&payload_text),
        RunOpts {
            system_prompt: Some(CHANGELOG_SYSTEM_PROMPT.to_string()),
            ..opts
        },
    )?;

    Ok(AiChangelog {
        text: result.text,
        from_ref,
        to_ref,
        commit_count,
        cost_usd: result.cost_usd,
    })
}

/// Most recent tag (annotated or lightweight) reachable from `target_oid`,
/// EXCLUDING a tag pointing AT `target_oid` itself; returns `(tag_shorthand,
/// oid)`. `None` => no earlier tag (caller => `AiFailed`). Mirrors `git describe
/// --tags --abbrev=0 <target>^` semantics using git2 tag enumeration +
/// merge-base reachability + committer-time ordering (§3, OQ7).
fn resolve_last_tag(
    repo: &git2::Repository,
    target_oid: git2::Oid,
) -> Result<Option<(String, git2::Oid)>, AppError> {
    // Track the best (shorthand, commit oid, committer time). Ties keep the
    // first-seen tag (tag_names is sorted, so this is deterministic).
    let mut best: Option<(String, git2::Oid, i64)> = None;

    // `StringArray::iter()` yields `Result<Option<&str>, _>`; the shipped idiom
    // (branches.rs) drops Utf8 errors + None entries to iterate valid `&str`.
    let names = repo.tag_names(None)?;
    for name in names.iter().filter_map(Result::ok).flatten() {
        // Peel the tag ref to a commit (handles both lightweight + annotated).
        let commit = match repo.revparse_single(&format!("refs/tags/{name}")) {
            Ok(obj) => match obj.peel_to_commit() {
                Ok(c) => c,
                Err(_) => continue, // tag does not point at a commit — skip
            },
            Err(_) => continue,
        };
        let tag_oid = commit.id();

        // Exclude a tag pointing AT the target's own tip (§3, OQ7).
        if tag_oid == target_oid {
            continue;
        }
        // Keep only tags reachable from the target: `tag_oid` is an ancestor of
        // `target_oid` iff their merge-base IS `tag_oid`. Unrelated histories
        // (merge_base errors) are not reachable.
        let reachable = matches!(repo.merge_base(tag_oid, target_oid), Ok(mb) if mb == tag_oid);
        if !reachable {
            continue;
        }

        let t = commit.time().seconds();
        match &best {
            Some((_, _, best_t)) if *best_t >= t => {}
            _ => best = Some((name.to_string(), tag_oid, t)),
        }
    }

    Ok(best.map(|(name, oid, _)| (name, oid)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    /// Serialize env-mutating tests: `BONSAI_CLAUDE_BIN` is process-global, so
    /// parallel tests that touch it would race (mirrors `ai_compose` / `ai::mod`).
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

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

    /// A per-message unique tree (single evolving blob) — no workdir writes.
    fn tree_with<'r>(repo: &'r git2::Repository, key: &str) -> git2::Tree<'r> {
        let blob = repo.blob(key.as_bytes()).expect("blob");
        let mut tb = repo.treebuilder(None).expect("treebuilder");
        tb.insert("f.txt", blob, 0o100644).expect("insert");
        let oid = tb.write().expect("tree write");
        repo.find_tree(oid).expect("find tree")
    }

    /// Commit with a controlled committer time + a unique tree keyed on `msg`.
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

    /// Chain A->B->C->D->E (increasing times) with lightweight tags v1@A, v2@C,
    /// v3@E. Returns (dir, [a, b, c, d, e]).
    fn tag_fixture() -> (tempfile::TempDir, [git2::Oid; 5]) {
        let dir = init_scratch();
        let repo = git2::Repository::open(dir.path()).expect("open");
        let t = 1_700_000_000i64;
        let a = commit_at(&repo, Some("HEAD"), "A", t, &[]);
        let a_c = repo.find_commit(a).expect("A");
        let b = commit_at(&repo, Some("HEAD"), "B", t + 10, &[&a_c]);
        let b_c = repo.find_commit(b).expect("B");
        let c = commit_at(&repo, Some("HEAD"), "C", t + 20, &[&b_c]);
        let c_c = repo.find_commit(c).expect("C");
        let d = commit_at(&repo, Some("HEAD"), "D", t + 30, &[&c_c]);
        let d_c = repo.find_commit(d).expect("D");
        let e = commit_at(&repo, Some("HEAD"), "E", t + 40, &[&d_c]);
        let e_c = repo.find_commit(e).expect("E");
        for (tag, oid) in [("v1", a), ("v2", c), ("v3", e)] {
            let obj = repo.find_object(oid, None).expect("object");
            repo.tag_lightweight(tag, &obj, false).expect("tag");
        }
        drop((a_c, b_c, c_c, d_c, e_c));
        (dir, [a, b, c, d, e])
    }

    /// §7.1: `ChangelogRange` deserializes the EXACT JSON the TS union sends.
    #[test]
    fn changelog_range_deserializes_each_variant() {
        let br: ChangelogRange =
            serde_json::from_str(r#"{"kind":"betweenRefs","from":"v1","to":"v2"}"#)
                .expect("betweenRefs");
        match br {
            ChangelogRange::BetweenRefs { from, to } => {
                assert_eq!(from, "v1");
                assert_eq!(to, "v2");
            }
            other => panic!("expected BetweenRefs, got {other:?}"),
        }

        // `target` omitted => None (via #[serde(default)]).
        let slt: ChangelogRange =
            serde_json::from_str(r#"{"kind":"sinceLastTag"}"#).expect("sinceLastTag");
        match slt {
            ChangelogRange::SinceLastTag { target } => assert_eq!(target, None),
            other => panic!("expected SinceLastTag, got {other:?}"),
        }

        // `target` present as a string.
        let slt2: ChangelogRange =
            serde_json::from_str(r#"{"kind":"sinceLastTag","target":"HEAD"}"#)
                .expect("sinceLastTag target");
        match slt2 {
            ChangelogRange::SinceLastTag { target } => assert_eq!(target.as_deref(), Some("HEAD")),
            other => panic!("expected SinceLastTag, got {other:?}"),
        }
    }

    /// §7.2: serde casing matches the TS `AiChangelog`
    /// (`text`/`fromRef`/`toRef`/`commitCount`/`costUsd`; `None` => `null`).
    #[test]
    fn changelog_wire_shape_is_camel_case() {
        let v = serde_json::to_value(AiChangelog {
            text: "notes".to_string(),
            from_ref: "v1.2.0".to_string(),
            to_ref: "v1.3.0".to_string(),
            commit_count: 4,
            cost_usd: Some(0.012),
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "text": "notes",
                "fromRef": "v1.2.0",
                "toRef": "v1.3.0",
                "commitCount": 4,
                "costUsd": 0.012
            })
        );

        let v = serde_json::to_value(AiChangelog {
            text: "n".to_string(),
            from_ref: "a".to_string(),
            to_ref: "b".to_string(),
            commit_count: 0,
            cost_usd: None,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "text": "n",
                "fromRef": "a",
                "toRef": "b",
                "commitCount": 0,
                "costUsd": null
            })
        );
    }

    /// §7.3: `resolve_last_tag` finds the most recent EARLIER tag, excludes a tag
    /// AT the target's tip, returns None when none is earlier, and picks the
    /// nearest reachable earlier tag for a target between two tags.
    #[test]
    fn resolve_last_tag_finds_previous() {
        let (dir, [a, _b, c, d, e]) = tag_fixture();
        let repo = git2::Repository::open(dir.path()).expect("open");

        // From E: v3@E is excluded (points at E); previous reachable = v2@C.
        assert_eq!(
            resolve_last_tag(&repo, e).expect("resolve E"),
            Some(("v2".to_string(), c))
        );
        // From A: v1@A is excluded (points at A); no earlier tag reachable.
        assert_eq!(resolve_last_tag(&repo, a).expect("resolve A"), None);
        // From D (between C and E): nearest reachable earlier tag = v2@C
        // (v3@E is not an ancestor of D).
        assert_eq!(
            resolve_last_tag(&repo, d).expect("resolve D"),
            Some(("v2".to_string(), c))
        );
    }

    /// §7.5: `SinceLastTag{target:"v3"}` maps to_ref = the target ("v3") and
    /// from_ref = the previous tag reachable from v3's tip = "v2". Verified at the
    /// resolver level (pure); the end-to-end from/to echo is covered in
    /// `tests/ai_changelog_cli.rs`.
    #[test]
    fn since_last_tag_maps_to_previous_tag() {
        let (dir, [_a, _b, c, _d, e]) = tag_fixture();
        let repo = git2::Repository::open(dir.path()).expect("open");
        let v3_tip = repo
            .revparse_single("v3")
            .expect("v3")
            .peel_to_commit()
            .expect("commit")
            .id();
        assert_eq!(v3_tip, e, "v3 points at E");
        let (from_tag, from_oid) = resolve_last_tag(&repo, v3_tip)
            .expect("resolve")
            .expect("a previous tag exists");
        assert_eq!(from_tag, "v2");
        assert_eq!(from_oid, c);
    }

    /// §7.6: `from == to` => `AiFailed("no changes …")` BEFORE any CLI call.
    /// `BONSAI_CLAUDE_BIN` points at a nonexistent path: a regressed spawn would
    /// return `AiUnavailable` (a DIFFERENT variant), so the precise `AiFailed`
    /// assertion proves the pre-CLI bail.
    #[test]
    fn empty_range_fails_before_cli() {
        let _g = env_lock();
        std::env::set_var(ai::CLAUDE_BIN_ENV, "D:/nonexistent/claude-must-not-spawn.exe");

        let (dir, _) = tag_fixture();
        let err = generate_changelog(
            dir.path(),
            ChangelogRange::BetweenRefs {
                from: "v3".to_string(),
                to: "v3".to_string(),
            },
            RunOpts::default(),
        )
        .expect_err("empty range must fail");
        std::env::remove_var(ai::CLAUDE_BIN_ENV);

        match err {
            AppError::AiFailed(m) => assert_eq!(m, "no changes between v3 and v3"),
            other => panic!("expected AiFailed (pre-CLI), got {other:?} — a spawn would be AiUnavailable"),
        }
    }

    /// §7.7: an untagged repo => `SinceLastTag` => `AiFailed` BEFORE any CLI call
    /// (fake bin would surface as `AiUnavailable` if spawned).
    #[test]
    fn no_earlier_tag_fails_before_cli() {
        let _g = env_lock();
        std::env::set_var(ai::CLAUDE_BIN_ENV, "D:/nonexistent/claude-must-not-spawn.exe");

        let dir = init_scratch();
        let repo = git2::Repository::open(dir.path()).expect("open");
        let t = 1_700_000_000i64;
        let a = commit_at(&repo, Some("HEAD"), "A", t, &[]);
        let a_c = repo.find_commit(a).expect("A");
        let _b = commit_at(&repo, Some("HEAD"), "B", t + 10, &[&a_c]);
        drop(a_c);

        let err = generate_changelog(
            dir.path(),
            ChangelogRange::SinceLastTag { target: None },
            RunOpts::default(),
        )
        .expect_err("no earlier tag must fail");
        std::env::remove_var(ai::CLAUDE_BIN_ENV);

        match err {
            AppError::AiFailed(m) => assert!(
                m.contains("no earlier tag found before HEAD"),
                "got {m}"
            ),
            other => panic!("expected AiFailed (pre-CLI), got {other:?} — a spawn would be AiUnavailable"),
        }
    }

    /// §7.8: the prompt consts MUST be single-line (Windows argv constraint).
    #[test]
    fn prompts_are_single_line() {
        for s in [CHANGELOG_SYSTEM_PROMPT, CHANGELOG_PROMPT] {
            assert!(!s.contains('\n'), "prompt must be single-line: {s:?}");
            assert!(!s.contains('\r'), "prompt must be single-line: {s:?}");
        }
    }
}
