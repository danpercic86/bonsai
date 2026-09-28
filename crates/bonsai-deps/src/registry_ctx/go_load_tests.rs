//! The Go half of `RegistryContext::load`: glob variables, GOPROXY (Q2),
//! GOSUMDB and GOINSECURE, from the env and the GOENV file.

use std::fs;
use std::path::PathBuf;

use super::*;

const DEFAULT: DepSource = DepSource::Registry(RegistryRef::Default);

struct Repo {
    _dir: tempfile::TempDir,
    wd: PathBuf,
}

fn repo() -> Repo {
    let dir = tempfile::tempdir().unwrap();
    let wd = fs::canonicalize(dir.path()).unwrap().join("wd");
    fs::create_dir_all(&wd).unwrap();
    Repo { _dir: dir, wd }
}

fn load(r: &Repo, env: &FakeHostEnv) -> RegistryContext {
    RegistryContext::load(&r.wd, &["go.mod".to_string()], env)
}

#[test]
fn go_globs_come_from_env_and_the_goenv_file() {
    let r = repo();
    let env = FakeHostEnv::new()
        .var("GOPRIVATE", "corp.invalid/*, ,*.corp.example")
        .user_file(
            UserConfig::GoEnv,
            "GONOSUMDB=git.corp.invalid\r\nGOAUTH=zz-sentinel-token\nGOFLAGS=-mod=mod\n",
        );
    let ctx = load(&r, &env);
    let e = ctx.eco(Ecosystem::Go);
    assert!(!e.fail_closed);
    assert_eq!(
        e.go_private_globs,
        ["corp.invalid/*", "*.corp.example", "git.corp.invalid"]
    );
    for m in [
        "corp.invalid/zzsentinel/mod",
        "x.corp.example/y",
        "git.corp.invalid/a/b",
    ] {
        assert_eq!(
            ctx.private_reason(Ecosystem::Go, m, &DEFAULT),
            Some((PrivateReason::GoPrivate, None)),
            "{m}"
        );
    }
    assert_eq!(
        ctx.private_reason(Ecosystem::Go, "github.com/pkg/errors", &DEFAULT),
        None
    );
    // Only the routing keys are read (GOAUTH / GOFLAGS never are).
    assert_eq!(env.values_read(), ["GOPRIVATE"]);
    let env = FakeHostEnv::new().user_unreadable(UserConfig::GoEnv);
    assert!(load(&r, &env).fail_closed(Ecosystem::Go));
}

#[test]
fn go_proxy_sumdb_and_insecure_route_or_close() {
    let r = repo();
    let go = |env: &FakeHostEnv| load(&r, env);
    // Q2: a corporate proxy makes every module private, with its host.
    let env = FakeHostEnv::new().var(
        "GOPROXY",
        "https://u:zz-sentinel-token@goproxy.corp.invalid|https://proxy.golang.org,direct",
    );
    let ctx = go(&env);
    let e = ctx.eco(Ecosystem::Go);
    assert!(!e.fail_closed && !e.default_public);
    assert_eq!(e.default_host.as_deref(), Some("goproxy.corp.invalid"));
    assert_eq!(
        ctx.private_reason(Ecosystem::Go, "github.com/pkg/errors", &DEFAULT),
        Some((
            PrivateReason::CustomRegistry,
            Some("goproxy.corp.invalid".into())
        ))
    );
    // The same from the GOENV file.
    let env =
        FakeHostEnv::new().user_file(UserConfig::GoEnv, "GOPROXY=https://athens.corp.invalid\n");
    assert!(!go(&env).eco(Ecosystem::Go).default_public);
    // The public defaults stay public.
    for v in [
        "https://proxy.golang.org,direct",
        "off",
        "direct",
        "https://proxy.golang.org/",
    ] {
        let ctx = go(&FakeHostEnv::new().var("GOPROXY", v));
        assert!(ctx.eco(Ecosystem::Go).default_public, "{v}");
    }
    // A proxy without a host, GOSUMDB off / custom => closed.
    for (k, v) in [
        ("GOPROXY", "file:///srv/goproxy"),
        ("GOSUMDB", "off"),
        ("GOSUMDB", "sum.corp.invalid+abc https://sum.corp.invalid"),
        ("GOSUMDB", "sum.golang.org https://sum.corp.invalid"),
    ] {
        let env = FakeHostEnv::new().var(k, v);
        assert!(go(&env).fail_closed(Ecosystem::Go), "{k}={v}");
    }
    let env = FakeHostEnv::new().user_file(UserConfig::GoEnv, "GOSUMDB=off\n");
    assert!(go(&env).fail_closed(Ecosystem::Go));
    for v in [
        "sum.golang.org",
        "sum.golang.org+033de0ae+Ac4zctda0e5eza+HJyk9SxEdh https://sum.golang.org",
        "",
    ] {
        let env = FakeHostEnv::new().var("GOSUMDB", v);
        assert!(!go(&env).fail_closed(Ecosystem::Go), "{v}");
    }
    // GOINSECURE globs are private globs.
    let ctx = go(&FakeHostEnv::new().var("GOINSECURE", "insecure.corp.invalid"));
    assert_eq!(
        ctx.private_reason(Ecosystem::Go, "insecure.corp.invalid/m", &DEFAULT),
        Some((PrivateReason::GoPrivate, None))
    );
}
