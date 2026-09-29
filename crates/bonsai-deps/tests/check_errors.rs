//! `run_check` failure paths, cancellation, OSV pagination, advisory reuse
//! and progress units (contract §5.1 / §5.3), all over a `FakeTransport`.

mod common;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bonsai_deps::check::ServiceFault;
use bonsai_deps::model::{Availability, CachedReport, DepReport, Relation, UnknownReason};
use bonsai_deps::registry_ctx::FakeHostEnv;
use bonsai_deps::remote::{FakeTransport, Method, RawResponse, RecordedRequest, TransportError};
use bonsai_deps::{run_check, CancelToken, CheckCtx, CheckError};

use common::corpus::{corpus_repo, fixtures_dir};
use common::fixture::FixtureRepo;

fn run_with(
    r: &FixtureRepo,
    t: &FakeTransport,
    cancel: &CancelToken,
    prev: Option<&DepReport>,
) -> Result<CachedReport, CheckError> {
    let ctx = CheckCtx {
        transport: t,
        env: &FakeHostEnv::new(),
        cancel,
        progress: &|_, _| {},
        prev,
        now_ms: 7,
    };
    run_check(&r.wd, "rid", &ctx)
}

fn run(r: &FixtureRepo, t: &FakeTransport) -> Result<CachedReport, CheckError> {
    run_with(r, t, &CancelToken::new(), None)
}

/// Every deps.dev request answers `res`; OSV comes from the fixtures.
fn respond_depsdev(res: Result<RawResponse, TransportError>) -> FakeTransport {
    let res = res.map(|r| (r.status, r.body));
    FakeTransport::new()
        .with_fixtures(&fixtures_dir())
        .with_responder(Box::new(move |r: &RecordedRequest| {
            r.url.contains("api.deps.dev").then(|| match &res {
                Ok((status, body)) => Ok(RawResponse {
                    status: *status,
                    body: body.clone(),
                }),
                Err(e) => Err(*e),
            })
        }))
}

fn status(s: u16, body: &str) -> Result<RawResponse, TransportError> {
    Ok(RawResponse {
        status: s,
        body: body.as_bytes().to_vec(),
    })
}

#[test]
fn error_mapping_follows_the_table() {
    for (res, want) in [
        (status(429, ""), CheckError::RateLimited),
        (
            status(503, ""),
            CheckError::Service(ServiceFault::Status(503)),
        ),
        (status(301, ""), CheckError::Service(ServiceFault::Redirect)),
        (
            status(200, "{not json"),
            CheckError::Service(ServiceFault::BadJson),
        ),
        (Err(TransportError::Connect), CheckError::Offline),
        (Err(TransportError::Timeout), CheckError::Timeout),
        // (`TransportError::TooLarge` from deps.dev degrades the package
        // instead — P125h §2, `an_oversize_body_degrades_each_package`.)
        (Err(TransportError::Gated), CheckError::Cancelled),
    ] {
        let t = respond_depsdev(res);
        assert_eq!(run(&corpus_repo(), &t).err(), Some(want));
    }
}

/// P125h §2: a deps.dev body over `DEPSDEV_BODY_CAP` (the real 8 MiB cap,
/// enforced by the fake) is `Unknown { reason: TooLarge }` for that package;
/// the check succeeds (6 lookups, under the breaker).
#[test]
fn an_oversize_body_degrades_each_package() {
    let big = format!(
        "{{\"versions\":[],\"pad\":\"{}\"}}",
        "x".repeat(8 * 1024 * 1024)
    );
    let t = respond_depsdev(status(200, &big));
    let rep = match run(&corpus_repo(), &t) {
        Ok(c) => c.report,
        Err(e) => panic!("{e:?}"),
    };
    let direct_public: Vec<_> = rep
        .groups
        .iter()
        .flat_map(|g| &g.rows)
        .filter(|r| r.registry_url.is_some() && r.relation == Relation::Direct)
        .collect();
    assert_eq!(direct_public.len(), 6);
    for r in direct_public {
        assert_eq!(
            r.availability,
            Availability::Unknown {
                reason: UnknownReason::TooLarge
            },
            "{}",
            r.name
        );
    }
}

#[test]
fn a_cancelled_token_sends_nothing() {
    let t = FakeTransport::new();
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(
        run_with(&corpus_repo(), &t, &cancel, None).err(),
        Some(CheckError::Cancelled)
    );
    assert!(t.requests().is_empty());
}

