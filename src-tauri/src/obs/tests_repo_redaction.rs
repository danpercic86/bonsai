//! P117 §2.2 point 4 / AC2-7 — what the record's `repo` base field is allowed
//! to look like ON DISK, in each redaction mode.
//!
//! Its own file rather than an addition to `tests_writer.rs` (507 lines, at its
//! size-ratchet ceiling) or `tests_strict.rs` (the generic shape heuristic): the
//! concern here is one FIELD-NAME rule and the specific gap that forced it.
//!
//! **The gap, stated once.** `strict::redact_names`' generic walk splits runs on
//! `is_run_char`, which excludes whitespace — so `D:\Repos\my project` is two
//! runs, and only the first is path-shaped. Left to the heuristic, `project`
//! reaches a strict file in the clear. A field-name rule inside
//! `strict::enforce` is therefore load-bearing and must not be relaxed to
//! "`strict::enforce` already redacts paths".

use std::path::Path;

use super::record::{LogLevel, LogPayload, LogRecord, LogSource, RedactionMode};
use super::writer::{Limits, LogWriter, WriterConfig};

/// A repoId with a SPACE in it — the shape the generic heuristic mis-handles.
const REPO: &str = r"C:\Users\jane\Repos\my project";

/// Every fragment of [`REPO`] that a strict line must not contain. `project` is
/// the one that catches the `is_run_char` whitespace gap — a line that only
/// fails `contains("my project")` would ALSO pass under the broken generic
/// heuristic, so it does not discriminate the field-name rule at all.
/// `Users`/`jane`/`C:` catch a missing rule outright.
const FRAGMENTS: &[&str] = &["my project", "project", "Repos", "Users", "jane", "C:"];

/// Asserts not one fragment of [`REPO`] survives in `text`. Used on BOTH the
/// Rust-sourced span line and the UI-sourced sink output — the weaker
/// single-fragment form on the UI path was the P117 audit's finding 5.
fn assert_no_path_fragment(text: &str, what: &str) {
    for fragment in FRAGMENTS {
        assert!(
            !text.contains(fragment),
            "{what} must not carry {fragment:?}: {text}"
        );
    }
}

fn test_redactor() -> std::sync::Arc<super::redact::Redactor> {
    std::sync::Arc::new(super::redact::Redactor::with_salt([9; 16]))
}

fn cfg(dir: &Path, redaction: RedactionMode, home_mask: Option<&str>) -> WriterConfig {
    WriterConfig {
        dir: dir.to_path_buf(),
        session_id: "sp117repo".into(),
        started_secs: 1_772_200_991,
        app_version: "1.6.0".into(),
        os: "windows".into(),
        level: LogLevel::Debug,
        redaction,
        home_mask: home_mask.map(str::to_string),
        limits: Limits::default(),
    }
}

/// A `span{op:"graph.get"}` carrying `repo` — the Rust-side producer's shape.
fn span_rec(repo: &str) -> LogRecord {
    LogRecord {
        seq: 0,
        ts: 1_772_200_991_000,
        mono: 7,
        src: LogSource::Rust,
        lvl: LogLevel::Debug,
        trace: Some("t1".into()),
        span: None,
        caused_by: None,
        repo: Some(repo.to_string()),
        payload: LogPayload::Span {
            op: "graph.get".into(),
            ms: 12.0,
            phases: None,
            queued_ms: None,
            pool_inflight: None,
            pool_max: None,
            deadline_frac: None,
            cache: Some("miss".into()),
            items: None,
            outcome: Some("ok".into()),
        },
    }
}

/// Writes `recs` in `mode` and returns the non-header lines.
fn written(mode: RedactionMode, home_mask: Option<&str>, recs: Vec<LogRecord>) -> Vec<String> {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut w = LogWriter::open(cfg(dir.path(), mode, home_mask), test_redactor()).expect("open");
    let name = w.active_file().to_string();
    for r in recs {
        w.write_record(r).expect("write");
    }
    w.flush().expect("flush");
    drop(w);
    std::fs::read_to_string(dir.path().join(&name))
        .expect("read log file")
        .lines()
        .skip(1) // the session header
        .map(str::to_string)
        .collect()
}

