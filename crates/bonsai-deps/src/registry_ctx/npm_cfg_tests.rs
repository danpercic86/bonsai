//! `[MEDIUM-1]` extractor tests: host-only extraction and key filtering.

use super::extract_host;
use super::npm_cfg::*;
use super::EcoRegistry;

fn open(f: impl FnOnce(&mut NpmSignals) -> Result<(), Closed>) -> Result<EcoRegistry, Closed> {
    let mut s = NpmSignals::default();
    f(&mut s)?;
    Ok(s.into_open())
}

#[test]
fn extract_host_is_host_only() {
    let cases = [
        ("https://registry.npmjs.org/", Some("registry.npmjs.org")),
        (
            "https://u:zz-sentinel-token@NPM.Corp.Invalid:8443/a/b?q=1#f",
            Some("npm.corp.invalid"),
        ),
        ("http://npm.corp.invalid.", Some("npm.corp.invalid")),
        ("  https://h.invalid/  ", Some("h.invalid")),
        ("//npm.corp.invalid/", None),
        ("npm.corp.invalid", None),
        ("file:///etc/npmrc", None),
        ("git+https://h/", None),
        ("https://", None),
        ("", None),
    ];
    for (raw, want) in cases {
        assert_eq!(extract_host(raw).as_deref(), want, "{raw}");
    }
}

#[test]
fn npmrc_keeps_only_registry_keys() {
    let text = "; comment\n# c\n[section]\n\
                //npm.corp.invalid/:_authToken=zz-sentinel-token\n\
                _auth = zz-sentinel-token\n\
                always-auth=true\n\
                @Corp:registry = \"https://u:zz-sentinel-token@npm.corp.invalid/\"\n\
                registry=https://registry.npmjs.org/\n";
    let e = open(|s| npmrc(s, text)).unwrap();
    assert!(e.default_public);
    assert_eq!(
        e.scoped.get("@corp"),
        Some(&Some("npm.corp.invalid".to_string()))
    );
    assert_eq!(e.scoped.len(), 1);
}

#[test]
fn npmrc_non_public_default_and_failures() {
    let e = open(|s| npmrc(s, "registry=https://mirror.corp.invalid/npm/\n")).unwrap();
    assert!(!e.default_public);
    assert_eq!(e.default_host.as_deref(), Some("mirror.corp.invalid"));
    for bad in [
        "registry=${CORP_REGISTRY}\n",
        "registry=https://${HOST}/\n",
        "registry=not a url\n",
        "@corp:registry=//npm.corp.invalid/\n",
        "${SCOPE}:registry=https://h.invalid/\n",
        "@:registry=https://h.invalid/\n",
    ] {
        assert!(open(|s| npmrc(s, bad)).is_err(), "{bad}");
    }
    // A public scope registry is not a private signal.
    let e = open(|s| npmrc(s, "@x:registry=https://registry.npmjs.org/\n")).unwrap();
    assert!(e.scoped.is_empty());
}

#[test]
fn yarnrc_v1_lines() {
    let text = "# yarn v1\nregistry \"https://npm.corp.invalid/\"\n\
                \"@zz:registry\" \"https://zz.corp.invalid\"\n\
                \"//npm.corp.invalid/:_authToken\" \"zz-sentinel-token\"\n\
                --registry https://other.invalid\n";
    let e = open(|s| yarnrc_v1(s, text)).unwrap();
    assert_eq!(e.default_host.as_deref(), Some("npm.corp.invalid"));
    assert!(e.scoped.contains_key("@zz"));
    assert!(open(|s| yarnrc_v1(s, "\"unterminated\n")).is_err());
}

#[test]
fn yarnrc_yml_registry_and_scopes() {
    let text = "npmRegistryServer: \"https://npm.corp.invalid\"\n\
                npmAuthToken: zz-sentinel-token\n\
                npmScopes:\n  zzign:\n    npmRegistryServer: \"https://ign.corp.invalid\"\n    npmAuthToken: zz-sentinel-token\n\
                  public:\n    npmAlwaysAuth: true\n";
    let e = open(|s| yarnrc_yml(s, text)).unwrap();
    assert_eq!(e.default_host.as_deref(), Some("npm.corp.invalid"));
    assert_eq!(
        e.scoped.get("@zzign"),
        Some(&Some("ign.corp.invalid".into()))
    );
    for bad in [
        "npmRegistryServer: [a]\n",
        "npmScopes: 1\n",
        "npmRegistryServer: \"${REG}\"\n",
        "a: &x 1\n",
        "npmScopes:\n  s:\n    npmRegistryServer: {a: 1}\n",
        "npmRegistryServer: [unclosed\n",
    ] {
        assert!(open(|s| yarnrc_yml(s, bad)).is_err(), "{bad}");
    }
}

