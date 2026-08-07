//! Commit / content search (P50a).
//!
//! One entry point — [`search_commits`] — dispatches by [`SearchField`]:
//!
//! * **message / author / all** → a header-only git2 revwalk (no diff, no
//!   subprocess): read each commit's message / author identity and substring-
//!   match. Cheap and the common case. `all` = message OR author (message wins
//!   the `matched` label). The walk is bounded at [`MAX_SEARCH_SCAN`].
//! * **path / content** → shell out to `git log` through an injected
//!   [`GitRunner`] (git's TREESAME-pruned path walk + optimized pickaxe; parity
//!   with the CLI is near-tautological). `path` → `git log -- <pathspec>`;
//!   `content` → `git log -S<text>` (literal) or `-G<text>` (regex).
//!
//! Both paths cap results with the cap+1 trick (collect up to cap+1, set
//! `truncated = len > cap`, then truncate) — exact for either backend.
//!
//! Injection-safety (mirrors P49): every user string (`text`, `scope_ref`) is a
//! single argv element handed to `git` directly — never a shell — so a `;` or
//! `&&` in the query is literal, never a second command.
//!
//! v1 scope (orchestrator decisions on the contract's open questions): the
//! `regex` flag applies to CONTENT only; message/author/path are plain
//! substring/pathspec. `since`/`until` date scope is deferred and OMITTED from
//! the wire type entirely. Match metadata is a single `matched` field plus an
//! optional path-only `snippet`.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::AppError;

/// Default result count and hard cap (compact rows; a single `invoke`, no channel).
pub const MAX_SEARCH_RESULTS: u32 = 1000;
/// Upper bound on commits examined by the git2 revwalk before giving up with
/// `truncated = true` (message/author/all modes).
pub const MAX_SEARCH_SCAN: usize = 200_000;

// ---- wire types ---------------------------------------------------------------

/// Which field(s) to search. `All` = message OR author (both header-only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchField {
    All,
    Message,
    Author,
    Path,
    Content,
}

/// Which field actually matched a result row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchedField {
    Message,
    Author,
    Path,
    Content,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub text: String,
    pub field: SearchField,
    /// CONTENT only: false = `-S` literal, true = `-G` regex. Ignored elsewhere (v1).
    #[serde(default)]
    pub regex: bool,
    /// Default false ⇒ case-insensitive (`-i` for grep/author/`-G`); a `-S`
    /// literal is always case-sensitive regardless.
    #[serde(default)]
    pub case_sensitive: bool,
    /// 0 ⇒ [`MAX_SEARCH_RESULTS`]; otherwise clamped to that hard cap.
    #[serde(default)]
    pub max_results: u32,
    /// None ⇒ all refs (`git log --all` seeding); Some ⇒ walk only that ref/oid.
    #[serde(default)]
    pub scope_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMatch {
    /// Full 40-hex — feeds `revealCommitByOid`; the row is derived frontend-side.
    pub oid: String,
    /// First message line, capped at 120 chars.
    pub summary: String,
    pub author_name: String,
    /// Author time, seconds since epoch.
    pub author_ts: i64,
    /// Which field matched. In `all` mode Message wins over Author.
    pub matched: MatchedField,
    /// v1: the matched pathspec for Path mode; None otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    /// Newest-first (commit-date desc, same as `git log`).
    pub matches: Vec<SearchMatch>,
    /// A cap or scan-bound was hit — "there may be more".
    pub truncated: bool,
}

// ---- injected git runner ------------------------------------------------------

/// Injected so the argv-builder + parser are unit-testable without launching
/// git; the oracle tests drive the real [`SpawnGitRunner`] against a fixture.
pub trait GitRunner {
    /// Run `git <args>` in `cwd`; return stdout (utf8-lossy). A spawn failure or
    /// non-zero exit is an [`AppError::Git`] carrying a stderr tail.
    fn run(&self, args: &[String], cwd: &Path) -> Result<String, AppError>;
}

/// Production runner: capture stdout, never prompt (`GIT_TERMINAL_PROMPT=0`),
/// and suppress the transient console window on Windows (mirrors `remote.rs`).
pub struct SpawnGitRunner;

