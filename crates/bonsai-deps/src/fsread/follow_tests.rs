//! `[Q9]` the following config read (above the workdir / user files).

use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::fsread::tests::{try_symlink_dir, try_symlink_file};

const CAP: u64 = 1024;

fn root() -> (tempfile::TempDir, PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let r = fs::canonicalize(d.path()).unwrap();
    (d, r)
}

fn text(o: ReadOutcome) -> String {
    match o {
        ReadOutcome::Content(s) => s,
        other => panic!("expected Content, got {other:?}"),
    }
}

#[test]
fn network_and_device_paths_are_recognised_on_every_os() {
    for p in [
        r"\\server\share\npmrc",
        "//server/share/npmrc",
        r"\\?\UNC\server\share\npmrc",
        r"\\?\unc\server\share",
        r"\\.\pipe\x",
        r"\\.\C:\x",
        r"\??\UNC\server\share",
        r"\\?\Volume{0000}\x",
        r"/\server\share",
    ] {
        assert!(is_network_path(p), "{p}");
    }
    for p in [
        r"\\?\C:\Users\me\.npmrc",
        r"C:\Users\me\.npmrc",
        "/home/me/.npmrc",
        "relative/.npmrc",
        r"\single\root",
    ] {
        assert!(!is_network_path(p), "{p}");
    }
}

#[test]
fn plain_files_read_and_non_files_do_not() {
    let (_d, r) = root();
    fs::create_dir_all(r.join(".cargo")).unwrap();
    fs::write(r.join(".cargo/config.toml"), "a = 1\n").unwrap();
    assert_eq!(
        text(read_config_follow(&r, ".cargo/config.toml", CAP)),
        "a = 1\n"
    );
    assert!(matches!(
        read_config_follow(&r, "missing", CAP),
        ReadOutcome::Absent
    ));
    assert!(matches!(
        read_config_follow(&r, ".cargo", CAP),
        ReadOutcome::NotAFile
    ));
    fs::write(r.join("file"), "x").unwrap();
    assert!(matches!(
        read_config_follow(&r, "file/config.toml", CAP),
        ReadOutcome::NotAFile
    ));
    fs::write(r.join("big"), vec![b'a'; 2048]).unwrap();
    assert!(matches!(
        read_config_follow(&r, "big", CAP),
        ReadOutcome::TooLarge
    ));
    assert!(matches!(
        read_config_follow(&r, "../x", CAP),
        ReadOutcome::NotAFile
    ));
}

fn link_file(target: &Path, link: &Path) -> bool {
    let ok = try_symlink_file(target, link);
    if !ok {
        eprintln!("skipping: cannot create symlinks here");
    }
    ok
}

#[test]
fn a_symlinked_config_to_a_regular_file_is_followed() {
    let (_d, r) = root();
    fs::create_dir_all(r.join("dotfiles")).unwrap();
    fs::write(r.join("dotfiles/npmrc"), "@a:registry=https://x.invalid/\n").unwrap();
    // A RELATIVE target, the dotfile-manager shape.
    if !link_file(Path::new("dotfiles/npmrc"), &r.join(".npmrc")) {
        return;
    }
    assert!(text(read_config_follow(&r, ".npmrc", CAP)).contains("x.invalid"));
    // A linked `.cargo` directory is followed too.
    fs::create_dir_all(r.join("real")).unwrap();
    fs::write(r.join("real/config.toml"), "b = 2\n").unwrap();
    if try_symlink_dir(&r.join("real"), &r.join(".cargo")) {
        assert_eq!(
            text(read_config_follow(&r, ".cargo/config.toml", CAP)),
            "b = 2\n"
        );
    }
}

#[test]
fn symlinks_to_dirs_dangling_or_network_targets_are_not_content() {
    let (_d, r) = root();
    fs::create_dir_all(r.join("dir")).unwrap();
    if !link_file(&r.join("dir"), &r.join("to-dir")) {
        return;
    }
    assert!(matches!(
        read_config_follow(&r, "to-dir", CAP),
        ReadOutcome::NotAFile
    ));
    // Dangling ⇒ Unreadable (the ecosystem fails closed), never Absent.
    assert!(link_file(&r.join("gone"), &r.join("dangling")));
    assert!(matches!(
        read_config_follow(&r, "dangling", CAP),
        ReadOutcome::Unreadable
    ));
    // A UNC target is rejected from the link TEXT, before it is opened.
    let unc = if cfg!(windows) {
        r"\\zz-no-such-host\share\npmrc"
    } else {
        "//zz-no-such-host/share/npmrc"
    };
    assert!(link_file(Path::new(unc), &r.join("unc")));
    assert!(matches!(
        read_config_follow(&r, "unc", CAP),
        ReadOutcome::NotAFile
    ));
    // A two-hop chain ending at a network target is rejected too.
    assert!(link_file(&r.join("unc"), &r.join("chain")));
    assert!(matches!(
        read_config_follow(&r, "chain", CAP),
        ReadOutcome::NotAFile
    ));
    // A loop ends.
    assert!(link_file(&r.join("loop-b"), &r.join("loop-a")));
    assert!(link_file(&r.join("loop-a"), &r.join("loop-b")));
    assert!(matches!(
        read_config_follow(&r, "loop-a", CAP),
        ReadOutcome::Unreadable
    ));
    #[cfg(unix)]
    {
        assert!(link_file(Path::new("/dev/null"), &r.join("dev")));
        assert!(matches!(
            read_config_follow(&r, "dev", CAP),
            ReadOutcome::NotAFile
        ));
    }
}

