//! `[HIGH-1]` the PyPI half of `RegistryContext::load`: pip / uv / Poetry
//! configs, env vars and `requirements*.txt` includes route private or fail
//! closed; the key filter keeps tokens out.

use std::fs;
use std::path::PathBuf;

use super::*;
use crate::fixtures::pypi::{PIP_CONF, PYPROJECT_POETRY, PYPROJECT_UV, UV_TOML_DEFAULT};

struct Repo {
    _dir: tempfile::TempDir,
    wd: PathBuf,
}

fn repo() -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let wd = fs::canonicalize(dir.path()).unwrap().join("wd");
    fs::create_dir_all(wd.join("svc")).unwrap();
    Repo { _dir: dir, wd }
}

fn write(r: &Repo, rel: &str, body: &str) {
    let p = r.wd.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, body).unwrap();
}

fn manifests() -> Vec<String> {
    vec!["svc/requirements.txt".into(), "svc/pyproject.toml".into()]
}

fn load(r: &Repo, env: &FakeHostEnv) -> RegistryContext {
    RegistryContext::load(&r.wd, &manifests(), env)
}

const DEFAULT: DepSource = DepSource::Registry(RegistryRef::Default);
const PY: Ecosystem = Ecosystem::Pypi;

fn reason(ctx: &RegistryContext, src: &DepSource) -> Option<(PrivateReason, Option<String>)> {
    ctx.private_reason(PY, "six", src)
}

fn all_private(ctx: &RegistryContext, host: &str) -> bool {
    !ctx.fail_closed(PY)
        && reason(ctx, &DEFAULT) == Some((PrivateReason::CustomRegistry, Some(host.into())))
}

#[test]
fn no_config_opens_pypi_publicly() {
    let r = repo();
    write(&r, "svc/requirements.txt", "six\n");
    let ctx = load(&r, &FakeHostEnv::new());
    assert!(!ctx.fail_closed(PY));
    assert_eq!(reason(&ctx, &DEFAULT), None);
}

#[test]
fn pip_ini_continuation_lines_and_the_key_filter() {
    let r = repo();
    let env = FakeHostEnv::new().user_file(UserConfig::PipUser, PIP_CONF);
    let ctx = load(&r, &env);
    // The second continuation line is the private one.
    assert!(all_private(&ctx, "pip.corp.invalid"));
    // `client_cert` (holding the token) was never a URL: no fail-closed.
    let env = FakeHostEnv::new().user_file(
        UserConfig::PipSystem,
        "[global]\nclient-cert = zz-sentinel-token\ntimeout=5\n",
    );
    assert_eq!(reason(&load(&r, &env), &DEFAULT), None);
}

#[test]
fn every_bad_pip_config_fails_closed() {
    let r = repo();
    for bad in [
        "index-url = https://pypi.corp.invalid/simple\n",
        "[global\nindex-url = https://x.invalid/\n",
        "[global]\nno delimiter here\n",
        "[global]\nindex-url = /srv/wheels\n",
        "[install]\nfind-links = ${WHEELS}\n",
    ] {
        let env = FakeHostEnv::new().user_file(UserConfig::PipConfigFile, bad);
        assert!(load(&r, &env).fail_closed(PY), "{bad:?}");
    }
    let env = FakeHostEnv::new().user_unreadable(UserConfig::PipUser);
    assert!(load(&r, &env).fail_closed(PY));
}

#[test]
fn pip_and_uv_env_vars_route_private_or_fail_closed() {
    let r = repo();
    let env = FakeHostEnv::new().var(
        "PIP_INDEX_URL",
        "https://u:zz-sentinel-token@pip.corp.invalid/simple",
    );
    assert!(all_private(&load(&r, &env), "pip.corp.invalid"));
    let env = FakeHostEnv::new().var("UV_DEFAULT_INDEX", "corp=https://uv.corp.invalid/simple");
    assert!(all_private(&load(&r, &env), "uv.corp.invalid"));
    let env = FakeHostEnv::new().var(
        "UV_INDEX",
        "https://pypi.org/simple other=https://o.corp.invalid/",
    );
    assert!(all_private(&load(&r, &env), "o.corp.invalid"));
    let env = FakeHostEnv::new().var("PIP_EXTRA_INDEX_URL", "https://pypi.org/simple");
    assert_eq!(reason(&load(&r, &env), &DEFAULT), None);
    let env = FakeHostEnv::new().var("UV_CONFIG_FILE", "/etc/corp-uv.toml");
    assert!(load(&r, &env).fail_closed(PY));
    // Token-bearing names are never collected.
    let env = FakeHostEnv::new()
        .var("UV_INDEX_CORP_PASSWORD", "zz-sentinel-token")
        .var("PIP_CLIENT_CERT", "zz-sentinel-token");
    let _ = load(&r, &env);
    assert!(env.values_read().is_empty(), "{:?}", env.values_read());
}