impl GitRunner for SpawnGitRunner {
    fn run(&self, args: &[String], cwd: &Path) -> Result<String, AppError> {
        let mut cmd = Command::new("git");
        cmd.args(args)
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let output = cmd
            .output()
            .map_err(|e| AppError::Git(format!("failed to run `git log`: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Git(format!(
                "`git log` failed: {}",
                tail_chars(stderr.trim(), 400)
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

// ---- entry point --------------------------------------------------------------

/// Blocking. Search commits reachable from `scope_ref` (or all refs) for `text`.
/// Empty/whitespace `text` ⇒ `Ok(empty)` with NO git spawned and NO repo opened.
/// Never panics; non-UTF-8 commit data is read lossily.
pub fn search_commits(
    workdir: &Path,
    runner: &dyn GitRunner,
    query: &SearchQuery,
) -> Result<SearchResults, AppError> {
    if query.text.trim().is_empty() {
        return Ok(SearchResults {
            matches: Vec::new(),
            truncated: false,
        });
    }
    let cap = effective_cap(query);
    let (matches, truncated) = match query.field {
        SearchField::Path | SearchField::Content => {
            let stdout = runner.run(&build_log_args(query, cap), workdir)?;
            parse_log_output(&stdout, cap, query.field, &query.text)
        }
        SearchField::All | SearchField::Message | SearchField::Author => {
            let repo = open_repo_at(workdir)?;
            revwalk_search(&repo, query, cap)?
        }
    };
    Ok(SearchResults { matches, truncated })
}

// ---- pure helpers -------------------------------------------------------------

/// `q.max_results == 0` ⇒ the default cap; otherwise clamp to the hard cap.
fn effective_cap(q: &SearchQuery) -> u32 {
    if q.max_results == 0 {
        MAX_SEARCH_RESULTS
    } else {
        q.max_results.min(MAX_SEARCH_RESULTS)
    }
}

/// Char-safe last-`max` characters of `s` (panic-free stderr tail).
fn tail_chars(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    let start = chars.len().saturating_sub(max);
    chars[start..].iter().collect()
}

/// First line of `s`, capped char-safe at `max` chars.
fn first_line_capped(s: &str, max: usize) -> String {
    s.lines().next().unwrap_or("").chars().take(max).collect()
}

/// `git log` argv for Path / Content. `text` and `scope_ref` are each a single
/// argv token (injection-safe). US (0x1f) separates the 4 record fields; the
/// cap+1 `--max-count` drives exact truncation detection.
fn build_log_args(q: &SearchQuery, cap: u32) -> Vec<String> {
    let mut args: Vec<String> = vec![
        // `--glob-pathspecs` is a MAIN git option (glob magic for pathspecs) and
        // MUST precede the subcommand — `git log --glob-pathspecs` is rejected.
        "--glob-pathspecs".to_string(),
        "log".to_string(),
        "--format=%H%x1f%s%x1f%an%x1f%at".to_string(),
        "--max-count".to_string(),
        (cap + 1).to_string(),
    ];
    if !q.case_sensitive {
        // Affects --grep/--author/-G; harmless for the literal -S.
        args.push("-i".to_string());
    }
    let scope = q.scope_ref.clone().unwrap_or_else(|| "--all".to_string());
    match q.field {
        SearchField::Path => {
            args.push(scope);
            args.push("--".to_string());
            args.push(q.text.clone());
        }
        SearchField::Content => {
            let flag = if q.regex { "-G" } else { "-S" };
            args.push(format!("{flag}{}", q.text));
            args.push(scope);
        }
        // Message/Author/All never shell out.
        SearchField::Message | SearchField::Author | SearchField::All => {}
    }
    args
}

/// Parse the US-separated `git log` records. `truncated` iff more than `cap`
/// records were returned (the `--max-count = cap+1` overflow row); the extra is
/// dropped. `matched`/`snippet` follow `field` (Path ⇒ snippet = the pathspec).
fn parse_log_output(
    stdout: &str,
    cap: u32,
    field: SearchField,
    text: &str,
) -> (Vec<SearchMatch>, bool) {
    let matched = match field {
        SearchField::Path => MatchedField::Path,
        _ => MatchedField::Content,
    };
    let snippet_src = if matches!(field, SearchField::Path) {
        Some(text.to_string())
    } else {
        None
    };
    let mut out: Vec<SearchMatch> = Vec::new();
    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }
        let mut it = line.splitn(4, '\u{1f}');
        let oid = it.next().unwrap_or("");
        let summary = it.next().unwrap_or("");
        let author_name = it.next().unwrap_or("");
        let author_ts = it.next().unwrap_or("").trim().parse::<i64>().unwrap_or(0);
        if oid.is_empty() {
            continue;
        }
        out.push(SearchMatch {
            oid: oid.to_string(),
            summary: first_line_capped(summary, 120),
            author_name: author_name.to_string(),
            author_ts,
            matched,
            snippet: snippet_src.clone(),
        });
    }
    let truncated = out.len() as u32 > cap;
    out.truncate(cap as usize);
    (out, truncated)
}

// ---- git2 revwalk (message / author / all) ------------------------------------

/// Opens the repo at `workdir` with `NO_SEARCH` (same as every git/ module).
fn open_repo_at(workdir: &Path) -> Result<git2::Repository, AppError> {
    Ok(git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?)
}

/// `format!("{name} <{email}>")` from a commit's author, matching what git
/// `--author` tests against (so the oracle lines up). Lossy for non-UTF-8.
fn author_ident(c: &git2::Commit) -> String {
    let a = c.author();
    format!(
        "{} <{}>",
        String::from_utf8_lossy(a.name_bytes()),
        String::from_utf8_lossy(a.email_bytes())
    )
}

/// Substring test folding case when `!case_sensitive`.
fn contains_fold(haystack: &str, needle: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        haystack.contains(needle)
    } else {
        haystack.to_lowercase().contains(needle)
    }
}

/// message/author/all search over a git2 revwalk (header-only; no diff). Stops
/// at [`MAX_SEARCH_SCAN`] examined commits or `cap`+1 matches (both ⇒ truncated).
fn revwalk_search(
    repo: &git2::Repository,
    q: &SearchQuery,
    cap: u32,
) -> Result<(Vec<SearchMatch>, bool), AppError> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(git2::Sort::TIME)?; // commit-date desc ~ git log default
    match &q.scope_ref {
        Some(r) => push_scope(repo, &mut walk, r)?,
        None => seed_all_refs(repo, &mut walk)?,
    }

    // Fold the needle once; haystacks fold per-commit in contains_fold.
    let cs = q.case_sensitive;
    let needle = if cs { q.text.clone() } else { q.text.to_lowercase() };

    let mut out: Vec<SearchMatch> = Vec::new();
    let mut examined = 0usize;
    for oid in walk {
        examined += 1;
        if examined > MAX_SEARCH_SCAN {
            return Ok((out, true));
        }
        let oid = oid?;
        let c = repo.find_commit(oid)?;
        let msg = String::from_utf8_lossy(c.message_bytes());
        let (hit, which) = match q.field {
            SearchField::Message => (contains_fold(&msg, &needle, cs), MatchedField::Message),
            SearchField::Author => (
                contains_fold(&author_ident(&c), &needle, cs),
                MatchedField::Author,
            ),
            SearchField::All => {
                if contains_fold(&msg, &needle, cs) {
                    (true, MatchedField::Message)
                } else if contains_fold(&author_ident(&c), &needle, cs) {
                    (true, MatchedField::Author)
                } else {
                    (false, MatchedField::Message)
                }
            }
            // Path/Content never reach the revwalk.
            SearchField::Path | SearchField::Content => (false, MatchedField::Message),
        };
        if hit {
            let summary = first_line_capped(
                &String::from_utf8_lossy(c.summary_bytes().unwrap_or_default()),
                120,
            );
            out.push(SearchMatch {
                oid: oid.to_string(),
                summary,
                author_name: String::from_utf8_lossy(c.author().name_bytes()).into_owned(),
                author_ts: c.author().when().seconds(),
                matched: which,
                snippet: None,
            });
            if out.len() as u32 > cap {
                out.truncate(cap as usize);
                return Ok((out, true));
            }
        }
    }
    Ok((out, false))
}

/// Push a single scope revision (branch name / "HEAD" / oid) onto the walk.
fn push_scope(repo: &git2::Repository, walk: &mut git2::Revwalk, r: &str) -> Result<(), AppError> {
    let commit = repo.revparse_single(r)?.peel_to_commit()?;
    walk.push(commit.id())?;
    Ok(())
}

/// Seed the walk from all refs like `git log --all`: local + remote-tracking
/// branches (skip `*/HEAD`), tags peeled to a commit, and HEAD. Mirrors
/// `graph::collect_refs`. Unresolvable / non-committish refs are skipped.
fn seed_all_refs(repo: &git2::Repository, walk: &mut git2::Revwalk) -> Result<(), AppError> {
    for entry in repo.branches(Some(git2::BranchType::Local))? {
        let (b, _) = entry?;
        if let Ok(c) = b.get().peel_to_commit() {
            walk.push(c.id())?;
        }
    }
    for entry in repo.branches(Some(git2::BranchType::Remote))? {
        let (b, _) = entry?;
        if matches!(b.name(), Ok(Some(n)) if n.ends_with("/HEAD")) {
            continue;
        }
        if let Ok(c) = b.get().peel_to_commit() {
            walk.push(c.id())?;
        }
    }
    for entry in repo.references_glob("refs/tags/*")? {
        let reference = entry?;
        if let Ok(obj) = reference.peel(git2::ObjectType::Commit) {
            walk.push(obj.id())?;
        }
    }
    if let Ok(head) = repo.head() {
        if let Some(oid) = head.target() {
            walk.push(oid)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeSet;
    use std::process::Command;

    fn have_git() -> bool {
        Command::new("git").arg("--version").output().is_ok()
    }

    // ---------------------------------------------------------- query builders

    fn q(field: SearchField, text: &str) -> SearchQuery {
        SearchQuery {
            text: text.to_string(),
            field,
            regex: false,
            case_sensitive: false,
            max_results: 0,
            scope_ref: None,
        }
    }

    // ---------------------------------------------------------- fake runners

    /// Records every `run` argv and returns canned stdout — no git launched.
    struct FakeGitRunner {
        stdout: String,
        calls: RefCell<Vec<Vec<String>>>,
    }
    impl FakeGitRunner {
        fn new(stdout: &str) -> FakeGitRunner {
            FakeGitRunner {
                stdout: stdout.to_string(),
                calls: RefCell::new(Vec::new()),
            }
        }
    }
    impl GitRunner for FakeGitRunner {
        fn run(&self, args: &[String], _cwd: &Path) -> Result<String, AppError> {
            self.calls.borrow_mut().push(args.to_vec());
            Ok(self.stdout.clone())
        }
    }

    /// Panics if ever called — proves empty/whitespace `text` shells out to nothing.
    struct PanicRunner;
    impl GitRunner for PanicRunner {
        fn run(&self, _args: &[String], _cwd: &Path) -> Result<String, AppError> {
            panic!("runner must not be called");
        }
    }

    // ---------------------------------------------------------- arg building

    fn base_args(max_count: &str) -> Vec<String> {
        vec![
            "--glob-pathspecs".to_string(),
            "log".to_string(),
            "--format=%H%x1f%s%x1f%an%x1f%at".to_string(),
            "--max-count".to_string(),
            max_count.to_string(),
        ]
    }

    #[test]
    fn build_log_args_path_default() {
        let args = build_log_args(&q(SearchField::Path, "src/lib.rs"), 1000);
        let mut expected = base_args("1001");
        expected.extend(["-i", "--all", "--", "src/lib.rs"].map(String::from));
        assert_eq!(args, expected);
    }

    #[test]
    fn build_log_args_content_literal_default() {
        let args = build_log_args(&q(SearchField::Content, "needle"), 1000);
        let mut expected = base_args("1001");
        expected.extend(["-i", "-Sneedle", "--all"].map(String::from));
        assert_eq!(args, expected);
    }

    #[test]
    fn build_log_args_content_regex_case_sensitive() {
        let query = SearchQuery {
            text: "re.*x".to_string(),
            field: SearchField::Content,
            regex: true,
            case_sensitive: true,
            max_results: 0,
            scope_ref: None,
        };
        let args = build_log_args(&query, 1000);
        let mut expected = base_args("1001");
        // No -i (case-sensitive); -G flag (regex).
        expected.extend(["-Gre.*x", "--all"].map(String::from));
        assert_eq!(args, expected);
    }

    #[test]
    fn build_log_args_scope_ref_overrides_all() {
        let query = SearchQuery {
            scope_ref: Some("dev".to_string()),
            ..q(SearchField::Path, "f.txt")
        };
        let args = build_log_args(&query, 50);
        let mut expected = base_args("51");
        expected.extend(["-i", "dev", "--", "f.txt"].map(String::from));
        assert_eq!(args, expected);
    }

    #[test]
    fn build_log_args_metachars_stay_one_token() {
        // A `;`/space-bearing pathspec is exactly ONE argv token — never split,
        // never a second command.
        let path_args = build_log_args(&q(SearchField::Path, "a b; rm -rf /"), 1000);
        assert_eq!(path_args.last().unwrap(), "a b; rm -rf /");
        assert_eq!(path_args[path_args.len() - 2], "--");

        // Content: the whole needle rides inside the single `-S…` token.
        let content_args = build_log_args(&q(SearchField::Content, "x; rm -rf /"), 1000);
        assert!(content_args.contains(&"-Sx; rm -rf /".to_string()));
    }

    #[test]
    fn effective_cap_clamps_and_defaults() {
        assert_eq!(effective_cap(&q(SearchField::Message, "x")), MAX_SEARCH_RESULTS);
        let small = SearchQuery {
            max_results: 50,
            ..q(SearchField::Message, "x")
        };
        assert_eq!(effective_cap(&small), 50);
        let over = SearchQuery {
            max_results: 5000,
            ..q(SearchField::Message, "x")
        };
        assert_eq!(effective_cap(&over), MAX_SEARCH_RESULTS);
    }

    // ---------------------------------------------------------- parsing

    fn record(oid: &str, summary: &str, author: &str, ts: &str) -> String {
        format!("{oid}\u{1f}{summary}\u{1f}{author}\u{1f}{ts}")
    }

    #[test]
    fn parse_log_output_fills_fields_content() {
        let a = "a".repeat(40);
        let b = "b".repeat(40);
        let stdout = format!(
            "{}\n{}\n",
            record(&a, "add feature", "Ada", "1000"),
            record(&b, "fix bug", "Grace", "2000"),
        );
        let (matches, truncated) = parse_log_output(&stdout, 1000, SearchField::Content, "feat");
        assert!(!truncated);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].oid, a);
        assert_eq!(matches[0].summary, "add feature");
        assert_eq!(matches[0].author_name, "Ada");
        assert_eq!(matches[0].author_ts, 1000);
        assert_eq!(matches[0].matched, MatchedField::Content);
        assert_eq!(matches[0].snippet, None);
    }

    #[test]
    fn parse_log_output_path_sets_snippet() {
        let a = "a".repeat(40);
        let stdout = format!("{}\n", record(&a, "touch it", "Ada", "1000"));
        let (matches, _) = parse_log_output(&stdout, 1000, SearchField::Path, "src/x.rs");
        assert_eq!(matches[0].matched, MatchedField::Path);
        assert_eq!(matches[0].snippet.as_deref(), Some("src/x.rs"));
    }

    #[test]
    fn parse_log_output_truncates_at_cap_plus_one() {
        let mut stdout = String::new();
        for i in 0..3 {
            let oid = format!("{i:040x}");
            stdout.push_str(&record(&oid, "s", "Ada", "1"));
            stdout.push('\n');
        }
        let (matches, truncated) = parse_log_output(&stdout, 2, SearchField::Content, "s");
        assert!(truncated, "3 records with cap 2 ⇒ truncated");
        assert_eq!(matches.len(), 2);
    }

    // ---------------------------------------------------------- empty / cap (git2)

    #[test]
    fn empty_text_returns_ok_without_running_git() {
        // Whitespace text short-circuits BEFORE any subprocess (PanicRunner proves it).
        let out = search_commits(Path::new("."), &PanicRunner, &q(SearchField::Content, "   "))
            .expect("empty ⇒ Ok");
        assert!(out.matches.is_empty());
        assert!(!out.truncated);
    }

    // ---------------------------------------------------------- wire shapes

    #[test]
    fn search_results_wire_shape_camel_case() {
        let results = SearchResults {
            matches: vec![SearchMatch {
                oid: "a".repeat(40),
                summary: "hi".to_string(),
                author_name: "Ada".to_string(),
                author_ts: 1234,
                matched: MatchedField::Message,
                snippet: None,
            }],
            truncated: true,
        };
        let v = serde_json::to_value(&results).expect("json");
        let m = &v["matches"][0];
        assert_eq!(m["authorName"], "Ada");
        assert_eq!(m["authorTs"], 1234);
        assert_eq!(m["matched"], "message");
        // snippet omitted when None (skip_serializing_if).
        assert!(m.get("snippet").is_none());
        assert_eq!(v["truncated"], true);

        // Path snippet present + camelCase matched.
        let path_v = serde_json::to_value(SearchMatch {
            oid: "b".repeat(40),
            summary: "s".to_string(),
            author_name: "Grace".to_string(),
            author_ts: 1,
            matched: MatchedField::Path,
            snippet: Some("src/x.rs".to_string()),
        })
        .expect("json");
        assert_eq!(path_v["matched"], "path");
        assert_eq!(path_v["snippet"], "src/x.rs");
    }

    #[test]
    fn search_query_deserializes_with_defaults() {
        // Only text+field required; the rest default (regex/case false, cap 0, no scope).
        let query: SearchQuery =
            serde_json::from_value(serde_json::json!({ "text": "hi", "field": "author" }))
                .expect("deserialize");
        assert_eq!(query.text, "hi");
        assert_eq!(query.field, SearchField::Author);
        assert!(!query.regex);
        assert!(!query.case_sensitive);
        assert_eq!(query.max_results, 0);
        assert_eq!(query.scope_ref, None);
    }

    // ---------------------------------------------------------- oracle fixture

    /// Init a `main`-headed repo with a pinned identity + `core.autocrlf=false`
    /// (shared by the oracle fixtures so `git log` and the git2 revwalk agree on
    /// order and identity).
    fn init_repo(dir: &Path) -> git2::Repository {
        let repo = git2::Repository::init_opts(
            dir,
            git2::RepositoryInitOptions::new().initial_head("main"),
        )
        .expect("init repo");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test User").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
            cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        }
        repo
    }

