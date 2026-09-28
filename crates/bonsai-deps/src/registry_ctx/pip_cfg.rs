//! `[MEDIUM-1]` key-filtered extractors for PyPI registry configs (§4.4, E3
//! pip + uv / Poetry rows):
//! - pip `pip.conf` / `pip.ini`: a raw-line INI scan following pip 25.0.1
//!   `configuration.py` (`RawConfigParser`; keys lowercased, `_` → `-`, a
//!   leading `--` dropped; `=` or `:` delimits; `#` / `;` full-line
//!   comments; INDENTED CONTINUATION LINES extend the previous value). Only
//!   `index-url`, `extra-index-url` and `find-links` values are split and
//!   host-extracted; every other key (e.g. `client-cert`) is skipped
//!   without being stored.
//! - uv `uv.toml` and `pyproject.toml` `[tool.uv]` / `[tool.poetry]` /
//!   `[tool.pdm]` / `[tool.rye]`: typed DTOs with fields for index URLs /
//!   names / flags only.
//! - the `PIP_*` / `UV_*` index env vars.
//!
//! Judgement: private if ANY reachable definition is a non-public index
//! (never first-seen). A uv index that is not `explicit` is consulted for
//! every package (uv's default `first-index` strategy searches it before
//! the default), so it counts like `default = true` (a superset of E2);
//! an explicit uv index / Poetry source only supplies the display host of
//! the deps pinned to it by name.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::catalog::is_public_host;
use crate::eco::pypi::index_key;
use crate::eco::requirements::physical_lines;
use crate::model::Ecosystem;
use crate::toml_scan::check_depth;

use super::{extract_host, Closed, EcoRegistry};

/// The PyPI signals gathered so far; opened only by [`PypiSignals::into_open`].
#[derive(Default)]
pub(crate) struct PypiSignals {
    /// A non-explicit index that is not PyPI ⇒ every dep CustomRegistry.
    private: bool,
    /// Its (first) host, for display.
    host: Option<String>,
    /// Explicit named indexes (uv `explicit`, Poetry `explicit`) → hosts.
    named: BTreeMap<String, BTreeSet<String>>,
}

fn public(h: &str) -> bool {
    is_public_host(Ecosystem::Pypi, h)
}

impl PypiSignals {
    /// A non-explicit index URL; no host ⇒ `Closed` `[HIGH-1 item 2]`.
    pub(crate) fn index(&mut self, raw: &str) -> Result<(), Closed> {
        let h = extract_host(raw).ok_or(Closed)?;
        if !public(&h) {
            self.private = true;
            self.host.get_or_insert(h);
        }
        Ok(())
    }

    /// Every dep is private with no known host (`--no-index` in an include).
    pub(crate) fn private_unknown(&mut self) {
        self.private = true;
    }

    fn explicit(&mut self, name: &str, raw: &str) -> Result<(), Closed> {
        let h = extract_host(raw).ok_or(Closed)?;
        self.named.entry(index_key(name)).or_default().insert(h);
        Ok(())
    }

