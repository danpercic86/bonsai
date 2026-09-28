//! The Go half of `RegistryContext::load` (§4.4, E3 Go row). Each key is
//! read from the environment AND the `GOENV` file (`go env -w`); Go lets
//! the environment override the file, the union is taken instead — a
//! superset that can only add private signals. Only the keys below are
//! ever read (key-filtered; `GOAUTH`, `GOFLAGS`, … are never touched).
//!
//! - `GOPRIVATE` / `GONOPROXY` / `GONOSUMDB` / `GOINSECURE`: globs ⇒
//!   `GoPrivate` for matching modules (an insecurely fetched module is
//!   treated as private).
//! - `GOPROXY` (user ruling Q2: mirrors ⇒ private): any entry other than
//!   `https://proxy.golang.org`, `direct` or `off` ⇒ the default registry
//!   is not public; `default_host` is the first such proxy's host. An entry
//!   without a host (`file://…`) fails closed.
//! - `GOSUMDB`: `off`, or anything but `sum.golang.org`, fails closed —
//!   module names then leave no glob trail to judge them by.
//!
//! ASSUMED (no Go toolchain or docs available locally): the env file is
//! `KEY=VALUE` lines, values unquoted.

use super::host_env::{HostEnv, UserConfig, UserRead};
use super::{extract_host, Closed, EcoRegistry};

/// The glob-list variables.
pub(crate) const GO_GLOB_KEYS: &[&str] = &["GOPRIVATE", "GONOPROXY", "GONOSUMDB", "GOINSECURE"];
const GOPROXY: &str = "GOPROXY";
const GOSUMDB: &str = "GOSUMDB";

/// `GOPROXY` entries that keep modules on the public path.
const PUBLIC_PROXIES: &[&str] = &["https://proxy.golang.org", "direct", "off"];
const PUBLIC_SUMDB: &str = "sum.golang.org";

fn unquote(v: &str) -> &str {
    let v = v.trim();
    v.strip_prefix('"')
        .and_then(|l| l.strip_suffix('"'))
        .unwrap_or(v)
}

fn push_globs(out: &mut Vec<String>, list: &str) {
    out.extend(
        unquote(list)
            .split(',')
            .map(str::trim)
            .filter(|g| !g.is_empty())
            .map(str::to_string),
    );
}

/// `key`'s values in a go env file (every matching line: a superset).
fn file_values<'a>(text: &'a str, key: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    text.split(['\r', '\n']).filter_map(move |line| {
        let (k, v) = line.split_once('=')?;
        (k.trim() == key).then_some(v)
    })
}

/// Every value of `key` (environment first, then the file).
fn values(env: &dyn HostEnv, file: Option<&str>, key: &str) -> Vec<String> {
    let mut out: Vec<String> = env.var(key).into_iter().collect();
    if let Some(text) = file {
        out.extend(file_values(text, key).map(str::to_string));
    }
    out
}

/// `GOPROXY`: the first non-public proxy's host, `Ok(None)` when every
/// entry is public. Entries are separated by `,` or `|`.
fn proxy_host(list: &str) -> Result<Option<String>, Closed> {
    for entry in unquote(list).split([',', '|']).map(str::trim) {
        let norm = entry.trim_end_matches('/');
        if norm.is_empty() || PUBLIC_PROXIES.iter().any(|p| p.eq_ignore_ascii_case(norm)) {
            continue;
        }
        return extract_host(entry).map(Some).ok_or(Closed);
    }
    Ok(None)
}

/// `GOSUMDB` = `name[+key] [url]`: only the default database passes.
fn check_sumdb(v: &str) -> Result<(), Closed> {
    let mut parts = unquote(v).split_whitespace();
    let Some(first) = parts.next() else {
        return Ok(()); // empty ⇒ Go's default
    };
    let name = first.split('+').next().unwrap_or(first);
    let url_ok = parts
        .next()
        .is_none_or(|u| extract_host(u).is_some_and(|h| h == PUBLIC_SUMDB));
    if name.eq_ignore_ascii_case(PUBLIC_SUMDB) && url_ok && parts.next().is_none() {
        Ok(())
    } else {
        Err(Closed)
    }
}

/// The Go signals, or `Closed`.
pub(super) fn load(env: &dyn HostEnv) -> Result<EcoRegistry, Closed> {
    let file = match env.read_user_file(UserConfig::GoEnv) {
        UserRead::Absent => None,
        UserRead::Content(text) => Some(text),
        UserRead::Unreadable => return Err(Closed),
    };
    let file = file.as_deref();
    let mut e = EcoRegistry::public();
    for key in GO_GLOB_KEYS {
        for v in values(env, file, key) {
            push_globs(&mut e.go_private_globs, &v);
        }
    }
    for v in values(env, file, GOSUMDB) {
        check_sumdb(&v)?;
    }
    for v in values(env, file, GOPROXY) {
        if let Some(host) = proxy_host(&v)? {
            e.default_public = false;
            e.default_host.get_or_insert(host);
        }
    }
    Ok(e)
}
