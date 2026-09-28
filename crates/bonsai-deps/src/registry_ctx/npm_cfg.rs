//! `[MEDIUM-1]` key-filtered extractors for the npm / pnpm / yarn registry
//! configs (§4.4, E3). Only registry-routing keys are ever materialised:
//! every other line (`//host/:_authToken=…`, `_auth`, `always-auth`, …) is
//! skipped WITHOUT being stored, and the caller drops the raw text right
//! after. A value that does not yield a host, or a config that does not
//! parse, is [`Closed`] — the whole npm ecosystem then fails closed.

use std::collections::BTreeMap;

use crate::catalog::is_public_host;
use crate::model::Ecosystem;
use crate::yaml_events::{parse_first, YNode};

use super::{extract_host, EcoRegistry};

/// The npm ecosystem must fail closed. Deliberately carries nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Closed;

/// The npm-family signals gathered so far. It has NO fail-open flag: a
/// context is opened only by [`NpmSignals::into_open`], which the loader
/// calls after every step returned `Ok`.
#[derive(Default)]
pub(crate) struct NpmSignals {
    /// First non-public default registry seen (any level: a non-public
    /// default anywhere makes unscoped registry deps CustomRegistry).
    default_host: Option<String>,
    default_overridden: bool,
    scoped: BTreeMap<String, Option<String>>,
    named: BTreeMap<String, Option<String>>,
}

/// A registry URL's host, or `Closed`. `${VAR}` placeholders are never
/// resolved (npm, pnpm and berry all interpolate env vars in configs), so
/// they cannot be judged public.
fn host_of(raw: &str) -> Result<String, Closed> {
    let v = unquote(raw);
    if v.contains("${") {
        return Err(Closed);
    }
    extract_host(v).ok_or(Closed)
}

fn unquote(s: &str) -> &str {
    let s = s.trim();
    for q in ['"', '\''] {
        if let Some(inner) = s.strip_prefix(q).and_then(|x| x.strip_suffix(q)) {
            return inner.trim();
        }
    }
    s
}

/// `@scope` (lowercase, with the `@`) from a scope written with or without
/// it. An empty or placeholder scope is `Closed`.
fn norm_scope(raw: &str) -> Result<String, Closed> {
    let s = unquote(raw).trim_start_matches('@');
    if s.is_empty() || s.contains("${") || s.contains('/') {
        return Err(Closed);
    }
    Ok(format!("@{}", s.to_ascii_lowercase()))
}

impl NpmSignals {
    pub(crate) fn set_default(&mut self, url: &str) -> Result<(), Closed> {
        let h = host_of(url)?;
        if !is_public_host(Ecosystem::Npm, &h) {
            self.default_overridden = true;
            self.default_host.get_or_insert(h);
        }
        Ok(())
    }

    pub(crate) fn set_scope(&mut self, scope: &str, url: &str) -> Result<(), Closed> {
        let scope = norm_scope(scope)?;
        let h = host_of(url)?;
        if !is_public_host(Ecosystem::Npm, &h) {
            self.scoped.entry(scope).or_insert(Some(h));
        }
        Ok(())
    }

    fn set_named(&mut self, alias: &str, url: &str) -> Result<(), Closed> {
        let h = host_of(url)?;
        self.named
            .entry(unquote(alias).to_ascii_lowercase())
            .or_insert(Some(h));
        Ok(())
    }

    /// The ONLY way to an open (not fail-closed) npm registry context.
    pub(crate) fn into_open(self) -> EcoRegistry {
        let mut e = EcoRegistry::public();
        e.default_public = !self.default_overridden;
        e.default_host = self.default_host;
        e.scoped = self.scoped;
        e.custom_hosts = self.named;
        e
    }
}

/// `@scope` from a `@scope:registry` key (case-insensitive suffix).
fn scope_registry_key(key: &str) -> Option<&str> {
    let lower_ok = key.len() > ":registry".len()
        && key.is_char_boundary(key.len() - ":registry".len())
        && key[key.len() - ":registry".len()..].eq_ignore_ascii_case(":registry");
    let scope = lower_ok.then(|| &key[..key.len() - ":registry".len()])?;
    scope.starts_with('@').then_some(scope)
}

/// One `key` / `value` pair from an npmrc / yarnrc / env source.
fn apply_pair(sig: &mut NpmSignals, key: &str, value: &str) -> Result<(), Closed> {
    let key = unquote(key).trim_start_matches("--");
    if key.eq_ignore_ascii_case("registry") {
        return sig.set_default(value);
    }
    if let Some(scope) = scope_registry_key(key) {
        return sig.set_scope(scope, value);
    }
    // A placeholder in a registry-ish key: the scope cannot be known.
    let lower = key.to_ascii_lowercase();
    if lower.contains("${") && lower.contains("registry") {
        return Err(Closed);
    }
    Ok(())
}

/// One leading U+FEFF dropped (JS `trim()` strips it, so npm / yarn see the
/// first key without it).
fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{FEFF}').unwrap_or(text)
}

/// Lines split on `\r` AND `\n`, as npm's `ini` (`/[\r\n]+/`) does: a
/// CR-only line break must not hide a key.
fn config_lines(text: &str) -> impl Iterator<Item = &str> {
    strip_bom(text).split(['\r', '\n'])
}

/// `.npmrc` and pnpm `rc` (ini): raw-line scan.
pub(crate) fn npmrc(sig: &mut NpmSignals, text: &str) -> Result<(), Closed> {
    for line in config_lines(text) {
        let l = line.trim();
        if l.is_empty() || l.starts_with([';', '#', '[']) {
            continue;
        }
        let Some((k, v)) = l.split_once('=') else {
            continue;
        };
        apply_pair(sig, k, v)?;
    }
    Ok(())
}

