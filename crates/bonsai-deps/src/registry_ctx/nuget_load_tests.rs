//! `[HIGH-1]` `[Q7]` the NuGet half of `RegistryContext::load`: the
//! NuGet.Config chain (`<clear/>`, disabled sources, source mapping,
//! machine-wide set, above-workdir walk) and RestoreSources.

use std::fs;
use std::path::PathBuf;

use super::*;
use crate::fixtures::nuget::{CONFIG_BROKEN, CONFIG_CLEAR_PUBLIC, CONFIG_MAPPED, CONFIG_PRIVATE};
use crate::fixtures::xml_bombs;

struct Repo {
    _dir: tempfile::TempDir,
    root: PathBuf,
    wd: PathBuf,
}

fn repo() -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    let wd = root.join("wd");
    fs::create_dir_all(wd.join("src/App")).unwrap();
    Repo {
        _dir: dir,
        root,
        wd,
    }
}

fn write(r: &Repo, rel: &str, body: &str) {
    let p = r.wd.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, body).unwrap();
}

fn manifests() -> Vec<String> {
    vec!["src/App/App.csproj".into()]
}

fn load(r: &Repo, env: &FakeHostEnv) -> RegistryContext {
    RegistryContext::load(&r.wd, &manifests(), env)
}

const NU: Ecosystem = Ecosystem::Nuget;

/// nuget.org only, without `<clear/>`.
const PUBLIC_NO_CLEAR: &str = "<configuration><packageSources><add key=\"nuget.org\" value=\"https://api.nuget.org/v3/index.json\"/></packageSources></configuration>";
const DEFAULT: DepSource = DepSource::Registry(RegistryRef::Default);

fn reason(ctx: &RegistryContext, id: &str) -> Option<(PrivateReason, Option<String>)> {
    assert!(!ctx.fail_closed(NU), "unexpectedly closed");
    ctx.private_reason(NU, id, &DEFAULT)
}

fn custom(host: &str) -> Option<(PrivateReason, Option<String>)> {
    Some((PrivateReason::CustomRegistry, Some(host.into())))
}

#[test]
fn no_config_opens_nuget_publicly() {
    let r = repo();
    assert_eq!(reason(&load(&r, &FakeHostEnv::new()), "Serilog"), None);
}

#[test]
fn a_private_feed_anywhere_in_the_chain_makes_every_package_private() {
    let r = repo();
    write(&r, "NuGet.Config", CONFIG_PRIVATE);
    assert_eq!(
        reason(&load(&r, &FakeHostEnv::new()), "Serilog"),
        custom("nuget.corp.invalid")
    );
    let r = repo();
    let env = FakeHostEnv::new().user_file(UserConfig::NugetUser, CONFIG_PRIVATE);
    assert_eq!(
        reason(&load(&r, &env), "Serilog"),
        custom("nuget.corp.invalid")
    );
    // [Q7] a parent-dir config (nearest ancestor above the workdir).
    let env = FakeHostEnv::new().above(&r.root, "NuGet.Config", CONFIG_PRIVATE);
    assert_eq!(
        reason(&load(&r, &env), "Serilog"),
        custom("nuget.corp.invalid")
    );
    // A second machine-wide file.
    let env = FakeHostEnv::new()
        .user_file(UserConfig::NugetMachineWide, PUBLIC_NO_CLEAR)
        .user_file_also(UserConfig::NugetMachineWide, CONFIG_PRIVATE);
    assert_eq!(
        reason(&load(&r, &env), "Serilog"),
        custom("nuget.corp.invalid")
    );
}

#[test]
fn clear_drops_every_farther_level_only() {
    let r = repo();
    write(&r, "src/App/nuget.config", CONFIG_CLEAR_PUBLIC);
    let env = FakeHostEnv::new()
        .above(&r.root, "NuGet.Config", CONFIG_PRIVATE)
        .user_file(UserConfig::NugetUser, CONFIG_PRIVATE)
        .user_file(UserConfig::NugetMachineWide, CONFIG_PRIVATE);
    assert_eq!(reason(&load(&r, &env), "Serilog"), None);
    // A NEARER private feed is not cleared by a farther `<clear/>`.
    write(&r, "src/App/nuget.config", CONFIG_PRIVATE);
    write(&r, "NuGet.Config", CONFIG_CLEAR_PUBLIC);
    assert_eq!(
        reason(&load(&r, &FakeHostEnv::new()), "Serilog"),
        custom("nuget.corp.invalid")
    );
    // Another project's chain keeps the root-level private feed.
    let r = repo();
    write(&r, "NuGet.Config", CONFIG_PRIVATE);
    write(&r, "src/App/nuget.config", CONFIG_CLEAR_PUBLIC);
    let ms = vec!["src/App/App.csproj".to_string(), "Root.csproj".to_string()];
    let ctx = RegistryContext::load(&r.wd, &ms, &FakeHostEnv::new());
    assert_eq!(reason(&ctx, "Serilog"), custom("nuget.corp.invalid"));
}