#[test]
fn pnpm_workspace_registries() {
    let text = "packages:\n  - 'packages/*'\n\
                registries:\n  default: https://registry.npmjs.org/\n  \"@my-org\": https://private.corp.invalid/\n\
                namedRegistries:\n  work: https://npm.work.invalid/\n\
                catalog:\n  react: ^18\n";
    let e = open(|s| pnpm_workspace(s, text)).unwrap();
    assert!(e.default_public);
    assert!(e.scoped.contains_key("@my-org"));
    assert_eq!(
        e.custom_hosts.get("work"),
        Some(&Some("npm.work.invalid".into()))
    );
    let e = open(|s| pnpm_workspace(s, "registry: https://mirror.invalid/\n")).unwrap();
    assert!(!e.default_public);
    for bad in [
        "registries:\n  weird: https://h.invalid/\n",
        "registries:\n  default: [x]\n",
        "namedRegistries:\n  a: not-a-url\n",
        "catalog: &c\n  react: ^18\n",
    ] {
        assert!(open(|s| pnpm_workspace(s, bad)).is_err(), "{bad}");
    }
}

#[test]
fn env_key_filters_never_keep_auth_keys() {
    for k in [
        "npm_config_registry",
        "NPM_CONFIG_REGISTRY",
        "npm_config_@zzenv:registry",
        "PNPM_CONFIG_REGISTRY",
        "pnpm_config_@corp:registry",
    ] {
        assert!(keep_npm_env(k), "{k}");
    }
    for k in [
        "npm_config_//npm.corp.invalid/:_authToken",
        "npm_config__auth",
        "pnpm_config__auth",
        "NPM_TOKEN",
        "npm_config_cache",
        "npm_config_registry_x",
    ] {
        assert!(!keep_npm_env(k), "{k}");
    }
    assert!(keep_yarn_env("YARN_NPM_REGISTRY_SERVER"));
    assert!(keep_yarn_env("yarn_rc_filename"));
    assert!(!keep_yarn_env("YARN_NPM_AUTH_TOKEN"));
}

#[test]
fn env_pairs() {
    let e =
        open(|s| env_pair(s, "npm_config_@ZzEnv:registry", "https://env.corp.invalid/")).unwrap();
    assert!(e.scoped.contains_key("@zzenv"));
    let e = open(|s| env_pair(s, "YARN_REGISTRY", "https://y.corp.invalid/")).unwrap();
    assert!(!e.default_public);
    assert!(open(|s| env_pair(s, "YARN_NPM_SCOPES", "{}")).is_err());
    assert!(open(|s| env_pair(s, "YARN_RC_FILENAME", ".x.yml")).is_err());
    assert!(open(|s| env_pair(s, "npm_config_registry", "${X}")).is_err());
}

/// `[HIGH-1]` npm's `ini` splits on `/[\r\n]+/` and JS `trim()` drops U+FEFF;
/// a BOM or a CR-only break must not hide a scope key.
#[test]
fn bom_and_cr_only_breaks_do_not_hide_keys() {
    type Ex = fn(&mut NpmSignals, &str) -> Result<(), Closed>;
    let cases: &[(&str, Ex, &str, &str)] = &[
        (
            "npmrc",
            npmrc,
            "\u{FEFF}@corp:registry=https://npm.corp.invalid/\n",
            "a=b\r@corp:registry=https://npm.corp.invalid/\r",
        ),
        (
            "yarnrc_v1",
            yarnrc_v1,
            "\u{FEFF}\"@corp:registry\" \"https://npm.corp.invalid/\"\n",
            "# c\r\"@corp:registry\" \"https://npm.corp.invalid/\"\r",
        ),
        (
            "yarnrc_yml",
            yarnrc_yml,
            "\u{FEFF}npmScopes:\n  corp:\n    npmRegistryServer: https://npm.corp.invalid/\n",
            "npmScopes:\r  corp:\r    npmRegistryServer: https://npm.corp.invalid/\r",
        ),
        (
            "pnpm_workspace",
            pnpm_workspace,
            "\u{FEFF}registries:\n  '@corp': https://npm.corp.invalid/\n",
            "registries:\r  '@corp': https://npm.corp.invalid/\r",
        ),
    ];
    for (label, ex, bom, cr) in cases {
        for (form, text) in [("bom", bom), ("cr", cr)] {
            let e = open(|s| ex(s, text)).unwrap_or_else(|_| panic!("{label} {form}: closed"));
            assert_eq!(
                e.scoped.get("@corp"),
                Some(&Some("npm.corp.invalid".to_string())),
                "{label} {form}"
            );
        }
    }
}

#[test]
fn non_map_registry_tables_fail_closed() {
    for bad in [
        "registries: https://h.invalid/\n",
        "registries: [https://h.invalid/]\n",
        "namedRegistries: https://h.invalid/\n",
        "namedRegistries:\n  - https://h.invalid/\n",
    ] {
        assert!(open(|s| pnpm_workspace(s, bad)).is_err(), "{bad}");
    }
}