    /// One commit built directly from `parent`'s tree + `files`, on HEAD
    /// (refs/heads/main), with BOTH author and committer time pinned to `t` so
    /// `git log` and the git2 revwalk share one deterministic order.
    fn mk_commit(
        repo: &git2::Repository,
        parent: Option<git2::Oid>,
        files: &[(&str, &str)],
        msg: &str,
        author: &str,
        t: i64,
    ) -> git2::Oid {
        let email = format!("{}@example.com", author.to_lowercase().replace(' ', "."));
        let sig = git2::Signature::new(author, &email, &git2::Time::new(t, 0)).expect("sig");
        let parent_commit = parent.map(|p| repo.find_commit(p).expect("parent"));
        let mut tb = match &parent_commit {
            Some(pc) => repo
                .treebuilder(Some(&pc.tree().expect("parent tree")))
                .expect("treebuilder"),
            None => repo.treebuilder(None).expect("treebuilder"),
        };
        for (name, content) in files {
            let blob = repo.blob(content.as_bytes()).expect("blob");
            tb.insert(name, blob, 0o100_644).expect("insert");
        }
        let tree = repo.find_tree(tb.write().expect("write tree")).expect("tree");
        let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
            .expect("commit")
    }

    /// Fixture: 4 commits on `main` with distinct timestamps, known messages /
    /// authors, and real blob edits; plus an `early` branch at C1 (scope subset).
    /// Returns the owning dir + `[c0, c1, c2, c3]`.
    fn build_fixture() -> (tempfile::TempDir, [git2::Oid; 4]) {
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        let c0 = mk_commit(&repo, None, &[("a.txt", "alpha\n")], "grace period work", "Ada Lovelace", 1000);
        let c1 = mk_commit(&repo, Some(c0), &[("b.txt", "beta\n")], "add beta module", "Grace Hopper", 2000);
        let c2 = mk_commit(&repo, Some(c1), &[("a.txt", "alpha and more\n")], "fix alpha work", "Ada Lovelace", 3000);
        let c3 = mk_commit(&repo, Some(c2), &[("c.txt", "gamma\n")], "Feature gamma", "Linus Torvalds", 4000);
        repo.branch("early", &repo.find_commit(c1).expect("c1"), false)
            .expect("branch early");
        (dir, [c0, c1, c2, c3])
    }

