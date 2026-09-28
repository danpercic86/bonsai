//! `CheckError` (contract §5.1 / §5.3). `[MEDIUM-1]` no String payload
//! anywhere: every variant is a tag or a status code, so neither `Debug`
//! nor a rendered message can carry response text, config text or a token.

use crate::remote::transport::TransportError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceFault {
    /// A non-2xx status other than 404 / 429 / 3xx.
    Status(u16),
    Redirect,
    BadJson,
    /// Oversize body, or more than `MAX_VERSIONS` deps.dev versions.
    TooLarge,
    TooManyAdvisories,
    PageLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalFault {
    GitOpen,
    GitIndex,
    Io,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckError {
    Cancelled,
    Offline,
    RateLimited,
    Timeout,
    Service(ServiceFault),
    Internal(InternalFault),
}

impl From<TransportError> for CheckError {
    /// §5.3. `Gated` (the src-tauri gate refused: tracker disabled or a
    /// stale epoch) is a cancellation; `Other` (a transport failure that is
    /// neither connect nor timeout) reads as offline.
    fn from(e: TransportError) -> Self {
        match e {
            TransportError::Connect | TransportError::Other => CheckError::Offline,
            TransportError::Timeout => CheckError::Timeout,
            TransportError::TooLarge => CheckError::Service(ServiceFault::TooLarge),
            TransportError::Gated => CheckError::Cancelled,
        }
    }
}

/// §5.3 status mapping for everything except the caller-handled 200 / 404.
pub(crate) fn status_error(status: u16) -> CheckError {
    match status {
        429 => CheckError::RateLimited,
        300..=399 => CheckError::Service(ServiceFault::Redirect),
        s => CheckError::Service(ServiceFault::Status(s)),
    }
}
