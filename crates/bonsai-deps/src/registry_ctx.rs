//! `RegistryContext` (contract §4.4): the per-ecosystem registry signals that
//! `privacy::classify` consults, and the loader that gathers them.
//!
//! P125b loads the npm family (npm / pnpm / yarn, E3); P125c1 adds Cargo
//! and Go; P125c2 adds PyPI (pip / uv / Poetry) and NuGet (the NuGet.Config
//! chain incl. the Q7 above-workdir walk).
//!
//! `[MEDIUM-1]` `RegistryContext` has NO Debug, Serialize, Deserialize or
//! Clone (and deliberately no Default: a default context would be a
//! fail-OPEN one). Its fields are hosts, scope names, globs and bools only.

mod cargo_cfg;
#[cfg(any(test, feature = "test-support"))]
mod fake_env;
mod globs;
mod host_env;
mod load_cargo;
mod load_go;
mod load_npm;
mod load_nuget;
mod load_pypi;
mod npm_cfg;
mod nuget_cfg;
mod nuget_chain;
mod pip_cfg;
mod walk;

use std::collections::BTreeMap;
use std::path::Path;

/// Test double; only with `cfg(test)` or the `test-support` feature, so a
/// production build cannot construct an (empty, fail-open) fake.
#[cfg(any(test, feature = "test-support"))]
pub use fake_env::FakeHostEnv;
pub use host_env::{HostEnv, UserConfig, UserRead};

use crate::adapter::{DepSource, RegistryRef};
use crate::catalog::is_public_host;
use crate::model::{Ecosystem, PrivateReason};

/// The ecosystem being loaded must fail closed. Deliberately carries
/// nothing (no text, no path) `[MEDIUM-1]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Closed;

/// One `<packageSourceMapping>` id pattern (`*`, `Prefix.*` or an exact id),
/// already resolved to "maps only to nuget.org" or not.
pub(crate) struct NugetMapping {
    /// Lowercase pattern.
    pub(crate) pattern: String,
    pub(crate) public_only: bool,
    /// Display host of the (first) private source it maps to.
    pub(crate) host: Option<String>,
}

/// The registry signals for ONE ecosystem.
pub(crate) struct EcoRegistry {
    /// `[HIGH-1 item 2]` a config was unreadable, oversize, symlinked,
    /// unparseable, or a URL did not yield a host ⇒ everything Private.
    pub(crate) fail_closed: bool,
    /// false ⇒ the default registry was overridden by a non-public one
    /// (npm `registry=`, pip `index-url`, …); `RegistryRef::Default` deps
    /// become CustomRegistry.
    pub(crate) default_public: bool,
    /// Display host of the overriding default registry, if known.
    pub(crate) default_host: Option<String>,
    /// Every dep of the ecosystem is CustomRegistry, whatever its source
    /// (Cargo `[source.crates-io] replace-with`, a non-nuget.org NuGet
    /// source, a uv/poetry default index).
    pub(crate) replace_all: bool,
    /// `[P125h §3]` the reason `replace_all` reports: `CustomRegistry`
    /// unless every source behind it is a local NuGet feed (`LocalFeed`).
    pub(crate) all_reason: PrivateReason,
    /// Named registries (Cargo `[registries.<name>]`) → display host.
    pub(crate) custom_hosts: BTreeMap<String, Option<String>>,
    /// npm/yarn/pnpm scope (`@corp`, lowercase) → display host.
    pub(crate) scoped: BTreeMap<String, Option<String>>,
    /// Go `GOPRIVATE` / `GONOPROXY` / `GONOSUMDB` globs, one per entry.
    pub(crate) go_private_globs: Vec<String>,
    pub(crate) nuget_mapping: Vec<NugetMapping>,
}

impl EcoRegistry {
    pub(crate) fn public() -> Self {
        EcoRegistry {
            fail_closed: false,
            default_public: true,
            default_host: None,
            replace_all: false,
            all_reason: PrivateReason::CustomRegistry,
            custom_hosts: BTreeMap::new(),
            scoped: BTreeMap::new(),
            go_private_globs: Vec::new(),
            nuget_mapping: Vec::new(),
        }
    }
}

