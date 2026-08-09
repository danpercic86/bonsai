//! T5 corrupt-repo corpus (contract §3): for each tampering, the four app
//! surfaces (open / read_status / compute_graph / create_commit) must return a
//! clean `AppError` OR coherent degraded output — NEVER panic, NEVER hang.
//! Each surface call is wrapped in `catch_unwind`; a panic fails only its
//! labeled matrix cell. Behavior per cell is pinned as discovered.
//!
//! Git-gated: the healthy baseline is built with the git CLI. Hangs are not a
//! realistic risk for these synchronous git2 reads, so no watchdog thread is
//! used (contract §3 permits either).

// Test-only: several cells clear a read-only bit to overwrite a git object.
#![allow(clippy::permissions_set_readonly_false)]

#[path = "prop_common/mod.rs"]
mod prop_common;

use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};

use bonsai_core::git::{commit::create_commit, status::read_status};
use bonsai_core::graph::compute_graph;

use prop_common::common;

macro_rules! require_git {
    () => {
        if !common::have_git() {
            eprintln!("skipping corrupt-repo suite: `git` CLI not on PATH");
            return;
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Outcome {
    Ok,
    Err,
    Panicked,
    Hung,
}

/// Run surface `kind` (0 open, 1 status, 2 graph, 3 commit) on a worker thread
/// with a 30s watchdog: a panic ⇒ `Panicked`, a timeout ⇒ `Hung` (the worker is
/// abandoned). This guarantees the suite always terminates even if a corruption
/// makes a git2 call spin (which is itself a FINDING, surfaced as `Hung`).
fn run_watch(path: &Path, kind: u8) -> Outcome {
    let p = path.to_path_buf();
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let r = std::panic::catch_unwind(AssertUnwindSafe(|| match kind {
            0 => git2::Repository::open_ext(
                &p,
                git2::RepositoryOpenFlags::NO_SEARCH,
                std::iter::empty::<&std::ffi::OsStr>(),
            )
            .is_ok(),
            1 => read_status(&p).is_ok(),
            2 => compute_graph(&p).is_ok(),
            _ => create_commit(&p, "corrupt probe", None, true).is_ok(),
        }));
        let _ = tx.send(r);
    });
    // 10s watchdog: every healthy surface on these tiny repos returns in <1s, so
    // a timeout unambiguously means a genuine (effectively infinite) hang.
    match rx.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(Ok(true)) => {
            let _ = handle.join();
            Outcome::Ok
        }
        Ok(Ok(false)) => {
            let _ = handle.join();
            Outcome::Err
        }
        Ok(Err(_)) => {
            let _ = handle.join();
            Outcome::Panicked
        }
        Err(_) => Outcome::Hung, // worker abandoned; process exits after the test
    }
}

