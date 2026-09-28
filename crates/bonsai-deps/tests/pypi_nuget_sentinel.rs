//! E4 row 6 `[HIGH-1 item 5]` — PyPI and NuGet part (P125c2), incl. (b) the
//! env-only signals (`PIP_INDEX_URL`, `UV_DEFAULT_INDEX`; NuGet has no env
//! var, so an unparseable machine-wide config stands in), (d) an
//! unparseable `nuget.config`, and the `[Q7]` AC6 (f) NuGet half: a private
//! feed in a parent-dir `NuGet.Config` must never be sent.

mod common;

use bonsai_deps::model::{LocalReason, PrivateReason};
use bonsai_deps::privacy::Visibility;
use bonsai_deps::registry_ctx::{FakeHostEnv, UserConfig};

use common::fixture::{try_symlink_file, FixtureRepo};
use common::fixture_pn::{
    CSPROJ, CSPROJ_LIB, NUGET_BROKEN, NUGET_MAPPED, PYPROJECT, Q7_NUGET_CONFIG, REQ_DEV, REQ_ROOT,
    UV_LOCK,
};
use common::{assert_no_sentinel, assert_no_sentinel_except, local_check, LocalCheck};

const NEEDLES: &[&str] = &["zz-sentinel", "zz_sentinel", "zzsentinel", "corp.invalid"];

/// Public unless the Q7 parent config applies.
const ABOVE: &str = "ZzSentinel.Above";