/// The per-ecosystem registry signals. `[MEDIUM-1]` no-derive policy, pinned:
///
/// ```compile_fail
/// fn leak(c: &bonsai_deps::registry_ctx::RegistryContext) -> String {
///     format!("{c:?}") // no Debug
/// }
/// ```
///
/// ```compile_fail
/// use bonsai_deps::registry_ctx::RegistryContext;
/// fn dup(c: &RegistryContext) {
///     let _copy: RegistryContext = c.clone(); // no Clone
/// }
/// ```
///
/// ```compile_fail
/// fn ser<T: serde::Serialize>(_: &T) {}
/// fn f(c: &bonsai_deps::registry_ctx::RegistryContext) {
///     ser(c) // no Serialize
/// }
/// ```
///
/// ```compile_fail
/// fn de<T: serde::de::DeserializeOwned>() {}
/// fn f() {
///     de::<bonsai_deps::registry_ctx::RegistryContext>() // no Deserialize
/// }
/// ```
///
/// ```compile_fail
/// fn open() -> bonsai_deps::registry_ctx::RegistryContext {
///     Default::default() // no Default: that would be a fail-OPEN context
/// }
/// ```
///
/// Positive control (the path resolves; the blocks above fail on the
/// missing impls):
///
/// ```
/// fn ser<T: serde::Serialize>(_: &T) {}
/// fn closed(c: &bonsai_deps::registry_ctx::RegistryContext) -> bool {
///     fn de<T: serde::de::DeserializeOwned>() {}
///     ser(&bonsai_deps::model::Ecosystem::Npm); // serde resolves in doctests
///     de::<bonsai_deps::model::Ecosystem>();
///     c.fail_closed(bonsai_deps::model::Ecosystem::Npm)
/// }
/// ```
pub struct RegistryContext {
    ecos: [EcoRegistry; 5],
}

fn idx(eco: Ecosystem) -> usize {
    match eco {
        Ecosystem::Npm => 0,
        Ecosystem::Cargo => 1,
        Ecosystem::Pypi => 2,
        Ecosystem::Go => 3,
        Ecosystem::Nuget => 4,
    }
}

impl RegistryContext {
    /// §4.4. Starts with EVERY ecosystem fail-closed; an ecosystem is opened
    /// only when its loader returned `Ok` for every step (so any early
    /// return or error leaves it closed). `workdir` is canonicalised here;
    /// failing that, everything stays closed.
    pub fn load(workdir: &Path, manifests: &[String], env: &dyn HostEnv) -> RegistryContext {
        let mut ctx = Self::all_closed();
        let Ok(canon) = std::fs::canonicalize(workdir) else {
            return ctx;
        };
        if let Ok(signals) = load_npm::load(&canon, manifests, env) {
            *ctx.eco_mut(Ecosystem::Npm) = signals.into_open();
        }
        if let Ok(signals) = load_cargo::load(&canon, manifests, env) {
            *ctx.eco_mut(Ecosystem::Cargo) = signals.into_open();
        }
        if let Ok(open) = load_go::load(env) {
            *ctx.eco_mut(Ecosystem::Go) = open;
        }
        if let Ok(signals) = load_pypi::load(&canon, manifests, env) {
            *ctx.eco_mut(Ecosystem::Pypi) = signals.into_open();
        }
        if let Ok(open) = load_nuget::load(&canon, manifests, env) {
            *ctx.eco_mut(Ecosystem::Nuget) = open;
        }
        ctx
    }

    fn all_closed() -> Self {
        let mut ctx = Self::no_signals();
        for eco in Ecosystem::ALL {
            ctx.eco_mut(eco).fail_closed = true;
        }
        ctx
    }

