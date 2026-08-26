//! AI commit composer — PROPOSE side (P54a). Gathers the HEAD→working-tree
//! change set, asks the local `claude` CLI to group the changed FILES into a
//! small number of logical commits, and returns a NORMALIZED, always-apply-able
//! proposal. WRITES NOTHING — the user reviews/edits the plan, then the separate
//! (non-AI) apply command (P54b) performs the mutation.
//!
//! Rust is the referee (contract §3.3, D3): [`parse_compose_response`] turns
//! ARBITRARY model output into a strict PARTITION of the real change set —
//! unknown paths dropped, overlaps first-wins, uncovered files collected into
//! `unassigned`, groups capped. Unparseable output is NOT an error; it degrades
//! to manual grouping. v1 is FILE-LEVEL (D2): each changed file belongs to at
//! most one group. Pure git2 + crate::ai.

use std::collections::HashSet;
use std::path::Path;

use crate::ai::payload;
use crate::ai::{self, RunOpts};
use crate::error::AppError;
use crate::git::ai_explain;
use crate::git::diff::FileDiff;

/// Cap on proposed groups (overflow folds into `unassigned`). Bounds output size
/// and keeps the review UI sane.
pub const MAX_COMPOSE_GROUPS: usize = 10;

/// System prompt (via `--append-system-prompt`): role + strict JSON output
/// contract (§3.1). SINGLE line — on Windows the `claude` CLI is a `.cmd` shim
/// and Rust's `Command` REFUSES an argv argument containing a newline. Multi-line
/// content only ever flows through the stdin payload.
const COMPOSE_SYSTEM_PROMPT: &str = "You are organizing a messy working tree into a small number of clean, logical git commits. Standard input lists the changed files (use these EXACT paths) and their diffs (HEAD vs working tree). Group the files into 1 to 10 logical commits so each commit is one coherent, self-contained change (a feature, a fix, a refactor, tests, docs, or formatting). Prefer a few well-scoped commits over many tiny ones. For each group write a Conventional Commits message: a short imperative summary of at most 72 characters, then, only if warranted, a blank line and brief bullet points explaining WHY the change was made. Assign every changed file to exactly one group; never invent a path that is not in the list; never place a file in two groups. Output ONLY a JSON object of the form {\"groups\":[{\"message\":\"...\",\"files\":[\"path\",...]}]} — no prose, no explanation, no markdown, no code fences.";

/// The `-p` positional prompt (§3.1, verbatim single line). Optional `guidance`
/// is appended by [`compose_prompt`] (whitespace-collapsed to stay single-line).
const COMPOSE_PROMPT: &str = "Group the changed files described on standard input into logical commits and return the JSON object.";

/// One proposed logical commit: a set of changed files + a message. v1 =
/// FILE-LEVEL (each changed file appears in exactly ONE group across the plan;
/// enforced by the normalizer/apply-validator). Both Serialize (proposal out)
/// and Deserialize (edited plan in, P54b) so the review UI round-trips one shape.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeGroup {
    /// Repo-relative paths (NEW path for renames), forward slashes.
    pub files: Vec<String>,
    /// Commit message (summary + optional body); trimmed.
    pub message: String,
}

/// The NORMALIZED composer proposal — ALWAYS an apply-able partition of the real
/// change set (§3.3), whatever the model returned. Serialize only.
///
/// NOTE: `Eq` is NOT derived (contract §2.1 shows it, but `Option<f64>` is not
/// `Eq`); we mirror the sibling `CommitMessageProposal`/`AiAnalysis` which carry
/// `cost_usd: Option<f64>` and derive `PartialEq` only.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposeProposal {
    pub groups: Vec<ComposeGroup>,
    /// Changed files the model did NOT place (or overflow past the group cap).
    /// Surfaced so nothing is silently dropped. Empty on full coverage.
    pub unassigned: Vec<String>,
    /// Human notes about what the normalizer changed (dropped unknown path,
    /// resolved an overlap first-wins, capped groups, unparseable output). For
    /// the UI info line; never an error.
    pub notes: Vec<String>,
    pub cost_usd: Option<f64>,
}

/// Pure, normalized result of [`parse_compose_response`] (no cost — the caller
/// attaches `cost_usd` from the CLI envelope). ALWAYS a disjoint cover of the
/// input `changed` set: `groups ∪ unassigned == changed`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedCompose {
    groups: Vec<ComposeGroup>,
    unassigned: Vec<String>,
    notes: Vec<String>,
}

