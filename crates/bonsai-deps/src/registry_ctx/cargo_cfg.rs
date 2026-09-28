//! `[MEDIUM-1]` key-filtered extractor for Cargo registry configs (§4.4,
//! E3 Cargo row): `.cargo/config{,.toml}` (repo walk + Q7 above-workdir
//! walk), `$CARGO_HOME/config{,.toml}` and the `CARGO_*` env vars.
//!
//! Only `source.*.replace-with`, `source.*.registry` and
//! `registries.*.index` are materialised (plus `registry.default`, and the
//! PRESENCE of `include` / `patch` / `paths`). The typed DTO below has no
//! field for `[registry] token`, `registries.*.token` or any credential
//! provider, so those values never reach a type of ours; the raw text is
//! dropped by the caller right after. Every step is `Result<_, Closed>`.

use std::collections::{BTreeMap, BTreeSet};

use serde::de::IgnoredAny;
use serde::Deserialize;

use crate::catalog::is_public_host;
use crate::eco::cargo::{cargo_url_host, registry_key};
use crate::model::Ecosystem;
use crate::toml_scan::check_depth;

use super::{Closed, EcoRegistry};

/// The source every crates.io dependency comes from.
const CRATES_IO: &str = "crates-io";

/// `[MEDIUM-1]` no Debug: only the keys below are ever deserialized.
#[derive(Deserialize)]
struct ConfigDto {
    #[serde(default)]
    source: BTreeMap<String, SourceDto>,
    #[serde(default)]
    registries: BTreeMap<String, RegistryDto>,
    /// Only `default`; `token` / `credential-provider` have no field.
    #[serde(default)]
    registry: Option<RegistryTableDto>,
    /// Presence only: each hides routing we do not follow ⇒ `Closed`.
    #[serde(default)]
    include: Option<IgnoredAny>,
    #[serde(default)]
    patch: Option<IgnoredAny>,
    #[serde(default)]
    paths: Option<IgnoredAny>,
}

#[derive(Deserialize)]
struct SourceDto {
    #[serde(default, rename = "replace-with")]
    replace_with: Option<String>,
    #[serde(default)]
    registry: Option<String>,
    /// Presence only: a source with no URL host (vendored / local / git).
    #[serde(default)]
    directory: Option<IgnoredAny>,
    #[serde(default, rename = "local-registry")]
    local_registry: Option<IgnoredAny>,
    #[serde(default)]
    git: Option<IgnoredAny>,
}

#[derive(Deserialize)]
struct RegistryDto {
    #[serde(default)]
    index: Option<String>,
}

#[derive(Deserialize)]
struct RegistryTableDto {
    #[serde(default)]
    default: Option<String>,
}

/// One definition of a source / registry at some level.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Def {
    Host(String),
    /// `directory` / `local-registry` / `git`: never the public index.
    Hostless,
}

/// The Cargo signals gathered so far. No fail-open flag: the context is
/// opened only by [`CargoSignals::into_open`] after every step was `Ok`.
///
/// Review round 3: EVERY definition from every level (repo walk, above
/// the workdir, `$CARGO_HOME`, env) is kept. Cargo's "nearest wins" cannot
/// be reproduced exactly (config is per invocation directory), so the
/// judgement is: private if ANY reachable definition is non-public.
#[derive(Default)]
pub(crate) struct CargoSignals {
    /// Every `source.<name>.replace-with` edge (normalised names).
    source_replace: BTreeMap<String, BTreeSet<String>>,
    /// Every `source.<name>` definition.
    source_defs: BTreeMap<String, BTreeSet<Def>>,
    /// Every `registries.<name>.index` host.
    registries: BTreeMap<String, BTreeSet<String>>,
}

/// An index / registry URL's host (`sparse+https://…` or plain http(s));
/// ssh, file or a typo ⇒ `Closed`.
fn cargo_host(raw: &str) -> Result<String, Closed> {
    cargo_url_host(raw).ok_or(Closed)
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    (s.len() >= prefix.len()
        && s.is_char_boundary(prefix.len())
        && s[..prefix.len()].eq_ignore_ascii_case(prefix))
    .then(|| &s[prefix.len()..])
}

fn public(h: &str) -> bool {
    is_public_host(Ecosystem::Cargo, h)
}

impl CargoSignals {
    fn replace(&mut self, source: &str, target: &str) {
        self.source_replace
            .entry(registry_key(source))
            .or_default()
            .insert(registry_key(target));
    }

    fn source_def(&mut self, source: &str, def: Def) {
        self.source_defs
            .entry(registry_key(source))
            .or_default()
            .insert(def);
    }

    fn registry_index(&mut self, name: &str, url: &str) -> Result<(), Closed> {
        let host = cargo_host(url)?;
        self.registries
            .entry(registry_key(name))
            .or_default()
            .insert(host);
        Ok(())
    }

