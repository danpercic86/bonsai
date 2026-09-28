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
            UserConfig::PnpmAuthIni,
            "//npm.corp.invalid/:_authToken=zz-sentinel-token\n",
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

/// E4 row 6′ Cargo / Go parts (P125c1): `[registry] token`,
/// `registries.*.token`, credentialed index URLs in `$CARGO_HOME` and in the
/// `[Q7]` parent-dir config, the token env vars, and a go env file.
/// `credentials.toml` is never read: `UserConfig` has no variant for it.
fn cargo_env(parent: &std::path::Path) -> FakeHostEnv {
    FakeHostEnv::new()
        .user_file(
            UserConfig::CargoHomeConfigToml,
            "[registry]\ntoken = \"zz-sentinel-token\"\n\n\
             [registries.corp]\nindex = \"https://u:zz-sentinel-token@cargo.corp.invalid/index\"\n\
             token = \"zz-sentinel-token\"\ncredential-provider = \"zz-sentinel-token\"\n",
        )
        .user_file(
            UserConfig::CargoHomeConfig,
            "[registries.other]\ntoken = \"zz-sentinel-token\"\n",
        )
        .user_file(
            UserConfig::GoEnv,
            "GOAUTH=zz-sentinel-token\nGOPRIVATE=corp.invalid/*\n",
        )
        .above(
            parent,
            ".cargo/config.toml",
            "[source.crates-io]\nreplace-with = \"corp\"\n\n[source.corp]\n\
             registry = \"sparse+https://u:zz-sentinel-token@cargo.corp.invalid/index/\"\n",
        )
        .var("CARGO_REGISTRY_TOKEN", TOKEN)
        .var("CARGO_REGISTRIES_CORP_TOKEN", TOKEN)
        .var("CARGO_REGISTRIES_CORP_CREDENTIAL_PROVIDER", TOKEN)
        .var("GOAUTH", TOKEN)
}

fn cargo_fixture() -> FixtureRepo {
    let r = FixtureRepo::new();
    r.track(
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\ncorp-lib = { version = \"1\", registry = \"corp\" }\n",
    )
    .track(
        "Cargo.lock",
        "version = 4\n\n[[package]]\nname = \"corp-lib\"\nversion = \"1.0.0\"\n\
         source = \"sparse+https://u:zz-sentinel-token@cargo.corp.invalid/index/\"\n",
    )
    // A malformed manifest whose bad line holds the token.
    .track("broken/Cargo.toml", "[dependencies]\nx = zz-sentinel-token\n")
    .track("go.mod", "module example.com/app\n\ngo 1.22\n\nrequire github.com/pkg/errors v0.9.1\n");
    r
}

#[test]
fn cargo_and_go_tokens_never_reach_outbound_errors_or_hosts() {
    let r = cargo_fixture();
    let env = cargo_env(&r.parent);
    let check = local_check(&r.wd, &env);
    let public = check.public();
    // Control: Go is still sent; every crate is private (replace-with).
    assert!(public.iter().any(|p| p.name() == "github.com/pkg/errors"));
    assert!(public.iter().all(|p| p.name() != "serde"));
    assert_no_sentinel(&outbound_from_public(&public), &[TOKEN, "zz-sentinel"]);
    assert_eq!(check.errors.len(), 1, "the broken manifest");
    for e in &check.errors {
        assert!(!format!("{e:?}").contains(TOKEN));
    }
    let mut hosts = 0;
    for (_, v) in &check.rows {
        if let Visibility::Private(_, Some(h)) = v {
            hosts += 1;
            assert!(!h.contains('@') && !h.contains(TOKEN), "{h}");
        }
        assert!(!format!("{v:?}").contains(TOKEN));
    }
    assert!(hosts > 0, "the private crates carry their display host");
    // [NEW-2] no token-bearing variable value was ever materialised.
    assert!(env.values_read().is_empty(), "{:?}", env.values_read());
}
