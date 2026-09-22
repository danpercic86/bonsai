//! P91 §7.1 — **strict-mode enforcement, applied by the writer, not trusted to
//! the producer.**
//!
//! ONE concern: making the sentence in every strict file's own header true —
//! *"safe to hand to a third party without reading it first"*.
//!
//! ## Why this exists as a writer-side pass
//!
//! `LogPayload::IpcCall::args` is *documented* as raw-mode-only and `error`
//! messages are *documented* as scrubbed, but documentation is not enforcement:
//! `log_append` accepts arbitrary records from the frontend, so a frontend bug, a
//! mode-toggle race, or any future emit site that forgets the rule would write
//! real paths, refs and URLs into a file whose header says `redaction: "strict"`.
//! The writer is the ONE place that knows the file's mode and sees every record,
//! so the guarantee is enforced there and cannot be bypassed by a producer.
//!
//! ## Order of operations (fixed by §7.2)
//!
//! `serialize → enforce (this module, strict only) → scrub_value (credentials,
//! both modes) → bytes`. The credential scrubber runs LAST so that a token which
//! only becomes visible after a name is replaced is still caught.
//!
//! ## What it does
//!
//! * drops `args` (raw-mode-only by §7.1) from every record;
//! * replaces, in every string field: URLs → `remote#N(scheme,forge)`, ref names
//!   → `ref#N(branch|remote|tag)`, and path-shaped runs → `path#N.ext`.
//!
//! Over-redaction is the deliberate failure direction. A message fragment that
//! merely *looks* like a path becomes `path#N`; a leaked username does not become
//! anything. The ordinals still correlate (§7.2 (a)), so the anomaly rules — which
//! key off `cmd`, `argsHash`, counts and timings, never off content — are
//! unaffected, exactly as §5 promises.

use serde_json::Value;

use super::redact::{Kind, Redactor};

/// Characters that can appear inside a URL / path / ref run. `:` is included so
/// `https://…` and `C:\…` survive as ONE run and can be classified as a whole.
fn is_run_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '_' | '-' | '.' | '~' | '+' | '/' | '\\' | ':' | '@' | '%' | '#' | '?' | '&' | '='
        )
}

/// Trailing sentence punctuation is not part of the name it follows.
fn split_trailing_punctuation(run: &str) -> (&str, &str) {
    let end = run
        .trim_end_matches(['.', ',', ';', ':', ')', ']', '}', '!', '?', '\''])
        .len();
    run.split_at(end)
}

/// `github` / `gitlab` / `bitbucket` / `azure` / `other` — the forge KIND is kept
/// (§7.1 keeps `remote#1(https,github)`); the host itself is not.
fn forge_kind(host: &str) -> &'static str {
    let h = host.to_ascii_lowercase();
    if h.contains("github") {
        "github"
    } else if h.contains("gitlab") {
        "gitlab"
    } else if h.contains("bitbucket") {
        "bitbucket"
    } else if h.contains("dev.azure") || h.contains("visualstudio") {
        "azure"
    } else {
        "other"
    }
}

/// `refs/heads/x` → `branch`, `refs/remotes/…` → `remote`, `refs/tags/…` → `tag`.
fn ref_kind(name: &str) -> &'static str {
    let rest = name.trim_start_matches("refs/");
    match rest.split('/').next().unwrap_or("") {
        "heads" => "branch",
        "remotes" => "remote",
        "tags" => "tag",
        _ => "ref",
    }
}

/// Is this run path-shaped enough to redact?
///
/// One separator alone is not enough — `and/or` in a sentence is not a file. A
/// run qualifies when it is rooted (`/x`, `C:\x`, `\server`), OR has two or more
/// separators, OR carries a file extension.
fn is_path_shaped(run: &str) -> bool {
    let seps = run.chars().filter(|c| *c == '/' || *c == '\\').count();
    if seps == 0 {
        return false;
    }
    let rooted = run.starts_with('/')
        || run.starts_with('\\')
        || run.chars().nth(1) == Some(':') && run.len() > 2;
    let has_ext = run
        .rsplit(['/', '\\'])
        .next()
        .and_then(|f| f.rsplit_once('.'))
        .is_some_and(|(stem, ext)| {
            !stem.is_empty()
                && (1..=12).contains(&ext.len())
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
        });
    rooted || seps >= 2 || has_ext
}