    /// Whether crates.io is routed anywhere non-public, and the first
    /// non-public host seen. Walks every `replace-with` edge reachable from
    /// `crates-io` (iterative, visited set); a reachable name with no
    /// definition at all, or a hostless one, counts as private.
    fn crates_io_route(&self) -> (bool, Option<String>) {
        let mut private = false;
        let mut host = None;
        let mut note = |h: &str| {
            if !public(h) {
                private = true;
                if host.is_none() {
                    host = Some(h.to_string());
                }
            }
        };
        // crates-io's own redefinition (`[source.crates-io] registry`).
        let own = self.source_defs.get(CRATES_IO);
        let mut stack: Vec<&str> = self
            .source_replace
            .get(CRATES_IO)
            .map(|t| t.iter().map(String::as_str).collect())
            .unwrap_or_default();
        let mut hostless = false;
        for d in own.into_iter().flatten() {
            match d {
                Def::Host(h) => note(h),
                Def::Hostless => hostless = true,
            }
        }
        let replaced = !stack.is_empty();
        let mut resolved_any = false;
        let mut seen = BTreeSet::new();
        while let Some(name) = stack.pop() {
            if !seen.insert(name) {
                continue;
            }
            let defs = self.source_defs.get(name);
            let regs = self.registries.get(name);
            let edges = self.source_replace.get(name);
            if defs.is_none() && regs.is_none() && edges.is_none() {
                // Unresolvable target (or crates-io itself in a cycle).
                hostless = true;
            }
            resolved_any |= defs.is_some() || regs.is_some();
            for d in defs.into_iter().flatten() {
                match d {
                    Def::Host(h) => note(h),
                    Def::Hostless => hostless = true,
                }
            }
            for h in regs.into_iter().flatten() {
                note(h);
            }
            stack.extend(edges.into_iter().flatten().map(String::as_str));
        }
        // A chain (or cycle) that never reaches a definition.
        if replaced && !resolved_any {
            hostless = true;
        }
        (private || hostless, host)
    }

    /// The ONLY way to an open (not fail-closed) Cargo registry context.
    pub(crate) fn into_open(self) -> EcoRegistry {
        let mut e = EcoRegistry::public();
        // A replacement that only ever resolves to crates.io's own public
        // index (`sparse+https://index.crates.io/`) routes nothing away.
        let (private, host) = self.crates_io_route();
        if private {
            // E2: `[source.crates-io] replace-with` ⇒ ALL CustomRegistry.
            e.replace_all = true;
            e.default_public = false;
            e.default_host = host;
        }
        // Named registries are always CustomRegistry; the host is display
        // only (the first non-public one, else the first).
        e.custom_hosts = self
            .registries
            .into_iter()
            .map(|(k, hosts)| {
                let h = hosts
                    .iter()
                    .find(|h| !public(h))
                    .or_else(|| hosts.iter().next())
                    .cloned();
                (k, h)
            })
            .collect();
        e
    }
}

/// `registry.default` / `CARGO_REGISTRY_DEFAULT`. Cargo documents it as the
/// registry for publish-type commands only, but anything other than
/// crates.io is treated as a routing signal we cannot judge ⇒ `Closed`.
fn check_default(name: &str) -> Result<(), Closed> {
    if registry_key(name) == CRATES_IO {
        Ok(())
    } else {
        Err(Closed)
    }
}

/// One `.cargo/config{,.toml}` / `$CARGO_HOME/config{,.toml}` file.
pub(crate) fn cargo_config(sig: &mut CargoSignals, text: &str) -> Result<(), Closed> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    check_depth(text).map_err(|_| Closed)?;
    // `[MEDIUM-1]` the error is dropped unread (its Display quotes input).
    let dto: ConfigDto = toml::from_str(text).map_err(|_| Closed)?;
    if dto.include.is_some() || dto.patch.is_some() || dto.paths.is_some() {
        return Err(Closed);
    }
    if let Some(name) = dto.registry.and_then(|r| r.default) {
        check_default(&name)?;
    }
    for (name, src) in &dto.source {
        if let Some(target) = &src.replace_with {
            sig.replace(name, target);
        }
        if let Some(url) = &src.registry {
            sig.source_def(name, Def::Host(cargo_host(url)?));
        }
        if src.directory.is_some() || src.local_registry.is_some() || src.git.is_some() {
            sig.source_def(name, Def::Hostless);
        }
    }
    for (name, reg) in &dto.registries {
        if let Some(url) = &reg.index {
            sig.registry_index(name, url)?;
        }
    }
    Ok(())
}

const ENV_REPLACE: &str = "CARGO_SOURCE_CRATES_IO_REPLACE_WITH";
const ENV_DEFAULT: &str = "CARGO_REGISTRY_DEFAULT";
const ENV_REGISTRIES: &str = "CARGO_REGISTRIES_";
const ENV_INDEX: &str = "_INDEX";

/// The `<NAME>` of `CARGO_REGISTRIES_<NAME>_INDEX` (case-insensitive).
fn registries_index_name(key: &str) -> Option<&str> {
    let rest = strip_prefix_ci(key, ENV_REGISTRIES)?;
    let cut = rest.len().checked_sub(ENV_INDEX.len())?;
    (cut > 0 && rest.is_char_boundary(cut) && rest[cut..].eq_ignore_ascii_case(ENV_INDEX))
        .then(|| &rest[..cut])
}

/// `[NEW-2]` the env key filter: sees only the NAME, so
/// `CARGO_REGISTRY_TOKEN`, `CARGO_REGISTRIES_*_TOKEN` /
/// `_CREDENTIAL_PROVIDER` values are never collected.
pub(crate) fn keep_cargo_env(key: &str) -> bool {
    key.eq_ignore_ascii_case(ENV_REPLACE)
        || key.eq_ignore_ascii_case(ENV_DEFAULT)
        || registries_index_name(key).is_some()
}

/// Applies one kept env var.
pub(crate) fn cargo_env_pair(sig: &mut CargoSignals, key: &str, value: &str) -> Result<(), Closed> {
    if key.eq_ignore_ascii_case(ENV_REPLACE) {
        // Not honoured by cargo per its config reference ("Environment: not
        // supported"); kept because it can only ADD a private signal.
        sig.replace(CRATES_IO, value);
        return Ok(());
    }
    if key.eq_ignore_ascii_case(ENV_DEFAULT) {
        return check_default(value);
    }
    match registries_index_name(key) {
        Some(name) => sig.registry_index(name, value),
        None => Err(Closed),
    }
}