/// AC2-7 (a) — strict mode: `repo#N` and NOT ONE FRAGMENT of the path anywhere
/// in the line. `project` is the assertion that catches the `is_run_char`
/// whitespace gap; `Users`/`jane` catch a missing rule outright.
#[test]
fn strict_mode_writes_repo_as_an_ordinal_with_no_path_fragment() {
    let lines = written(RedactionMode::Strict, None, vec![span_rec(REPO)]);
    let row: serde_json::Value = serde_json::from_str(&lines[0]).expect("valid JSON");
    let repo = row["repo"].as_str().expect("repo survives as a string");
    assert!(
        repo.starts_with("repo#") && repo[5..].chars().all(|c| c.is_ascii_digit()),
        "strict repo must match ^repo#\\d+$, got {repo:?}"
    );
    assert_no_path_fragment(&lines[0], "the strict span line");
}

/// AC2-7 (b) — stable within a session: the same repoId gets the same ordinal,
/// which is what lets two records of one repo be recognised as one repo by a
/// reader of the file (the detector itself never sees this form).
#[test]
fn strict_mode_repo_ordinal_is_stable_within_a_session() {
    let lines = written(
        RedactionMode::Strict,
        None,
        vec![span_rec(REPO), span_rec(r"D:\Repos\other"), span_rec(REPO)],
    );
    let repo_of = |i: usize| {
        serde_json::from_str::<serde_json::Value>(&lines[i]).expect("valid JSON")["repo"]
            .as_str()
            .expect("repo")
            .to_string()
    };
    assert_eq!(repo_of(0), repo_of(2), "same repoId, same ordinal");
    assert_ne!(
        repo_of(0),
        repo_of(1),
        "different repoIds, different ordinals"
    );
}

/// AC2-7 (c) — raw mode: the real path, home-masked, exactly as `openRepo`'s
/// `args.path` already appears in a raw file. Nothing new is exposed, and the
/// space is preserved (no ordinalisation runs at all).
#[test]
fn raw_mode_writes_the_home_masked_path() {
    // Pre-FOLDED, as `home_resolve::normalize_home` hands it to the writer:
    // lowercase, `/` separators (`mask_home_with` compares against that form).
    let lines = written(
        RedactionMode::Raw,
        Some("c:/users/jane"),
        vec![span_rec(REPO)],
    );
    let row: serde_json::Value = serde_json::from_str(&lines[0]).expect("valid JSON");
    assert_eq!(row["repo"], r"<home>\Repos\my project");
    assert!(
        !lines[0].contains("jane"),
        "the OS account name is masked even in raw mode"
    );
}

/// AC2-7 (d), half one — the detector observes the RAW value because redaction
/// runs on a SEPARATE `serde_json::Value` copy. Asserted directly: enforcement
/// rewrites the copy and cannot reach the `LogRecord` the sink hands on to
/// `detector.observe`.
#[test]
fn strict_enforcement_cannot_mutate_the_in_memory_record() {
    let rec = span_rec(REPO);
    let r = test_redactor();
    let mut value = serde_json::to_value(&rec).expect("serialise");
    super::strict::enforce(&mut value, &r);
    assert_ne!(value["repo"], REPO, "the COPY is redacted");
    assert_eq!(
        rec.repo.as_deref(),
        Some(REPO),
        "the record the detector sees keeps the raw repoId"
    );
}