    /// Like [`mk_commit`] but attaches the new commit to NO ref (dangling), so the
    /// caller can point a remote-tracking ref or tag at it — letting a fixture put
    /// a commit out of reach of every LOCAL branch.
    fn mk_dangling(
        repo: &git2::Repository,
        parent: git2::Oid,
        files: &[(&str, &str)],
        msg: &str,
        author: &str,
        t: i64,
    ) -> git2::Oid {
        let email = format!("{}@example.com", author.to_lowercase().replace(' ', "."));
        let sig = git2::Signature::new(author, &email, &git2::Time::new(t, 0)).expect("sig");
        let parent_commit = repo.find_commit(parent).expect("parent");
        let mut tb = repo
            .treebuilder(Some(&parent_commit.tree().expect("parent tree")))
            .expect("treebuilder");
        for (name, content) in files {
            let blob = repo.blob(content.as_bytes()).expect("blob");
            tb.insert(name, blob, 0o100_644).expect("insert");
        }
        let tree = repo.find_tree(tb.write().expect("write tree")).expect("tree");
        repo.commit(None, &sig, &sig, msg, &tree, &[&parent_commit])
            .expect("commit")
    }

    /// Fixture exercising the FULL `seed_all_refs` seeding (not just local
    /// branches): `c_remote` is reachable ONLY via a remote-tracking ref
    /// (`refs/remotes/origin/feature`), `c_tag` ONLY via a lightweight tag
    /// (`refs/tags/v1.0`), and an `origin/HEAD` symbolic ref is present so the
    /// `*/HEAD`-skip branch is exercised (and must not break the walk). All three
    /// real commits carry "work" in the message so an all-refs search is
    /// cross-checkable against `git log --all --grep=work`. Distinct timestamps
    /// (1000/2000/3000) fix a deterministic newest-first order.
    /// Returns the owning dir + `[c_base, c_remote, c_tag]`.
    fn build_refs_fixture() -> (tempfile::TempDir, [git2::Oid; 3]) {
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        let c_base = mk_commit(
            &repo,
            None,
            &[("base.txt", "base\n")],
            "base setup work",
            "Ada Lovelace",
            1000,
        );
        // Reachable ONLY via a remote-tracking ref (no local branch points here).
        let c_remote = mk_dangling(
            &repo,
            c_base,
            &[("r.txt", "remote\n")],
            "remote feature work",
            "Ada Lovelace",
            2000,
        );
        repo.reference("refs/remotes/origin/feature", c_remote, false, "seed remote")
            .expect("remote ref");
        // A `*/HEAD` remote ref that seed_all_refs must SKIP (never peeled).
        repo.reference_symbolic(
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/feature",
            false,
            "seed origin/HEAD",
        )
        .expect("origin/HEAD");
        // Reachable ONLY via a tag.
        let c_tag = mk_dangling(
            &repo,
            c_base,
            &[("t.txt", "tag\n")],
            "tagged release work",
            "Ada Lovelace",
            3000,
        );
        let tag_obj = repo
            .find_object(c_tag, Some(git2::ObjectType::Commit))
            .expect("tag object");
        repo.tag_lightweight("v1.0", &tag_obj, false).expect("tag");
        (dir, [c_base, c_remote, c_tag])
    }