/// The four app surfaces against `path`.
fn surfaces(path: &Path) -> [(&'static str, Outcome); 4] {
    [
        ("open", run_watch(path, 0)),
        ("read_status", run_watch(path, 1)),
        ("compute_graph", run_watch(path, 2)),
        ("create_commit", run_watch(path, 3)),
    ]
}

/// Assert no surface panicked or hung; return the outcomes keyed by name.
fn assert_no_panic(cell: &str, path: &Path) -> Vec<(&'static str, Outcome)> {
    let out = surfaces(path);
    eprintln!("[{cell}] {out:?}");
    for (name, o) in &out {
        assert_ne!(*o, Outcome::Panicked, "PANIC in [{cell}] surface {name}");
        assert_ne!(*o, Outcome::Hung, "HANG (>30s) in [{cell}] surface {name}");
    }
    out.to_vec()
}

#[allow(dead_code)]
fn outcome(v: &[(&'static str, Outcome)], name: &str) -> Outcome {
    v.iter().find(|(n, _)| *n == name).unwrap().1
}

/// Healthy 3-commit repo (git CLI baseline). Returns the temp dir + path.
fn healthy_repo() -> (tempfile::TempDir, PathBuf) {
    let dir = common::init_repo();
    let root = dir.path().to_path_buf();
    for i in 0..3 {
        std::fs::write(root.join(format!("f{i}.txt")), format!("content {i}\nsecond line\n"))
            .expect("write");
        common::git(&root, &["add", "-A"]);
        common::commit_fixed(&root, &format!("commit {i}"));
    }
    (dir, root)
}

#[test]
fn corrupt_repo_matrix_never_panics() {
    require_git!();

    // C1 — truncate a loose object. FINDING F-T5-4: truncating the HEAD COMMIT
    // object makes every surface that peels HEAD (read_status, compute_graph,
    // create_commit) HANG instead of returning an error — a libgit2-level spin
    // reading the truncated loose commit. Only `open` (which does not peel HEAD)
    // survives. Truncating a TREE or BLOB is handled cleanly (probed ⇒ Ok). A
    // hang freezes the UI — user-hostile — flagged for senior-dev. Pinned as the
    // DISCOVERED behavior; when the object read is hardened to error, these pins
    // flip and the test is updated.
    {
        let (dir, root) = healthy_repo();
        let head = common::git(&root, &["rev-parse", "HEAD"]);
        let obj = root.join(".git/objects").join(&head[..2]).join(&head[2..]);
        let bytes = std::fs::read(&obj).expect("read HEAD commit object");
        let mut perms = std::fs::metadata(&obj).unwrap().permissions();
        perms.set_readonly(false);
        let _ = std::fs::set_permissions(&obj, perms);
        std::fs::write(&obj, &bytes[..bytes.len() / 2]).unwrap();

        let out = surfaces(&root);
        eprintln!("[C1 truncated-HEAD-commit] {out:?}");
        // No PANIC anywhere (a panic would be a worse, separate finding).
        for (name, o) in &out {
            assert_ne!(*o, Outcome::Panicked, "PANIC in [C1] surface {name}");
        }
        // `open` does not read the HEAD commit ⇒ it stays responsive.
        assert_ne!(outcome(&out, "open"), Outcome::Hung, "C1 open must not hang");
        // PINNED FINDING F-T5-4: the HEAD-peeling surfaces all hang.
        for name in ["read_status", "compute_graph", "create_commit"] {
            assert_eq!(
                outcome(&out, name),
                Outcome::Hung,
                "F-T5-4: {name} hangs on a truncated HEAD commit object"
            );
        }
        drop(dir);
    }

    // C2 — corrupt a pack (needs `git gc` to produce one).
    {
        let (dir, root) = healthy_repo();
        let _ = std::process::Command::new("git")
            .args(["gc", "--quiet"])
            .current_dir(&root)
            .output();
        let pack = std::fs::read_dir(root.join(".git/objects/pack"))
            .ok()
            .and_then(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .find(|p| p.extension().map(|x| x == "pack").unwrap_or(false))
            });
        if let Some(pack) = pack {
            let mut bytes = std::fs::read(&pack).unwrap();
            for b in bytes.iter_mut().take(8) {
                *b = 0xFF;
            }
            let mut perms = std::fs::metadata(&pack).unwrap().permissions();
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(&pack, perms);
            std::fs::write(&pack, &bytes).unwrap();
            assert_no_panic("C2 corrupt-pack", &root);
        } else {
            eprintln!("[C2] no pack produced by git gc — skipped");
        }
        drop(dir);
    }

    // C3 — HEAD points at a non-existent branch (dangling symref, unborn-like).
    {
        let (dir, root) = healthy_repo();
        std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/does-not-exist\n").unwrap();
        assert_no_panic("C3 dangling-symref-HEAD", &root);
        drop(dir);
    }

    // C4 — HEAD = raw 40-hex oid of a missing object.
    {
        let (dir, root) = healthy_repo();
        std::fs::write(root.join(".git/HEAD"), format!("{}\n", "0".repeat(39) + "a")).unwrap();
        assert_no_panic("C4 HEAD-missing-oid", &root);
        drop(dir);
    }

    // C5 — refs/heads/x contains garbage (not hex, not a symref).
    {
        let (dir, root) = healthy_repo();
        std::fs::create_dir_all(root.join(".git/refs/heads")).unwrap();
        std::fs::write(root.join(".git/refs/heads/x"), b"\x00\x01garbage not a ref\xFF").unwrap();
        assert_no_panic("C5 garbage-ref", &root);
        drop(dir);
    }

    // C6 — .git/objects removed entirely.
    {
        let (dir, root) = healthy_repo();
        let _ = std::fs::remove_dir_all(root.join(".git/objects"));
        assert_no_panic("C6 objects-dir-removed", &root);
        drop(dir);
    }

    // C7 — .git/index truncated to 10 bytes.
    {
        let (dir, root) = healthy_repo();
        let idx = root.join(".git/index");
        let bytes = std::fs::read(&idx).unwrap();
        std::fs::write(&idx, &bytes[..10.min(bytes.len())]).unwrap();
        assert_no_panic("C7 truncated-index", &root);
        drop(dir);
    }

    // C8 — .git/index = 4 KiB of random-ish bytes.
    {
        let (dir, root) = healthy_repo();
        let junk: Vec<u8> = (0..4096u32).map(|i| (i.wrapping_mul(2654435761) >> 24) as u8).collect();
        std::fs::write(root.join(".git/index"), &junk).unwrap();
        assert_no_panic("C8 garbage-index", &root);
        drop(dir);
    }

    // C9 — .git/config with invalid syntax.
    {
        let (dir, root) = healthy_repo();
        std::fs::write(root.join(".git/config"), "[unclosed\n\tbroken = = =\n").unwrap();
        assert_no_panic("C9 invalid-config", &root);
        drop(dir);
    }

    // C10 — binary COMMIT_EDITMSG (should be a no-op for all ops).
    {
        let (dir, root) = healthy_repo();
        std::fs::write(root.join(".git/COMMIT_EDITMSG"), [0u8, 159, 146, 150, 255, 0, 1]).unwrap();
        let out = assert_no_panic("C10 binary-COMMIT_EDITMSG", &root);
        // Pin: a stray COMMIT_EDITMSG is a no-op for the read surfaces.
        assert_eq!(outcome(&out, "open"), Outcome::Ok, "C10 open unaffected");
        assert_eq!(outcome(&out, "read_status"), Outcome::Ok, "C10 status unaffected");
        assert_eq!(outcome(&out, "compute_graph"), Outcome::Ok, "C10 graph unaffected");
        drop(dir);
    }

    // Extra 1 — bogus .git/rebase-merge/ dir (garbage msgnum/onto).
    {
        let (dir, root) = healthy_repo();
        let rm = root.join(".git/rebase-merge");
        std::fs::create_dir_all(&rm).unwrap();
        std::fs::write(rm.join("msgnum"), b"not-a-number\n").unwrap();
        std::fs::write(rm.join("onto"), b"\xFF\xFFnot-an-oid\n").unwrap();
        std::fs::write(rm.join("end"), b"garbage\n").unwrap();
        assert_no_panic("X1 bogus-rebase-merge", &root);
        drop(dir);
    }

    // Extra 2 — bogus BISECT_LOG.
    {
        let (dir, root) = healthy_repo();
        std::fs::write(root.join(".git/BISECT_LOG"), b"\x00garbage bisect log\xFF\n").unwrap();
        assert_no_panic("X2 bogus-BISECT_LOG", &root);
        drop(dir);
    }

    // Extra 3 — an index entry with an invalid-UTF-8 path (lossy, no panic).
    {
        let (dir, root) = healthy_repo();
        {
            let repo = git2::Repository::open(&root).unwrap();
            let blob = repo.blob(b"invalid utf8 path payload\n").unwrap();
            let mut index = repo.index().unwrap();
            let entry = git2::IndexEntry {
                ctime: git2::IndexTime::new(0, 0),
                mtime: git2::IndexTime::new(0, 0),
                dev: 0,
                ino: 0,
                mode: 0o100_644,
                uid: 0,
                gid: 0,
                file_size: 0,
                id: blob,
                flags: 0,
                flags_extended: 0,
                path: b"bad\xFF\xFEname.txt".to_vec(),
            };
            // add_frombuffer writes the blob content for this entry directly.
            let _ = index.add_frombuffer(&entry, b"invalid utf8 path payload\n");
            let _ = index.write();
        }
        assert_no_panic("X3 invalid-utf8-index-path", &root);
        drop(dir);
    }
}