/// yarn v1 `.yarnrc`: `key value` lines (`registry "https://…"`,
/// `"@corp:registry" "https://…"`, `--registry …`).
pub(crate) fn yarnrc_v1(sig: &mut NpmSignals, text: &str) -> Result<(), Closed> {
    for line in config_lines(text) {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let (k, v) = match l.strip_prefix('"') {
            // A quoted key may contain spaces.
            Some(rest) => match rest.split_once('"') {
                Some((k, v)) => (k, v),
                None => return Err(Closed),
            },
            None => l.split_once(char::is_whitespace).unwrap_or((l, "")),
        };
        apply_pair(sig, k, v)?;
    }
    Ok(())
}

fn scalar(n: &YNode) -> Result<&str, Closed> {
    n.as_str().ok_or(Closed)
}

/// yarn berry `.yarnrc.yml`: `npmRegistryServer` and
/// `npmScopes.<scope>.npmRegistryServer`.
pub(crate) fn yarnrc_yml(sig: &mut NpmSignals, text: &str) -> Result<(), Closed> {
    let keep = |p: &[String]| match p.len() {
        0 => true,
        1 => matches!(p[0].as_str(), "npmRegistryServer" | "npmScopes"),
        2 => p[0] == "npmScopes",
        3 => p[0] == "npmScopes" && p[2] == "npmRegistryServer",
        _ => false,
    };
    let doc = parse_first(strip_bom(text), &keep).map_err(|_| Closed)?;
    if let Some(v) = doc.get("npmRegistryServer") {
        sig.set_default(scalar(v)?)?;
    }
    if let Some(scopes) = doc.get("npmScopes") {
        if !matches!(scopes, YNode::Map(_)) {
            return Err(Closed);
        }
        for (scope, body) in scopes.entries() {
            if let Some(v) = body.get("npmRegistryServer") {
                sig.set_scope(scope, scalar(v)?)?;
            }
        }
    }
    Ok(())
}

/// pnpm ≥ 11 `pnpm-workspace.yaml`: `registry`, `registries` (`default` +
/// `@scope` keys) and `namedRegistries` (alias → URL).
pub(crate) fn pnpm_workspace(sig: &mut NpmSignals, text: &str) -> Result<(), Closed> {
    let keep = |p: &[String]| match p.len() {
        0 => true,
        1 => matches!(p[0].as_str(), "registry" | "registries" | "namedRegistries"),
        2 => matches!(p[0].as_str(), "registries" | "namedRegistries"),
        _ => false,
    };
    let doc = parse_first(strip_bom(text), &keep).map_err(|_| Closed)?;
    if let Some(v) = doc.get("registry") {
        sig.set_default(scalar(v)?)?;
    }
    if let Some(regs) = doc.get("registries") {
        if !matches!(regs, YNode::Map(_)) {
            return Err(Closed);
        }
        for (k, v) in regs.entries() {
            let url = scalar(v)?;
            match k.as_str() {
                "default" => sig.set_default(url)?,
                s if s.starts_with('@') => sig.set_scope(s, url)?,
                _ => return Err(Closed),
            }
        }
    }
    if let Some(named) = doc.get("namedRegistries") {
        if !matches!(named, YNode::Map(_)) {
            return Err(Closed);
        }
        for (alias, v) in named.entries() {
            sig.set_named(alias, scalar(v)?)?;
        }
    }
    Ok(())
}

/// `[NEW-2]` env key filters: they see only the variable NAME (full, as
/// set), so values of every other `npm_config_*` / `pnpm_config_*` /
/// `yarn_*` variable (auth tokens included) are never collected.
fn rest_after<'a>(key: &'a str, prefix: &str) -> Option<&'a str> {
    (key.len() >= prefix.len()
        && key.is_char_boundary(prefix.len())
        && key[..prefix.len()].eq_ignore_ascii_case(prefix))
    .then(|| &key[prefix.len()..])
}

pub(crate) fn keep_npm_env(key: &str) -> bool {
    ["npm_config_", "pnpm_config_"].iter().any(|p| {
        rest_after(key, p)
            .is_some_and(|r| r.eq_ignore_ascii_case("registry") || scope_registry_key(r).is_some())
    })
}

/// yarn berry maps `YARN_<SETTING>` onto settings; these are the routing
/// ones, plus `YARN_RC_FILENAME`, which renames the rc file.
pub(crate) const YARN_ENV_KEYS: &[&str] = &[
    "registry",
    "npm_registry_server",
    "npm_scopes",
    "rc_filename",
];

pub(crate) fn keep_yarn_env(key: &str) -> bool {
    rest_after(key, "yarn_")
        .is_some_and(|r| YARN_ENV_KEYS.iter().any(|k| r.eq_ignore_ascii_case(k)))
}

/// Applies one kept env var.
pub(crate) fn env_pair(sig: &mut NpmSignals, key: &str, value: &str) -> Result<(), Closed> {
    if let Some(r) = rest_after(key, "yarn_") {
        let r = r.to_ascii_lowercase();
        return match r.as_str() {
            "registry" | "npm_registry_server" => sig.set_default(value),
            // A scope map in an env var, or a renamed rc file: not readable
            // with confidence ⇒ fail closed.
            _ => Err(Closed),
        };
    }
    let rest = rest_after(key, "npm_config_")
        .or_else(|| rest_after(key, "pnpm_config_"))
        .ok_or(Closed)?;
    apply_pair(sig, rest, value)
}