#[test]
fn disabled_sources_and_source_mapping() {
    let r = repo();
    write(&r, "nuget.config", CONFIG_MAPPED);
    let ctx = load(&r, &FakeHostEnv::new());
    assert_eq!(reason(&ctx, "Newtonsoft.Json"), None);
    assert_eq!(
        reason(&ctx, "ZzSentinel.Pkg"),
        Some((
            PrivateReason::SourceMapping,
            Some("nuget.corp.invalid".into())
        ))
    );
    // Disabling the private feed nearer re-opens everything.
    let r = repo();
    write(&r, "NuGet.Config", CONFIG_PRIVATE);
    write(
        &r,
        "src/App/NuGet.Config",
        "<configuration><disabledPackageSources><add key=\"corp\" value=\"True\"/></disabledPackageSources></configuration>",
    );
    assert_eq!(reason(&load(&r, &FakeHostEnv::new()), "Serilog"), None);
    // …but a key matched only case-insensitively is NOT disabled (NuGet).
    write(
        &r,
        "src/App/NuGet.Config",
        "<configuration><disabledPackageSources><add key=\"CORP\" value=\"true\"/></disabledPackageSources></configuration>",
    );
    assert_eq!(
        reason(&load(&r, &FakeHostEnv::new()), "Serilog"),
        custom("nuget.corp.invalid")
    );
}

#[test]
fn mapping_gaps_fall_to_private() {
    let r = repo();
    // Only `Corp.*` is mapped (to nuget.org!); an unmapped id is private,
    // and a pattern mapped to an undefined key is private.
    write(
        &r,
        "nuget.config",
        r#"<configuration><packageSources>
             <add key="nuget.org" value="https://api.nuget.org/v3/index.json"/>
             <add key="corp" value="https://nuget.corp.invalid/v3/index.json"/>
           </packageSources><packageSourceMapping>
             <packageSource key="nuget.org"><package pattern="Public.*"/></packageSource>
             <packageSource key="ghost"><package pattern="Ghost.*"/></packageSource>
           </packageSourceMapping></configuration>"#,
    );
    let ctx = load(&r, &FakeHostEnv::new());
    assert_eq!(reason(&ctx, "Public.Lib"), None);
    assert!(matches!(
        reason(&ctx, "Other"),
        Some((PrivateReason::SourceMapping, _))
    ));
    assert!(matches!(
        reason(&ctx, "Ghost.X"),
        Some((PrivateReason::SourceMapping, _))
    ));
}

#[test]
fn every_bad_config_fails_closed_at_every_level() {
    let bad: Vec<String> = vec![
        CONFIG_BROKEN.into(),
        xml_bombs::billion_laughs(),
        xml_bombs::XXE.into(),
        xml_bombs::deep_nesting(1000),
        "<configuration><packageSources><add key=\"empty\" value=\" \"/></packageSources></configuration>".into(),
        "<configuration><fallbackPackageFolders><add key=\"f\"/></fallbackPackageFolders></configuration>".into(),
        "<configuration><packageSources><add key=\"k\"/></packageSources></configuration>".into(),
        "<configuration><PackageSources><clear/></PackageSources></configuration>".into(),
        "<configuration><packageSources/><packageSources/></configuration>".into(),
        "<configuration><packageSources><CLEAR/></packageSources></configuration>".into(),
        "<settings/>".into(),
    ];
    for b in &bad {
        let r = repo();
        write(&r, "NuGet.Config", b);
        assert!(load(&r, &FakeHostEnv::new()).fail_closed(NU), "repo: {b}");
        let r = repo();
        let env = FakeHostEnv::new().above(&r.root, "nuget.config", b);
        assert!(load(&r, &env).fail_closed(NU), "above: {b}");
        let env = FakeHostEnv::new().user_file(UserConfig::NugetUser, b);
        assert!(load(&r, &env).fail_closed(NU), "user: {b}");
        let env = FakeHostEnv::new()
            .user_file(UserConfig::NugetMachineWide, PUBLIC_NO_CLEAR)
            .user_file_also(UserConfig::NugetMachineWide, b);
        assert!(load(&r, &env).fail_closed(NU), "machine: {b}");
    }
    let r = repo();
    let env = FakeHostEnv::new().above_unreadable(&r.root, "NuGet.Config");
    assert!(load(&r, &env).fail_closed(NU));
    let env = FakeHostEnv::new().user_unreadable(UserConfig::NugetMachineWide);
    assert!(load(&r, &env).fail_closed(NU));
}

