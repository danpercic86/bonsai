//! REGRESSION (audit F5) — observability artifacts are owner-only.
//!
//! Log parts, export zips and `usage.json` carry absolute repo paths and real
//! branch names. Created through plain `File::create` they land at
//! `0666 & ~umask` (typically `0644`), so any other local account on a shared
//! Linux/macOS box can read them.
//!
//! Two layers of test, because the mode bits only exist on unix:
//! * `#[cfg(unix)]` tests assert the ACTUAL bits (`0600` / `0700`);
//! * a source-scan test runs EVERYWHERE (including the Windows CI this project
//!   is developed on) and fails if an observability write path goes back to a
//!   raw `File::create` / `OpenOptions`, which is how the regression would
//!   return unnoticed on a platform that cannot check the bits.

use std::path::Path;

use super::{append_file_private, create_dir_private, create_file_private};

fn scratch(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "bonsai-fsperm-{}-{}-{:?}",
        name,
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&p);
    p
}

#[cfg(unix)]
#[test]
fn created_files_and_dirs_are_owner_only() {
    use super::super::mode_of;
    let dir = scratch("modes");
    create_dir_private(&dir).expect("dir");
    assert_eq!(mode_of(&dir), Some(0o700), "directories are owner-only");

    let created = dir.join("a.json");
    drop(create_file_private(&created).expect("create"));
    assert_eq!(mode_of(&created), Some(0o600), "files are owner-only");

    let appended = dir.join("b.jsonl");
    drop(append_file_private(&appended).expect("append"));
    assert_eq!(mode_of(&appended), Some(0o600), "log parts are owner-only");

    // A file that already exists with loose bits is tightened on the next open —
    // `OpenOptions::mode` alone applies only when the open CREATES the file.
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&appended, std::fs::Permissions::from_mode(0o644))
            .expect("loosen");
    }
    drop(append_file_private(&appended).expect("reopen"));
    assert_eq!(mode_of(&appended), Some(0o600));

    let _ = std::fs::remove_dir_all(&dir);
}

/// The helpers must still behave like their std counterparts on every platform.
#[test]
fn helpers_create_usable_files_on_every_platform() {
    use std::io::Write;
    let dir = scratch("usable");
    create_dir_private(&dir.join("nested")).expect("nested dir");
    let p = dir.join("nested").join("x.txt");
    {
        let mut f = create_file_private(&p).expect("create");
        f.write_all(b"one").expect("write");
    }
    {
        let mut f = append_file_private(&p).expect("append");
        f.write_all(b"-two").expect("write");
    }
    assert_eq!(std::fs::read_to_string(&p).expect("read"), "one-two");
    // `create_file_private` truncates, like `File::create`.
    drop(create_file_private(&p).expect("recreate"));
    assert_eq!(std::fs::read_to_string(&p).expect("read"), "");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every observability write path must go through `fs_perm`. A raw
/// `File::create` / `OpenOptions::new()` in one of these files is the exact
/// regression, and this check runs on platforms where the mode bits cannot be
/// asserted at all.
#[test]
fn observability_write_paths_use_the_private_helpers() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = [
        src.join("obs").join("writer.rs"),
        src.join("obs").join("metrics_file.rs"),
        src.join("commands").join("obs.rs"),
    ];
    let mut hits = Vec::new();
    for path in files {
        let body = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        // Line-based, comment-skipping scan for the two direct creators.
        for (i, line) in body.lines().enumerate() {
            let code = line.trim_start();
            // Doc/line comments describe the history; only real code counts.
            if code.starts_with("//") {
                continue;
            }
            if code.contains("File::create(") || code.contains("create_dir_all(") {
                hits.push(format!("{}:{} — {code}", path.display(), i + 1));
            }
        }
        // A builder chain spans lines, so look at the window after each
        // `OpenOptions::new()`. A read-only open (`sync_parent_dir`'s directory
        // handle, for fsync) creates nothing and needs no mode.
        let mut from = 0usize;
        while let Some(off) = body[from..].find("OpenOptions::new()") {
            let at = from + off;
            let window = &body[at..body.len().min(at + 300)];
            if window.contains(".create(") {
                let line = body[..at].lines().count();
                hits.push(format!(
                    "{}:{line} — creating OpenOptions chain",
                    path.display()
                ));
            }
            from = at + 18;
        }
    }
    assert!(
        hits.is_empty(),
        "observability files must create paths via obs::fs_perm (0600/0700):\n{}",
        hits.join("\n")
    );
}
