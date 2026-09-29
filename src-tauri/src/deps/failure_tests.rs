//! The fixed `CheckFailure.message` table and its TS mirror
//! (`DEP_FAILURE_MESSAGES` in `src/ipc/fixtures/depTracker.ts`). Rust is
//! authoritative: if the parity test fails, edit the TS table to match.

use bonsai_deps::check::{CheckError, InternalFault, ServiceFault};

use super::failure::*;
use super::CheckFailureKind;

/// `kind: 'message'` pairs of the TS `DEP_FAILURE_MESSAGES` object literal.
fn ts_messages() -> Vec<(String, String)> {
    // Read at test time: the no-process scan (§7.4) bans compile-time file
    // inclusion in this directory.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("src")
        .join("ipc")
        .join("fixtures")
        .join("depTracker.ts");
    let ts = std::fs::read_to_string(&path).expect("read the TS fixture table");
    let start = ts
        .find("export const DEP_FAILURE_MESSAGES")
        .expect("DEP_FAILURE_MESSAGES is exported");
    let body = &ts[start..];
    let open = body.find("= {").expect("object literal") + 3;
    let close = body[open..].find("};").expect("literal end") + open;
    body[open..close]
        .lines()
        .filter_map(|l| {
            let (k, v) = l.trim().split_once(':')?;
            let v = v.trim().trim_end_matches(',');
            let v = v.strip_prefix('\'')?.strip_suffix('\'')?;
            Some((k.trim().to_owned(), v.to_owned()))
        })
        .collect()
}

fn msg(e: CheckError) -> (CheckFailureKind, Option<u16>, String) {
    let f = render(e, 7).expect("a failure");
    assert_eq!(f.at_ms, 7);
    (f.kind, f.http_status, f.message)
}

#[test]
fn ts_failure_messages_match_the_rust_table() {
    let ts = ts_messages();
    let expected = [
        ("offline", msg(CheckError::Offline).2),
        ("rateLimited", msg(CheckError::RateLimited).2),
        // The TS fixture renders its `service` case with status 503.
        (
            "service",
            msg(CheckError::Service(ServiceFault::Status(503))).2,
        ),
        ("timeout", msg(CheckError::Timeout).2),
        ("internal", msg(CheckError::Internal(InternalFault::Io)).2),
    ];
    assert_eq!(ts.len(), expected.len(), "TS table: {ts:?}");
    for (k, v) in expected {
        let got = ts.iter().find(|(tk, _)| tk == k).map(|(_, tv)| tv.as_str());
        assert_eq!(got, Some(v.as_str()), "TS DEP_FAILURE_MESSAGES.{k}");
    }
}

#[test]
fn every_check_error_maps_to_a_fixed_kind_status_and_message() {
    use CheckFailureKind as K;
    assert!(render(CheckError::Cancelled, 0).is_none());
    assert_eq!(
        msg(CheckError::Offline),
        (K::Offline, None, MSG_OFFLINE.into())
    );
    assert_eq!(
        msg(CheckError::RateLimited),
        (K::RateLimited, Some(429), MSG_RATE_LIMITED.into())
    );
    assert_eq!(
        msg(CheckError::Timeout),
        (K::Timeout, None, MSG_TIMEOUT.into())
    );
    assert_eq!(
        msg(CheckError::Service(ServiceFault::Status(502))),
        (
            K::Service,
            Some(502),
            "the package services returned HTTP 502".into()
        )
    );
    for f in [
        ServiceFault::Redirect,
        ServiceFault::BadJson,
        ServiceFault::TooLarge,
        ServiceFault::TooManyAdvisories,
        ServiceFault::PageLimit,
    ] {
        assert_eq!(
            msg(CheckError::Service(f)),
            (K::Service, None, MSG_SERVICE_INVALID.into())
        );
    }
    for f in [
        InternalFault::GitOpen,
        InternalFault::GitIndex,
        InternalFault::Io,
    ] {
        assert_eq!(
            msg(CheckError::Internal(f)),
            (K::Internal, None, MSG_INTERNAL.into())
        );
    }
    assert_eq!(internal(3).message, MSG_INTERNAL);
}
