//! E4 row 6 `[HIGH-1 item 5]` — npm part (P125b). No sentinel may reach the
//! outbound set, matched after percent- and JSON-decoding and on each bare
//! marker `[NEW-1]`; the sentinel rows must read local / private.
//!
//! The Cargo / PyPI / Go / NuGet sentinels arrive with their adapters
//! (P125c1/c2); P125d swaps the rendered outbound set for recorded requests.

mod common;

use bonsai_deps::model::{LocalReason, PrivateReason};
use bonsai_deps::privacy::Visibility;
use bonsai_deps::registry_ctx::FakeHostEnv;

use common::fixture::{try_symlink_file, FixtureRepo, PNPM_V9_IGN, YARN_V1_ROOT};
use common::{assert_no_sentinel, assert_report_clean, local_check};

/// Full sentinels and their bare markers.
const NEEDLES: &[&str] = &[
    "zz-sentinel-path",
    "zz-sentinel-ws",
    "@zzsentinel/scoped",
    "zzsentinel",
    "zz-sentinel-trans",
    "@zzign/pkg",
    "zzign",
    "@zzenv/pkg",
    "zzenv",
    "@zzws/b",
    "zzws",
    "corp.invalid",
];

const ROOT_MANIFEST: &str = r#"{
  "name": "root-app",
  "private": true,
  "workspaces": ["packages/*"],
  "dependencies": {
    "left-pad": "^1.3.0",
    "zz-sentinel-path": "file:./local/zz-sentinel-path",
    "zz-sentinel-ws": "workspace:*",
    "@zzsentinel/scoped": "^1.0.0",
    "@zzenv/pkg": "^1.0.0",
    "@zzws/b": "^1.0.0"
  }
}"#;

fn npm_fixture() -> FixtureRepo {
    let r = FixtureRepo::new();
    r.track("package.json", ROOT_MANIFEST)
        .track("yarn.lock", YARN_V1_ROOT)
        // A tracked scope config.
        .track(".npmrc", "@zzsentinel:registry=https://npm.corp.invalid/\n")
        // (c) a yarn v1 workspace member with no lock entry.
        .track(
            "packages/b/package.json",
            r#"{"name": "@zzws/b", "version": "1.0.0"}"#,
        )
        // (a) a scope that lives ONLY in a gitignored .npmrc, pnpm-v9 lock
        // without tarball URLs.
        .track(".gitignore", "ign/.npmrc\n")
        .track(
            "ign/package.json",
            r#"{"name": "ign", "dependencies": {"@zzign/pkg": "^2.0.0", "is-odd": "^3.0.0"}}"#,
        )
        .track("ign/pnpm-lock.yaml", PNPM_V9_IGN)
        .write("ign/.npmrc", "@zzign:registry=https://ign.corp.invalid/\n");
    r
}

/// (b) the env-only scope registry.
fn env() -> FakeHostEnv {
    FakeHostEnv::new().var("npm_config_@zzenv:registry", "https://env.corp.invalid/")
}

fn only(check: &common::LocalCheck, name: &str) -> Visibility {
    let v = check.visibility(name);
    assert!(!v.is_empty(), "no row for {name}");
    let first = v[0].clone();
    assert!(v.iter().all(|x| **x == first), "{name}: {v:?}");
    first
}

#[test]
fn npm_sentinels_never_reach_the_outbound_set() {
    let r = npm_fixture();
    let check = local_check(&r.wd, &env());
    assert!(check.errors.is_empty(), "{:?}", check.errors);
    let public = check.public();
    // Controls: the public deps ARE sent, so the assertion is not vacuous.
    let names: Vec<&str> = public.iter().map(|p| p.name()).collect();
    assert!(
        names.contains(&"left-pad") && names.contains(&"is-odd"),
        "{names:?}"
    );
    assert!(check.sent("left-pad") && check.sent("is-odd"));
    assert_no_sentinel(&check.outbound, NEEDLES);
}

#[test]
fn npm_sentinel_rows_read_local_or_private() {
    let r = npm_fixture();
    let check = local_check(&r.wd, &env());
    assert_eq!(
        only(&check, "zz-sentinel-path"),
        Visibility::Local(LocalReason::Path)
    );
    assert_eq!(
        only(&check, "zz-sentinel-ws"),
        Visibility::Local(LocalReason::Workspace)
    );
    assert_eq!(
        only(&check, "@zzws/b"),
        Visibility::Local(LocalReason::Workspace)
    );
    assert!(matches!(
        only(&check, "@zzsentinel/scoped"),
        Visibility::Private(PrivateReason::ScopedRegistry, _)
    ));
    assert_eq!(
        only(&check, "@zzign/pkg"),
        Visibility::Private(
            PrivateReason::ScopedRegistry,
            Some("ign.corp.invalid".into())
        )
    );
    assert_eq!(
        only(&check, "@zzenv/pkg"),
        Visibility::Private(
            PrivateReason::ScopedRegistry,
            Some("env.corp.invalid".into())
        )
    );
    assert_eq!(
        only(&check, "zz-sentinel-trans"),
        Visibility::Private(
            PrivateReason::CustomRegistry,
            Some("npm.corp.invalid".into())
        )
    );
}

