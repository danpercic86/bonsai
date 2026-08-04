//! P40 CLI-oracle git-config tests (contract §9).
//!
//! Bonsai's runtime-free `config.rs` read/write path is cross-checked against
//! the real `git config` CLI on the SAME scratch repo. Local-level ops touch
//! only the repo (safe). Global-level ops are ISOLATED (contract §2/§9): both
//! the `git` subprocess and libgit2 are redirected to a scratch global file
//! under `D:\Temp\bonsai-scratch` — the developer's real `~/.gitconfig` is
//! NEVER read or written.
//!
//! Isolation mechanics:
//! - `git` subprocess: `GIT_CONFIG_GLOBAL` (+ `HOME`/`USERPROFILE`) → scratch.
//! - libgit2 in-process: `git2::opts::set_search_path(Global, <scratch>)`, so
//!   its global level resolves to `<scratch>/.gitconfig` (the SAME file the CLI
//!   uses). Set ONCE, before the first `config.rs` call.
//!
//! Everything lives in ONE `#[test]` fn so the process-global search-path
//! override never races another test in this binary. Skips if `git` is absent.

mod common;

use std::path::Path;
use std::process::Command;

use bonsai_core::error::AppError;
use bonsai_core::git::config::{
    read_config, set_config, unset_config, ConfigLevelArg, ConfigLevelName,
};
use common::{git, git_ok, have_git, scratch_dir};