/// Lenient deserialize shape for the model's JSON. Unknown fields ignored;
/// missing fields default — so partial/sloppy output still deserializes and the
/// NORMALIZER (not serde) enforces the real invariants.
#[derive(serde::Deserialize)]
struct RawCompose {
    #[serde(default)]
    groups: Vec<RawGroup>,
}

#[derive(serde::Deserialize)]
struct RawGroup {
    #[serde(default)]
    message: String,
    #[serde(default)]
    files: Vec<String>,
}

/// Blocking. Gathers the HEAD→working-tree change set, asks the CLI to group it
/// into logical commits, and returns a NORMALIZED, apply-able proposal.
/// - Clean tree (no changes) => `NothingToCommit` BEFORE any CLI call.
/// - CLI hard-failure (timeout/nonzero/empty) => `AiFailed` (propagated).
/// - CLI returned text => parse+normalize (never errors on bad grouping).
pub fn compose_commits(
    workdir: &Path,
    guidance: Option<&str>,
    opts: RunOpts,
) -> Result<ComposeProposal, AppError> {
    // 1. Gather the HEAD→working-tree change set (staged + unstaged + untracked),
    //    index-aware, in one pass (reuses P53's promoted helper). Clean tree =>
    //    NothingToCommit BEFORE any CLI call (mirrors gather_staged).
    let files = ai_explain::gather_worktree(workdir)?;
    if files.is_empty() {
        return Err(AppError::NothingToCommit);
    }

    // 2. The authoritative path list the referee validates the model against.
    let changed: Vec<String> = files.iter().map(|f| f.path.clone()).collect();

    // 3. Grounding: the exact path list (constrains the model to real paths, cuts
    //    hallucination) + per-file hunk diffs (intent context). One combined
    //    byte-cap over the whole string keeps a pathological long-line diff bounded.
    let payload_text = ai_explain::cap_review_payload(render_grounding(&changed, &files));

    // 4. Ask the CLI (system prompt set here; caller's `opts` carry model/timeout).
    //    `?` propagates a CLI hard-failure as AiFailed.
    let result = ai::run_claude(
        workdir,
        &compose_prompt(guidance),
        Some(&payload_text),
        RunOpts {
            system_prompt: Some(COMPOSE_SYSTEM_PROMPT.to_string()),
            ..opts
        },
    )?;

    // 5. Referee: normalize arbitrary model output into an apply-able partition of
    //    `changed` (never errors — bad grouping degrades to manual assignment).
    let parsed = parse_compose_response(&result.text, &changed);

    Ok(ComposeProposal {
        groups: parsed.groups,
        unassigned: parsed.unassigned,
        notes: parsed.notes,
        cost_usd: result.cost_usd,
    })
}

/// Renders the grounding payload (§3.2): the labeled CHANGED FILES path list
/// followed by the per-file hunk diffs. `changed` is the authoritative list so
/// the model sees EXACTLY the paths the normalizer will validate against.
fn render_grounding(changed: &[String], files: &[FileDiff]) -> String {
    let mut s = String::from("WORKING CHANGES (HEAD vs working tree):\n\n");
    s.push_str("CHANGED FILES (assign each to exactly one group; use these exact paths):\n");
    for p in changed {
        s.push_str(p);
        s.push('\n');
    }
    s.push_str("\nDIFFS:\n");
    s.push_str(&payload::render_file_diffs(files).text);
    s
}

/// Builds the `-p` prompt, appending optional user `guidance` (§3.1, OQ3). The
/// guidance is free text (never a path/arg); all whitespace runs — including
/// newlines — are collapsed to single spaces so the argv prompt stays SINGLE
/// line (Windows `claude.cmd` rejects a newline-bearing argument).
fn compose_prompt(guidance: Option<&str>) -> String {
    match guidance {
        Some(g) => {
            let collapsed = g.split_whitespace().collect::<Vec<_>>().join(" ");
            if collapsed.is_empty() {
                COMPOSE_PROMPT.to_string()
            } else {
                format!("{COMPOSE_PROMPT} Extra guidance: {collapsed}")
            }
        }
        None => COMPOSE_PROMPT.to_string(),
    }
}