#[test]
fn credentials_are_never_decoded() {
    // A credentials section whose values are not even valid references
    // would fail if decoded; it must not affect the result.
    let r = repo();
    write(
        &r,
        "NuGet.Config",
        "<configuration><packageSourceCredentials><corp><add key=\"ClearTextPassword\" value=\"zz&amp;\" Value=\"dup\"/></corp></packageSourceCredentials></configuration>",
    );
    assert_eq!(reason(&load(&r, &FakeHostEnv::new()), "Serilog"), None);
}

#[test]
fn restore_sources_in_project_files_and_build_props() {
    let r = repo();
    write(
        &r,
        "src/App/App.csproj",
        "<Project><PropertyGroup><RestoreSources>$(RestoreSources);https://restore.corp.invalid/v3/index.json</RestoreSources></PropertyGroup></Project>",
    );
    assert_eq!(
        reason(&load(&r, &FakeHostEnv::new()), "Serilog"),
        custom("restore.corp.invalid")
    );
    let r = repo();
    write(
        &r,
        "Directory.Build.props",
        "<Project><PropertyGroup><RestoreAdditionalProjectSources>https://add.corp.invalid/</RestoreAdditionalProjectSources></PropertyGroup></Project>",
    );
    assert_eq!(
        reason(&load(&r, &FakeHostEnv::new()), "Serilog"),
        custom("add.corp.invalid")
    );
    for bad in [
        "<Project><PropertyGroup><RestoreSources>$(CorpFeed)</RestoreSources></PropertyGroup></Project>",
        "<Project><PropertyGroup><RestoreSources>C:\\feed</RestoreSources></PropertyGroup></Project>",
        "<Project><PropertyGroup><RestoreConfigFile>corp.config</RestoreConfigFile></PropertyGroup></Project>",
        "<Project><PropertyGroup>",
    ] {
        let r = repo();
        write(&r, "src/App/App.csproj", bad);
        assert!(load(&r, &FakeHostEnv::new()).fail_closed(NU), "{bad}");
    }
}

#[test]
fn q9_a_symlinked_config_above_the_workdir_is_followed_only_in_home() {
    let r = repo();
    let target = r.root.join("dotfiles-nuget.xml");
    fs::write(&target, CONFIG_PRIVATE).unwrap();
    if !crate::fsread::tests::try_symlink_file(&target, &r.root.join("NuGet.Config")) {
        eprintln!("skipping: cannot create symlinks here");
        return;
    }
    let not_home = FakeHostEnv::new().above_from_disk_within(&r.root);
    assert!(load(&r, &not_home).fail_closed(NU), "[Q9-narrowed]");
    let home = FakeHostEnv::new()
        .above_from_disk_within(&r.root)
        .follow_root(&r.root);
    assert_eq!(
        reason(&load(&r, &home), "Serilog"),
        custom("nuget.corp.invalid")
    );
    fs::remove_file(r.root.join("NuGet.Config")).unwrap();
    fs::create_dir_all(r.root.join("a-dir")).unwrap();
    assert!(crate::fsread::tests::try_symlink_file(
        &r.root.join("a-dir"),
        &r.root.join("NuGet.Config")
    ));
    assert!(load(&r, &home).fail_closed(NU), "link to a directory");
}

#[test]
fn a_symlinked_config_inside_the_workdir_fails_closed() {
    let r = repo();
    let target = r.root.join("outside.config");
    fs::write(&target, CONFIG_CLEAR_PUBLIC).unwrap();
    if !crate::fsread::tests::try_symlink_file(&target, &r.wd.join("NuGet.Config")) {
        eprintln!("skipping: cannot create symlinks here");
        return;
    }
    assert!(load(&r, &FakeHostEnv::new()).fail_closed(NU));
}

/// A hostless feed (local folder / UNC / `%VAR%`) in `packageSources`.
fn local_feed(value: &str, mapping: &str) -> String {
    format!(
        "<configuration><packageSources><add key=\"nuget.org\" value=\"https://api.nuget.org/v3/index.json\"/>         <add key=\"local\" value=\"{value}\"/></packageSources>{mapping}</configuration>"
    )
}