/// `git config <args>` with env overrides; returns `Some(stdout)` on success,
/// `None` on non-zero exit (e.g. an unset key). Used to assert unset keys.
fn git_get_env(dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(dir);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

fn curated_value(view: &bonsai_core::git::config::ConfigView, key: &str) -> Option<String> {
    view.curated
        .iter()
        .find(|c| c.key == key)
        .and_then(|c| c.target_value.clone())
}

#[test]
fn config_oracle_local_and_isolated_global() {
    if !have_git() {
        eprintln!("skipping: `git` CLI not found on PATH");
        return;
    }

    // ---- Isolated scratch global, wired BEFORE any config.rs call ----
    let global_dir = scratch_dir();
    let global_file = global_dir.path().join(".gitconfig");
    std::fs::write(&global_file, "").expect("seed empty global config");
    let global_file_str = global_file.to_str().expect("utf8 global path");
    let global_dir_str = global_dir.path().to_str().expect("utf8 global dir");
    // SAFETY: process-global; this is the ONLY search-path mutation in this
    // binary and all tests here run in one fn (no concurrent mutation).
    unsafe {
        git2::opts::set_search_path(git2::ConfigLevel::Global, global_dir.path())
            .expect("set global search path");
    }
    let genv: [(&str, &str); 3] = [
        ("GIT_CONFIG_GLOBAL", global_file_str),
        ("HOME", global_dir_str),
        ("USERPROFILE", global_dir_str),
    ];

    // ---- Scratch repo ----
    let repo = scratch_dir();
    let path = repo.path();
    git(path, &["init", "-b", "main"]);

    // ================================================= Local oracle (§9.1-6)
    git(path, &["config", "--local", "user.name", "Local Person"]);
    git(path, &["config", "--local", "user.email", "lp@x.io"]);

    // (2) curated identity effective+target match; effectiveLevel == Local.
    let view = read_config(path, ConfigLevelArg::Local).expect("read local");
    let name = view.curated.iter().find(|c| c.key == "user.name").expect("user.name");
    assert_eq!(name.effective_value.as_deref(), Some("Local Person"));
    assert_eq!(name.target_value.as_deref(), Some("Local Person"));
    assert_eq!(name.effective_level, Some(ConfigLevelName::Local));

    // (3) set alias.co → matches CLI --get + appears in advanced.
    set_config(path, ConfigLevelArg::Local, "alias.co", "checkout").expect("set alias");
    assert_eq!(git(path, &["config", "--local", "--get", "alias.co"]), "checkout");
    let view = read_config(path, ConfigLevelArg::Local).expect("read after alias");
    assert!(
        view.advanced.iter().any(|e| e.name == "alias.co" && e.value == "checkout"),
        "advanced missing alias.co: {:?}",
        view.advanced
    );

    // (4) set core.autocrlf enum → matches CLI --get.
    set_config(path, ConfigLevelArg::Local, "core.autocrlf", "input").expect("set autocrlf");
    assert_eq!(git(path, &["config", "--local", "--get", "core.autocrlf"]), "input");

    // (5) unset alias.co → CLI --get now exits non-zero (unset).
    unset_config(path, ConfigLevelArg::Local, "alias.co").expect("unset alias");
    assert!(
        !git_ok(path, &["config", "--local", "--get", "alias.co"]),
        "alias.co should be unset"
    );

    // (6) bad key + bad enum → InvalidName, and nothing was written.
    match set_config(path, ConfigLevelArg::Local, "nodot", "x") {
        Err(AppError::InvalidName(_)) => {}
        other => panic!("expected InvalidName for bad key, got {other:?}"),
    }
    match set_config(path, ConfigLevelArg::Local, "pull.ff", "bogus") {
        Err(AppError::InvalidName(_)) => {}
        other => panic!("expected InvalidName for bad enum, got {other:?}"),
    }
    assert!(
        !git_ok(path, &["config", "--local", "--get", "pull.ff"]),
        "rejected pull.ff must not have been written"
    );

    // ============================================ Isolated Global oracle (§9.7-8)
    // (7) Global write lands in the scratch global file (NOT real ~/.gitconfig).
    set_config(path, ConfigLevelArg::Global, "user.name", "Global Person")
        .expect("set global user.name");
    assert_eq!(
        git_get_env(path, &["config", "--global", "--get", "user.name"], &genv).as_deref(),
        Some("Global Person"),
        "CLI (isolated global) should read the value config.rs wrote"
    );
    let global_body = std::fs::read_to_string(&global_file).expect("read scratch global");
    assert!(
        global_body.contains("Global Person"),
        "scratch global file must contain the write: {global_body}"
    );

    // (8) Per-level isolation: Local override wins for the effective value, but
    // the Global view's target value is the global write.
    let lview = read_config(path, ConfigLevelArg::Local).expect("read local w/ global set");
    let lname = lview.curated.iter().find(|c| c.key == "user.name").expect("user.name");
    assert_eq!(
        lname.effective_level,
        Some(ConfigLevelName::Local),
        "local override must win for the effective value"
    );
    let gview = read_config(path, ConfigLevelArg::Global).expect("read global");
    assert_eq!(
        curated_value(&gview, "user.name").as_deref(),
        Some("Global Person"),
        "global target_value must be the isolated global write"
    );

    // ================================ Added coverage: P40 §9 test-plan gaps ================================
    // These extend the single isolated-global test fn (process-global search
    // path is already wired above) so no new fn can race it.

    // (a) A Local key OVERRIDES a Global value: effective is the Local value at
    //     Local level, while the Global value still exists at its own level.
    //     Cross-check the CLI --get at each level independently.
    assert_eq!(
        git(path, &["config", "--local", "--get", "user.name"]),
        "Local Person",
        "local user.name must exist at Local"
    );
    assert_eq!(
        git_get_env(path, &["config", "--global", "--get", "user.name"], &genv).as_deref(),
        Some("Global Person"),
        "global user.name must coexist at Global"
    );
    let oview = read_config(path, ConfigLevelArg::Local).expect("read local override");
    let oname = oview
        .curated
        .iter()
        .find(|c| c.key == "user.name")
        .expect("user.name curated");
    assert_eq!(oname.effective_value.as_deref(), Some("Local Person"));
    assert_eq!(oname.effective_level, Some(ConfigLevelName::Local));
    assert_eq!(oname.target_value.as_deref(), Some("Local Person"));

    // (c) An Advanced multivar key collapses to the LAST value (contract §2/§4.2).
    git(path, &["config", "--local", "--add", "custom.multi", "first"]);
    git(path, &["config", "--local", "--add", "custom.multi", "second"]);
    git(path, &["config", "--local", "--add", "custom.multi", "third"]);
    let mview = read_config(path, ConfigLevelArg::Local).expect("read multivar");
    let multi = mview
        .advanced
        .iter()
        .find(|e| e.name == "custom.multi")
        .expect("custom.multi must appear in advanced");
    assert_eq!(
        multi.value, "third",
        "multivar must collapse to the LAST value"
    );

    // (d) Malformed keys are rejected server-side by set_config; nothing lands.
    for bad in ["nosection", "", "  ", ".leadingdot", "trailing."] {
        match set_config(path, ConfigLevelArg::Local, bad, "x") {
            Err(AppError::InvalidName(_)) => {}
            other => panic!("expected InvalidName for {bad:?}, got {other:?}"),
        }
    }

    // (b) Unsetting the Local key falls back to the Global effective value; the
    //     Global value itself survives the Local unset.
    unset_config(path, ConfigLevelArg::Local, "user.name").expect("unset local user.name");
    assert!(
        !git_ok(path, &["config", "--local", "--get", "user.name"]),
        "user.name must be unset at Local after unset_config"
    );
    assert_eq!(
        git_get_env(path, &["config", "--global", "--get", "user.name"], &genv).as_deref(),
        Some("Global Person"),
        "Global value must survive the Local unset"
    );
    let fview = read_config(path, ConfigLevelArg::Local).expect("read after local unset");
    let fname = fview
        .curated
        .iter()
        .find(|c| c.key == "user.name")
        .expect("user.name curated");
    assert_eq!(
        fname.effective_value.as_deref(),
        Some("Global Person"),
        "effective value must fall back to the Global level"
    );
    assert_eq!(fname.effective_level, Some(ConfigLevelName::Global));
    assert_eq!(
        fname.target_value, None,
        "no Local target value remains after unset"
    );

    // Best-effort cleanup of the process-global override so no later binary
    // in the same process inherits it.
    // SAFETY: same single-fn, no concurrent config access.
    unsafe {
        let _ = git2::opts::reset_search_path(git2::ConfigLevel::Global);
    }
}