#[test]
fn dos_device_names_are_recognised_on_every_os() {
    for n in [
        "NUL",
        "nul",
        "Con",
        "PRN",
        "aux",
        "COM1",
        "com9",
        "COM0",
        "LPT1",
        "lpt9",
        "NUL.txt",
        "con.tar.gz",
        "COM1 ",
        "AUX.",
        "CONIN$",
        "conout$",
        "COM¹",
        "LPT³",
    ] {
        assert!(is_dos_device_name(n), "{n}");
    }
    for n in [
        "console",
        "COM10",
        "LPT",
        "COM",
        "nul_",
        "auxiliary",
        ".npmrc",
        "config.toml",
        "CONIN",
    ] {
        assert!(!is_dos_device_name(n), "{n}");
    }
}

#[test]
fn above_workdir_reads_are_no_follow_outside_the_follow_roots() {
    let (_d, r) = root();
    fs::write(r.join("plain"), "p").unwrap();
    // A plain file reads in both modes.
    assert_eq!(text(read_above_workdir_config(&r, "plain", CAP, &[])), "p");
    if !link_file(&r.join("plain"), &r.join("link")) {
        return;
    }
    assert!(matches!(
        read_above_workdir_config(&r, "link", CAP, &[]),
        ReadOutcome::NotAFile
    ));
    assert_eq!(
        text(read_above_workdir_config(
            &r,
            "link",
            CAP,
            std::slice::from_ref(&r)
        )),
        "p"
    );
    // A follow root must be the ancestor ITSELF, not a parent of it.
    let sub = r.join("sub");
    fs::create_dir_all(&sub).unwrap();
    assert!(link_file(&r.join("plain"), &sub.join("link")));
    assert!(matches!(
        read_above_workdir_config(&sub, "link", CAP, std::slice::from_ref(&r)),
        ReadOutcome::NotAFile
    ));
}

#[test]
fn a_missing_file_behind_a_followed_dir_link_is_absent_not_dangling() {
    // [Q9-2] ~/.cargo -> real dir without the legacy `config`.
    let (_d, r) = root();
    fs::create_dir_all(r.join("real")).unwrap();
    if !crate::fsread::tests::try_symlink_dir(&r.join("real"), &r.join(".cargo")) {
        eprintln!("skipping: cannot create dir symlinks here");
        return;
    }
    assert!(matches!(
        read_config_follow(&r, ".cargo/config", CAP),
        ReadOutcome::Absent
    ));
    // A link whose OWN target is missing is still dangling.
    assert!(link_file(&r.join("real/gone"), &r.join("real/cfg")));
    assert!(matches!(
        read_config_follow(&r, ".cargo/cfg", CAP),
        ReadOutcome::Unreadable
    ));
}

#[test]
fn the_user_read_mapping_fails_closed_on_every_non_content_outcome() {
    use crate::registry_ctx::UserRead;
    // [Q9-3] the production mapping every HostEnv shares.
    assert!(matches!(
        UserRead::from_outcome(ReadOutcome::Absent),
        UserRead::Absent
    ));
    assert!(matches!(
        UserRead::from_outcome(ReadOutcome::Content("x".into())),
        UserRead::Content(s) if s == "x"
    ));
    for o in [
        ReadOutcome::NotAFile,
        ReadOutcome::TooLarge,
        ReadOutcome::Unreadable,
    ] {
        assert!(matches!(UserRead::from_outcome(o), UserRead::Unreadable));
    }
}

#[test]
fn config_dirs_resolve_locally_and_refuse_network_or_non_dir_targets() {
    let (_d, r) = root();
    fs::create_dir_all(r.join("Config")).unwrap();
    fs::write(r.join("file"), "x").unwrap();
    assert_eq!(
        resolve_config_dir(&r.join("Config")).unwrap(),
        r.join("Config")
    );
    assert!(matches!(
        resolve_config_dir(&r.join("missing")),
        Err(ReadOutcome::Absent)
    ));
    assert!(matches!(
        resolve_config_dir(&r.join("file")),
        Err(ReadOutcome::NotAFile)
    ));
    assert!(matches!(
        resolve_config_dir(Path::new("relative/Config")),
        Err(ReadOutcome::Unreadable)
    ));
    let unc = if cfg!(windows) {
        r"\\zz-no-such-host\share"
    } else {
        "//zz-no-such-host/share"
    };
    assert!(matches!(
        resolve_config_dir(Path::new(unc)),
        Err(ReadOutcome::NotAFile)
    ));
    // A linked config dir is followed to its local target; a link whose
    // target is a network share is refused from the link text.
    if try_symlink_dir(&r.join("Config"), &r.join("linked")) {
        assert_eq!(
            resolve_config_dir(&r.join("linked")).unwrap(),
            r.join("Config")
        );
    }
    if link_file(Path::new(unc), &r.join("to-unc")) {
        assert!(matches!(
            resolve_config_dir(&r.join("to-unc")),
            Err(ReadOutcome::NotAFile)
        ));
    }
}
