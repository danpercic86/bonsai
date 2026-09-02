//! Guard for the `common::init_repo()` fixture optimisation.
//!
//! `init_repo()` used to spawn `git init` plus FOUR `git config` processes; it
//! now spawns `git init` and writes the same four keys in-process via libgit2.
//! Hundreds of tests depend on that fixture, so a silent divergence in repo
//! state would be far worse than the ~0.2 s/repo it saves. This test pins the
//! equivalence directly: a control repo built with the literal `git config`
//! invocations must have a byte-identical `git config --local --list`.

use std::path::Path;

use crate::common;

/// Sorted `git config --local --list` — repo-local keys only, so the ambient
/// global/system config cannot leak into the comparison.
fn local_config(dir: &Path) -> Vec<String> {
    let mut lines: Vec<String> = common::git(dir, &["config", "--local", "--list"])
        .lines()
        .map(str::to_string)
        .collect();
    lines.sort();
    lines
}

#[test]
fn init_repo_local_config_matches_the_four_git_config_spawns() {
    if !common::have_git() {
        return;
    }

    let fixture = common::init_repo();

    // Control: `git init -b main` + the four `git config` invocations the
    // helper used to make, verbatim.
    let control = common::scratch_dir();
    let cp = control.path();
    common::git(cp, &["init", "-b", "main"]);
    common::git(cp, &["config", "user.name", "Test User"]);
    common::git(cp, &["config", "user.email", "test@example.com"]);
    common::git(cp, &["config", "status.renames", "true"]);
    common::git(cp, &["config", "core.autocrlf", "false"]);

    assert_eq!(
        local_config(fixture.path()),
        local_config(cp),
        "in-process fixture config diverges from the `git config` control"
    );
}

#[test]
fn init_repo_config_is_readable_by_both_git_and_libgit2() {
    if !common::have_git() {
        return;
    }
    let fixture = common::init_repo();
    let root = fixture.path();

    // The git CLI sees every key with the expected value...
    for (key, value) in common::FIXTURE_CONFIG {
        assert_eq!(
            &common::git(root, &["config", "--local", "--get", key]),
            value,
            "git CLI read of {key}"
        );
    }

    // ...and so does libgit2, which is what `create_commit` actually uses.
    let repo = git2::Repository::open(root).expect("open fixture repo");
    let cfg = repo.config().expect("repo config");
    for (key, value) in common::FIXTURE_CONFIG {
        assert_eq!(&cfg.get_string(key).expect("get_string"), value, "libgit2 read of {key}");
    }

    // The identity actually resolves into a usable signature (this is the
    // property the whole fixture exists for).
    let sig = repo.signature().expect("signature from fixture identity");
    assert_eq!(sig.to_string(), "Test User <test@example.com>");
}

#[test]
fn init_repo_config_can_still_be_overridden_by_the_git_cli() {
    if !common::have_git() {
        return;
    }
    // Several suites layer `git config core.autocrlf true` on top of the
    // fixture; the in-process write must not break that (e.g. by duplicating
    // the `[core]` section into an ambiguous multivar).
    let fixture = common::init_repo();
    let root = fixture.path();
    common::git(root, &["config", "core.autocrlf", "true"]);
    assert_eq!(
        common::git(root, &["config", "--local", "--get", "core.autocrlf"]),
        "true"
    );
    assert_eq!(
        common::git(root, &["config", "--local", "--get-all", "core.autocrlf"]),
        "true",
        "override must replace, not append a second value"
    );
}