/// Extracts a candidate JSON substring from raw model text (§3.3 step 1): drops
/// triple-backtick code-fence lines, trims, then takes the bracket span that OPENS first — an
/// object `{...}` (the normal `{"groups":[...]}` shape) or a bare array `[...]`
/// (the `{`-inside-a-bare-array case is handled by comparing opening positions).
/// Surrounding prose is stripped because it lies outside the outermost brackets.
fn extract_json(raw: &str) -> String {
    let de_fenced: String = raw
        .lines()
        .filter(|l| !l.trim_start().starts_with("```"))
        .collect::<Vec<_>>()
        .join("\n");
    let s = de_fenced.trim();

    let obj = match (s.find('{'), s.rfind('}')) {
        (Some(i), Some(j)) if i <= j => Some((i, j)),
        _ => None,
    };
    let arr = match (s.find('['), s.rfind(']')) {
        (Some(i), Some(j)) if i <= j => Some((i, j)),
        _ => None,
    };
    match (obj, arr) {
        // Both bracket kinds present: pick whichever OPENS first. A bare array
        // `[{...}]` opens with '[' before its inner '{'; an object opens with '{'.
        (Some((oi, oj)), Some((ai, aj))) => {
            if ai < oi {
                s[ai..=aj].to_string()
            } else {
                s[oi..=oj].to_string()
            }
        }
        (Some((oi, oj)), None) => s[oi..=oj].to_string(),
        (None, Some((ai, aj))) => s[ai..=aj].to_string(),
        (None, None) => s.to_string(),
    }
}

/// Repo-relative path normalization for a model-returned path (§3.3): trim, then
/// backslashes → forward slashes (the grounding paths are always forward-slash).
fn normalize_path(p: &str) -> String {
    p.trim().replace('\\', "/")
}

/// THE REFEREE (§3.3, D3). Pure. Turns arbitrary model text into a strict
/// PARTITION of `changed`:
/// - unparseable => `groups:[]`, `unassigned == changed` (input order) + a note;
/// - normalize each path; drop paths not in `changed` (+note); overlap =>
///   first-wins (+note); empty group => dropped (+note); cap at
///   [`MAX_COMPOSE_GROUPS`] (tail folded into `unassigned` +note).
///
/// INVARIANT: `groups ∪ unassigned == changed`, disjoint, `unassigned` preserves
/// `changed` order. The proposal is therefore ALWAYS apply-able as-is.
fn parse_compose_response(raw: &str, changed: &[String]) -> ParsedCompose {
    let changed_set: HashSet<&str> = changed.iter().map(String::as_str).collect();

    // Extract + lenient deserialize (object shape first, then a bare array).
    let json = extract_json(raw);
    let parsed = serde_json::from_str::<RawCompose>(&json).or_else(|_| {
        serde_json::from_str::<Vec<RawGroup>>(&json).map(|groups| RawCompose { groups })
    });

    let parsed = match parsed {
        Ok(p) => p,
        Err(_) => {
            // NOT a hard error (D3): degrade to manual grouping.
            return ParsedCompose {
                groups: Vec::new(),
                unassigned: changed.to_vec(),
                notes: vec![
                    "AI output could not be parsed into groups; group the files manually."
                        .to_string(),
                ],
            };
        }
    };

    // Normalize into a PARTITION of `changed`.
    let mut assigned: HashSet<String> = HashSet::new();
    let mut groups: Vec<ComposeGroup> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    for rg in parsed.groups {
        if groups.len() == MAX_COMPOSE_GROUPS {
            notes.push("reached the group limit; remaining files left unassigned".to_string());
            break;
        }
        let mut files: Vec<String> = Vec::new();
        for p in &rg.files {
            let q = normalize_path(p);
            if q.is_empty() {
                continue;
            }
            if !changed_set.contains(q.as_str()) {
                notes.push(format!("dropped unknown path {q}"));
                continue;
            }
            if assigned.contains(&q) {
                notes.push(format!(
                    "path {q} already assigned; kept in the earlier group"
                ));
                continue;
            }
            assigned.insert(q.clone());
            files.push(q);
        }
        if files.is_empty() {
            notes.push("dropped an empty group".to_string());
            continue;
        }
        // Empty message is KEPT here (OQ5) — the UI requires a message before
        // "Create" is enabled and apply rejects EmptyMessage.
        groups.push(ComposeGroup {
            files,
            message: rg.message.trim().to_string(),
        });
    }

    // Never-mentioned + overflow files => unassigned, preserving `changed` order.
    let unassigned: Vec<String> = changed
        .iter()
        .filter(|c| !assigned.contains(c.as_str()))
        .cloned()
        .collect();

    ParsedCompose {
        groups,
        unassigned,
        notes,
    }
}

#[cfg(test)]
mod tests;