#[test]
fn without_the_ignored_npmrc_the_scope_would_leak() {
    // Guards the fixture: the gitignored .npmrc is what keeps @zzign private.
    let r = npm_fixture();
    std::fs::remove_file(r.wd.join("ign/.npmrc")).expect("rm");
    let check = local_check(&r.wd, &env());
    assert!(matches!(only(&check, "@zzign/pkg"), Visibility::Public(_)));
}

#[test]
fn a_symlinked_npmrc_fails_closed() {
    // (e)
    let r = npm_fixture();
    let target = r.parent.join("elsewhere.npmrc");
    std::fs::write(&target, "registry=https://registry.npmjs.org/\n").expect("write");
    std::fs::remove_file(r.wd.join(".npmrc")).expect("rm");
    if !try_symlink_file(&target, &r.wd.join(".npmrc")) {
        eprintln!("skipping: cannot create symlinks here");
        return;
    }
    let check = local_check(&r.wd, &env());
    assert!(check.public().is_empty());
    assert_eq!(
        only(&check, "left-pad"),
        Visibility::Private(PrivateReason::ConfigUnreadable, None)
    );
}

#[test]
fn an_unreadable_config_signal_fails_the_whole_ecosystem_closed() {
    // The in-process stand-in for (e) that runs on every OS: a directory
    // where the config file should be.
    let r = npm_fixture();
    std::fs::create_dir_all(r.wd.join("ign/.yarnrc.yml")).expect("mkdir");
    let check = local_check(&r.wd, &env());
    assert!(check.public().is_empty());
    assert_no_sentinel(&check.outbound, NEEDLES);
}