    /// oids our search returns, newest-first.
    fn our_oids(dir: &Path, query: &SearchQuery) -> Vec<String> {
        search_commits(dir, &SpawnGitRunner, query)
            .expect("search")
            .matches
            .into_iter()
            .map(|m| m.oid)
            .collect()
    }

    /// `git log …` full oids (newest-first, empty lines dropped).
    fn cli_oids(dir: &Path, args: &[&str]) -> Vec<String> {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("spawn git log");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect()
    }

    fn oid_hex(o: git2::Oid) -> String {
        o.to_string()
    }

    // ---------------------------------------------------------- oracle: git2 modes

    #[test]
    fn oracle_message_matches_cli_ordered() {
        if !have_git() {
            eprintln!("skipping: `git` CLI not found");
            return;
        }
        let (dir, [c0, _c1, c2, _c3]) = build_fixture();
        let ours = our_oids(dir.path(), &q(SearchField::Message, "work"));
        let cli = cli_oids(
            dir.path(),
            &["log", "--all", "-i", "-F", "--grep=work", "--format=%H"],
        );
        assert_eq!(ours, cli, "message search == git log --grep");
        // Newest-first C2 then C0 (both messages carry "work").
        assert_eq!(ours, vec![oid_hex(c2), oid_hex(c0)]);
    }