/// Replaces every repo-identifying name in one string with its ordinal (§7.1).
pub fn redact_names(s: &str, r: &Redactor) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < chars.len() {
        if !is_run_char(chars[i]) {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && is_run_char(chars[i]) {
            i += 1;
        }
        let full: String = chars[start..i].iter().collect();
        let (run, tail) = split_trailing_punctuation(&full);
        out.push_str(&classify_run(run, r));
        out.push_str(tail);
    }
    out
}

fn classify_run(run: &str, r: &Redactor) -> String {
    if run.is_empty() {
        return String::new();
    }
    // 1. URL — keep scheme + forge kind, drop everything identifying.
    if let Some(idx) = run.find("://") {
        let scheme = &run[..idx];
        let after = &run[idx + 3..];
        let host = after
            .split(['/', '?', '#', '@'])
            .next_back()
            .unwrap_or(after);
        // `user@host` — the host is the segment after the last `@`.
        let host = after.split('@').next_back().unwrap_or(host);
        let host = host.split(['/', '?', '#']).next().unwrap_or(host);
        let tag = r.tag(Kind::Remote, run);
        return format!("{tag}({scheme},{})", forge_kind(host));
    }
    // 2. Fully-qualified ref name — keep the ref KIND (§7.1).
    if run.starts_with("refs/") {
        let tag = r.tag(Kind::Ref, run);
        return format!("{tag}({})", ref_kind(run));
    }
    // 3. Path — keep the extension only.
    if is_path_shaped(run) {
        return r.tag_path(run);
    }
    run.to_string()
}

/// Applies the strict-mode guarantee to a serialized record, IN PLACE.
///
/// Called by the writer only when the file's mode is `Strict`; in `raw` the
/// names are what the user opted into, and only the credential scrubber runs.
pub fn enforce(v: &mut Value, r: &Redactor) {
    if let Value::Object(map) = v {
        // `args` is raw-mode-only (§7.1 "Argument values: elided → argsHash +
        // argsShape"). Removed unconditionally: only `ipc.call` carries it, and a
        // producer that invents the key elsewhere must not get a free pass.
        map.remove("args");
        // P117 §2.2 point 4 — the record's `repo` base field is a canonical
        // `repoId`, i.e. an absolute worktree PATH, and it must reach a strict
        // file as `repo#N` and nothing else.
        //
        // A FIELD-NAME rule, deliberately, and NOT left to `redact_names`' shape
        // heuristic below: `is_run_char` splits runs on whitespace, so
        // `D:\Repos\my project` would ordinalise `D:\Repos\my` and leave
        // `project` in the clear. Do not relax this to "the generic walk already
        // redacts paths" — for this one field it demonstrably does not. The
        // payload is `#[serde(flatten)]`, so `repo` is a top-level key.
        //
        // Runs BEFORE `walk`, which then sees `repo#N` — separator-free, so not
        // path-shaped, so left alone. `Kind::Repo`'s own ordinal namespace keeps
        // `repo#3` unrelated to `path#3`.
        if let Some(Value::String(raw)) = map.get("repo") {
            let tag = r.tag(Kind::Repo, raw);
            map.insert("repo".to_string(), Value::String(tag));
        }
    }
    walk(v, r);
}

fn walk(v: &mut Value, r: &Redactor) {
    match v {
        Value::String(s) => {
            let next = redact_names(s, r);
            if &next != s {
                *s = next;
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|i| walk(i, r)),
        Value::Object(map) => map.iter_mut().for_each(|(_, val)| walk(val, r)),
        _ => {}
    }
}