#[test]
fn a_hostless_feed_is_a_private_source_not_a_closed_config() {
    // [P125c2 review] no mapping ⇒ every package private, no display host.
    for v in ["../feed", r"\\server\share", "%CORP_FEED%", r"C:\packages"] {
        let r = repo();
        write(&r, "NuGet.Config", &local_feed(v, ""));
        assert_eq!(
            reason(&load(&r, &FakeHostEnv::new()), "Serilog"),
            Some((PrivateReason::CustomRegistry, None)),
            "{v}"
        );
    }
    // With a mapping only the patterns mapped to the local key are private.
    let r = repo();
    let mapping = "<packageSourceMapping>        <packageSource key=\"nuget.org\"><package pattern=\"*\"/></packageSource>        <packageSource key=\"local\"><package pattern=\"ZzSentinel.*\"/></packageSource>        </packageSourceMapping>";
    write(&r, "NuGet.Config", &local_feed("../feed", mapping));
    let ctx = load(&r, &FakeHostEnv::new());
    assert_eq!(reason(&ctx, "Serilog"), None);
    assert_eq!(
        reason(&ctx, "ZzSentinel.Local"),
        Some((PrivateReason::SourceMapping, None))
    );
}

#[test]
fn fallback_package_folders_make_every_package_private_even_when_mapped() {
    let r = repo();
    write(
        &r,
        "NuGet.Config",
        "<configuration><packageSources><clear/><add key=\"nuget.org\" value=\"https://api.nuget.org/v3/index.json\"/></packageSources>         <packageSourceMapping><packageSource key=\"nuget.org\"><package pattern=\"*\"/></packageSource></packageSourceMapping>         <fallbackPackageFolders><add key=\"f\" value=\"C:\\fallback\"/></fallbackPackageFolders></configuration>",
    );
    assert_eq!(
        reason(&load(&r, &FakeHostEnv::new()), "Serilog"),
        Some((PrivateReason::CustomRegistry, None))
    );
    // A nearer `<clear/>` of the fallback section drops it.
    write(
        &r,
        "src/App/NuGet.Config",
        "<configuration><fallbackPackageFolders><clear/></fallbackPackageFolders></configuration>",
    );
    assert_eq!(reason(&load(&r, &FakeHostEnv::new()), "Serilog"), None);
}

#[test]
fn files_of_one_level_disagreeing_on_clear_fail_closed() {
    // Two machine-wide files stand in for the name variants of one dir.
    let r = repo();
    let clear_map =
        "<configuration><packageSourceMapping><clear/></packageSourceMapping></configuration>";
    let env = FakeHostEnv::new()
        .user_file(UserConfig::NugetMachineWide, clear_map)
        .user_file_also(UserConfig::NugetMachineWide, PUBLIC_NO_CLEAR);
    assert!(load(&r, &env).fail_closed(NU));
    let env = FakeHostEnv::new()
        .user_file(UserConfig::NugetMachineWide, CONFIG_CLEAR_PUBLIC)
        .user_file_also(UserConfig::NugetMachineWide, PUBLIC_NO_CLEAR);
    assert!(load(&r, &env).fail_closed(NU));
    // Agreeing files are fine.
    let env = FakeHostEnv::new()
        .user_file(UserConfig::NugetMachineWide, CONFIG_CLEAR_PUBLIC)
        .user_file_also(UserConfig::NugetMachineWide, CONFIG_CLEAR_PUBLIC);
    assert_eq!(reason(&load(&r, &env), "Serilog"), None);
}

#[test]
fn q7_directory_build_props_above_the_workdir_is_read_for_restore_sources() {
    let r = repo();
    let props = "<Project><PropertyGroup><RestoreAdditionalProjectSources>https://above.corp.invalid/v3/index.json</RestoreAdditionalProjectSources></PropertyGroup></Project>";
    let env = FakeHostEnv::new().above(&r.root, "Directory.Build.props", props);
    assert_eq!(
        reason(&load(&r, &env), "Serilog"),
        custom("above.corp.invalid")
    );
    let env = FakeHostEnv::new().above_unreadable(&r.root, "Directory.Build.props");
    assert!(load(&r, &env).fail_closed(NU));
    let env = FakeHostEnv::new().above(&r.root, "Directory.Build.props", "<Project>");
    assert!(load(&r, &env).fail_closed(NU));
    // Served from disk: a plain props file with no restore settings.
    fs::write(r.root.join("Directory.Build.props"), "<Project/>").unwrap();
    let env = FakeHostEnv::new().above_from_disk_within(&r.root);
    assert_eq!(reason(&load(&r, &env), "Serilog"), None);
}