    #[test]
    fn oracle_message_case_sensitivity_differs() {
        if !have_git() {
            return;
        }
        let (dir, [_c0, _c1, _c2, c3]) = build_fixture();
        // Insensitive matches "Feature gamma".
        let ci = our_oids(dir.path(), &q(SearchField::Message, "feature"));
        let ci_cli = cli_oids(
            dir.path(),
            &["log", "--all", "-i", "-F", "--grep=feature", "--format=%H"],
        );
        assert_eq!(ci, ci_cli);
        assert_eq!(ci, vec![oid_hex(c3)]);
        // Sensitive matches nothing ("feature" != "Feature").
        let cs_query = SearchQuery {
            case_sensitive: true,
            ..q(SearchField::Message, "feature")
        };
        let cs = our_oids(dir.path(), &cs_query);
        let cs_cli = cli_oids(
            dir.path(),
            &["log", "--all", "-F", "--grep=feature", "--format=%H"],
        );
        assert_eq!(cs, cs_cli);
        assert!(cs.is_empty());
        assert_ne!(ci, cs, "case flag must change the result set");
    }

    #[test]
    fn oracle_author_matches_cli() {
        if !have_git() {
            return;
        }
        let (dir, [c0, _c1, c2, _c3]) = build_fixture();
        let ours = our_oids(dir.path(), &q(SearchField::Author, "ada"));
        let cli = cli_oids(
            dir.path(),
            &["log", "--all", "-i", "-F", "--author=ada", "--format=%H"],
        );
        assert_eq!(ours, cli);
        assert_eq!(ours, vec![oid_hex(c2), oid_hex(c0)]);
    }

