//! `[P125h §3]` hostless NuGet feeds: the `feed_ref` text classifier, and
//! RestoreSources / fallback folders that are local paths ⇒ every package
//! `Private(LocalFeed, None)` instead of a fail-closed config.

use std::fs;
use std::path::PathBuf;

use super::nuget_cfg::{feed_ref, FeedRef};
use super::*;

struct Repo {
    _dir: tempfile::TempDir,
    wd: PathBuf,
}

fn repo() -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let wd = fs::canonicalize(dir.path()).unwrap().join("wd");
    fs::create_dir_all(wd.join("src/App")).unwrap();
    Repo { _dir: dir, wd }
}

fn write(r: &Repo, rel: &str, body: &str) {
    let p = r.wd.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, body).unwrap();
}

const NU: Ecosystem = Ecosystem::Nuget;
const DEFAULT: DepSource = DepSource::Registry(RegistryRef::Default);

fn load_with(r: &Repo, manifests: &[&str]) -> RegistryContext {
    let ms: Vec<String> = manifests.iter().map(|m| m.to_string()).collect();
    RegistryContext::load(&r.wd, &ms, &FakeHostEnv::new())
}

fn reason_of(ctx: &RegistryContext, id: &str) -> Option<(PrivateReason, Option<String>)> {
    assert!(!ctx.fail_closed(NU), "unexpectedly closed");
    ctx.private_reason(NU, id, &DEFAULT)
}

fn restore(value: &str) -> String {
    format!("<Project><PropertyGroup><RestoreSources>{value}</RestoreSources></PropertyGroup></Project>")
}

const LOCAL: Option<(PrivateReason, Option<String>)> = Some((PrivateReason::LocalFeed, None));

#[test]
fn feed_ref_covers_every_arm() {
    let host = |h: &str| Some(FeedRef::Host(h.into()));
    let cases: &[(&str, Option<FeedRef>)] = &[
        (
            "https://nuget.corp.invalid/v3/index.json",
            host("nuget.corp.invalid"),
        ),
        ("http://Feed.Corp.Invalid./x", host("feed.corp.invalid")),
        ("https://api.nuget.org/v3/index.json", host("api.nuget.org")),
        // `[P125h2 LOW-H1]` expansions / userinfo win over the trailing host.
        ("https://%FEED%@api.nuget.org/x", Some(FeedRef::Unresolved)),
        ("https://$(Feed)@api.nuget.org/x", None),
        (
            "https://user:pass@api.nuget.org/x",
            Some(FeedRef::Unresolved),
        ),
        ("HTTPS://@api.nuget.org/", Some(FeedRef::Unresolved)),
        // Userinfo before a PRIVATE host stays that (bare) private host.
        (
            "https://u:tok@nuget.corp.invalid/v3/index.json",
            host("nuget.corp.invalid"),
        ),
        ("http://feed.corp.invalid/x%20y", Some(FeedRef::Unresolved)),
        ("$(Foo)", None),
        ("@(Feeds)", None),
        ("%(Item.Url)", None),
        ("%FEED%", Some(FeedRef::Unresolved)),
        ("%FEED%/v3/index.json", Some(FeedRef::Unresolved)),
        ("file:///x", Some(FeedRef::LocalPath)),
        ("FILE://srv/share", Some(FeedRef::LocalPath)),
        (r"\\srv\s", Some(FeedRef::LocalPath)),
        ("//srv/s", Some(FeedRef::LocalPath)),
        ("/srv/feed", Some(FeedRef::LocalPath)),
        ("./feed", Some(FeedRef::LocalPath)),
        ("../feed", Some(FeedRef::LocalPath)),
        (r".\feed", Some(FeedRef::LocalPath)),
        (r"..\feed", Some(FeedRef::LocalPath)),
        (r"C:\f", Some(FeedRef::LocalPath)),
        ("d:/f", Some(FeedRef::LocalPath)),
        ("packages", Some(FeedRef::LocalPath)),
        ("feeds/local", Some(FeedRef::LocalPath)),
        ("ftp://x", None),
        ("ssh://git@corp/feed", None),
        ("C:relative", None),
        ("https://", None),
    ];
    for (v, want) in cases {
        assert_eq!(&feed_ref(v), want, "{v}");
    }
}

#[test]
fn a_restore_sources_folder_is_a_local_feed_not_a_closed_config() {
    for v in [
        "./feed",
        r"\\srv\ZzSentinelFolder",
        "file:///srv/feed",
        "packages",
    ] {
        let r = repo();
        write(&r, "src/App/App.csproj", &restore(v));
        let ctx = load_with(&r, &["src/App/App.csproj"]);
        assert_eq!(reason_of(&ctx, "ZzSentinel.Local"), LOCAL, "{v}");
    }
    // Same via Directory.Build.props, next to the config's self-reference.
    let r = repo();
    write(
        &r,
        "Directory.Build.props",
        "<Project><PropertyGroup><RestoreAdditionalProjectSources>$(RestoreAdditionalProjectSources);../feed</RestoreAdditionalProjectSources></PropertyGroup></Project>",
    );
    assert_eq!(
        reason_of(&load_with(&r, &["src/App/App.csproj"]), "X"),
        LOCAL
    );
}

