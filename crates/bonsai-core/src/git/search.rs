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
//! `&&` in the query is literal, never a second command. Additionally (audit
//! §2.6), a `scope_ref` starting with `-` is rejected up front AND the shell
//! argv carries `--end-of-options` before the scope token, so a
//! leading-dash value can never be parsed as a `git log` OPTION either (e.g.
//! `--output=<file>` — an arbitrary-file-write primitive). The `-S`/`-G`
//! pickaxe token stays BEFORE `--end-of-options` because it IS an option.
//!
//! v1 scope (orchestrator decisions on the contract's open questions): the
//! `regex` flag applies to CONTENT only; message/author/path are plain
//! substring/pathspec. `since`/`until` date scope is deferred and OMITTED from
//! the wire type entirely. Match metadata is a single `matched` field plus an
//! optional path-only `snippet`.

use std::path::Path;
use std::process::Stdio;

use crate::error::AppError;
use crate::gitbin;

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
    /// Default false ⇒ case-insensitive: `-i` applies to `--grep`/`--author`/
    /// `-G` AND the `-S` literal (git sets `DIFF_PICKAXE_IGNORE_CASE` for the
    /// pickaxe too — T2.6 F-A6-C pinned decision).
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
/// and suppress the transient console window on Windows (the latter now comes
/// from [`gitbin::git_command`], which also resolves the git executable even
/// when the inherited PATH is broken — P70).
pub struct SpawnGitRunner;

impl GitRunner for SpawnGitRunner {
    fn run(&self, args: &[String], cwd: &Path) -> Result<String, AppError> {
        // The subcommand ACTUALLY being run — this runner is shared by commit
        // search, `commit-graph write` (P52) and every other `&dyn GitRunner`
        // consumer, so hard-coding `log` in the messages was a lie (P70 §3.2).
        // Empty argv falls back to "" — NOT "git", which would render as the
        // nonsense ``failed to run `git git` ``.
        let subcmd = args.first().map(String::as_str).unwrap_or("");
        let mut cmd = gitbin::git_command();
        cmd.args(args)
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = cmd.output().map_err(|e| gitbin::spawn_error(subcmd, &e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Git(format!(
                "`git {subcmd}` failed: {}",
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
    // Defense-in-depth (audit §2.6): git itself refuses ref names starting
    // with `-`, so a leading-dash scope can only be an option-injection
    // attempt (or garbage) — reject it before it goes anywhere near an argv
    // or a revparse. Applies uniformly to all modes.
    if let Some(scope) = &query.scope_ref {
        if scope.starts_with('-') {
            return Err(AppError::Other(format!("invalid scope ref: {scope}")));
        }
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
        // Affects --grep/--author/-G AND the -S literal (DIFF_PICKAXE_IGNORE_CASE
        // — F-A6-C: -S under the default is case-INsensitive, by decision).
        args.push("-i".to_string());
    }
    let scope = q.scope_ref.clone().unwrap_or_else(|| "--all".to_string());
    // `--end-of-options` (audit §2.6) sits after every OPTION (format/count/
    // `-i`, and the `-S`/`-G` pickaxe token in Content mode) and before the
    // scope token, so a hostile `scope_ref` can never be parsed as a `git log`
    // option. NOTE: the default `--all` scope must stay an option, so the
    // marker is emitted only for an explicit (already leading-dash-rejected)
    // scope — belt-and-suspenders on top of that rejection.
    let end_of_options = q.scope_ref.is_some();
    match q.field {
        SearchField::Path => {
            if end_of_options {
                args.push("--end-of-options".to_string());
            }
            args.push(scope);
            args.push("--".to_string());
            args.push(q.text.clone());
        }
        SearchField::Content => {
            let flag = if q.regex { "-G" } else { "-S" };
            args.push(format!("{flag}{}", q.text));
            if end_of_options {
                args.push("--end-of-options".to_string());
            }
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
        // Per-commit degradation (audit §3.16, mirrors health.rs): one corrupt
        // commit/odb entry skips that row instead of aborting the whole search.
        let Ok(oid) = oid else { continue };
        let Ok(c) = repo.find_commit(oid) else { continue };
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
/// `graph::collect_refs`. Unresolvable / non-committish refs are skipped, and
/// so is a GARBLED entry (corrupt loose-ref file, invalid name) — one bad ref
/// must degrade to a skip, never abort the whole search (F-A6-D, same
/// best-effort rule as the per-commit path).
///
/// `pub(crate)` so `history_index` reuses the same all-refs seeding for its
/// reachable walk (P57 OQ9) instead of carrying a fourth private copy.
pub(crate) fn seed_all_refs(repo: &git2::Repository, walk: &mut git2::Revwalk) -> Result<(), AppError> {
    for entry in repo.branches(Some(git2::BranchType::Local))? {
        let Ok((b, _)) = entry else { continue }; // garbled ref → skip (F-A6-D)
        if let Ok(c) = b.get().peel_to_commit() {
            walk.push(c.id())?;
        }
    }
    for entry in repo.branches(Some(git2::BranchType::Remote))? {
        let Ok((b, _)) = entry else { continue }; // garbled ref → skip (F-A6-D)
        if matches!(b.name(), Ok(Some(n)) if n.ends_with("/HEAD")) {
            continue;
        }
        if let Ok(c) = b.get().peel_to_commit() {
            walk.push(c.id())?;
        }
    }
    for entry in repo.references_glob("refs/tags/*")? {
        let Ok(reference) = entry else { continue }; // garbled ref → skip (F-A6-D)
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
mod test_support;
#[cfg(test)]
mod tests_unit;
#[cfg(test)]
mod tests_oracle;