    /// The ONLY way to an open (not fail-closed) PyPI registry context.
    pub(crate) fn into_open(self) -> EcoRegistry {
        let mut e = EcoRegistry::public();
        if self.private {
            e.replace_all = true;
            e.default_public = false;
            e.default_host = self.host;
        }
        e.custom_hosts = self
            .named
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

/// pip `_normalize_name`.
fn pip_key(k: &str) -> String {
    let k = k.trim().to_ascii_lowercase().replace('_', "-");
    k.strip_prefix("--").map(str::to_string).unwrap_or(k)
}

/// The pip keys whose values are materialised.
fn pip_kept(k: &str) -> bool {
    matches!(k, "index-url" | "extra-index-url" | "find-links")
}

/// pip splits list options on whitespace; a comma is split too (a URL
/// fragment that then fails to parse only fails closed).
fn values(v: &str) -> impl Iterator<Item = &str> {
    v.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
}

/// One `pip.conf` / `pip.ini` (repo files never; user / site / env file).
pub(crate) fn pip_ini(sig: &mut PypiSignals, text: &str) -> Result<(), Closed> {
    let mut in_section = false;
    // `Some(kept)` while an option's value may continue.
    let mut cur: Option<bool> = None;
    for line in physical_lines(text) {
        let t = line.trim();
        if t.is_empty() || t.starts_with(['#', ';']) {
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            if let Some(kept) = cur {
                if kept {
                    for v in values(t) {
                        sig.index(v)?;
                    }
                }
                continue;
            }
        }
        if let Some(head) = t.strip_prefix('[') {
            if !head.ends_with(']') {
                return Err(Closed);
            }
            in_section = true;
            cur = None;
            continue;
        }
        // configparser: a key line outside a section is an error.
        if !in_section {
            return Err(Closed);
        }
        let cut = t.find(['=', ':']).ok_or(Closed)?;
        let key = pip_key(&t[..cut]);
        let kept = pip_kept(&key);
        if kept {
            for v in values(&t[cut + 1..]) {
                sig.index(v)?;
            }
        }
        cur = Some(kept);
    }
    Ok(())
}

/// uv's index settings (`uv.toml` top level, `[tool.uv]`, and their
/// `[pip]` sub-tables). No other key is deserialized.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct UvIndexDto {
    #[serde(default)]
    index_url: Option<String>,
    #[serde(default)]
    extra_index_url: Vec<String>,
    #[serde(default)]
    find_links: Vec<String>,
    #[serde(default)]
    index: Vec<UvIndexEntry>,
    #[serde(default)]
    pip: Option<UvPipDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct UvPipDto {
    #[serde(default)]
    index_url: Option<String>,
    #[serde(default)]
    extra_index_url: Vec<String>,
    #[serde(default)]
    find_links: Vec<String>,
}

#[derive(Deserialize)]
struct UvIndexEntry {
    #[serde(default)]
    name: Option<String>,
    url: String,
    #[serde(default)]
    explicit: Option<bool>,
}

#[derive(Deserialize)]
struct PyprojectDto {
    #[serde(default)]
    tool: Option<ToolDto>,
}

#[derive(Deserialize)]
struct ToolDto {
    #[serde(default)]
    uv: Option<UvIndexDto>,
    #[serde(default)]
    poetry: Option<PoetryDto>,
    /// `[P125c2 review]` `[[tool.pdm.source]]`.
    #[serde(default)]
    pdm: Option<PdmDto>,
    /// `[P125c2 review]` `[[tool.rye.sources]]`.
    #[serde(default)]
    rye: Option<RyeDto>,
}

#[derive(Deserialize)]
struct PdmDto {
    #[serde(default)]
    source: Vec<UrlSource>,
}

#[derive(Deserialize)]
struct RyeDto {
    #[serde(default)]
    sources: Vec<UrlSource>,
}

/// A PDM / Rye source: only its URL (required) is deserialized; the
/// `username` / `password` keys have no field.
#[derive(Deserialize)]
struct UrlSource {
    url: String,
}

#[derive(Deserialize)]
struct PoetryDto {
    #[serde(default)]
    source: Vec<PoetrySource>,
}

#[derive(Deserialize)]
struct PoetrySource {
    name: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    priority: Option<String>,
}

fn apply_uv(sig: &mut PypiSignals, uv: &UvIndexDto) -> Result<(), Closed> {
    let pip = uv.pip.as_ref();
    let urls = uv
        .index_url
        .iter()
        .chain(&uv.extra_index_url)
        .chain(&uv.find_links)
        .chain(pip.into_iter().flat_map(|p| {
            p.index_url
                .iter()
                .chain(&p.extra_index_url)
                .chain(&p.find_links)
        }));
    for u in urls {
        sig.index(u)?;
    }
    for i in &uv.index {
        // A non-explicit index serves every package; an unnamed explicit
        // one cannot be pinned to, so it is judged the same way.
        if i.explicit != Some(true) || i.name.is_none() {
            sig.index(&i.url)?;
        }
        // Any named index can be pinned to (`{ index = "name" }`).
        if let Some(n) = &i.name {
            sig.explicit(n, &i.url)?;
        }
    }
    Ok(())
}

fn toml_dto<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, Closed> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    check_depth(text).map_err(|_| Closed)?;
    // `[MEDIUM-1]` the error is dropped unread (its Display quotes input).
    toml::from_str(text).map_err(|_| Closed)
}

/// One `uv.toml` (repo walk or user / system).
pub(crate) fn uv_toml(sig: &mut PypiSignals, text: &str) -> Result<(), Closed> {
    let dto: UvIndexDto = toml_dto(text)?;
    apply_uv(sig, &dto)
}

/// The index definitions of one `pyproject.toml` (repo walk).
pub(crate) fn pyproject(sig: &mut PypiSignals, text: &str) -> Result<(), Closed> {
    let dto: PyprojectDto = toml_dto(text)?;
    let Some(tool) = dto.tool else {
        return Ok(());
    };
    if let Some(uv) = &tool.uv {
        apply_uv(sig, uv)?;
    }
    // PDM / Rye (ASSUMED from their docs): every source (index or
    // find-links) may serve any package ⇒ a non-PyPI URL is global; a
    // missing URL fails the parse, a hostless one (`${VAR}`, a path)
    // fails closed (Q2: mirrors ⇒ private).
    let pdm = tool.pdm.iter().flat_map(|p| &p.source);
    let rye = tool.rye.iter().flat_map(|r| &r.sources);
    for s in pdm.chain(rye) {
        sig.index(&s.url)?;
    }
    for s in tool.poetry.iter().flat_map(|p| &p.source) {
        let Some(url) = &s.url else {
            // Poetry allows `name = "PyPI"` with no URL; anything else
            // without a URL is unresolvable.
            if index_key(&s.name) == "pypi" {
                continue;
            }
            return Err(Closed);
        };
        let explicit = s
            .priority
            .as_deref()
            .is_some_and(|p| p.eq_ignore_ascii_case("explicit"));
        if !explicit {
            // primary / default / supplemental / legacy flags: all packages.
            sig.index(url)?;
        }
        sig.explicit(&s.name, url)?;
    }
    Ok(())
}

const PIP_ENV: &[&str] = &["PIP_INDEX_URL", "PIP_EXTRA_INDEX_URL", "PIP_FIND_LINKS"];
const UV_ENV: &[&str] = &[
    "UV_INDEX_URL",
    "UV_EXTRA_INDEX_URL",
    "UV_INDEX",
    "UV_DEFAULT_INDEX",
    "UV_FIND_LINKS",
];
/// Redirects uv to a config file we cannot locate ⇒ `Closed`.
const UV_CONFIG_FILE: &str = "UV_CONFIG_FILE";

/// `[NEW-2]` the env key filter: sees only the NAME.
pub(crate) fn keep_pypi_env(key: &str) -> bool {
    PIP_ENV
        .iter()
        .chain(UV_ENV)
        .chain(&[UV_CONFIG_FILE])
        .any(|k| k.eq_ignore_ascii_case(key))
}

/// Applies one kept env var. `UV_INDEX` / `UV_DEFAULT_INDEX` entries may be
/// `name=url`.
pub(crate) fn pypi_env_pair(sig: &mut PypiSignals, key: &str, value: &str) -> Result<(), Closed> {
    if key.eq_ignore_ascii_case(UV_CONFIG_FILE) {
        return Err(Closed);
    }
    for v in values(value) {
        let url = match v.split_once('=') {
            Some((name, url)) if !name.contains([':', '/']) => url,
            _ => v,
        };
        sig.index(url)?;
    }
    Ok(())
}
