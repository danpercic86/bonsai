//! E4 row 6′ `[MEDIUM-1]` — npm part (P125b). `zz-sentinel-token` sits in
//! every npm-family FakeHostEnv user config, in auth env vars, in in-repo
//! configs and in a malformed manifest. It must be absent from the outbound
//! set (decoded, `[NEW-1]`), from `Debug` of every `ParseError`, and from
//! every private row's `registry_host`, which never contains `@`.
//!
//! The cache-bytes and serialized-`DepTrackerView` halves need `cache.rs` /
//! `check.rs` and land in P125d.

mod common;

use bonsai_deps::privacy::Visibility;
use bonsai_deps::registry_ctx::{FakeHostEnv, UserConfig};

use common::fixture::FixtureRepo;
use common::{assert_no_sentinel, local_check, outbound_from_public};

const TOKEN: &str = "zz-sentinel-token";

fn env(parent: &std::path::Path) -> FakeHostEnv {
    FakeHostEnv::new()
        .user_file(
            UserConfig::NpmUserRc,
            "//npm.corp.invalid/:_authToken=zz-sentinel-token\n\
             @corp:registry=https://u:zz-sentinel-token@npm.corp.invalid/\n\
             _auth=zz-sentinel-token\n",
        )
        .user_file(
            UserConfig::NpmGlobalRc,
            "//registry.npmjs.org/:_authToken=zz-sentinel-token\n",
        )
        .user_file(
            UserConfig::PnpmRc,
            "//npm.corp.invalid/:_password=zz-sentinel-token\n",
        )
        .user_file(
            UserConfig::YarnRc,
            "\"//npm.corp.invalid/:_authToken\" \"zz-sentinel-token\"\n",
        )
        .user_file(
            UserConfig::YarnRcYml,
            "npmAuthToken: zz-sentinel-token\nnpmScopes:\n  corp:\n    npmAuthToken: zz-sentinel-token\n",
        )
        .above(
            parent,
            ".yarnrc.yml",
            "npmAuthIdent: \"u:zz-sentinel-token\"\n",
        )
        .var("npm_config_//npm.corp.invalid/:_authToken", TOKEN)
        .var("NPM_TOKEN", TOKEN)
        .var("pnpm_config__auth", TOKEN)
        .var("YARN_NPM_AUTH_TOKEN", TOKEN)
}

fn fixture() -> FixtureRepo {
    let r = FixtureRepo::new();
    r.track(
        "package.json",
        r#"{"name": "app", "dependencies": {"left-pad": "^1.3.0", "@corp/ui": "^2.0.0"}}"#,
    )
    .track(
        ".npmrc",
        "//npm.corp.invalid/:_authToken=zz-sentinel-token\nalways-auth=true\n",
    )
    .track(
        ".yarnrc.yml",
        "npmScopes:\n  corp:\n    npmAuthToken: zz-sentinel-token\n",
    )
    // A malformed manifest whose bad line holds the token.
    .track(
        "broken/package.json",
        "{\"name\": \"broken\",\n  \"x\": zz-sentinel-token\n}",
    );
    r
}

#[test]
fn npm_token_never_reaches_outbound_errors_or_hosts() {
    let r = fixture();
    let env = env(&r.parent);
    let check = local_check(&r.wd, &env);

    // Control: a public dep is sent, the private scope is not.
    let public = check.public();
    assert!(public.iter().any(|p| p.name() == "left-pad"));
    assert!(public.iter().all(|p| p.name() != "@corp/ui"));
    assert_no_sentinel(&outbound_from_public(&public), &[TOKEN, "zz-sentinel"]);

    assert_eq!(check.errors.len(), 1, "the broken manifest");
    for e in &check.errors {
        let dbg = format!("{e:?}");
        assert!(!dbg.contains(TOKEN), "{dbg}");
    }

    let mut hosts = 0;
    for (_, v) in &check.rows {
        if let Visibility::Private(_, Some(h)) = v {
            hosts += 1;
            assert!(!h.contains('@') && !h.contains(TOKEN), "{h}");
        }
        assert!(!format!("{v:?}").contains(TOKEN));
    }
    assert!(hosts > 0, "the scoped row carries its display host");
}

#[test]
fn auth_env_values_are_never_read() {
    let r = fixture();
    let env = env(&r.parent);
    let _ = local_check(&r.wd, &env);
    assert!(
        env.values_read().is_empty(),
        "only registry keys may be read: {:?}",
        env.values_read()
    );
}
