//! Static data only (contract §1): the public-host allowlists that
//! `RegistryContext::private_reason` consults, the not-tracked table, the
//! discovery skip list and the registry-page URL templates.

use crate::model::Ecosystem;

/// Lockfile / index hosts that are the ecosystem's public registry (E2).
/// Hosts are compared lowercase, exactly (no suffix match).
pub(crate) fn public_hosts(eco: Ecosystem) -> &'static [&'static str] {
    match eco {
        Ecosystem::Npm => &["registry.npmjs.org", "registry.yarnpkg.com"],
        // NOT github.com: a private registry index can live there. The Cargo
        // adapter maps the exact lock source
        // `registry+https://github.com/rust-lang/crates.io-index` to
        // `RegistryRef::Default` instead (E2).
        Ecosystem::Cargo => &["index.crates.io"],
        Ecosystem::Pypi => &["pypi.org", "files.pythonhosted.org"],
        Ecosystem::Go => &["proxy.golang.org"],
        Ecosystem::Nuget => &["api.nuget.org"],
    }
}

pub(crate) fn is_public_host(eco: Ecosystem, host: &str) -> bool {
    public_hosts(eco)
        .iter()
        .any(|h| h.eq_ignore_ascii_case(host))
}

/// E2 "Not tracked": exact file name → ecosystem label. One `NotTracked`
/// group per file, zero rows, never parsed.
pub(crate) const NOT_TRACKED: &[(&str, &str)] = &[
    ("pom.xml", "Maven"),
    ("build.gradle", "Gradle"),
    ("build.gradle.kts", "Gradle"),
    ("composer.json", "Composer"),
    ("pubspec.yaml", "Pub"),
    ("Gemfile", "RubyGems"),
    ("Package.swift", "SwiftPM"),
    ("Podfile", "CocoaPods"),
    ("mix.exs", "Hex"),
];

pub(crate) fn not_tracked_label(file_name: &str) -> Option<&'static str> {
    NOT_TRACKED
        .iter()
        .find(|(f, _)| *f == file_name)
        .map(|(_, l)| *l)
}

/// Path segments never descended into by discovery (§4.1).
pub(crate) const SKIPPED_SEGMENTS: &[&str] = &[
    "node_modules",
    "bower_components",
    ".venv",
    "venv",
    "vendor",
    "target",
    ".git",
];

/// §7.6 registry-page URL templates; `{}` is the percent-encoded, already
/// validated name.
pub(crate) fn registry_page_template(eco: Ecosystem) -> &'static str {
    match eco {
        Ecosystem::Npm => "https://www.npmjs.com/package/{}",
        Ecosystem::Cargo => "https://crates.io/crates/{}",
        Ecosystem::Pypi => "https://pypi.org/project/{}/",
        Ecosystem::Go => "https://pkg.go.dev/{}",
        Ecosystem::Nuget => "https://www.nuget.org/packages/{}",
    }
}
