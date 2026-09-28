//! `[HIGH-2]` §0.1 corpus check (ignored; run by hand):
//!
//! `BONSAI_YAML_CORPUS="D:/a;D:/b" cargo test -p bonsai-deps yaml_corpus -- --ignored --nocapture`
//!
//! Walks every root (node_modules included, `.git` skipped), runs the
//! bounded event reader over each `pnpm-lock.yaml` and each berry
//! `yarn.lock`, and fails if any uses an alias or anchor. The yarn v1 locks
//! found are fed to the v1 line parser as a smoke check.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use super::*;

fn walk(root: PathBuf, out: &mut Vec<PathBuf>) {
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let p = e.path();
            if ft.is_dir() {
                if e.file_name() != ".git" {
                    stack.push(p);
                }
            } else if ft.is_file()
                && matches!(e.file_name().to_str(), Some("pnpm-lock.yaml" | "yarn.lock"))
            {
                out.push(p);
            }
        }
    }
}

#[test]
#[ignore = "manual corpus run; needs BONSAI_YAML_CORPUS"]
fn yaml_corpus() {
    let Ok(roots) = std::env::var("BONSAI_YAML_CORPUS") else {
        panic!("set BONSAI_YAML_CORPUS to ';'-separated roots");
    };
    let mut files = Vec::new();
    for r in roots.split(';').filter(|r| !r.is_empty()) {
        walk(PathBuf::from(r), &mut files);
    }
    let keep = |p: &[String]| p.len() <= 1 || (p.len() == 2 && p[0] == "__metadata");
    let (mut pnpm, mut berry, mut v1) = (0usize, 0usize, 0usize);
    let mut versions: BTreeMap<String, usize> = BTreeMap::new();
    let mut bad = Vec::new();
    for f in &files {
        let Ok(text) = fs::read_to_string(f) else {
            continue;
        };
        let is_pnpm = f.file_name().is_some_and(|n| n == "pnpm-lock.yaml");
        let is_berry = !is_pnpm && text.lines().any(|l| l.starts_with("__metadata:"));
        if !is_pnpm && !is_berry {
            v1 += 1;
            let files = crate::discover::FileSet::default();
            let input = crate::adapter::ParseInput {
                manifest_path: "package.json",
                manifest: "{}",
                lockfile: Some(("yarn.lock", &text)),
                files: &files,
            };
            use crate::adapter::EcosystemAdapter;
            if let Err(e) = crate::eco::npm::NpmAdapter.parse(&input) {
                bad.push(format!("{} v1 {:?}", f.display(), e.kind));
            }
            continue;
        }
        match parse_documents(&text, &keep) {
            Ok(docs) => {
                let v = if is_pnpm {
                    pnpm += 1;
                    docs.iter()
                        .find_map(|d| d.get("lockfileVersion").and_then(YNode::as_str))
                        .map(|v| format!("pnpm {v}"))
                } else {
                    berry += 1;
                    docs.first()
                        .and_then(|d| d.get("__metadata"))
                        .and_then(|m| m.get("version"))
                        .and_then(YNode::as_str)
                        .map(|v| format!("berry {v}"))
                };
                *versions.entry(v.unwrap_or_else(|| "?".into())).or_default() += 1;
            }
            Err(e) => bad.push(format!("{} {:?}", f.display(), e.kind)),
        }
    }
    eprintln!("corpus: {pnpm} pnpm, {berry} berry, {v1} yarn-v1; versions {versions:?}");
    assert!(bad.is_empty(), "{bad:#?}");
}