#[test]
fn cancelling_mid_check_stops_before_the_next_send() {
    let cancel = CancelToken::new();
    let c2 = cancel.clone();
    let t = FakeTransport::new().with_responder(Box::new(move |_| {
        c2.cancel();
        None
    }));
    assert_eq!(
        run_with(&corpus_repo(), &t, &cancel, None).err(),
        Some(CheckError::Cancelled)
    );
    // At most one request per worker was already in flight.
    assert!(t.requests().len() <= bonsai_deps::DEPSDEV_PARALLEL);
    assert!(t.requests().iter().all(|r| r.url.contains("api.deps.dev")));
}

#[test]
fn nothing_public_means_zero_requests() {
    let r = FixtureRepo::new();
    r.track(
        "package.json",
        r#"{"dependencies": {"a": "file:./a", "b": "workspace:*"}}"#,
    );
    let t = FakeTransport::new();
    let rep = run(&r, &t).map(|c| c.report);
    assert!(rep.is_ok());
    assert!(t.requests().is_empty());
}

#[test]
fn progress_counts_lookups_plus_batches() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    let progress = move |d: u32, t: u32| s2.lock().unwrap_or_else(|e| e.into_inner()).push((d, t));
    let t = FakeTransport::new().with_fixtures(&fixtures_dir());
    let cancel = CancelToken::new();
    let ctx = CheckCtx {
        transport: &t,
        env: &FakeHostEnv::new(),
        cancel: &cancel,
        progress: &progress,
        prev: None,
        now_ms: 0,
    };
    assert!(run_check(&corpus_repo().wd, "rid", &ctx).is_ok());
    let seen = seen.lock().unwrap_or_else(|e| e.into_inner()).clone();
    // 6 direct lookups (lodash, serde, tokio, requests, websocket,
    // Newtonsoft.Json) + 1 querybatch chunk; advisory GETs are not units.
    assert_eq!(seen.first(), Some(&(0, 7)));
    assert_eq!(seen.last(), Some(&(7, 7)));
    let batches = t
        .requests()
        .iter()
        .filter(|r| r.method == Method::Post)
        .count();
    assert_eq!(batches, 1);
}

#[test]
fn advisory_data_is_reused_when_modified_matches() {
    let r = corpus_repo();
    let t1 = FakeTransport::new().with_fixtures(&fixtures_dir());
    let first = match run(&r, &t1) {
        Ok(c) => c.report,
        Err(e) => panic!("{e:?}"),
    };
    assert!(t1.requests().iter().any(|q| q.url.contains("/v1/vulns/")));
    let t2 = FakeTransport::new().with_fixtures(&fixtures_dir());
    let second = run_with(&r, &t2, &CancelToken::new(), Some(&first)).map(|c| c.report);
    assert!(t2.requests().iter().all(|q| !q.url.contains("/v1/vulns/")));
    assert_eq!(second.map(|s| s.groups), Ok(first.groups));
}

/// A querybatch responder: queries 0 and 2 of the FIRST batch page (with a
/// fresh token each round, while `n < max_pages`); query 1 never pages.
fn pager(max_pages: u32) -> (FakeTransport, Arc<AtomicU32>) {
    let calls = Arc::new(AtomicU32::new(0));
    let c2 = calls.clone();
    let t = FakeTransport::new().with_responder(Box::new(move |r: &RecordedRequest| {
        if r.method != Method::Post {
            return None;
        }
        let n = c2.fetch_add(1, Ordering::SeqCst) + 1;
        let body: serde_json::Value = serde_json::from_str(r.body.as_deref()?).ok()?;
        let queries = body["queries"].as_array()?.clone();
        let results: Vec<serde_json::Value> = queries
            .iter()
            .map(|q| {
                let name = q["package"]["name"].as_str().unwrap_or("");
                if name == "itoa" {
                    return serde_json::json!({"vulns": [{"id": "GHSA-itoa-once", "modified": "m"}]});
                }
                if n < max_pages {
                    serde_json::json!({
                        "vulns": [{"id": format!("GHSA-{name}-{n}"), "modified": "m"}],
                        "next_page_token": format!("tok-{name}-{n}")
                    })
                } else {
                    serde_json::json!({})
                }
            })
            .collect();
        Some(Ok(RawResponse {
            status: 200,
            body: serde_json::json!({ "results": results }).to_string().into_bytes(),
        }))
    }));
    (t, calls)
}

const CRATES_IO: &str = "source = \"registry+https://github.com/rust-lang/crates.io-index\"\n";

