//! Static data only (contract §1). P125a: the public-host allowlists that
//! `RegistryContext::private_reason` consults. The not-tracked table and the
//! registry-page URL templates land in P125b.

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