fn fixture() -> FixtureRepo {
    let r = FixtureRepo::new();
    r.track("requirements.txt", REQ_ROOT)
        .track("py/requirements-dev.txt", REQ_DEV)
        .track("py/pyproject.toml", PYPROJECT)
        .track("py/uv.lock", UV_LOCK)
        .track("dotnet/App/App.csproj", CSPROJ)
        .track("dotnet/Lib/ZzSentinel.Proj.csproj", CSPROJ_LIB);
    r
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

fn private(host: &str) -> Visibility {
    Visibility::Private(PrivateReason::CustomRegistry, Some(host.into()))
}

const UNREADABLE: Visibility = Visibility::Private(PrivateReason::ConfigUnreadable, None);

#[test]
fn pypi_and_nuget_sentinels_never_reach_the_outbound_set() {
    let r = fixture();
    let check = local_check(&r.wd, &FakeHostEnv::new());
    assert!(check.errors.is_empty(), "{:?}", check.errors);
    let public = public_names(&check);
    for n in ["six", "httpx", "Newtonsoft.Json", ABOVE] {
        assert!(public.iter().any(|p| p == n), "{n} missing: {public:?}");
        assert!(check.sent(n), "{n} not in the recorded outbound set");
    }
    assert_no_sentinel_except(&check.outbound, NEEDLES, &[ABOVE]);
}

#[test]
fn pypi_and_nuget_sentinel_rows_read_local_or_private() {
    let r = fixture();
    let check = local_check(&r.wd, &FakeHostEnv::new());
    assert_eq!(
        only(&check, "zz-sentinel-idx"),
        private("pypi.corp.invalid")
    );
    assert_eq!(
        only(&check, "zz_sentinel_pyvcs"),
        Visibility::Local(LocalReason::Git)
    );
    // Direct (pinned to the explicit index) and lock rows agree.
    assert_eq!(
        only(&check, "zz-sentinel-uvpin"),
        private("uv.corp.invalid")
    );
    // A transitive private lock entry.
    assert_eq!(
        only(&check, "zz-sentinel-uvtrans"),
        private("uv.corp.invalid")
    );
    for n in ["zz-sentinel-pyapp", "ZzSentinel.Proj"] {
        assert_eq!(
            only(&check, n),
            Visibility::Local(LocalReason::Workspace),
            "{n}"
        );
    }
}

#[test]
fn env_only_pypi_indexes_keep_every_package_private() {
    // (b) PyPI: one env-only registry each for pip and uv.
    for (k, v, host) in [
        (
            "PIP_INDEX_URL",
            "https://pip.corp.invalid/simple",
            "pip.corp.invalid",
        ),
        (
            "UV_DEFAULT_INDEX",
            "corp=https://uv-env.corp.invalid/simple",
            "uv-env.corp.invalid",
        ),
    ] {
        let r = fixture();
        let check = local_check(&r.wd, &FakeHostEnv::new().var(k, v));
        assert_eq!(only(&check, "six"), private(host), "{k}");
        let public = public_names(&check);
        assert!(
            public.iter().all(|n| n != "six" && n != "httpx"),
            "{k}: {public:?}"
        );
        assert!(
            public.iter().any(|n| n == "Newtonsoft.Json"),
            "NuGet unaffected"
        );
        assert_no_sentinel_except(&check.outbound, NEEDLES, &[ABOVE]);
    }
}

#[test]
fn an_unparseable_machine_wide_nuget_config_fails_nuget_closed() {
    // (b) NuGet stand-in: no env var exists.
    let r = fixture();
    let env = FakeHostEnv::new().user_file(UserConfig::NugetMachineWide, NUGET_BROKEN);
    let check = local_check(&r.wd, &env);
    assert_eq!(only(&check, "Newtonsoft.Json"), UNREADABLE);
    assert_eq!(only(&check, ABOVE), UNREADABLE);
    assert!(public_names(&check).iter().any(|n| n == "six"));
}

#[test]
fn a_tracked_mapped_nuget_config_keeps_the_corp_pattern_private() {
    // E4 row 6 `ZzSentinel.Pkg` (private nuget.config); only tracked here,
    // since without this config it is a public package.
    let r = fixture();
    r.track("dotnet/nuget.config", NUGET_MAPPED).track(
        "dotnet/Pkg/Pkg.csproj",
        "<Project><ItemGroup><PackageReference Include=\"ZzSentinel.Pkg\" Version=\"1.0.0\"/></ItemGroup></Project>",
    );
    let check = local_check(&r.wd, &FakeHostEnv::new());
    let mapped = Visibility::Private(
        PrivateReason::SourceMapping,
        Some("nuget.corp.invalid".into()),
    );
    assert_eq!(only(&check, "ZzSentinel.Pkg"), mapped);
    assert_eq!(only(&check, ABOVE), mapped);
    assert!(public_names(&check).iter().any(|n| n == "Newtonsoft.Json"));
    assert_no_sentinel(&check.outbound, NEEDLES);
}

#[test]
fn an_unparseable_nuget_config_makes_every_nuget_dep_config_unreadable() {
    // (d)
    let r = fixture();
    r.track("dotnet/App/nuget.config", NUGET_BROKEN);
    let check = local_check(&r.wd, &FakeHostEnv::new());
    assert_eq!(only(&check, "Newtonsoft.Json"), UNREADABLE);
    assert_eq!(only(&check, ABOVE), UNREADABLE);
    assert!(public_names(&check).iter().any(|n| n == "six"));
    assert_no_sentinel(&check.outbound, NEEDLES);
}

fn q7_env(r: &FixtureRepo) -> FakeHostEnv {
    FakeHostEnv::new().above_from_disk_within(&r.parent)
}

#[test]
fn q7_a_parent_dir_nuget_config_keeps_every_package_private() {
    // (f) NuGet half.
    let r = fixture();
    std::fs::write(r.parent.join("NuGet.Config"), Q7_NUGET_CONFIG).expect("write");
    let check = local_check(&r.wd, &q7_env(&r));
    assert_eq!(only(&check, ABOVE), private("nuget.corp.invalid"));
    assert_eq!(
        only(&check, "Newtonsoft.Json"),
        private("nuget.corp.invalid")
    );
    assert!(
        public_names(&check).iter().any(|n| n == "six"),
        "PyPI control"
    );
    assert_no_sentinel(&check.outbound, NEEDLES);
}

#[test]
fn q7_without_the_parent_nuget_config_the_package_would_be_sent() {
    let r = fixture();
    let check = local_check(&r.wd, &q7_env(&r));
    assert!(public_names(&check).iter().any(|n| n == ABOVE));
}

#[test]
fn q9_a_symlinked_parent_nuget_config_fails_closed_unless_the_parent_is_home() {
    let r = fixture();
    let target = r.parent.join("dotfiles-nuget.xml");
    std::fs::write(&target, Q7_NUGET_CONFIG).expect("write");
    if !try_symlink_file(&target, &r.parent.join("NuGet.Config")) {
        eprintln!("skipping: cannot create symlinks here");
        return;
    }
    let check = local_check(&r.wd, &q7_env(&r));
    assert_eq!(only(&check, ABOVE), UNREADABLE);
    let home = q7_env(&r).follow_root(&r.parent);
    let check = local_check(&r.wd, &home);
    assert_eq!(only(&check, ABOVE), private("nuget.corp.invalid"));
    assert_no_sentinel(&check.outbound, NEEDLES);
    // A link to a directory ⇒ ConfigUnreadable, even in $HOME.
    std::fs::remove_file(r.parent.join("NuGet.Config")).expect("rm");
    std::fs::create_dir_all(r.parent.join("a-dir")).expect("mkdir");
    assert!(try_symlink_file(
        &r.parent.join("a-dir"),
        &r.parent.join("NuGet.Config")
    ));
    let check = local_check(&r.wd, &home);
    assert_eq!(only(&check, ABOVE), UNREADABLE);
    // A dangling link ⇒ ConfigUnreadable.
    std::fs::remove_file(r.parent.join("NuGet.Config")).expect("rm");
    assert!(try_symlink_file(
        &r.parent.join("gone.xml"),
        &r.parent.join("NuGet.Config")
    ));
    let check = local_check(&r.wd, &home);
    assert_eq!(only(&check, ABOVE), UNREADABLE);
}

#[test]
fn a_pdm_or_rye_source_keeps_every_pypi_package_private() {
    // [P125c2 review] PDM / Rye index definitions (Q2: mirrors ⇒ private).
    for (tool, host) in [
        ("[[tool.pdm.source]]\nname = \"corp\"\nurl = \"https://u:zz-sentinel-token@pdm.corp.invalid/simple\"\n", "pdm.corp.invalid"),
        ("[[tool.rye.sources]]\nname = \"corp\"\nurl = \"https://rye.corp.invalid/simple\"\n", "rye.corp.invalid"),
    ] {
        let r = fixture();
        r.track(
            "pdm/pyproject.toml",
            &format!("[project]\nname = \"zz-sentinel-pdm\"\ndependencies = [\"zz-sentinel-pdmdep\"]\n\n{tool}"),
        );
        let check = local_check(&r.wd, &FakeHostEnv::new());
        assert_eq!(only(&check, "zz-sentinel-pdmdep"), private(host), "{tool}");
        assert_eq!(only(&check, "six"), private(host), "{tool}");
        assert!(public_names(&check).iter().any(|n| n == "Newtonsoft.Json"));
        assert_no_sentinel_except(&check.outbound, NEEDLES, &[ABOVE]);
    }
}

#[test]
fn an_includer_index_keeps_the_included_group_private() {
    // [P125c2 review] MUST-FIX 1 end to end.
    let r = FixtureRepo::new();
    r.track(
        "requirements.txt",
        "-i https://top.corp.invalid/simple\n-r requirements-base.txt\n",
    )
    .track("requirements-base.txt", "zz-sentinel-based==1.0\n");
    let check = local_check(&r.wd, &FakeHostEnv::new());
    assert_eq!(
        only(&check, "zz-sentinel-based"),
        private("top.corp.invalid")
    );
    assert!(check.public().is_empty());
}
