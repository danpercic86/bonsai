//! `RegistryContext` (contract §4.4): the per-ecosystem registry signals that
//! `privacy::classify` consults. P125a ships the TYPE and its no-derive
//! policy; the loader is a fail-closed stub until P125b.
//!
//! `[MEDIUM-1]` `RegistryContext` has NO Debug, Serialize, Deserialize or
//! Clone (and deliberately no Default: a default context would be a
//! fail-OPEN one). Its fields are hosts, scope names, globs and bools only.

mod globs;
mod host_env;

use std::collections::BTreeMap;
use std::path::Path;

pub use host_env::{HostEnv, UserConfig, UserRead};

use crate::adapter::{DepSource, RegistryRef};
use crate::catalog::is_public_host;
use crate::model::{Ecosystem, PrivateReason};

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
    /// Named registries (Cargo `[registries.<name>]`) → display host.
    pub(crate) custom_hosts: BTreeMap<String, Option<String>>,
    /// npm/yarn/pnpm scope (`@corp`, lowercase) → display host.
    pub(crate) scoped: BTreeMap<String, Option<String>>,
    /// Go `GOPRIVATE` / `GONOPROXY` / `GONOSUMDB` globs, one per entry.
    pub(crate) go_private_globs: Vec<String>,
    pub(crate) nuget_mapping: Vec<NugetMapping>,
}

impl EcoRegistry {
    fn public() -> Self {
        EcoRegistry {
            fail_closed: false,
            default_public: true,
            default_host: None,
            replace_all: false,
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
    /// STUB until P125b (ancestor walk, E3 inventory, key-filtered
    /// extractors). It fails CLOSED for every ecosystem, so nothing wired to
    /// it early can classify a dependency as Public.
    pub fn load(_workdir: &Path, _manifests: &[String], _env: &dyn HostEnv) -> RegistryContext {
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
            return Some((PrivateReason::CustomRegistry, e.default_host.clone()));
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

/// `@scope/name` ⇒ `@scope` (lowercase).
fn npm_scope(name: &str) -> Option<String> {
    let (scope, _) = name.strip_prefix('@')?.split_once('/')?;
    Some(format!("@{}", scope.to_ascii_lowercase()))
}