fn three_crates() -> FixtureRepo {
    let r = FixtureRepo::new();
    r.track(
        "Cargo.toml",
        "[package]\nname = \"p\"\nversion = \"0.1.0\"\n[dependencies]\nlibc = \"0.2\"\nitoa = \"1\"\nserde = \"1\"\n",
    )
    .track(
        "Cargo.lock",
        &format!(
            "version = 4\n[[package]]\nname = \"libc\"\nversion = \"0.2.150\"\n{CRATES_IO}\
             [[package]]\nname = \"itoa\"\nversion = \"1.0.11\"\n{CRATES_IO}\
             [[package]]\nname = \"serde\"\nversion = \"1.0.200\"\n{CRATES_IO}"
        ),
    );
    r
}

fn posts(t: &FakeTransport) -> Vec<serde_json::Value> {
    t.requests()
        .iter()
        .filter(|r| r.method == Method::Post)
        .filter_map(|r| serde_json::from_str(r.body.as_deref()?).ok())
        .collect()
}

fn names(q: &serde_json::Value) -> Vec<(String, Option<String>)> {
    q["queries"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|q| {
            (
                q["package"]["name"].as_str().unwrap_or("").to_string(),
                q.get("page_token")
                    .and_then(|t| t.as_str())
                    .map(str::to_string),
            )
        })
        .collect()
}

#[test]
fn pagination_resends_only_the_paged_queries_with_their_tokens() {
    let (t, calls) = pager(3);
    let rep = run(&three_crates(), &t);
    assert!(rep.is_ok(), "{:?}", rep.err());
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    let posts = posts(&t);
    // Batch 1 mixes paged (libc, serde) and unpaged (itoa) results.
    let first = names(&posts[0]);
    assert_eq!(first.len(), 3);
    assert!(first.iter().all(|(_, tok)| tok.is_none()));
    // Requests 2 and 3 carry ONLY the paged queries, each with its token.
    for (i, p) in posts.iter().enumerate().skip(1) {
        let mut got = names(p);
        got.sort();
        assert_eq!(
            got,
            [
                ("libc".to_string(), Some(format!("tok-libc-{i}"))),
                ("serde".to_string(), Some(format!("tok-serde-{i}")))
            ],
            "request {}",
            i + 1
        );
    }
    let mut ids: Vec<String> = rep
        .map(|c| c.report.groups)
        .unwrap_or_default()
        .iter()
        .flat_map(|g| &g.rows)
        .flat_map(|r| r.vulns.iter().map(|a| a.id.clone()))
        .collect();
    ids.sort();
    assert_eq!(
        ids,
        [
            "GHSA-itoa-once",
            "GHSA-libc-1",
            "GHSA-libc-2",
            "GHSA-serde-1",
            "GHSA-serde-2"
        ]
    );
}

#[test]
fn an_eleventh_page_is_a_page_limit_and_never_sent() {
    let (t, calls) = pager(u32::MAX);
    assert_eq!(
        run(&three_crates(), &t).err(),
        Some(CheckError::Service(ServiceFault::PageLimit))
    );
    // Ten requests went out; the 11th page is refused before sending.
    assert_eq!(calls.load(Ordering::SeqCst), 10);
    assert!(t.requests().iter().all(|r| !r.url.contains("/v1/vulns/")));
}

#[test]
fn malformed_advisory_ids_are_dropped() {
    // [P125d review, LOW-D2] §7.5: a bad id is dropped, the check goes on,
    // and the valid id of the same result is kept.
    let t = FakeTransport::new().with_responder(Box::new(|r: &RecordedRequest| {
        (r.method == Method::Post).then(|| {
            Ok(RawResponse {
                status: 200,
                body: br#"{"results":[{"vulns":[{"id":"../../x","modified":"m"},{"id":"a b","modified":"m"},{"id":"GHSA-good-good-good","modified":"m"}]},{},{}]}"#.to_vec(),
            })
        })
    }));
    let rep = match run(&three_crates(), &t) {
        Ok(c) => c.report,
        Err(e) => panic!("{e:?}"),
    };
    let ids: Vec<&str> = rep
        .groups
        .iter()
        .flat_map(|g| &g.rows)
        .flat_map(|r| r.vulns.iter().map(|a| a.id.as_str()))
        .collect();
    assert_eq!(ids, ["GHSA-good-good-good"]);
    let gets: Vec<String> = t
        .requests()
        .iter()
        .filter(|r| r.url.contains("/v1/vulns/"))
        .map(|r| r.url.clone())
        .collect();
    assert_eq!(gets, ["https://api.osv.dev/v1/vulns/GHSA-good-good-good"]);
}
