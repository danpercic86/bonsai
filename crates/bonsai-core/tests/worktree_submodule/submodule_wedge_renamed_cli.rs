//! P73 WEDGE → RECONNECT for a RENAMED submodule (`.gitmodules` section name !=
//! checked-out path; contract OPEN-1 and the §7 row-16 leftover-data message) —
//! split out of `submodule_wedge_cli.rs` to keep each file under the ~500-line
//! limit. Declared as a child module of `submodule_wedge_cli`, so it reuses that
//! file's `require_git!` macro, constants and wedge fixtures.
//!
//! All submodules use a LOCAL `file://` URL (no creds, no network). Scratch on
//! D:. Skips (passes with a note) w/o `git`.

use super::{assert_sentinel_intact, build_sub, only, read_lf, wedge, SUB_PATH};
use crate::common;
use crate::common::{commit_fixed, file_url, git, init_repo, scratch_dir};
use bonsai_core::git::submodule::{update_submodule, SubmoduleStatus};

/// Superproject whose `.gitmodules` section NAME differs from the checked-out
/// PATH (`git submodule add --name`). git keys the module gitdir on the NAME, so
/// the cached dir is `.git/modules/<name>` while libgit2's clone would key on
/// `<path>` — the divergence contract OPEN-1 resolves in favour of `name`.
fn build_super_with_renamed_sub(url: &str, name: &str, path: &str) -> tempfile::TempDir {
    let dir = init_repo();
    let p = dir.path();
    std::fs::write(p.join("top.txt"), "super\n").unwrap();
    git(p, &["add", "-A"]);
    commit_fixed(p, "super: initial");
    git(
        p,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "--name",
            name,
            url,
            path,
        ],
    );
    git(p, &["add", "-A"]);
    commit_fixed(p, "super: add renamed submodule");
    dir
}

// ---------------------------------------- renamed submodule (name != path)

/// OPEN-1: for a renamed submodule (`.gitmodules` section name != checked-out
/// path) the cached gitdir git itself writes is `<modules>/<name>`. With a DECOY
/// `<modules>/<path>` also present (a valid repo pointing at a DIFFERENT url),
/// `name` must win — picking `path` would surface a url-mismatch refusal
/// instead of repairing.
#[test]
fn reconnect_renamed_submodule_prefers_name_keyed_gitdir() {
    require_git!();
    let (_sub, url, _v1, v2) = build_sub();
    let (_other, other_url, _o1, _o2) = build_sub();
    let name = "renamed-sub";
    let dir = build_super_with_renamed_sub(&url, name, SUB_PATH);
    let p = dir.path();

    // Precondition: git keyed the cache on the NAME, not the path.
    assert!(
        p.join(".git")
            .join("modules")
            .join(name)
            .join("HEAD")
            .exists(),
        "precondition: the cached gitdir is name-keyed"
    );
    let (_module_dir, sentinel) = wedge(p, name, SUB_PATH);

    // Decoy path-keyed gitdir for a DIFFERENT remote.
    let decoy = p.join(".git").join("modules").join(SUB_PATH);
    std::fs::create_dir_all(&decoy).expect("mkdir decoy");
    git(&decoy, &["init", "-b", "main"]);
    git(&decoy, &["remote", "add", "origin", &other_url]);

    update_submodule(p, name).expect("the name-keyed gitdir must be the one reconnected");

    assert_sentinel_intact(&sentinel);
    assert_eq!(read_lf(&p.join(SUB_PATH).join("lib.txt")), "sub v2\n");
    let row = only(p);
    assert_eq!(row.name, name, "the row is the renamed section");
    assert_eq!(row.path, SUB_PATH);
    assert_eq!(row.status, SubmoduleStatus::UpToDate, "row after reconnect");
    assert_eq!(
        row.wt_oid.as_deref(),
        Some(v2.as_str()),
        "workdir at the pinned v2"
    );
    // The gitlink resolves into the NAME-keyed dir.
    let resolved = git(&p.join(SUB_PATH), &["rev-parse", "--absolute-git-dir"]).replace('\\', "/");
    assert!(
        resolved.ends_with(&format!("/.git/modules/{name}")),
        "must resolve into the name-keyed gitdir, got: {resolved}"
    );
}

/// The leftover-data refusal (§7 row 16) names the **path**-keyed folder, because
/// libgit2 keys the init it fails on `sm->path`. With `name != path` a name-keyed
/// message would send the user to a folder that does not exist.
#[test]
fn leftover_data_refusal_names_path_keyed_folder_for_renamed_submodule() {
    require_git!();
    let (_sub, url, _v1, _v2) = build_sub();
    let name = "renamed-sub";
    let super_dir = build_super_with_renamed_sub(&url, name, SUB_PATH);

    // Fresh clone: registered in `.gitmodules`, never cloned → no cached gitdir.
    let parent = scratch_dir();
    git(
        parent.path(),
        &["clone", &file_url(super_dir.path()), "work"],
    );
    let work = parent.path().join("work");
    assert!(
        !work.join(".git").join("modules").exists(),
        "precondition: nothing cached in the fresh clone"
    );

    // Forge a "looks like a repo, cannot be opened" dir under the PATH key —
    // exactly what an aborted libgit2 clone leaves behind.
    let garbage = work.join(".git").join("modules").join(SUB_PATH);
    std::fs::create_dir_all(garbage.join("objects")).expect("mkdir objects");
    std::fs::create_dir_all(garbage.join("refs")).expect("mkdir refs");
    std::fs::write(garbage.join("HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
    std::fs::write(garbage.join("config"), "[core\nnot valid ini\n").expect("write config");

    let err = match update_submodule(&work, name) {
        Err(e) => e.to_string(),
        Ok(()) => panic!("an unopenable module gitdir must not silently succeed"),
    };
    assert!(
        !err.to_lowercase().contains("reinitialize"),
        "the raw libgit2 message must never reach the UI, got: {err}"
    );
    assert!(
        err.contains(&format!("\".git/modules/{SUB_PATH}\"")),
        "the refusal must name the PATH-keyed folder, got: {err}"
    );
    assert!(
        !err.contains(&format!("\".git/modules/{name}\"")),
        "it must NOT name the (nonexistent) name-keyed folder, got: {err}"
    );
    assert!(garbage.exists(), "Bonsai must not delete the folder itself");
}
