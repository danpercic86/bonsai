//! E4 row 6′ `[MEDIUM-1]` — PyPI and NuGet parts (P125c2).
//! `zz-sentinel-token` sits in the pip user config (`index-url` userinfo
//! and a non-index key), `PIP_INDEX_URL`, uv token env vars, NuGet
//! `packageSourceCredentials` / `apikeys` in the user config and in the
//! `[Q7]` parent-dir config, and in a malformed requirements line and
//! project file. It must be absent from the outbound set, from `Debug` of
//! every `ParseError`, and from every `registry_host` (which has no `@`).

mod common;

use bonsai_deps::privacy::Visibility;
use bonsai_deps::registry_ctx::{FakeHostEnv, UserConfig};

use common::fixture::FixtureRepo;
use common::{assert_no_sentinel, local_check, outbound_from_public};

const TOKEN: &str = "zz-sentinel-token";

fn env(parent: &std::path::Path) -> FakeHostEnv {
    FakeHostEnv::new()
        .user_file(
            UserConfig::PipUser,
            "[global]\nindex-url = https://u:zz-sentinel-token@pip.corp.invalid/simple\n\
             client-cert = /home/u/zz-sentinel-token.pem\n",
        )
        .user_file(
            UserConfig::NugetUser,
            "<configuration><packageSources>\
             <add key=\"corp\" value=\"https://u:zz-sentinel-token@nuget.corp.invalid/v3/index.json\"/>\
             </packageSources><packageSourceCredentials><corp>\
             <add key=\"ClearTextPassword\" value=\"zz-sentinel-token\"/></corp>\
             </packageSourceCredentials><apikeys>\
             <add key=\"https://nuget.corp.invalid/\" value=\"zz-sentinel-token\"/></apikeys>\
             </configuration>",
        )
        .above(
            parent,
            "NuGet.Config",
            "<configuration><packageSourceCredentials><corp>\
             <add key=\"Password\" value=\"zz-sentinel-token\"/></corp>\
             </packageSourceCredentials></configuration>",
        )
        .var(
            "PIP_INDEX_URL",
            "https://u:zz-sentinel-token@pip.corp.invalid/simple",
        )
        .var("UV_INDEX_CORP_PASSWORD", TOKEN)
        .var("UV_PUBLISH_TOKEN", TOKEN)
        .var("PIP_CLIENT_CERT", TOKEN)
}

fn fixture() -> FixtureRepo {
    let r = FixtureRepo::new();
    r.track("requirements.txt", "six\n")
        .track(
            "App/App.csproj",
            "<Project><ItemGroup><PackageReference Include=\"Newtonsoft.Json\" Version=\"13.0.3\"/></ItemGroup></Project>",
        )
        // Malformed manifests whose bad line holds the token.
        .track("broken/requirements.txt", "six\n$$ zz-sentinel-token\n")
        .track(
            "Broken/Broken.csproj",
            "<Project>\n<ItemGroup>zz-sentinel-token</Project>\n",
        );
    r
}

#[test]
fn pypi_and_nuget_tokens_never_reach_outbound_errors_or_hosts() {
    let r = fixture();
    let env = env(&r.parent);
    let check = local_check(&r.wd, &env);
    // Both ecosystems are private (user configs); nothing leaves.
    let public = check.public();
    assert!(
        public.is_empty(),
        "{:?}",
        public.iter().map(|p| p.name()).collect::<Vec<_>>()
    );
    assert_no_sentinel(&outbound_from_public(&public), &[TOKEN, "zz-sentinel"]);
    assert_eq!(check.errors.len(), 2, "the two broken manifests");
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
    // The broken project file also hides its RestoreSources, so NuGet is
    // ConfigUnreadable (no host); the PyPI rows carry theirs.
    assert!(hosts >= 1, "the PyPI rows carry their display host");
}

#[test]
fn only_index_env_values_are_read() {
    let r = fixture();
    let env = env(&r.parent);
    let _ = local_check(&r.wd, &env);
    // [NEW-2] the token-bearing names are filtered before their value.
    assert_eq!(env.values_read(), ["PIP_INDEX_URL"]);
}

#[test]
fn a_credentialed_nuget_feed_shows_only_its_bare_host() {
    let r = FixtureRepo::new();
    r.track(
        "App/App.csproj",
        "<Project><ItemGroup><PackageReference Include=\"Newtonsoft.Json\" Version=\"13.0.3\"/></ItemGroup></Project>",
    );
    let check = local_check(&r.wd, &env(&r.parent));
    assert!(check.errors.is_empty());
    assert_eq!(
        check.visibility("Newtonsoft.Json"),
        [&Visibility::Private(
            bonsai_deps::model::PrivateReason::CustomRegistry,
            Some("nuget.corp.invalid".into())
        )]
    );
}
