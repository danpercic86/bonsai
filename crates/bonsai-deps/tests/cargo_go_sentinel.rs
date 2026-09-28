//! E4 row 6 `[HIGH-1 item 5]` — Cargo and Go part (P125c1), incl. the
//! `[Q7]` AC6 (f) Cargo half: a private registry in a parent-dir
//! `.cargo/config.toml` must never be sent.

mod common;

use bonsai_deps::model::{LocalReason, PrivateReason};
use bonsai_deps::privacy::Visibility;
use bonsai_deps::registry_ctx::FakeHostEnv;

use common::fixture::{
    try_symlink_file, FixtureRepo, CARGO_LOCK, CARGO_MEMBER, CARGO_ROOT, GO_MOD, Q7_CARGO_CONFIG,
};
use common::{assert_no_sentinel, local_check, outbound_from_public, LocalCheck};

/// Full sentinels and their bare markers.
const NEEDLES: &[&str] = &["zz-sentinel", "zz_sentinel", "zzsentinel", "corp.invalid"];

/// `zz-sentinel-above` is a PUBLIC crate unless the Q7 config applies.
const ABOVE: &str = "zz-sentinel-above";

fn fixture() -> FixtureRepo {
    let r = FixtureRepo::new();
    r.track("Cargo.toml", CARGO_ROOT)
        .track("Cargo.lock", CARGO_LOCK)
        .track("crates/zz-sentinel-member/Cargo.toml", CARGO_MEMBER)
        .track("go/go.mod", GO_MOD);
    r
}

fn env() -> FakeHostEnv {
    FakeHostEnv::new().var("GOPRIVATE", "corp.invalid/*")
}

/// Above-workdir reads served from disk, below the fixture's tempdir only.
fn q7_env(r: &FixtureRepo) -> FakeHostEnv {
    env().above_from_disk_within(&r.parent)
}

fn write_q7(r: &FixtureRepo) {
    std::fs::create_dir_all(r.parent.join(".cargo")).expect("mkdir");
    std::fs::write(r.parent.join(".cargo/config.toml"), Q7_CARGO_CONFIG).expect("write");
}

fn only(check: &LocalCheck, name: &str) -> Visibility {
    let v = check.visibility(name);
    assert!(!v.is_empty(), "no row for {name}");
    let first = v[0].clone();
    assert!(v.iter().all(|x| **x == first), "{name}: {v:?}");
    first
}

fn public_names(check: &LocalCheck) -> Vec<String> {
    check
        .public()
        .iter()
        .map(|p| p.name().to_string())
        .collect()
}

#[test]
fn cargo_and_go_sentinels_never_reach_the_outbound_set() {
    let r = fixture();
    let check = local_check(&r.wd, &env());
    assert!(check.errors.is_empty(), "{:?}", check.errors);
    let public = public_names(&check);
    // Controls: the public crates and modules ARE sent.
    for n in [
        "serde",
        "itoa",
        ABOVE,
        "github.com/pkg/errors",
        "golang.org/x/text",
    ] {
        assert!(public.iter().any(|p| p == n), "{n} missing: {public:?}");
    }
    let sent: Vec<_> = check
        .public()
        .into_iter()
        .filter(|p| p.name() != ABOVE)
        .collect();
    assert_no_sentinel(&outbound_from_public(&sent), NEEDLES);
}

#[test]
fn cargo_and_go_sentinel_rows_read_local_or_private() {
    let r = fixture();
    let check = local_check(&r.wd, &env());
    assert_eq!(
        only(&check, "zz_sentinel_git"),
        Visibility::Local(LocalReason::Git)
    );
    assert_eq!(
        only(&check, "zz-sentinel-reg"),
        Visibility::Private(PrivateReason::CustomRegistry, None)
    );
    // [patch] names and workspace members are declared ⇒ Local everywhere.
    for n in ["zz-sentinel-patched", "zz-sentinel-member"] {
        assert_eq!(
            only(&check, n),
            Visibility::Local(LocalReason::Workspace),
            "{n}"
        );
    }
    assert_eq!(
        only(&check, "zz-sentinel-trans-cargo"),
        Visibility::Private(
            PrivateReason::CustomRegistry,
            Some("cargo.corp.invalid".into())
        )
    );
    assert_eq!(
        only(&check, "corp.invalid/zzsentinel/mod"),
        Visibility::Private(PrivateReason::GoPrivate, None)
    );
    assert_eq!(
        only(&check, "example.com/zz-sentinel-local"),
        Visibility::Local(LocalReason::Path)
    );
}

#[test]
fn an_env_only_crates_io_replacement_makes_every_crate_private() {
    // (b) the env-only Cargo signal.
    let r = fixture();
    let env = env().var("CARGO_SOURCE_CRATES_IO_REPLACE_WITH", "corp");
    let check = local_check(&r.wd, &env);
    let public = public_names(&check);
    assert!(
        public.iter().all(|n| n.contains('/')),
        "only Go modules: {public:?}"
    );
    assert!(matches!(
        only(&check, "serde"),
        Visibility::Private(PrivateReason::CustomRegistry, _)
    ));
    assert_no_sentinel(&outbound_from_public(&check.public()), NEEDLES);
}