    #[test]
    fn oracle_all_is_union_of_message_and_author() {
        if !have_git() {
            return;
        }
        let (dir, _) = build_fixture();
        let ours: BTreeSet<String> =
            our_oids(dir.path(), &q(SearchField::All, "grace")).into_iter().collect();
        let msg: BTreeSet<String> = cli_oids(
            dir.path(),
            &["log", "--all", "-i", "-F", "--grep=grace", "--format=%H"],
        )
        .into_iter()
        .collect();
        let author: BTreeSet<String> = cli_oids(
            dir.path(),
            &["log", "--all", "-i", "-F", "--author=grace", "--format=%H"],
        )
        .into_iter()
        .collect();
        let union: BTreeSet<String> = msg.union(&author).cloned().collect();
        assert_eq!(ours, union, "all == message ∪ author");
        // Non-degenerate: neither alone equals the union (message hits C0, author hits C1).
        assert_ne!(ours, msg);
        assert_ne!(ours, author);
    }

    #[test]
    fn oracle_scope_ref_is_a_subset() {
        if !have_git() {
            return;
        }
        let (dir, [c0, _c1, _c2, _c3]) = build_fixture();
        let scoped = SearchQuery {
            scope_ref: Some("early".to_string()),
            ..q(SearchField::Message, "work")
        };
        let ours = our_oids(dir.path(), &scoped);
        let cli = cli_oids(
            dir.path(),
            &["log", "early", "-i", "-F", "--grep=work", "--format=%H"],
        );
        assert_eq!(ours, cli);
        // early = C1..C0, so only C0 carries "work" (C2 is outside the scope).
        assert_eq!(ours, vec![oid_hex(c0)]);
    }

    #[test]
    fn oracle_message_cap_truncates() {
        if !have_git() {
            return;
        }
        let (dir, _) = build_fixture();
        // Every message contains "a"; cap at 2 ⇒ truncated with exactly 2 rows.
        let capped = SearchQuery {
            max_results: 2,
            ..q(SearchField::Message, "a")
        };
        let results = search_commits(dir.path(), &SpawnGitRunner, &capped).expect("search");
        assert!(results.truncated, "4 matches, cap 2 ⇒ truncated");
        assert_eq!(results.matches.len(), 2);
    }

    #[test]
    fn oracle_empty_results_is_ok() {
        if !have_git() {
            return;
        }
        let (dir, _) = build_fixture();
        let results =
            search_commits(dir.path(), &SpawnGitRunner, &q(SearchField::Message, "zzznotfound"))
                .expect("search");
        assert!(results.matches.is_empty());
        assert!(!results.truncated);
    }

    #[test]
    fn oracle_all_refs_seeds_remotes_and_tags() {
        // Cross-checks `seed_all_refs`' remote-tracking + tag + `*/HEAD`-skip
        // seeding, which build_fixture (local branches only) never exercised.
        if !have_git() {
            return;
        }
        let (dir, [c_base, c_remote, c_tag]) = build_refs_fixture();
        // Default scope = all refs. c_remote is reachable ONLY via origin/feature
        // and c_tag ONLY via the v1.0 tag, so a full-message search finding them
        // proves those refs are seeded — and it must match `git log --all` exactly.
        let ours = our_oids(dir.path(), &q(SearchField::Message, "work"));
        let cli = cli_oids(
            dir.path(),
            &["log", "--all", "-i", "-F", "--grep=work", "--format=%H"],
        );
        assert_eq!(ours, cli, "all-refs message search == git log --all --grep");
        // Newest-first by timestamp: c_tag(3000), c_remote(2000), c_base(1000).
        assert_eq!(ours, vec![oid_hex(c_tag), oid_hex(c_remote), oid_hex(c_base)]);
        // Spell out the load-bearing claim: the remote-only and tag-only commits
        // are present (not just reachable via the local `main` branch at c_base).
        assert!(ours.contains(&oid_hex(c_remote)), "remote-tracking ref seeded");
        assert!(ours.contains(&oid_hex(c_tag)), "tag ref seeded");
    }

