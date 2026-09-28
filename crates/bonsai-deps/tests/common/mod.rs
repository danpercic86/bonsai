//! Shared sentinel harness for `privacy_sentinel.rs` and `token_sentinel.rs`
//! (E4 rows 6 / 6′).
//!
//! P125b/c1 have no transport, so [`local_check`] runs the pipeline that
//! exists (discover → `RegistryContext::load` → adapters → `classify`) and
//! [`outbound_from_public`] renders what a transport WOULD send for the
//! `Public` set (a deps.dev GET per package + one OSV querybatch body).
//! P125d replaces `outbound_from_public` with the recorded requests of a
//! FakeTransport under `run_check`; [`assert_no_sentinel`] stays as is.
#![allow(dead_code)]

pub mod fixture;

use std::path::Path;

use bonsai_deps::adapter::{adapter_for_manifest, local_names, ParseError, ParseInput, ADAPTERS};
use bonsai_deps::discover::{discover_full, workdir_canon};
use bonsai_deps::fsread::{safe_read, ReadOutcome, MAX_FILE_BYTES};
use bonsai_deps::model::Ecosystem;
use bonsai_deps::privacy::{classify, PublicPackage, Visibility};
use bonsai_deps::registry_ctx::{HostEnv, RegistryContext};

/// One request as it would leave the process.
pub struct Request {
    pub url: String,
    pub body: Option<String>,
}

/// Every classified dependency / lock entry, plus the parse errors.
pub struct LocalCheck {
    pub rows: Vec<(String, Visibility)>,
    pub errors: Vec<ParseError>,
}

impl LocalCheck {
    pub fn public(&self) -> Vec<PublicPackage> {
        self.rows
            .iter()
            .filter_map(|(_, v)| match v {
                Visibility::Public(p) => Some(p.clone()),
                _ => None,
            })
            .collect()
    }

    /// Every visibility recorded for `name` (direct rows and lock entries).
    pub fn visibility(&self, name: &str) -> Vec<&Visibility> {
        self.rows
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v)
            .collect()
    }
}

pub fn local_check(workdir: &Path, env: &dyn HostEnv) -> LocalCheck {
    let repo = git2::Repository::open(workdir).expect("fixture repo opens");
    let files = discover_full(&repo).expect("discovery runs");
    let wd = workdir_canon(&repo).expect("workdir");
    let ctx = RegistryContext::load(&wd, files.manifests(), env);
    let local = local_names(&wd, &files);
    let mut out = LocalCheck {
        rows: Vec::new(),
        errors: Vec::new(),
    };
    assert_eq!(ADAPTERS.len(), 3, "npm family, Cargo and Go (P125c1)");
    for m in files.manifests() {
        let name = m.rsplit('/').next().unwrap_or(m);
        let Some(adapter) = adapter_for_manifest(name) else {
            continue;
        };
        let ReadOutcome::Content(manifest) = safe_read(&wd, m, MAX_FILE_BYTES) else {
            continue;
        };
        let lock_path = adapter.lockfile_for(m, &files);
        let lock_text =
            lock_path
                .as_deref()
                .and_then(|p| match safe_read(&wd, p, MAX_FILE_BYTES) {
                    ReadOutcome::Content(t) => Some(t),
                    _ => None,
                });
        let lockfile = match (&lock_path, &lock_text) {
            (Some(p), Some(t)) => Some((p.as_str(), t.as_str())),
            _ => None,
        };
        let parsed = adapter.parse(&ParseInput {
            manifest_path: m,
            manifest: &manifest,
            lockfile,
            files: &files,
        });
        let group = match parsed {
            Ok(g) => g,
            Err(e) => {
                out.errors.push(e);
                continue;
            }
        };
        let eco = adapter.ecosystem();
        for d in &group.direct {
            let version = d.resolved.as_deref().or(d.declared.as_deref());
            let v = classify(eco, &d.name, version, &d.source, &ctx, &local);
            out.rows.push((d.name.clone(), v));
        }
        for e in &group.lock_entries {
            let v = classify(eco, &e.name, Some(&e.version), &e.source, &ctx, &local);
            out.rows.push((e.name.clone(), v));
        }
    }
    out
}

fn pct_encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// `(deps.dev system, OSV ecosystem)` names.
fn systems(eco: Ecosystem) -> (&'static str, &'static str) {
    match eco {
        Ecosystem::Npm => ("npm", "npm"),
        Ecosystem::Cargo => ("cargo", "crates.io"),
        Ecosystem::Pypi => ("pypi", "PyPI"),
        Ecosystem::Go => ("go", "Go"),
        Ecosystem::Nuget => ("nuget", "NuGet"),
    }
}

/// What a transport would send for `public` (deps.dev + OSV shapes, §0.1).
pub fn outbound_from_public(public: &[PublicPackage]) -> Vec<Request> {
    let mut reqs: Vec<Request> = public
        .iter()
        .map(|p| Request {
            url: format!(
                "https://api.deps.dev/v3/systems/{}/packages/{}",
                systems(p.ecosystem()).0,
                pct_encode(p.name())
            ),
            body: None,
        })
        .collect();
    let queries: Vec<serde_json::Value> = public
        .iter()
        .map(|p| {
            serde_json::json!({
                "package": {"name": p.name(), "ecosystem": systems(p.ecosystem()).1},
                "version": p.version(),
            })
        })
        .collect();
    reqs.push(Request {
        url: "https://api.osv.dev/v1/querybatch".into(),
        body: Some(serde_json::json!({ "queries": queries }).to_string()),
    });
    reqs
}

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Every string in a JSON document (keys and values), iteratively.
fn json_strings(v: &serde_json::Value, out: &mut Vec<String>) {
    let mut stack = vec![v];
    while let Some(v) = stack.pop() {
        match v {
            serde_json::Value::String(s) => out.push(s.clone()),
            serde_json::Value::Array(a) => stack.extend(a),
            serde_json::Value::Object(o) => {
                for (k, v) in o {
                    out.push(k.clone());
                    stack.push(v);
                }
            }
            _ => {}
        }
    }
}

/// `[NEW-1]` the haystacks a sentinel is matched against: each raw URL and
/// body, the percent-decoded URL, and every JSON-decoded body string.
pub fn haystacks(reqs: &[Request]) -> Vec<String> {
    let mut hay = Vec::new();
    for r in reqs {
        hay.push(r.url.clone());
        hay.push(pct_decode(&r.url));
        if let Some(b) = &r.body {
            hay.push(b.clone());
            match serde_json::from_str::<serde_json::Value>(b) {
                Ok(v) => json_strings(&v, &mut hay),
                Err(_) => panic!("outbound body is not JSON"),
            }
        }
    }
    hay.into_iter().map(|h| h.to_lowercase()).collect()
}

/// The first haystack holding a sentinel (or its bare marker),
/// case-insensitively.
pub fn find_sentinel(reqs: &[Request], needles: &[&str]) -> Option<String> {
    let hay = haystacks(reqs);
    needles.iter().find_map(|n| {
        let n = n.to_lowercase();
        hay.iter().find(|h| h.contains(&n)).cloned()
    })
}

/// Fails if any sentinel (or its bare marker) appears.
pub fn assert_no_sentinel(reqs: &[Request], needles: &[&str]) {
    if let Some(h) = find_sentinel(reqs, needles) {
        panic!("a sentinel reached the outbound set: {h}");
    }
}