#[test]
fn uv_toml_and_pyproject_index_definitions() {
    let r = repo();
    write(&r, "uv.toml", UV_TOML_DEFAULT);
    assert!(all_private(
        &load(&r, &FakeHostEnv::new()),
        "uv-default.corp.invalid"
    ));
    // An explicit uv index only supplies the host of the deps pinned to it.
    let r = repo();
    write(&r, "svc/pyproject.toml", PYPROJECT_UV);
    let ctx = load(&r, &FakeHostEnv::new());
    assert_eq!(reason(&ctx, &DEFAULT), None);
    assert_eq!(
        reason(
            &ctx,
            &DepSource::Registry(RegistryRef::Named("corp".into()))
        ),
        Some((
            PrivateReason::CustomRegistry,
            Some("uv.corp.invalid".into())
        ))
    );
    // Poetry: an explicit source, and a primary one.
    let r = repo();
    write(&r, "svc/pyproject.toml", PYPROJECT_POETRY);
    assert_eq!(reason(&load(&r, &FakeHostEnv::new()), &DEFAULT), None);
    let primary = PYPROJECT_POETRY.replace("priority = \"explicit\"", "priority = \"primary\"");
    write(&r, "svc/pyproject.toml", &primary);
    assert!(all_private(
        &load(&r, &FakeHostEnv::new()),
        "poetry.corp.invalid"
    ));
    // A non-explicit uv index (not `default`) is searched for every package.
    let plain = PYPROJECT_UV.replace("explicit = true", "");
    write(&r, "svc/pyproject.toml", &plain);
    assert!(all_private(
        &load(&r, &FakeHostEnv::new()),
        "uv.corp.invalid"
    ));
}

#[test]
fn an_unparseable_pyproject_or_uv_toml_fails_closed() {
    let r = repo();
    write(&r, "svc/pyproject.toml", "[tool.uv\n");
    assert!(load(&r, &FakeHostEnv::new()).fail_closed(PY));
    let r = repo();
    write(&r, "uv.toml", "[[index]]\nname = \"x\"\n");
    assert!(load(&r, &FakeHostEnv::new()).fail_closed(PY));
    let env = FakeHostEnv::new().user_file(UserConfig::UvUser, "index-url = 3\n");
    assert!(load(&repo(), &env).fail_closed(PY));
}

#[test]
fn an_index_in_an_included_requirements_file_privatises_pypi() {
    let r = repo();
    write(&r, "svc/requirements.txt", "-r reqs/base.txt\nsix\n");
    write(&r, "svc/reqs/base.txt", "-c ../../constraints.txt\nattrs\n");
    write(
        &r,
        "constraints.txt",
        "--extra-index-url https://inc.corp.invalid/simple\n",
    );
    assert!(all_private(
        &load(&r, &FakeHostEnv::new()),
        "inc.corp.invalid"
    ));
    // The top-level file's own index is the adapter's (per file), not ours.
    let r = repo();
    write(
        &r,
        "svc/requirements.txt",
        "-i https://own.corp.invalid/simple\nsix\n",
    );
    assert_eq!(reason(&load(&r, &FakeHostEnv::new()), &DEFAULT), None);
    // A top-level manifest that is also included still counts as included.
    let r = repo();
    let ms = vec![
        "svc/requirements.txt".to_string(),
        "svc/requirements-corp.txt".to_string(),
    ];
    write(
        &r,
        "svc/requirements.txt",
        "-r requirements-corp.txt
six
",
    );
    write(
        &r,
        "svc/requirements-corp.txt",
        "-i https://both.corp.invalid/simple
attrs
",
    );
    let ctx = RegistryContext::load(&r.wd, &ms, &FakeHostEnv::new());
    assert!(all_private(&ctx, "both.corp.invalid"));
    // A cycle is fine.
    let r = repo();
    write(&r, "svc/requirements.txt", "-r requirements.txt\nsix\n");
    assert!(!load(&r, &FakeHostEnv::new()).fail_closed(PY));
}

#[test]
fn unfollowable_includes_fail_closed() {
    for target in [
        "../../outside.txt",
        "/etc/reqs.txt",
        "https://h.invalid/reqs.txt",
        "missing.txt",
        "C:\\reqs.txt",
    ] {
        let r = repo();
        write(&r, "svc/requirements.txt", &format!("-r {target}\n"));
        assert!(load(&r, &FakeHostEnv::new()).fail_closed(PY), "{target}");
    }
    // Depth bound.
    let r = repo();
    for i in 0..=load_pypi::MAX_INCLUDE_DEPTH {
        write(
            &r,
            &format!("svc/r{i}.txt"),
            &format!("-r r{}.txt\n", i + 1),
        );
    }
    write(&r, "svc/requirements.txt", "-r r0.txt\n");
    assert!(load(&r, &FakeHostEnv::new()).fail_closed(PY));
}