    #[test]
    fn oracle_all_matched_label_message_wins() {
        // `all` mode checks message BEFORE author; when BOTH match, the row must
        // be labelled Message. (The union oracle ignores the `matched` label.)
        if !have_git() {
            return;
        }
        let dir = crate::testutil::scratch_dir();
        let repo = init_repo(dir.path());
        // "hopper" is in BOTH the message and the author (name + email).
        let c = mk_commit(
            &repo,
            None,
            &[("f.txt", "x\n")],
            "hopper refactor",
            "Grace Hopper",
            1000,
        );
        // CLI confirms the match is genuinely on both fields (not just our claim).
        let by_msg = cli_oids(
            dir.path(),
            &["log", "--all", "-i", "-F", "--grep=hopper", "--format=%H"],
        );
        let by_author = cli_oids(
            dir.path(),
            &["log", "--all", "-i", "-F", "--author=hopper", "--format=%H"],
        );
        assert_eq!(by_msg, vec![oid_hex(c)], "message matches");
        assert_eq!(by_author, vec![oid_hex(c)], "author matches");
        // Our `all` search returns the one commit, labelled Message (message wins).
        let results = search_commits(dir.path(), &SpawnGitRunner, &q(SearchField::All, "hopper"))
            .expect("search");
        assert_eq!(results.matches.len(), 1);
        assert_eq!(results.matches[0].oid, oid_hex(c));
        assert_eq!(results.matches[0].matched, MatchedField::Message);
    }

    // ---------------------------------------------------------- oracle: shell modes

    #[test]
    fn oracle_path_matches_cli() {
        if !have_git() {
            return;
        }
        let (dir, [c0, _c1, c2, _c3]) = build_fixture();
        let ours = our_oids(dir.path(), &q(SearchField::Path, "a.txt"));
        let cli = cli_oids(dir.path(), &["log", "--all", "--format=%H", "--", "a.txt"]);
        assert_eq!(ours, cli);
        assert_eq!(ours, vec![oid_hex(c2), oid_hex(c0)]);
    }

    #[test]
    fn oracle_content_pickaxe_s_matches_cli() {
        if !have_git() {
            return;
        }
        let (dir, _) = build_fixture();
        let ours = our_oids(dir.path(), &q(SearchField::Content, "alpha"));
        let cli = cli_oids(dir.path(), &["log", "--all", "--format=%H", "-Salpha"]);
        assert_eq!(ours, cli, "content -S == git log -S");
    }

    #[test]
    fn oracle_content_pickaxe_g_regex_matches_cli() {
        if !have_git() {
            return;
        }
        let (dir, _) = build_fixture();
        let regex_query = SearchQuery {
            regex: true,
            ..q(SearchField::Content, "al.ha")
        };
        let ours = our_oids(dir.path(), &regex_query);
        // -i is added (default case-insensitive) — mirror it in the oracle.
        let cli = cli_oids(dir.path(), &["log", "--all", "-i", "--format=%H", "-Gal.ha"]);
        assert_eq!(ours, cli, "content -G == git log -G");
        assert!(!ours.is_empty(), "regex should match the alpha edits");
    }

    #[test]
    fn oracle_shell_cap_truncates() {
        if !have_git() {
            return;
        }
        let (dir, [_c0, _c1, c2, _c3]) = build_fixture();
        // a.txt has 2 touching commits; cap 1 ⇒ --max-count 2 ⇒ truncated, 1 row.
        let capped = SearchQuery {
            max_results: 1,
            ..q(SearchField::Path, "a.txt")
        };
        let results = search_commits(dir.path(), &SpawnGitRunner, &capped).expect("search");
        assert!(results.truncated, "2 matches, cap 1 ⇒ truncated");
        assert_eq!(results.matches.len(), 1);
        assert_eq!(results.matches[0].oid, oid_hex(c2)); // newest kept
    }

    #[test]
    fn invalid_content_regex_is_git_error() {
        if !have_git() {
            return;
        }
        let (dir, _) = build_fixture();
        let bad = SearchQuery {
            regex: true,
            ..q(SearchField::Content, "[") // unterminated bracket ⇒ git exits non-zero
        };
        let err = search_commits(dir.path(), &SpawnGitRunner, &bad).expect_err("invalid regex");
        assert!(matches!(err, AppError::Git(_)), "got {err:?}");
    }

    // ---------------------------------------------------------- fake-runner argv

    #[test]
    fn spawn_path_passes_exact_argv_to_runner() {
        // The command dispatch hands the runner EXACTLY build_log_args' output.
        let runner = FakeGitRunner::new("");
        let query = q(SearchField::Path, "src/main.rs");
        let _ = search_commits(Path::new("/tmp/repo"), &runner, &query).expect("ok");
        let calls = runner.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], build_log_args(&query, MAX_SEARCH_RESULTS));
    }
}
