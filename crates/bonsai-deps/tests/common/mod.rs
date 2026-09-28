//! Shared sentinel harness for `privacy_sentinel.rs` and `token_sentinel.rs`
//! (E4 rows 6 / 6′).
//!
//! [`local_check`] runs discover → `RegistryContext::load` → adapters →
//! `classify` and exposes every visibility (incl. private lock entries,
//! which never become rows) for the row assertions. The OUTBOUND assertions
//! use [`recorded_outbound`] (P125d): the requests a recording
//! `FakeTransport` saw during a full `run_check`.
#![allow(dead_code)]

pub mod corpus;
pub mod fixture;
pub mod fixture_pn;

use std::path::Path;

use bonsai_deps::adapter::{adapter_for_manifest, local_names, ParseError, ParseInput, ADAPTERS};
use bonsai_deps::discover::{discover_full, workdir_canon};
use bonsai_deps::fsread::{safe_read, ReadOutcome, MAX_FILE_BYTES};
use bonsai_deps::model::CachedReport;
use bonsai_deps::privacy::{classify, PublicPackage, Visibility};
use bonsai_deps::registry_ctx::{HostEnv, RegistryContext};
use bonsai_deps::remote::FakeTransport;
use bonsai_deps::{run_check, CancelToken, CheckCtx, CheckError};

/// One request as it would leave the process.
pub struct Request {
    pub url: String,
    pub body: Option<String>,
}

/// Every classified dependency / lock entry, plus the parse errors, plus
/// the REAL outbound set and report of a full `run_check` over the same
/// repo and env (P125d).
pub struct LocalCheck {
    pub rows: Vec<(String, Visibility)>,
    pub errors: Vec<ParseError>,
    pub outbound: Vec<Request>,
    pub report: CachedReport,
}

impl LocalCheck {
    /// Positive control: `needle` appears in the recorded outbound set.
    pub fn sent(&self, needle: &str) -> bool {
        find_sentinel(&self.outbound, &[needle]).is_some()
    }

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
    let full = recorded_check(workdir, env);
    let report = match full.report {
        Ok(r) => r,
        Err(e) => panic!("run_check failed: {e:?}"),
    };
    let mut out = LocalCheck {
        rows: Vec::new(),
        errors: Vec::new(),
        outbound: full.requests,
        report,
    };
    assert_eq!(
        ADAPTERS.len(),
        5,
        "npm family, Cargo, Go, PyPI and NuGet (P125c2)"
    );
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

/// The result of a full [`run_check`] over a recording `FakeTransport`.
pub struct Checked {
    pub requests: Vec<Request>,
    pub report: Result<CachedReport, CheckError>,
}

impl Checked {
    /// Whether any recorded URL or body mentions `needle` (decoded,
    /// case-insensitive) — the positive controls of the sentinels.
    pub fn sent(&self, needle: &str) -> bool {
        find_sentinel(&self.requests, &[needle]).is_some()
    }
}

/// `[P125d]` the REAL outbound set: `run_check` over a recording
/// `FakeTransport` (empty defaults: deps.dev 404, no advisories), so every
/// sentinel asserts on the exact URLs and bodies the pipeline sends.
pub fn recorded_check(workdir: &Path, env: &dyn HostEnv) -> Checked {
    recorded_check_with(workdir, env, &FakeTransport::new())
}

pub fn recorded_check_with(workdir: &Path, env: &dyn HostEnv, t: &FakeTransport) -> Checked {
    let cancel = CancelToken::new();
    let ctx = CheckCtx {
        transport: t,
        env,
        cancel: &cancel,
        progress: &|_, _| {},
        prev: None,
        now_ms: 1,
    };
    let report = run_check(workdir, "repo-under-test", &ctx);
    let requests = t
        .requests()
        .into_iter()
        .map(|r| Request {
            url: r.url,
            body: r.body,
        })
        .collect();
    Checked { requests, report }
}

/// The outbound set of a full check. Panics if the check itself failed.
pub fn recorded_outbound(workdir: &Path, env: &dyn HostEnv) -> Vec<Request> {
    let c = recorded_check(workdir, env);
    if let Err(e) = &c.report {
        panic!("run_check failed: {e:?}");
    }
    c.requests
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
    assert_no_sentinel_except(reqs, needles, &[]);
}

/// [`assert_no_sentinel`], after deleting each `allowed` name (a control
/// package that is deliberately public in this fixture) from every decoded
/// haystack.
pub fn assert_no_sentinel_except(reqs: &[Request], needles: &[&str], allowed: &[&str]) {
    let hay: Vec<String> = haystacks(reqs)
        .into_iter()
        .map(|h| {
            allowed
                .iter()
                .fold(h, |h, a| h.replace(&a.to_lowercase(), ""))
        })
        .collect();
    for n in needles {
        let n = n.to_lowercase();
        if let Some(h) = hay.iter().find(|h| h.contains(&n)) {
            panic!("a sentinel reached the outbound set: {h}");
        }
    }
}

/// `[MEDIUM-1]` the persisted and serialized halves of E4 row 6′.
pub fn assert_report_clean(report: &CachedReport, token: &str) {
    let dir = tempfile::tempdir().expect("tempdir");
    bonsai_deps::cache::save(dir.path(), "0123abcd", report).expect("save");
    let path = bonsai_deps::cache::cache_path(dir.path(), "0123abcd").expect("path");
    let bytes = std::fs::read(path).expect("read cache");
    let text = String::from_utf8_lossy(&bytes).to_lowercase();
    assert!(!text.contains(token), "token in the cache file");
    assert!(
        bonsai_deps::cache::load(dir.path(), "0123abcd", &report.repo_id).is_some(),
        "the cache round-trips"
    );
    let view = serde_json::to_string(&report.report).expect("serialize");
    assert!(!view.contains(token), "token in the serialized report");
    assert!(!format!("{report:?}").contains(token));
}
