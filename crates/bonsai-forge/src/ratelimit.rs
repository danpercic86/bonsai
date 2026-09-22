//! P113a — turning a provider's rate-limit headers into a MACHINE-READABLE wait.
//!
//! Before P113a every provider formatted its hint into the
//! [`AppError::ForgeRateLimited`] message ("retry after 90s") and threw the
//! number away; a caller that wanted to back off had to parse prose. These two
//! helpers produce the seconds that now ride in
//! `AppError::ForgeRateLimited { retry_after_secs, .. }`.
//!
//! Both return `None` rather than a guess when the header is missing or
//! unusable — an absent hint is the caller's policy decision (what default wait
//! to apply), and a fabricated `0` would read as "retry immediately", which is
//! exactly the feedback loop P113a exists to stop.
//!
//! [`AppError::ForgeRateLimited`]: bonsai_core::error::AppError::ForgeRateLimited

use std::time::{SystemTime, UNIX_EPOCH};

/// Upper bound on any advertised wait (24 h). A absurd/garbage header must not
/// turn into a multi-year suppression window downstream.
const MAX_RETRY_SECS: u64 = 24 * 60 * 60;

/// Parse a `Retry-After` header value (Azure DevOps, Bitbucket).
///
/// Only the delta-seconds form is understood. RFC 9110 also allows an HTTP-date
/// (`Wed, 21 Oct 2015 07:28:00 GMT`); parsing dates would pull in a date crate
/// for a form neither provider sends, so it yields `None` and the caller falls
/// back to its own default.
pub fn parse_retry_after_secs(value: &str) -> Option<u32> {
    let secs: u64 = value.trim().parse().ok()?;
    clamp(secs)
}

/// Seconds from now until a `*-RateLimit-Reset` UNIX-epoch header
/// (GitHub, GitLab).
///
/// `None` when the epoch is unparseable or already in the past — a window that
/// has already elapsed advertises no wait at all.
pub fn secs_until_epoch(value: &str) -> Option<u32> {
    secs_until_epoch_at(value, now_epoch_secs()?)
}

/// Testable core of [`secs_until_epoch`] with the clock injected.
fn secs_until_epoch_at(value: &str, now_secs: u64) -> Option<u32> {
    let reset: u64 = value.trim().parse().ok()?;
    let delta = reset.checked_sub(now_secs)?;
    if delta == 0 {
        return None;
    }
    clamp(delta)
}

fn now_epoch_secs() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

/// `Some` only for a positive wait inside [`MAX_RETRY_SECS`]; anything larger is
/// capped (not dropped — the server did say "wait a long time").
fn clamp(secs: u64) -> Option<u32> {
    if secs == 0 {
        return None;
    }
    Some(secs.min(MAX_RETRY_SECS) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_delta_seconds() {
        assert_eq!(parse_retry_after_secs("90"), Some(90));
        assert_eq!(parse_retry_after_secs("  120 "), Some(120));
    }

    #[test]
    fn retry_after_rejects_zero_negative_and_http_date() {
        assert_eq!(parse_retry_after_secs("0"), None);
        assert_eq!(parse_retry_after_secs("-5"), None);
        assert_eq!(
            parse_retry_after_secs("Wed, 21 Oct 2015 07:28:00 GMT"),
            None
        );
        assert_eq!(parse_retry_after_secs(""), None);
    }

    #[test]
    fn retry_after_is_capped_not_dropped() {
        assert_eq!(parse_retry_after_secs("999999999"), Some(86_400));
    }

    #[test]
    fn reset_epoch_in_the_future_becomes_a_delta() {
        assert_eq!(secs_until_epoch_at("1700000060", 1_700_000_000), Some(60));
    }

    #[test]
    fn reset_epoch_in_the_past_or_now_is_no_hint() {
        assert_eq!(secs_until_epoch_at("1699999999", 1_700_000_000), None);
        assert_eq!(secs_until_epoch_at("1700000000", 1_700_000_000), None);
        assert_eq!(secs_until_epoch_at("nonsense", 1_700_000_000), None);
    }
}