    /// A context with no private signal for any ecosystem. Crate-private:
    /// only the loader (and tests) may start from it.
    pub(crate) fn no_signals() -> Self {
        RegistryContext {
            ecos: std::array::from_fn(|_| EcoRegistry::public()),
        }
    }

    pub(crate) fn eco_mut(&mut self, eco: Ecosystem) -> &mut EcoRegistry {
        &mut self.ecos[idx(eco)]
    }

    fn eco(&self, eco: Ecosystem) -> &EcoRegistry {
        &self.ecos[idx(eco)]
    }

    pub fn fail_closed(&self, eco: Ecosystem) -> bool {
        self.eco(eco).fail_closed
    }

    /// The E2 "Private" column for a registry-sourced dependency. `name` is
    /// the `privacy::normalize` output. None ⇒ no private signal.
    pub fn private_reason(
        &self,
        eco: Ecosystem,
        name: &str,
        src: &DepSource,
    ) -> Option<(PrivateReason, Option<String>)> {
        let e = self.eco(eco);
        if eco == Ecosystem::Go
            && e.go_private_globs
                .iter()
                .any(|g| globs::go_glob_matches(g, name))
        {
            return Some((PrivateReason::GoPrivate, None));
        }
        if eco == Ecosystem::Nuget {
            if let Some(m) = globs::nuget_best_mapping(&e.nuget_mapping, name) {
                return (!m.public_only).then(|| (PrivateReason::SourceMapping, m.host.clone()));
            }
        }
        if e.replace_all {
            return Some((e.all_reason, e.default_host.clone()));
        }
        if eco == Ecosystem::Npm {
            if let Some(host) = npm_scope(name).and_then(|s| e.scoped.get(&s)) {
                return Some((PrivateReason::ScopedRegistry, host.clone()));
            }
        }
        let DepSource::Registry(reg) = src else {
            return None;
        };
        match reg {
            RegistryRef::Default => {
                (!e.default_public).then(|| (PrivateReason::CustomRegistry, e.default_host.clone()))
            }
            RegistryRef::Host(h) => (!is_public_host(eco, h))
                .then(|| (PrivateReason::CustomRegistry, Some(h.to_ascii_lowercase()))),
            RegistryRef::Named(n) => Some((
                PrivateReason::CustomRegistry,
                e.custom_hosts.get(n).cloned().flatten(),
            )),
            RegistryRef::Scope(s) => Some((
                PrivateReason::ScopedRegistry,
                e.scoped.get(&s.to_ascii_lowercase()).cloned().flatten(),
            )),
        }
    }
}

/// `[MEDIUM-1]` real URL parse (http/https only); returns the lowercase
/// host alone — userinfo, port, path, query and fragment are dropped. A
/// value that does not parse (or has no host) ⇒ None, and the caller fails
/// closed.
pub(crate) fn extract_host(raw: &str) -> Option<String> {
    let url = url::Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

/// `@scope/name` ⇒ `@scope` (lowercase).
fn npm_scope(name: &str) -> Option<String> {
    let (scope, _) = name.strip_prefix('@')?.split_once('/')?;
    Some(format!("@{}", scope.to_ascii_lowercase()))
}

#[cfg(test)]
#[path = "registry_ctx/load_tests.rs"]
mod load_tests;

#[cfg(test)]
#[path = "registry_ctx/npm_cfg_tests.rs"]
mod npm_cfg_tests;

#[cfg(test)]
#[path = "registry_ctx/cargo_go_load_tests.rs"]
mod cargo_go_load_tests;

#[cfg(test)]
#[path = "registry_ctx/go_load_tests.rs"]
mod go_load_tests;

#[cfg(test)]
#[path = "registry_ctx/pypi_load_tests.rs"]
mod pypi_load_tests;

#[cfg(test)]
#[path = "registry_ctx/nuget_load_tests.rs"]
mod nuget_load_tests;

#[cfg(test)]
#[path = "registry_ctx/nuget_feed_tests.rs"]
mod nuget_feed_tests;
