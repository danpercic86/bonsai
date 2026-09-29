//! P125 milestone smoke (`docs/contracts/P125-smoke-checklist.md`): a
//! git2-built scratch repo covering every supported ecosystem plus an
//! unsupported, a malformed and a private-scoped manifest, checked with
//! `run_check` over the RECORDED fixtures only (no network).
//!
//! `#[ignore]`d because it builds a persistent repo outside a tempdir; run
//! it on its own (never together with the live `live_record.rs`):
//!
//! ```text
//! cargo test -p bonsai-deps --test smoke_scratch -- --ignored --nocapture
//! ```
//!
//! `P125_SMOKE_DIR` overrides the default `D:\Data\Temp\P125-smoke`.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use bonsai_deps::model::{Availability, GroupState, ParseErrorKind, PrivateReason, Relation};
use bonsai_deps::registry_ctx::FakeHostEnv;
use bonsai_deps::remote::FakeTransport;

use common::corpus::{self, fixtures_dir};
use common::recorded_check_with;

/// Names that must never leave the process.
const PRIVATE: &[&str] = &["@zzsentinel/scoped", "zzsentinel", "zz-smoke-broken"];

const PACKAGE_JSON: &str = r#"{
  "name": "smoke-web",
  "dependencies": { "lodash": "^4.17.20", "@zzsentinel/scoped": "^1.0.0" }
}"#;

fn smoke_root() -> PathBuf {
    std::env::var_os("P125_SMOKE_DIR").map_or_else(
        || {
            if cfg!(windows) {
                PathBuf::from("D:\\Data\\Temp\\P125-smoke")
            } else {
                std::env::temp_dir().join("P125-smoke")
            }
        },
        PathBuf::from,
    )
}

/// A fresh git2 repo at `<root>/repo` with `files` staged.
fn build(root: &Path, files: &[(&str, &str)]) -> PathBuf {
    let wd = root.join("repo");
    if wd.exists() {
        fs::remove_dir_all(&wd).expect("remove the previous smoke repo");
    }
    fs::create_dir_all(&wd).expect("workdir");
    let repo = git2::Repository::init(&wd).expect("init");
    let mut idx = repo.index().expect("index");
    for (rel, body) in files {
        let p = wd.join(rel);
        fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        fs::write(&p, body).expect("write");
        idx.add_path(Path::new(rel)).expect("add");
    }
    idx.write().expect("write index");
    fs::canonicalize(&wd).expect("canon")
}

#[test]
#[ignore = "builds a persistent scratch repo; run explicitly for the P125 smoke"]
fn smoke_scratch_repo_offline() {
    let wd = build(
        &smoke_root(),
        &[
            ("package.json", PACKAGE_JSON),
            ("pnpm-lock.yaml", corpus::PNPM_LOCK),
            (".npmrc", "@zzsentinel:registry=https://npm.corp.invalid/\n"),
            ("Cargo.toml", corpus::CARGO_TOML),
            ("Cargo.lock", corpus::CARGO_LOCK),
            ("py/requirements.txt", corpus::REQUIREMENTS),
            ("go/go.mod", corpus::GO_MOD),
            ("dotnet/App/App.csproj", corpus::CSPROJ),
            ("android/build.gradle", "plugins { id 'java' }\n"),
            (
                "broken/package.json",
                "{ \"name\": \"zz-smoke-broken\", \"dependencies\": { \"left-pad\": \n",
            ),
        ],
    );
    let t = FakeTransport::new().with_fixtures(&fixtures_dir());
    let checked = recorded_check_with(&wd, &FakeHostEnv::new(), &t);
    let rep = match &checked.report {
        Ok(c) => &c.report,
        Err(e) => panic!("run_check: {e:?}"),
    };

    println!("smoke repo: {}", wd.display());
    println!("requests: {}", checked.requests.len());
    for g in &rep.groups {
        println!(
            "group {} (lock {:?}, eco {:?}) state {:?}",
            g.manifest_path, g.lockfile_path, g.ecosystem, g.state
        );
        for r in &g.rows {
            let ids: Vec<&str> = r.vulns.iter().map(|v| v.id.as_str()).collect();
            println!(
                "  {:?} {} {:?} -> {:?} {:?} {:?} vulns={ids:?}",
                r.relation, r.name, r.current, r.latest, r.update, r.availability
            );
        }
    }
    println!("summary: {:?}", rep.summary);

    // Privacy: no private or malformed-manifest name in any URL or body.
    for n in PRIVATE {
        assert!(!checked.sent(n), "{n} reached the outbound set");
    }
    assert!(
        checked.sent("lodash"),
        "positive control: public names are sent"
    );

    let group = |p: &str| {
        rep.groups
            .iter()
            .find(|g| g.manifest_path == p)
            .unwrap_or_else(|| panic!("no group {p}"))
    };
    let paths: Vec<&str> = rep
        .groups
        .iter()
        .map(|g| g.manifest_path.as_str())
        .collect();
    assert_eq!(paths.len(), 7, "{paths:?}");

    let web = group("package.json");
    assert_eq!(web.state, GroupState::Ok);
    let scoped = web
        .rows
        .iter()
        .find(|r| r.name == "@zzsentinel/scoped")
        .expect("the private row is listed");
    assert!(matches!(
        scoped.availability,
        Availability::Private {
            reason: PrivateReason::ScopedRegistry,
            ..
        }
    ));
    assert!(scoped.registry_url.is_none());

    assert!(matches!(
        group("android/build.gradle").state,
        GroupState::NotTracked { .. }
    ));
    let GroupState::ParseError { category, .. } = &group("broken/package.json").state else {
        panic!("broken manifest: {:?}", group("broken/package.json").state)
    };
    assert_eq!(*category, ParseErrorKind::InvalidSyntax);
    assert_eq!(group("py/requirements.txt").state, GroupState::NoLockfile);

    // Vulnerable transitive smallvec (Cargo.lock) is listed as transitive.
    assert!(group("Cargo.toml").rows.iter().any(|r| r.name == "smallvec"
        && r.relation == Relation::Transitive
        && !r.vulns.is_empty()));
    assert_eq!(rep.summary.not_tracked, 1);
    assert_eq!(rep.summary.parse_errors, 1);
    assert!(rep.summary.private >= 1 && rep.summary.vulnerable >= 5);
}
