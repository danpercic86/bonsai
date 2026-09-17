//! P73 WEDGE → RECONNECT refusals (contract §9.6, §9.7 and the §8.1 cosmetic-url
//! tolerance) — split out of `submodule_wedge_cli.rs` to keep each file under the
//! ~500-line limit. Declared as a child module of `submodule_wedge_cli`, so it
//! reuses that file's `require_git!` macro, constants and wedge fixtures.
//!
//! All submodules use a LOCAL `file://` URL (no creds, no network). Scratch on
//! D:. Skips (passes with a note) w/o `git`.

use super::{
    assert_sentinel_intact, build_sub, build_super_with_sub, cli_status_char, only, wedge, SUB_PATH,
};
use crate::common;
use crate::common::git;
use bonsai_core::error::AppError;
use bonsai_core::git::submodule::{update_submodule, SubmoduleStatus};

// -------------------------------------------------------- criterion 6

/// Criterion 6 — refusal A: a workdir holding files but no `.git` link is NEVER
/// clobbered. The refusal is fail-closed: the stray file is byte-identical, no
/// gitlink was written, and the row is still `Uninitialized`.
///
/// NOTE the message asserted here is the §7 AMENDED user-facing copy ("The
/// folder already has files in it..."), not §9.6's original `no .git link`
/// wording — the amendment is the implemented contract.
#[test]
fn reconnect_refuses_non_empty_workdir() {
    require_git!();
    let (_sub, url, _v1, _v2) = build_sub();
    let dir = build_super_with_sub(&url);
    let p = dir.path();
    let (_module_dir, sentinel) = wedge(p, SUB_PATH, SUB_PATH);

    let sub_wd = p.join(SUB_PATH);
    let stray = sub_wd.join("keepme.txt");
    std::fs::write(&stray, "USER DATA - DO NOT DELETE\n").expect("write stray file");
    let before = std::fs::read(&stray).expect("read stray file");

    match update_submodule(p, SUB_PATH) {
        Err(AppError::Git(m)) => {
            assert!(
                m.contains("The folder already has files in it."),
                "got: {m}"
            );
            assert!(
                m.contains(SUB_PATH),
                "the message must name the path, got: {m}"
            );
            assert!(
                !m.to_lowercase().contains("reinitialize"),
                "no raw libgit2 prose, got: {m}"
            );
        }
        other => panic!("a non-empty workdir must be refused, got {other:?}"),
    }

    assert_eq!(
        std::fs::read(&stray).expect("stray survives"),
        before,
        "the user's file must be byte-identical after the refusal"
    );
    assert!(
        !sub_wd.join(".git").exists(),
        "no gitlink may be written on a refusal"
    );
    assert!(
        !sub_wd.join(".git.bonsai-tmp").exists(),
        "no atomic-write residue on a refusal"
    );
    assert_sentinel_intact(&sentinel);
    assert_eq!(
        only(p).status,
        SubmoduleStatus::Uninitialized,
        "row unchanged after refusal"
    );
    assert_eq!(
        cli_status_char(p, SUB_PATH),
        '-',
        "git still reports the wedge"
    );
}

// -------------------------------------------------------- criterion 7

/// Criterion 7 — refusal B: the cached gitdir's `origin` points somewhere else,
/// so ownership cannot be proven. Both urls are quoted and nothing is written.
#[test]
fn reconnect_refuses_url_mismatch() {
    require_git!();
    let (_sub, url, _v1, _v2) = build_sub();
    let (_other, other_url, _o1, _o2) = build_sub();
    let dir = build_super_with_sub(&url);
    let p = dir.path();
    let (module_dir, sentinel) = wedge(p, SUB_PATH, SUB_PATH);

    git(&module_dir, &["remote", "set-url", "origin", &other_url]);

    match update_submodule(p, SUB_PATH) {
        Err(AppError::Git(m)) => {
            assert!(
                m.contains(&other_url) && m.contains(&url),
                "the refusal must quote BOTH urls, got: {m}"
            );
            assert!(
                m.contains("Bonsai has cached data for a different remote URL"),
                "got: {m}"
            );
        }
        other => panic!("a url mismatch must be refused, got {other:?}"),
    }

    assert!(
        !p.join(SUB_PATH).join(".git").exists(),
        "no gitlink on a refusal"
    );
    assert_eq!(
        std::fs::read_dir(p.join(SUB_PATH)).unwrap().count(),
        0,
        "the workdir is still empty"
    );
    assert_sentinel_intact(&sentinel);
    assert_eq!(
        only(p).status,
        SubmoduleStatus::Uninitialized,
        "row unchanged after refusal"
    );
}

// ------------------------------------------ cosmetic-url tolerance (§8.1)

/// `urls_equivalent` normalization: a trailing `/` plus a `.git` suffix on the
/// CONFIGURED url must NOT be read as a mismatch — the reconnect still happens.
#[test]
fn reconnect_tolerates_url_cosmetic_difference() {
    require_git!();
    let (_sub, url, _v1, v2) = build_sub();
    let dir = build_super_with_sub(&url);
    let p = dir.path();
    let (_module_dir, sentinel) = wedge(p, SUB_PATH, SUB_PATH);

    // Cosmetically different, semantically identical: `<url>.git/`.
    let cosmetic = format!("{url}.git/");
    let key = format!("submodule.{SUB_PATH}.url");
    git(p, &["config", "--local", &key, &cosmetic]);
    git(p, &["config", "-f", ".gitmodules", &key, &cosmetic]);
    assert_ne!(cosmetic, url, "precondition: the strings really differ");

    update_submodule(p, SUB_PATH).expect("a cosmetic url difference must not block the reconnect");

    assert_sentinel_intact(&sentinel);
    let row = only(p);
    assert_eq!(row.status, SubmoduleStatus::UpToDate, "row after reconnect");
    assert_eq!(
        row.wt_oid.as_deref(),
        Some(v2.as_str()),
        "workdir at the pinned v2"
    );
}