#[test]
fn include_paths_resolve_lexically_inside_the_workdir() {
    use load_pypi::resolve_include;
    assert_eq!(resolve_include("a/b", "../c.txt"), Ok("a/c.txt".into()));
    assert_eq!(resolve_include("", "./x/y.txt"), Ok("x/y.txt".into()));
    assert_eq!(resolve_include("a", "x\\y.txt"), Ok("a/x/y.txt".into()));
    assert_eq!(resolve_include("a", "../../y.txt"), Err(Closed));
}

#[test]
fn an_includers_index_options_route_the_files_it_includes() {
    // [P125c2 review] MUST-FIX 1: `base`'s deps form their own group
    // (routed Default by the adapter), yet pip installs them from corp.
    let r = repo();
    let ms = vec![
        "svc/requirements.txt".to_string(),
        "svc/requirements-base.txt".to_string(),
    ];
    write(
        &r,
        "svc/requirements.txt",
        "-i https://top.corp.invalid/simple\n-r requirements-base.txt\n",
    );
    write(&r, "svc/requirements-base.txt", "six\n");
    let ctx = RegistryContext::load(&r.wd, &ms, &FakeHostEnv::new());
    assert!(all_private(&ctx, "top.corp.invalid"));
    // Two deep: the middle file's own options count too, and `--no-index`
    // / find-links in an includer are global as well.
    for (opt, host) in [
        (
            "--extra-index-url https://mid.corp.invalid/simple",
            Some("mid.corp.invalid"),
        ),
        ("--no-index", None),
        (
            "--find-links https://links.corp.invalid/",
            Some("links.corp.invalid"),
        ),
    ] {
        let r = repo();
        write(&r, "svc/requirements.txt", "-r a/r1.txt\nsix\n");
        write(&r, "svc/a/r1.txt", &format!("{opt}\n-r ../b/r2.txt\n"));
        write(&r, "svc/b/r2.txt", "attrs\n");
        let ctx = load(&r, &FakeHostEnv::new());
        assert_eq!(
            reason(&ctx, &DEFAULT),
            Some((PrivateReason::CustomRegistry, host.map(String::from))),
            "{opt}"
        );
    }
}

#[test]
fn pdm_and_rye_sources_are_index_definitions() {
    for (tool, host) in [
        ("[[tool.pdm.source]]\nname = \"corp\"\nurl = \"https://u:zz-sentinel-token@pdm.corp.invalid/simple\"\nverify_ssl = true\n", "pdm.corp.invalid"),
        ("[[tool.rye.sources]]\nname = \"corp\"\nurl = \"https://rye.corp.invalid/simple\"\n", "rye.corp.invalid"),
    ] {
        let r = repo();
        write(&r, "svc/pyproject.toml", &format!("[project]\nname = \"a\"\n\n{tool}"));
        assert!(all_private(&load(&r, &FakeHostEnv::new()), host), "{tool}");
    }
    // A PyPI source stays public.
    let r = repo();
    write(
        &r,
        "svc/pyproject.toml",
        "[[tool.pdm.source]]\nname = \"pypi\"\nurl = \"https://pypi.org/simple\"\n",
    );
    assert_eq!(reason(&load(&r, &FakeHostEnv::new()), &DEFAULT), None);
    // Hostless or malformed entries fail closed.
    for bad in [
        "[[tool.pdm.source]]\nname = \"corp\"\nurl = \"${PDM_INDEX}\"\n",
        "[[tool.pdm.source]]\nname = \"corp\"\n",
        "[[tool.rye.sources]]\nname = \"local\"\nurl = \"file:///srv/wheels\"\n",
        "[tool.rye]\nsources = \"x\"\n",
    ] {
        let r = repo();
        write(&r, "svc/pyproject.toml", bad);
        assert!(load(&r, &FakeHostEnv::new()).fail_closed(PY), "{bad}");
    }
}

#[test]
fn the_in_repo_virtualenv_pip_config_is_read() {
    for venv in [
        ".venv/pip.conf",
        ".venv/pip.ini",
        "venv/pip.conf",
        "venv/pip.ini",
    ] {
        let r = repo();
        write(
            &r,
            venv,
            "[global]\nindex-url = https://u:zz-sentinel-token@venv.corp.invalid/simple\n",
        );
        assert!(
            all_private(&load(&r, &FakeHostEnv::new()), "venv.corp.invalid"),
            "{venv}"
        );
    }
    let r = repo();
    write(&r, ".venv/pip.conf", "index-url = https://x.invalid/\n");
    assert!(load(&r, &FakeHostEnv::new()).fail_closed(PY));
}

#[test]
fn a_symlinked_in_repo_venv_pip_config_fails_closed() {
    let r = repo();
    let target = r.wd.join("real-pip.conf");
    fs::write(&target, "[global]\ntimeout = 5\n").unwrap();
    fs::create_dir_all(r.wd.join(".venv")).unwrap();
    if !crate::fsread::tests::try_symlink_file(&target, &r.wd.join(".venv/pip.conf")) {
        eprintln!("skipping: cannot create symlinks here");
        return;
    }
    assert!(load(&r, &FakeHostEnv::new()).fail_closed(PY));
}
