//! `CheckError` → `CheckFailure` (contract §5.3, `[MEDIUM-1]`). The message is
//! a FIXED table over (kind, HTTP status): no response text, config text or
//! path can reach it. The TS mirror is `DEP_FAILURE_MESSAGES` in
//! `src/ipc/fixtures/depTracker.ts` (parity-tested in `failure_tests.rs`).

use bonsai_deps::check::{CheckError, ServiceFault};

use super::{CheckFailure, CheckFailureKind};

pub(crate) const MSG_OFFLINE: &str = "could not reach the package services";
pub(crate) const MSG_RATE_LIMITED: &str = "the package services rate-limited the check";
/// A service fault without a status code (redirect, malformed or oversize
/// body, advisory / page limits).
pub(crate) const MSG_SERVICE_INVALID: &str = "the package services returned an invalid response";
pub(crate) const MSG_TIMEOUT: &str = "the check timed out";
pub(crate) const MSG_INTERNAL: &str = "internal error";

/// Only an HTTP status (a number) is ever interpolated.
pub(crate) fn service_status_message(status: u16) -> String {
    format!("the package services returned HTTP {status}")
}

/// Renders a failed attempt. `None` for `Cancelled`, which is not a failure
/// (a cancelled run writes nothing, §6.3).
pub(crate) fn render(e: CheckError, at_ms: i64) -> Option<CheckFailure> {
    let (kind, http_status, message) = match e {
        CheckError::Cancelled => return None,
        CheckError::Offline => (CheckFailureKind::Offline, None, MSG_OFFLINE.to_owned()),
        CheckError::RateLimited => (
            CheckFailureKind::RateLimited,
            Some(429),
            MSG_RATE_LIMITED.to_owned(),
        ),
        CheckError::Timeout => (CheckFailureKind::Timeout, None, MSG_TIMEOUT.to_owned()),
        CheckError::Service(ServiceFault::Status(s)) => (
            CheckFailureKind::Service,
            Some(s),
            service_status_message(s),
        ),
        CheckError::Service(
            ServiceFault::Redirect
            | ServiceFault::BadJson
            | ServiceFault::TooLarge
            | ServiceFault::TooManyAdvisories
            | ServiceFault::PageLimit,
        ) => (
            CheckFailureKind::Service,
            None,
            MSG_SERVICE_INVALID.to_owned(),
        ),
        CheckError::Internal(_) => (CheckFailureKind::Internal, None, MSG_INTERNAL.to_owned()),
    };
    Some(CheckFailure {
        kind,
        http_status,
        message,
        at_ms,
    })
}

/// A completed check whose report could not be persisted.
pub(crate) fn internal(at_ms: i64) -> CheckFailure {
    CheckFailure {
        kind: CheckFailureKind::Internal,
        http_status: None,
        message: MSG_INTERNAL.to_owned(),
        at_ms,
    }
}
