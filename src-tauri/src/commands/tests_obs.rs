//! P91 §6.2 / §10 `log_export_session` tests — the blocking half, driven against
//! a seeded config directory (no Tauri app required).
//!
//! Every fixture builds the REAL directory shape — `<config>/logs` next to
//! `<config>/exports` — because §6.2's whole point is which of the two a zip
//! lands in.

use std::path::{Path, PathBuf};

use super::obs::{count_exports, export_session};
use crate::obs::writer;

struct Fixture {
    _root: tempfile::TempDir,
    logs: PathBuf,
    exports: PathBuf,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().expect("tempdir");
    let logs = root.path().join("logs");
    let exports = root.path().join("exports");
    std::fs::create_dir_all(&logs).expect("logs dir");
    Fixture {
        _root: root,
        logs,
        exports,
    }
}

fn seed(dir: &Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).expect("seed log file");
}

fn zip_names(path: &str) -> Vec<String> {
    let file = std::fs::File::open(path).expect("open zip");
    let mut zip = zip::ZipArchive::new(file).expect("read zip");
    let mut names: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).expect("entry").name().to_string())
        .collect();
    names.sort();
    names
}

#[test]
fn exports_every_part_of_the_named_session() {
    let fx = fixture();
    seed(&fx.logs, "bonsai-2026-08-26T10-00-00-sold.jsonl", "{\"a\":1}\n");
    seed(&fx.logs, "bonsai-2026-08-27T10-00-00-snew.jsonl", "{\"b\":1}\n");
    seed(&fx.logs, "bonsai-2026-08-27T10-00-00-snew-1.jsonl", "{\"b\":2}\n");

    let out = export_session(&fx.logs, &fx.exports, Some("snew".into())).expect("export");
    assert_eq!(
        zip_names(&out),
        vec![
            "bonsai-2026-08-27T10-00-00-snew-1.jsonl".to_string(),
            "bonsai-2026-08-27T10-00-00-snew.jsonl".to_string(),
        ],
        "only the named session's parts, all of them"
    );
}

/// §6.2 — the default destination is `exports/`, and **no `.zip` is ever created
/// inside `logs/`**. A zip left in `logs/` would survive `logs_delete_all`
/// (scope: `*.jsonl`), so a raw-names archive could outlive the delete that
/// exists to erase it.
#[test]
fn default_destination_is_exports_and_never_logs() {
    let fx = fixture();
    seed(&fx.logs, "bonsai-2026-08-27T10-00-00-snew.jsonl", "{}\n");

    let out = export_session(&fx.logs, &fx.exports, None).expect("export");
    assert!(
        Path::new(&out).starts_with(&fx.exports),
        "export landed outside exports/: {out}"
    );
    let stray: Vec<_> = std::fs::read_dir(&fx.logs)
        .expect("read logs")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_ascii_lowercase())
        .filter(|n| n.ends_with(".zip"))
        .collect();
    assert!(stray.is_empty(), "a zip was created inside logs/: {stray:?}");
    // The log file itself is untouched by an export.
    assert_eq!(writer::list_log_files(&fx.logs).len(), 1);
}

/// The UI story is *enable → reproduce → turn Dev mode OFF → export*, so an
/// export with no live session must fall back to the newest session on disk
/// rather than refusing.
#[test]
fn exports_the_newest_session_when_dev_mode_is_off() {
    let fx = fixture();
    seed(&fx.logs, "bonsai-2026-08-26T10-00-00-sold.jsonl", "{}\n");
    seed(&fx.logs, "bonsai-2026-08-27T10-00-00-snew.jsonl", "{}\n");

    let out = export_session(&fx.logs, &fx.exports, None).expect("export");
    assert_eq!(zip_names(&out), vec!["bonsai-2026-08-27T10-00-00-snew.jsonl"]);
}

#[test]
fn export_rejects_clearly_when_there_is_nothing_to_export() {
    let fx = fixture();
    let err = export_session(&fx.logs, &fx.exports, None).expect_err("no files");
    assert!(err.to_string().contains("no log files"), "{err}");
}

/// REGRESSION (audit F4) — the export destination is the app-managed `exports/`
/// directory and NOTHING else. There is no `dest` parameter to redirect it,
/// because a path arriving over IPC is chosen by the webview, not by the user
/// (P91 ships no save dialog), and a zip written outside `exports/` would also
/// escape the `logs_delete_all` scope (§6.2).
///
/// The signature itself is the guard — this test pins the OUTPUT LOCATION so a
/// re-added parameter cannot quietly change where a zip lands.
#[test]
fn export_always_lands_inside_the_exports_directory() {
    let fx = fixture();
    seed(&fx.logs, "bonsai-2026-08-27T10-00-00-snew.jsonl", "{}\n");

    let out = PathBuf::from(export_session(&fx.logs, &fx.exports, None).expect("export"));
    assert_eq!(
        out.parent(),
        Some(fx.exports.as_path()),
        "the zip must be written into exports/, not anywhere else"
    );
    assert!(out.exists());
    // Nothing was created next to the logs directory (the old `dest` escape).
    assert!(!fx.logs.parent().expect("parent").join("saved").exists());
}

/// §6.2 — `log_session_info` reports the export zips so the delete-confirm copy
/// can name them. A missing directory is `None`, distinct from an empty one.
#[test]
fn export_counts_distinguish_missing_from_empty() {
    let fx = fixture();
    assert_eq!(count_exports(&fx.exports), (None, None));

    seed(&fx.logs, "bonsai-2026-08-27T10-00-00-snew.jsonl", "{}\n");
    export_session(&fx.logs, &fx.exports, None).expect("export");
    let (files, bytes) = count_exports(&fx.exports);
    assert_eq!(files, Some(1));
    assert!(bytes.expect("bytes") > 0);

    // Non-zip siblings are not counted.
    std::fs::write(fx.exports.join("notes.txt"), "hi").expect("seed");
    assert_eq!(count_exports(&fx.exports).0, Some(1));
}

/// §8.4 PRIVACY — `LogSessionInfo.writeFailed` is a BARE BOOLEAN on the wire and
/// NEVER carries the underlying `io::Error` text (whose Display embeds the log
/// path — exactly increment 1's leak). The `dir` field is an absolute path BY
/// DESIGN (Reveal/UI), so we do not forbid paths wholesale; we assert the flag is
/// a bool and that no io-error phrasing leaked into the payload.
#[test]
fn write_failed_is_a_bool_and_carries_no_error_text() {
    use super::obs::LogSessionInfo;
    use crate::obs::record::RedactionMode;

    let info = LogSessionInfo {
        session_id: "sfeedface".into(),
        dir: "/home/user/logs".into(),
        files: vec!["bonsai-a.jsonl".into()],
        bytes: 10,
        records: 1,
        anomalies: 0,
        dropped: 0,
        redaction: RedactionMode::Strict,
        salt: String::new(),
        total_files: 1,
        total_bytes: 10,
        dropped_parts: 0,
        write_failed: true,
        export_files: None,
        export_bytes: None,
    };
    let value = serde_json::to_value(&info).expect("serialize");
    assert_eq!(
        value["writeFailed"],
        serde_json::Value::Bool(true),
        "writeFailed must be a bare boolean on the wire",
    );
    let json = value.to_string();
    for leak in ["os error", "cannot write", "cannot flush", "denied", "Permission"] {
        assert!(
            !json.contains(leak),
            "the io::Error phrase {leak:?} must never reach the LogSessionInfo payload",
        );
    }
}