#[test]
fn a_local_feed_mixed_with_a_private_host_is_a_custom_registry() {
    let custom = Some((
        PrivateReason::CustomRegistry,
        Some("corp.invalid".to_string()),
    ));
    let r = repo();
    write(
        &r,
        "src/App/App.csproj",
        &restore("./feed;https://corp.invalid/v3/index.json"),
    );
    assert_eq!(
        reason_of(&load_with(&r, &["src/App/App.csproj"]), "X"),
        custom
    );
    // The private host may come from the config chain instead.
    let r = repo();
    write(&r, "src/App/App.csproj", &restore("./feed"));
    write(
        &r,
        "NuGet.Config",
        "<configuration><packageSources><add key=\"corp\" value=\"https://corp.invalid/v3/index.json\"/></packageSources></configuration>",
    );
    assert_eq!(
        reason_of(&load_with(&r, &["src/App/App.csproj"]), "X"),
        custom
    );
    // An unresolved `%VAR%` next to a local folder is not provably local.
    let r = repo();
    write(&r, "src/App/App.csproj", &restore("./feed;%CORP_FEED%"));
    assert_eq!(
        reason_of(&load_with(&r, &["src/App/App.csproj"]), "X"),
        Some((PrivateReason::CustomRegistry, None))
    );
    // A public nuget.org feed beside a local one does not change the reason.
    let r = repo();
    write(
        &r,
        "src/App/App.csproj",
        &restore("https://api.nuget.org/v3/index.json;./feed"),
    );
    assert_eq!(
        reason_of(&load_with(&r, &["src/App/App.csproj"]), "X"),
        LOCAL
    );
}

#[test]
fn fallback_folders_with_a_private_host_source_are_a_custom_registry() {
    let r = repo();
    write(
        &r,
        "NuGet.Config",
        "<configuration><packageSources><add key=\"corp\" value=\"https://corp.invalid/\"/></packageSources><fallbackPackageFolders><add key=\"f\" value=\"C:\\fallback\"/></fallbackPackageFolders></configuration>",
    );
    assert_eq!(
        reason_of(&load_with(&r, &["src/App/App.csproj"]), "X"),
        Some((
            PrivateReason::CustomRegistry,
            Some("corp.invalid".to_string())
        ))
    );
}

#[test]
fn chains_fold_custom_registry_over_local_feed() {
    // Two projects: one behind a local feed, one behind a private host.
    let r = repo();
    write(&r, "src/App/App.csproj", &restore("./feed"));
    write(&r, "src/Lib/Lib.csproj", &restore("https://corp.invalid/"));
    let ms = ["src/App/App.csproj", "src/Lib/Lib.csproj"];
    assert_eq!(
        reason_of(&load_with(&r, &ms), "X"),
        Some((
            PrivateReason::CustomRegistry,
            Some("corp.invalid".to_string())
        ))
    );
    // Both local ⇒ LocalFeed.
    write(&r, "src/Lib/Lib.csproj", &restore(r"\\srv\share"));
    assert_eq!(reason_of(&load_with(&r, &ms), "X"), LOCAL);
}

#[test]
fn a_mapped_private_pattern_in_another_chain_is_not_a_local_feed() {
    // `[P125h2 review]` chain A all-local; chain B maps `ZzCorp.*` to a
    // private host. A's `replace_all` must not label ZzCorp.* LocalFeed.
    let r = repo();
    write(&r, "src/App/App.csproj", &restore("./feed"));
    write(&r, "src/Lib/Lib.csproj", "<Project/>");
    write(
        &r,
        "src/Lib/NuGet.Config",
        r#"<configuration><packageSources>
             <add key="nuget.org" value="https://api.nuget.org/v3/index.json"/>
             <add key="corp" value="https://corp.invalid/v3/index.json"/>
           </packageSources><packageSourceMapping>
             <packageSource key="nuget.org"><package pattern="*"/></packageSource>
             <packageSource key="corp"><package pattern="ZzCorp.*"/></packageSource>
           </packageSourceMapping></configuration>"#,
    );
    let ctx = load_with(&r, &["src/App/App.csproj", "src/Lib/Lib.csproj"]);
    assert!(matches!(
        reason_of(&ctx, "ZzCorp.Pkg"),
        Some((PrivateReason::CustomRegistry, _))
    ));
    // Control: chain B (a real `Mapped`, private source present) mapping
    // only to nuget.org keeps A's LocalFeed.
    write(
        &r,
        "src/Lib/NuGet.Config",
        r#"<configuration><packageSources>
             <add key="nuget.org" value="https://api.nuget.org/v3/index.json"/>
             <add key="corp" value="https://corp.invalid/v3/index.json"/>
           </packageSources><packageSourceMapping>
             <packageSource key="nuget.org"><package pattern="*"/></packageSource>
           </packageSourceMapping></configuration>"#,
    );
    let ctx = load_with(&r, &["src/App/App.csproj", "src/Lib/Lib.csproj"]);
    assert_eq!(reason_of(&ctx, "ZzCorp.Pkg"), LOCAL);
}

#[test]
fn msbuild_expansions_and_unknown_schemes_still_fail_closed() {
    for v in ["$(Foo)", "@(Feeds)", "ftp://x", "C:relative"] {
        let r = repo();
        write(&r, "src/App/App.csproj", &restore(v));
        assert!(
            load_with(&r, &["src/App/App.csproj"]).fail_closed(NU),
            "restore {v}"
        );
        // `[P125h §3]` the same rule for a `packageSources` value.
        let r = repo();
        write(
            &r,
            "NuGet.Config",
            &format!("<configuration><packageSources><add key=\"k\" value=\"{v}\"/></packageSources></configuration>"),
        );
        assert!(
            load_with(&r, &["src/App/App.csproj"]).fail_closed(NU),
            "config {v}"
        );
    }
}