/// AC2-7 (d), half two — end to end through the real sink in STRICT mode: the
/// repo-keyed rule partitions correctly (same repo fires, two repos do not)
/// while the same file carries only `repo#N`.
#[test]
fn a_repo_keyed_rule_fires_correctly_in_strict_mode() {
    fn refresh_rec(ts: i64, repo: &str) -> LogRecord {
        LogRecord {
            seq: 0,
            ts,
            mono: 1,
            src: LogSource::Ui,
            lvl: LogLevel::Debug,
            trace: None,
            span: None,
            caused_by: None,
            repo: Some(repo.to_string()),
            payload: LogPayload::Refresh {
                round: 1,
                scope: "full".into(),
                origins: vec![],
                contributing_traces: vec![],
                collapsed: 0,
                ms: 1.0,
            },
        }
    }
    fn anomalies(pair: Vec<LogRecord>) -> (usize, String) {
        let dir = tempfile::tempdir().expect("tempdir");
        let sink =
            super::sink::Sink::start(cfg(dir.path(), RedactionMode::Strict, None)).expect("start");
        for r in pair {
            sink.enqueue(r);
        }
        sink.shutdown();
        let mut text = String::new();
        for (name, _) in super::writer::list_log_files(dir.path()) {
            let part = std::fs::read_to_string(dir.path().join(name)).expect("read log part");
            // The session header is dropped: it carries the pinned
            // `redactionNote` (which legitimately says the word "repo") and the
            // fragment loop below is about RECORD lines.
            for line in part.lines().skip(1) {
                text.push_str(line);
                text.push('\n');
            }
        }
        let n = text
            .lines()
            .filter(|l| l.contains("\"rule\":\"redundant-refresh\""))
            .count();
        (n, text)
    }
    let (same, text) = anomalies(vec![
        refresh_rec(1_772_200_991_000, REPO),
        refresh_rec(1_772_200_991_400, REPO),
    ]);
    assert_eq!(same, 1, "one repo, repeated scope — fires in strict mode");
    assert!(text.contains("repo#"), "and the file carries only ordinals");
    assert_no_path_fragment(&text, "the strict sink output (UI-sourced refresh)");

    let (cross, _) = anomalies(vec![
        refresh_rec(1_772_200_991_000, REPO),
        refresh_rec(1_772_200_991_141, r"D:\Repos\other"),
    ]);
    assert_eq!(cross, 0, "two repos — no firing, in strict mode too");
}

/// §2.2 — a record with no attribution must not gain a `repo` key on disk
/// (`skip_serializing_if`), in either mode: that is what keeps every other
/// record shape byte-identical to v2.
#[test]
fn an_unattributed_record_writes_no_repo_key() {
    for mode in [RedactionMode::Strict, RedactionMode::Raw] {
        let mut rec = span_rec(REPO);
        rec.repo = None;
        let lines = written(mode, None, vec![rec]);
        let row: serde_json::Value = serde_json::from_str(&lines[0]).expect("valid JSON");
        assert!(row.get("repo").is_none(), "{mode:?} emitted an empty repo");
    }
}

/// An `ipc.call` carrying `repo` — the UI-side producer's shape (`fetch` is one
/// of the 10 mutations review fix 1 gave a `repoId` position to). §2.2's
/// producer allow-list has three entries and this was the one no Rust test put
/// through the writer.
fn ipc_call_rec(repo: &str) -> LogRecord {
    LogRecord {
        seq: 0,
        ts: 1_772_200_991_000,
        mono: 3,
        src: LogSource::Ui,
        lvl: LogLevel::Debug,
        trace: Some("t2".into()),
        span: None,
        caused_by: None,
        repo: Some(repo.to_string()),
        payload: LogPayload::IpcCall {
            cmd: "fetch".into(),
            args_hash: "ah1".into(),
            args_shape: None,
            args: None,
            args_omitted: None,
        },
    }
}

/// Audit finding 5 — `enforce` is kind- and `src`-agnostic, so this is coverage
/// hardening of the property the whole design rests on rather than a hole being
/// closed: the third producer shape gets the same two assertions as the span.
#[test]
fn an_ipc_call_repo_is_redacted_like_every_other_producer() {
    let strict = written(RedactionMode::Strict, None, vec![ipc_call_rec(REPO)]);
    let row: serde_json::Value = serde_json::from_str(&strict[0]).expect("valid JSON");
    let repo = row["repo"].as_str().expect("repo survives as a string");
    assert!(
        repo.starts_with("repo#") && repo[5..].chars().all(|c| c.is_ascii_digit()),
        "strict ipc.call repo must match ^repo#\\d+$, got {repo:?}"
    );
    assert_no_path_fragment(&strict[0], "the strict ipc.call line");

    let raw = written(
        RedactionMode::Raw,
        Some("c:/users/jane"),
        vec![ipc_call_rec(REPO)],
    );
    let row: serde_json::Value = serde_json::from_str(&raw[0]).expect("valid JSON");
    assert_eq!(row["repo"], r"<home>\Repos\my project");
}

