//! P91 §F6 §8 items 8 + 11 — the `metrics/` folder purge.
//!
//! These pin the *policy*, not just the happy path: "every file, then the
//! directory", non-recursive, and never a recursive delete on a joined path.

use super::purge_metrics_dir;

/// §8 item 8 — the mixed directory: three metrics files plus one subdirectory
/// Bonsai never creates. Every file goes, the subdirectory is REPORTED (not
/// recursed into, not deleted), and because it survives, `remove_dir` fails and
/// `dir_removed` is honestly false.
#[test]
fn purge_removes_every_file_reports_a_subdir_and_keeps_the_dir_when_one_survives() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("metrics");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("usage.json"), vec![b'x'; 100]).expect("write");
    std::fs::write(dir.join("usage.json.bak"), vec![b'x'; 200]).expect("write");
    std::fs::write(dir.join("usage.json.tmp"), vec![b'x'; 50]).expect("write");
    std::fs::create_dir_all(dir.join("nested")).expect("mkdir nested");

    let c = purge_metrics_dir(&dir);

    assert_eq!(c.deleted_files, 3, "all three metrics files removed");
    assert_eq!(c.deleted_bytes, 350);
    assert_eq!(
        c.failed_files, 1,
        "the subdirectory is reported, not recursed"
    );
    assert!(
        !c.dir_removed,
        "a surviving entry means the folder is still there"
    );
    assert!(dir.join("nested").exists(), "never recursed into");
    assert!(!dir.join("usage.json").exists());
    assert!(!dir.join("usage.json.bak").exists());
    assert!(!dir.join("usage.json.tmp").exists());
}

/// §8 item 8 — a clean directory: every file removed AND the directory itself.
#[test]
fn purge_removes_the_directory_when_nothing_survives() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("metrics");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("usage.json"), b"{}").expect("write");

    let c = purge_metrics_dir(&dir);

    assert_eq!(c.deleted_files, 1);
    assert!(c.dir_removed, "the folder itself is gone");
    assert!(!dir.exists());
}

/// §8 item 8 — a missing directory is SUCCESS, not an error: a launch younger
/// than the first 60 s flush has nothing on disk yet, and the delete action must
/// still report the usage counts as cleared.
#[test]
fn purge_of_a_missing_dir_is_all_zero_and_reports_the_dir_gone() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("metrics");

    let c = purge_metrics_dir(&dir);

    assert_eq!(c.deleted_files, 0);
    assert_eq!(c.deleted_bytes, 0);
    assert_eq!(c.failed_files, 0);
    assert!(c.dir_removed);
}

/// The purge must not escape its own directory — the reason `remove_dir_all` was
/// rejected. A sibling `settings.json` and a sibling `logs/` must survive a purge
/// of `metrics/`.
#[test]
fn purge_never_touches_a_sibling_of_the_metrics_dir() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("metrics");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::create_dir_all(root.path().join("logs")).expect("mkdir logs");
    std::fs::write(dir.join("usage.json"), b"{}").expect("write");
    std::fs::write(root.path().join("settings.json"), b"{}").expect("write");

    let c = purge_metrics_dir(&dir);

    assert!(c.dir_removed);
    assert!(
        root.path().join("settings.json").exists(),
        "settings.json survives"
    );
    assert!(root.path().join("logs").exists(), "logs/ survives");
}

/// §8 item 11 — no PRODUCTION source under `obs/` may call `remove_dir_all`: a
/// recursive delete reachable from the webview on a path built by joining onto
/// the config directory is the failure mode `purge_metrics_dir` exists to avoid.
///
/// **Scope is deliberately production-only.** `tests_*.rs` files are skipped: a
/// test's own scratch-directory cleanup is not reachable from the webview and is
/// not the hazard (`tests_fs_perm.rs` has used it since P91 increment 3). Widening
/// the guard to test code would only teach the next author to work around it.
///
/// Two self-match defences, both necessary: the needle is SPLIT so this file is
/// not its own counter-example (the trick
/// `tests_metrics.rs::obs_tree_reaches_no_http_dependency` uses), and `//` lines
/// are skipped so the several doc comments that NAME the rejected call — here, in
/// `metrics_purge.rs`, and in the contract pointers — do not read as calls.
#[test]
fn obs_tree_never_calls_a_recursive_directory_delete() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/obs");
    let needle = format!("remove_dir{}all", "_");
    let mut hits = Vec::new();
    scan_for(&dir, &needle, &mut hits);
    assert!(
        hits.is_empty(),
        "obs/ must enumerate-then-remove_dir, never recurse: {hits:?}"
    );
}

fn scan_for(dir: &std::path::Path, needle: &str, hits: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_test_file = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("tests_"));
        if path.is_dir() {
            scan_for(&path, needle, hits);
        } else if path.extension().is_some_and(|e| e == "rs") && !is_test_file {
            if let Ok(src) = std::fs::read_to_string(&path) {
                for (n, line) in src.lines().enumerate() {
                    if !line.trim_start().starts_with("//") && line.contains(needle) {
                        hits.push(format!("{}:{}", path.display(), n + 1));
                    }
                }
            }
        }
    }
}
