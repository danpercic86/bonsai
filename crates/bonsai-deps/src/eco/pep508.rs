//! PEP 508 requirement strings (E2 PyPI row), shared by `requirements*.txt`
//! and `pyproject.toml`. An iterative char scanner, no recursion `[HIGH-2]`.
//!
//! `name [extras] (spec | @ url) [; marker]`. Extras are dropped; the
//! environment marker is dropped too — the dependency is kept whatever the
//! marker says (the tracker reports every declared dependency, not the ones
//! for this interpreter).

use crate::adapter::{DepSource, RegistryRef};

/// One parsed requirement.
pub(crate) struct Req {
    pub(crate) name: String,
    /// The version specifier, whitespace removed (`>=1.0,<2`).
    pub(crate) spec: Option<String>,
    pub(crate) source: DepSource,
}

impl Req {
    /// E2: `==X` (no wildcard, one clause) ⇒ resolved.
    pub(crate) fn resolved(&self) -> Option<String> {
        self.spec.as_deref().and_then(exact_pin)
    }
}

/// `==X` alone (not `===`, no `*`, no second clause) ⇒ `X`.
pub(crate) fn exact_pin(spec: &str) -> Option<String> {
    let v = spec.trim().strip_prefix("==")?;
    let bad = v.is_empty() || v.starts_with('=') || v.contains(['*', ',', ';', ' ']);
    (!bad).then(|| v.to_string())
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
}

/// A direct-reference URL (`name @ url`, `-e url`, a bare URL line).
pub(crate) fn url_source(url: &str) -> DepSource {
    let u = url.trim().to_ascii_lowercase();
    if u.starts_with("file:") {
        DepSource::Path
    } else if ["git+", "hg+", "svn+", "bzr+"]
        .iter()
        .any(|p| u.starts_with(p))
    {
        DepSource::Git
    } else {
        DepSource::Url
    }
}

/// The version-spec part: up to the marker `;`, enclosing parens dropped,
/// whitespace removed. Empty ⇒ None.
fn spec_part(rest: &str) -> Option<String> {
    let s = rest.split(';').next().unwrap_or("").trim();
    let s = s
        .strip_prefix('(')
        .and_then(|x| x.strip_suffix(')'))
        .unwrap_or(s);
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (!s.is_empty()).then_some(s)
}

/// Parses one requirement. None ⇒ not a PEP 508 requirement (the caller
/// decides whether that is an error).
pub(crate) fn parse_req(raw: &str) -> Option<Req> {
    let s = raw.trim();
    let name_end = s.find(|c: char| !is_name_char(c)).unwrap_or(s.len());
    let name = &s[..name_end];
    let first_ok = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric());
    if !first_ok {
        return None;
    }
    let mut rest = s[name_end..].trim_start();
    if let Some(after) = rest.strip_prefix('[') {
        let close = after.find(']')?;
        rest = after[close + 1..].trim_start();
    }
    if let Some(url) = rest.strip_prefix('@') {
        // PEP 508: the URL ends at whitespace (a marker needs ` ;`).
        let url = url.split_whitespace().next()?;
        return Some(Req {
            name: name.to_string(),
            spec: None,
            source: url_source(url),
        });
    }
    let spec = spec_part(rest);
    // What remains must start like a version clause or a marker.
    let ok = rest.is_empty()
        || rest.starts_with([';', '(', '=', '<', '>', '!', '~'])
        || rest.starts_with(char::is_whitespace);
    ok.then(|| Req {
        name: name.to_string(),
        spec,
        source: DepSource::Registry(RegistryRef::Default),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_specs_extras_and_markers() {
        let r =
            parse_req("Requests[socks, security] >= 2.8.1, == 2.8.* ; python_version < \"2.7\"")
                .unwrap();
        assert_eq!(r.name, "Requests");
        assert_eq!(r.spec.as_deref(), Some(">=2.8.1,==2.8.*"));
        assert_eq!(r.resolved(), None);
        let r = parse_req("flask==3.0.2").unwrap();
        assert_eq!(r.resolved().as_deref(), Some("3.0.2"));
        let r = parse_req("name (==1.0)").unwrap();
        assert_eq!(r.resolved().as_deref(), Some("1.0"));
        assert_eq!(parse_req("x===1.0").unwrap().resolved(), None);
        assert_eq!(parse_req("x==1.*").unwrap().resolved(), None);
        assert!(parse_req("bare").unwrap().spec.is_none());
    }

    #[test]
    fn direct_references_are_never_registry() {
        let src = |s: &str| parse_req(s).unwrap().source;
        assert_eq!(src("pkg @ file:///tmp/pkg"), DepSource::Path);
        assert_eq!(src("pkg @ git+https://g.invalid/p.git@v1"), DepSource::Git);
        assert_eq!(
            src("pkg@https://h.invalid/p-1.0.whl ; os_name=='nt'"),
            DepSource::Url
        );
    }

    #[test]
    fn garbage_is_not_a_requirement() {
        for s in [
            "",
            "-e .",
            "./local",
            "/abs",
            "==1.0",
            "pkg[unclosed",
            "pkg $x",
        ] {
            assert!(parse_req(s).is_none(), "{s}");
        }
    }
}