#[test]
fn q7_a_parent_dir_cargo_config_keeps_every_crate_private() {
    // (f) Cargo half.
    let r = fixture();
    write_q7(&r);
    let check = local_check(&r.wd, &q7_env(&r));
    let public = public_names(&check);
    assert!(
        public.iter().all(|n| n.contains('/')),
        "only Go modules: {public:?}"
    );
    // Go is unaffected (the control keeps the assertion non-vacuous).
    assert!(public.iter().any(|n| n == "github.com/pkg/errors"));
    // The display host carries no userinfo.
    let above = Visibility::Private(
        PrivateReason::CustomRegistry,
        Some("cargo.corp.invalid".into()),
    );
    assert_eq!(only(&check, ABOVE), above);
    assert_eq!(only(&check, "serde"), above);
    assert_no_sentinel(&outbound_from_public(&check.public()), NEEDLES);
}

#[test]
fn q7_without_the_parent_config_the_crate_would_be_sent() {
    // Guards the fixture: the parent-dir config is what keeps it private.
    let r = fixture();
    let check = local_check(&r.wd, &q7_env(&r));
    assert!(public_names(&check).iter().any(|n| n == ABOVE));
}

#[test]
fn q9_a_symlinked_parent_config_fails_closed_unless_the_parent_is_home() {
    // (f) + [Q9-narrowed]: in a superproject / parent dir a symlinked
    // config fails closed; only in $HOME / $CARGO_HOME is it followed.
    let r = fixture();
    let target = r.parent.join("dotfiles-cargo.toml");
    std::fs::write(&target, Q7_CARGO_CONFIG).expect("write");
    std::fs::create_dir_all(r.parent.join(".cargo")).expect("mkdir");
    if !try_symlink_file(&target, &r.parent.join(".cargo/config.toml")) {
        eprintln!("skipping: cannot create symlinks here");
        return;
    }
    let check = local_check(&r.wd, &q7_env(&r));
    assert_eq!(
        only(&check, ABOVE),
        Visibility::Private(PrivateReason::ConfigUnreadable, None)
    );
    let home = q7_env(&r).follow_root(&r.parent);
    let check = local_check(&r.wd, &home);
    assert_eq!(
        only(&check, ABOVE),
        Visibility::Private(
            PrivateReason::CustomRegistry,
            Some("cargo.corp.invalid".into())
        )
    );
    assert_no_sentinel(&outbound_from_public(&check.public()), NEEDLES);
    // A link to a directory ⇒ ConfigUnreadable, even in $HOME.
    std::fs::remove_file(r.parent.join(".cargo/config.toml")).expect("rm");
    std::fs::create_dir_all(r.parent.join("a-dir")).expect("mkdir");
    assert!(try_symlink_file(
        &r.parent.join("a-dir"),
        &r.parent.join(".cargo/config.toml")
    ));
    let check = local_check(&r.wd, &home);
    assert_eq!(
        only(&check, ABOVE),
        Visibility::Private(PrivateReason::ConfigUnreadable, None)
    );
    assert!(public_names(&check).iter().all(|n| n.contains('/')));
}

/// Go sentinels for the routing variables (review round 2).
fn go_rows(env: &FakeHostEnv) -> LocalCheck {
    let r = fixture();
    let check = local_check(&r.wd, env);
    // `zz-sentinel-above` is a public crate without the Q7 config.
    let sent: Vec<_> = check
        .public()
        .into_iter()
        .filter(|p| p.name() != ABOVE)
        .collect();
    assert_no_sentinel(&outbound_from_public(&sent), NEEDLES);
    check
}

#[test]
fn a_corporate_goproxy_without_goprivate_keeps_modules_private() {
    let env = FakeHostEnv::new().var(
        "GOPROXY",
        "https://u:zz-sentinel-token@goproxy.corp.invalid,direct",
    );
    let check = go_rows(&env);
    assert!(
        public_names(&check).iter().all(|n| !n.contains('/')),
        "no Go module sent"
    );
    assert_eq!(
        only(&check, "corp.invalid/zzsentinel/mod"),
        Visibility::Private(
            PrivateReason::CustomRegistry,
            Some("goproxy.corp.invalid".into())
        )
    );
    assert!(matches!(
        only(&check, "github.com/pkg/errors"),
        Visibility::Private(PrivateReason::CustomRegistry, _)
    ));
}

#[test]
fn gosumdb_off_fails_go_closed() {
    let check = go_rows(&FakeHostEnv::new().var("GOSUMDB", "off"));
    assert!(public_names(&check).iter().all(|n| !n.contains('/')));
    assert_eq!(
        only(&check, "corp.invalid/zzsentinel/mod"),
        Visibility::Private(PrivateReason::ConfigUnreadable, None)
    );
}

#[test]
fn goinsecure_globs_are_private() {
    let check = go_rows(&FakeHostEnv::new().var("GOINSECURE", "corp.invalid"));
    assert_eq!(
        only(&check, "corp.invalid/zzsentinel/mod"),
        Visibility::Private(PrivateReason::GoPrivate, None)
    );
    assert!(public_names(&check)
        .iter()
        .any(|n| n == "github.com/pkg/errors"));
}
