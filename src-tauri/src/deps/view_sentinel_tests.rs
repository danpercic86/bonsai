//! E4 row 6′ `[MEDIUM-1]`, `[P125d review]` re-run at the src-tauri layer:
//! a token placed in the user npmrc and in a malformed repo manifest must be
//! absent from the serialized `DepTrackerView`, its `Debug`, every emitted
//! payload and the cache bytes — and the private dependency is never sent.

use std::time::Duration;

use bonsai_deps::cache::CACHE_DIR;
use bonsai_deps::model::Availability;
use bonsai_deps::registry_ctx::UserConfig;

use super::exec::start_run_inner;
use super::test_support::*;
use super::*;

const TOKEN: &str = "zz-sentinel-token";

fn has_token(s: &str) -> bool {
    s.to_ascii_lowercase().contains(TOKEN)
}

#[test]
fn the_serialized_view_events_and_cache_carry_no_token() {
    let dir = scratch();
    let wd = repo(
        dir.path(),
        &[
            (
                "package.json",
                r#"{"name":"app","dependencies":{"@corp/secret":"^1.0.0"}}"#,
            ),
            // Malformed on purpose: the ParseError message is a fixed table.
            (
                "tools/Cargo.toml",
                "[dependencies\nzz-sentinel-token = \"1\n",
            ),
        ],
    );
    let rid = id(&wd);
    let mut env = TestEnv::default();
    env.user.insert(
        UserConfig::NpmUserRc,
        format!(
            "@corp:registry=https://u:{TOKEN}@npm.corp.invalid/\n\
             //npm.corp.invalid/:_authToken={TOKEN}\n"
        ),
    );
    let t = TestTransport::holding();
    t.release();
    let h = harness(dir.path(), true, t.clone(), env, Duration::from_secs(600));
    let (_, task) = start_run_inner(&h.state, &rid, wd.clone()).expect("start");
    tauri::async_runtime::block_on(async { task.expect("run").await.expect("task") });

    assert_eq!(t.sends(), 0, "the private dependency is never sent");
    let v = view(&h.state, &rid, &wd);
    let report = v.report.as_ref().expect("a report");
    let rows: Vec<_> = report.groups.iter().flat_map(|g| g.rows.iter()).collect();
    let secret = rows
        .iter()
        .find(|r| r.name == "@corp/secret")
        .expect("the private row is listed");
    match &secret.availability {
        Availability::Private { registry_host, .. } => {
            assert!(registry_host.as_deref().is_none_or(|h| !h.contains('@')));
        }
        other => panic!("expected private, got {other:?}"),
    }

    let json = serde_json::to_string(&v).expect("view serializes");
    assert!(
        !has_token(&json),
        "serialized DepTrackerView leaks the token"
    );
    assert!(!has_token(&format!("{v:?}")), "Debug of the view leaks it");
    let events = serde_json::to_string(&h.events.all()).expect("events serialize");
    assert!(!has_token(&events), "an event payload leaks it");
    let cache_dir = h.data_root.join(CACHE_DIR);
    let mut files = 0;
    for e in std::fs::read_dir(&cache_dir).expect("cache dir") {
        let bytes = std::fs::read(e.expect("entry").path()).expect("cache file");
        assert!(
            !has_token(&String::from_utf8_lossy(&bytes)),
            "cache bytes leak it"
        );
        files += 1;
    }
    assert_eq!(files, 1);
}
