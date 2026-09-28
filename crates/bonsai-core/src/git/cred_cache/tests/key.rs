//! Cache-key normalization (`normalize_key` / `key_for` + useHttpPath).

use super::*;

// 9. key normalization table (host-only, useHttpPath OFF).
#[test]
fn normalize_key_table() {
    assert_eq!(
        normalize_key("https://Host.COM/a/b.git?x=1#f", false),
        "https://host.com"
    );
    assert_eq!(
        normalize_key("https://user:pw@host.com/a", false),
        "https://host.com"
    );
    assert_eq!(
        normalize_key("https://host.com/other", false),
        "https://host.com"
    );
    assert_eq!(
        normalize_key("https://host.com:8443/a", false),
        "https://host.com:8443"
    );
    assert_eq!(normalize_key("HTTPS://HOST.com", false), "https://host.com");
    // non-`://` fallback -> lowercased input.
    assert_eq!(
        normalize_key("Git@GitHub.com:owner/repo.git", false),
        "git@github.com:owner/repo.git"
    );

    // The first three collapse to one shared key (host+scheme only).
    let a = normalize_key("https://Host.COM/a/b.git?x=1#f", false);
    let b = normalize_key("https://user:pw@host.com/a", false);
    let c = normalize_key("https://host.com/other", false);
    assert_eq!(a, b);
    assert_eq!(b, c);
}

// 9b. key normalization with useHttpPath ON: the PATH disambiguates
// per-org tokens on a shared host (F-A5-a — the dev.azure.com case). Path
// is case-preserved; scheme+host still lowercased; userinfo/query/fragment
// still dropped.
#[test]
fn normalize_key_with_http_path_includes_path() {
    // Two orgs on the SAME host now yield DISTINCT keys.
    let org_a = normalize_key("https://dev.azure.com/OrgA/_git/repo", true);
    let org_b = normalize_key("https://dev.azure.com/OrgB/_git/repo", true);
    assert_ne!(org_a, org_b, "different paths => different keys");
    assert_eq!(org_a, "https://dev.azure.com/OrgA/_git/repo");

    // Without useHttpPath they collapse to the shared host key (the old,
    // cross-contaminating behavior we are guarding against).
    assert_eq!(
        normalize_key("https://dev.azure.com/OrgA/_git/repo", false),
        normalize_key("https://dev.azure.com/OrgB/_git/repo", false)
    );

    // Path case is preserved; scheme+host lowercased; query/fragment dropped.
    assert_eq!(
        normalize_key("HTTPS://Dev.Azure.COM/OrgA/repo.git?x=1#f", true),
        "https://dev.azure.com/OrgA/repo.git"
    );
    // userinfo still dropped, path kept.
    assert_eq!(
        normalize_key("https://user:pw@host.com/a/b", true),
        "https://host.com/a/b"
    );
    // No path -> just the base (trailing slash-less).
    assert_eq!(normalize_key("https://host.com", true), "https://host.com");
}

// 9c. key_for reads credential.useHttpPath from the repo config: unset =>
// host-only key; set true => path-scoped key (F-A5-a). Also exercises the
// per-URL-scoped `credential.<host>.useHttpPath` form.
#[test]
fn key_for_honors_use_http_path_config() {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init");
    // Use a host that cannot appear in the machine's GLOBAL git config, and
    // set the LOCAL value explicitly so the test is immune to a global
    // `credential.useHttpPath` default (Azure DevOps devs often set one).
    let url = "https://git.example.test/OrgA/repo";

    {
        let mut cfg = repo.config().expect("config");
        cfg.set_bool("credential.useHttpPath", false).expect("off");
    }
    assert_eq!(
        key_for(Some(dir.path()), url),
        "https://git.example.test",
        "useHttpPath off => host-only key"
    );

    {
        let mut cfg = repo.config().expect("config");
        cfg.set_bool("credential.useHttpPath", true).expect("on");
    }
    assert_eq!(
        key_for(Some(dir.path()), url),
        "https://git.example.test/OrgA/repo",
        "useHttpPath on => path-scoped key"
    );

    // Two orgs on the same host now key distinctly.
    assert_ne!(
        key_for(Some(dir.path()), "https://git.example.test/OrgA/repo"),
        key_for(Some(dir.path()), "https://git.example.test/OrgB/repo"),
    );
}

// 9d. the URL-host-scoped config key wins independently of the unscoped one
// (how Azure DevOps users typically enable it).
#[test]
fn key_for_honors_host_scoped_use_http_path() {
    let dir = crate::testutil::scratch_dir();
    let repo = git2::Repository::init(dir.path()).expect("init");
    {
        let mut cfg = repo.config().expect("config");
        // Unscoped OFF locally (override any global default), host-scoped ON.
        cfg.set_bool("credential.useHttpPath", false)
            .expect("unscoped off");
        cfg.set_bool("credential.https://azdo.example.test.useHttpPath", true)
            .expect("set scoped");
    }
    // Host-scoped true => path included for that host.
    assert_eq!(
        key_for(Some(dir.path()), "https://azdo.example.test/OrgA/_git/repo"),
        "https://azdo.example.test/OrgA/_git/repo"
    );
    // A different host without the scoped key falls to the unscoped OFF.
    assert_eq!(
        key_for(Some(dir.path()), "https://other.example.test/OrgA/repo.git"),
        "https://other.example.test"
    );
}
