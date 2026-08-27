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

    let out = export_session(&fx.logs, &fx.exports, Some("snew".into()), None).expect("export");
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

    let out = export_session(&fx.logs, &fx.exports, None, None).expect("export");
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

    let out = export_session(&fx.logs, &fx.exports, None, None).expect("export");
    assert_eq!(zip_names(&out), vec!["bonsai-2026-08-27T10-00-00-snew.jsonl"]);
}

#[test]
fn export_rejects_clearly_when_there_is_nothing_to_export() {
    let fx = fixture();
    let err = export_session(&fx.logs, &fx.exports, None, None).expect_err("no files");
    assert!(err.to_string().contains("no log files"), "{err}");
}

/// A caller-supplied `dest` is the result of the OS save dialog — a path the
/// user chose explicitly — and is honoured verbatim, including outside the app
/// config directory.
#[test]
fn export_writes_to_the_requested_destination() {
    let fx = fixture();
    seed(&fx.logs, "bonsai-2026-08-27T10-00-00-snew.jsonl", "{}\n");
    let dest = fx.logs.parent().expect("parent").join("saved").join("s.zip");
    let out = export_session(
        &fx.logs,
        &fx.exports,
        None,
        Some(dest.to_string_lossy().to_string()),
    )
    .expect("export");
    assert_eq!(out, dest.to_string_lossy());
    assert!(dest.exists());
}

/// §6.2 — `log_session_info` reports the export zips so the delete-confirm copy
/// can name them. A missing directory is `None`, distinct from an empty one.
#[test]
fn export_counts_distinguish_missing_from_empty() {
    let fx = fixture();
    assert_eq!(count_exports(&fx.exports), (None, None));

    seed(&fx.logs, "bonsai-2026-08-27T10-00-00-snew.jsonl", "{}\n");
    export_session(&fx.logs, &fx.exports, None, None).expect("export");
    let (files, bytes) = count_exports(&fx.exports);
    assert_eq!(files, Some(1));
    assert!(bytes.expect("bytes") > 0);

    // Non-zip siblings are not counted.
    std::fs::write(fx.exports.join("notes.txt"), "hi").expect("seed");
    assert_eq!(count_exports(&fx.exports).0, Some(1));
}