#[test]
fn the_matcher_decodes_before_matching() {
    // [NEW-1] self-test: percent- and JSON-escaped forms are caught; the raw
    // strings alone would not contain the full sentinel.
    let url = common::Request {
        url: "https://api.deps.dev/v3/systems/npm/packages/%40zzign%2Fpkg".into(),
        body: None,
    };
    let body = common::Request {
        url: "https://api.osv.dev/v1/querybatch".into(),
        body: Some(r#"{"q":"\u0040zzenv\/pkg"}"#.into()),
    };
    assert!(!url.url.contains("@zzign/pkg"));
    assert!(common::find_sentinel(&[url], &["@zzign/pkg"]).is_some());
    assert!(!body.body.as_deref().unwrap_or("").contains("@zzenv/pkg"));
    assert!(common::find_sentinel(&[body], &["@zzenv/pkg"]).is_some());
    let clean = common::Request {
        url: "https://api.deps.dev/v3/systems/npm/packages/left-pad".into(),
        body: None,
    };
    assert!(common::find_sentinel(&[clean], NEEDLES).is_none());
}

#[test]
fn a_bom_prefixed_npmrc_still_routes_its_scope() {
    // [HIGH-1] npm strips a leading U+FEFF; so must the extractor.
    let r = npm_fixture();
    r.write(
        ".npmrc",
        "\u{FEFF}@zzsentinel:registry=https://npm.corp.invalid/\n",
    );
    let check = local_check(&r.wd, &env());
    assert!(matches!(
        only(&check, "@zzsentinel/scoped"),
        Visibility::Private(PrivateReason::ScopedRegistry, _)
    ));
    assert_no_sentinel(&check.outbound, NEEDLES);
}

#[test]
fn a_scope_in_the_workspace_root_npmrc_above_a_submodule_stays_private() {
    // [P125b review (i)] the workdir is a submodule inside an npm/pnpm
    // workspace; the scope registry lives only in the root's .npmrc ABOVE
    // the workdir (its package.json there is never read).
    let r = FixtureRepo::new();
    r.track(
        "package.json",
        r#"{"name": "sub", "dependencies": {"@zzsub/pkg": "^2.0.0", "is-odd": "^3.0.0"}}"#,
    );
    std::fs::write(
        r.parent.join("package.json"),
        r#"{"name": "root", "workspaces": ["repo"]}"#,
    )
    .expect("write");
    std::fs::write(
        r.parent.join("pnpm-workspace.yaml"),
        "packages:\n  - repo\n",
    )
    .expect("write");
    let env = FakeHostEnv::new().above_from_disk_within(&r.parent);
    let public = |check: &common::LocalCheck| -> Vec<String> {
        check
            .public()
            .iter()
            .map(|p| p.name().to_string())
            .collect()
    };
    // Control: without the root .npmrc the scope WOULD be sent.
    assert!(public(&local_check(&r.wd, &env)).contains(&"@zzsub/pkg".to_string()));
    std::fs::write(
        r.parent.join(".npmrc"),
        "@zzsub:registry=https://u:zz-sentinel-token@sub.corp.invalid/\n",
    )
    .expect("write");
    let check = local_check(&r.wd, &env);
    assert_eq!(
        only(&check, "@zzsub/pkg"),
        Visibility::Private(
            PrivateReason::ScopedRegistry,
            Some("sub.corp.invalid".into())
        )
    );
    assert_eq!(public(&check), ["is-odd"]);
    assert_no_sentinel(&check.outbound, &["@zzsub/pkg", "zzsub"]);
}

/// `[P125h §3]` the folder text of a local NuGet feed (lowercase, so the
/// lowercased cache bytes are matched too).
const FEED_DIR: &str = "zzsentinelfolder";

fn local_feed_repo(restore: Option<&str>) -> FixtureRepo {
    let props = restore.map_or(String::new(), |v| {
        format!("<PropertyGroup><RestoreSources>{v}</RestoreSources></PropertyGroup>")
    });
    let r = FixtureRepo::new();
    r.track(
        "package.json",
        r#"{"name": "app", "dependencies": {"left-pad": "^1.3.0"}}"#,
    )
    .track(
        "dotnet/App/App.csproj",
        &format!(
            "<Project Sdk=\"Microsoft.NET.Sdk\">{props}<ItemGroup>\
             <PackageReference Include=\"ZzSentinel.Local\" Version=\"1.0.0\" />\
             </ItemGroup></Project>"
        ),
    );
    r
}

#[test]
fn a_package_behind_a_local_nuget_feed_is_never_sent() {
    // Control: without RestoreSources the sentinel WOULD be sent.
    let r = local_feed_repo(None);
    assert!(local_check(&r.wd, &FakeHostEnv::new()).sent("ZzSentinel.Local"));
    for feed in [
        r"\\srv\zzsentinelfolder\feed",
        "./zzsentinelfolder",
        r"C:\zzsentinelfolder",
        "file:///srv/zzsentinelfolder",
    ] {
        let r = local_feed_repo(Some(feed));
        let check = local_check(&r.wd, &FakeHostEnv::new());
        assert!(check.errors.is_empty(), "{:?}", check.errors);
        // NOT fail-closed: the row names the local feed, with no host.
        assert_eq!(
            only(&check, "ZzSentinel.Local"),
            Visibility::Private(PrivateReason::LocalFeed, None),
            "{feed}"
        );
        assert!(check.sent("left-pad"), "the check ran: {feed}");
        assert_no_sentinel(
            &check.outbound,
            &["ZzSentinel.Local", "zzsentinel", FEED_DIR],
        );
        // The folder text is in no DTO, cache byte or Debug output.
        assert_report_clean(&check.report, FEED_DIR);
        for (_, v) in &check.rows {
            assert!(!format!("{v:?}").to_lowercase().contains(FEED_DIR), "{v:?}");
        }
    }
}

#[test]
fn a_feed_that_only_ends_in_the_public_host_is_never_sent() {
    // `[P125h2 LOW-H1]` the text before `@` may expand to any host.
    for feed in [
        "https://%FEED%@api.nuget.org/v3/index.json",
        "https://user:pass@api.nuget.org/v3/index.json",
    ] {
        let r = local_feed_repo(Some(feed));
        let check = local_check(&r.wd, &FakeHostEnv::new());
        assert!(check.errors.is_empty(), "{:?}", check.errors);
        assert_eq!(
            only(&check, "ZzSentinel.Local"),
            Visibility::Private(PrivateReason::CustomRegistry, None),
            "{feed}"
        );
        assert!(check.sent("left-pad"), "the check ran: {feed}");
        assert_no_sentinel(&check.outbound, &["ZzSentinel.Local", "zzsentinel"]);
    }
    // Control: the plain public feed still sends the package.
    let r = local_feed_repo(Some("https://api.nuget.org/v3/index.json"));
    assert!(local_check(&r.wd, &FakeHostEnv::new()).sent("ZzSentinel.Local"));
}