// ---- Audit finding 3: the writer-side scalar gate on `repo` ---------------

/// The free text a renderer bug could park in `repo` — the realistic shape is
/// reading `args[at]` after a signature change, so: long, and possibly
/// multi-line. Marked so a partial capture is detectable.
fn free_text(len: usize) -> String {
    "SECRETPROSE ".repeat(len / 12 + 1)
}

/// Raw mode is the exposing mode: an over-long `repo` is replaced wholesale, so
/// the channel is bounded free text in no mode. Mirrors the `args` gate exactly
/// (`is_allowed_scalar`) — "documentation is not enforcement" (A26):
/// `log_append` accepts arbitrary records from the frontend.
#[test]
fn raw_mode_replaces_an_over_long_repo_with_a_fixed_token() {
    let prose = free_text(600);
    assert!(prose.chars().count() > super::raw_args::RAW_ARG_MAX_STR);
    let mut rec = span_rec(REPO);
    rec.repo = Some(prose.clone());
    let lines = written(RedactionMode::Raw, None, vec![rec]);
    let row: serde_json::Value = serde_json::from_str(&lines[0]).expect("valid JSON");
    assert_eq!(row["repo"], "repo-rejected");
    assert!(
        !lines[0].contains(&prose[..20]),
        "not even a fragment of the rejected value: {}",
        lines[0]
    );
    // And it must not be readable as a strict-mode ordinal.
    assert!(!lines[0].contains("repo#"));
}

/// A multi-line value cannot forge a JSONL line (serde escapes control
/// characters), so this is about CAPTURE, not injection — and it is rejected for
/// the same reason an `args` value would be.
#[test]
fn raw_mode_replaces_a_multi_line_repo_with_a_fixed_token() {
    for raw in ["C:\\a\nSECRETPROSE", "C:\\a\rSECRETPROSE"] {
        let mut rec = span_rec(REPO);
        rec.repo = Some(raw.to_string());
        let lines = written(RedactionMode::Raw, None, vec![rec]);
        let row: serde_json::Value = serde_json::from_str(&lines[0]).expect("valid JSON");
        assert_eq!(row["repo"], "repo-rejected", "line: {}", lines[0]);
        assert!(!lines[0].contains("SECRETPROSE"));
    }
}

/// "Costs nothing in strict mode" is a claim, so it is proved: `strict::enforce`
/// runs FIRST and has already turned the value into `repo#N`, which passes the
/// gate — the same oversize input therefore still reaches disk as an ordinal,
/// not as the rejection token.
#[test]
fn strict_mode_is_unaffected_by_the_gate() {
    let mut rec = span_rec(REPO);
    rec.repo = Some(free_text(600));
    let lines = written(RedactionMode::Strict, None, vec![rec]);
    let row: serde_json::Value = serde_json::from_str(&lines[0]).expect("valid JSON");
    let repo = row["repo"].as_str().expect("repo survives as a string");
    assert!(
        repo.starts_with("repo#") && repo[5..].chars().all(|c| c.is_ascii_digit()),
        "strict ordinalises before the gate sees it, got {repo:?}"
    );
    assert!(!lines[0].contains("SECRETPROSE"));
}

/// The gate is a no-op for everything it exists to let through — a real repoId
/// (`raw_mode_writes_the_home_masked_path` above) and, at the boundary, a value
/// of EXACTLY the limit: `<=`, not `<`, so a deep-but-legitimate worktree path
/// is not silently replaced.
#[test]
fn a_repo_of_exactly_the_limit_passes_the_gate() {
    let max = super::raw_args::RAW_ARG_MAX_STR;
    let at_limit = format!(r"C:\Repos\{}", "d".repeat(max - 9));
    assert_eq!(at_limit.chars().count(), max);
    let mut rec = span_rec(REPO);
    rec.repo = Some(at_limit.clone());
    let lines = written(RedactionMode::Raw, None, vec![rec]);
    let row: serde_json::Value = serde_json::from_str(&lines[0]).expect("valid JSON");
    assert_eq!(row["repo"], at_limit);
}
