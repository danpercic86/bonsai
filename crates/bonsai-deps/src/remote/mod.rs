//! Outbound HTTP (contract §7): the transport seam, the deps.dev and OSV
//! request builders + response mappers, and the test doubles.
//!
//! `check.rs` sees only functions shaped like
//! `fn(&dyn Transport, &PublicPackage | &VulnRef) -> …`; `OutboundRequest`
//! can be built only in here (§7.3 item 3).
//!
//! Nothing under `remote/` may name adapter types; `tests/no_process.rs`
//! enforces it (§7.3 item 4).

pub mod depsdev;
#[cfg(any(test, feature = "test-support"))]
mod fake_transport;
pub mod osv;
pub mod osv_vuln;
#[cfg(any(test, feature = "test-support"))]
mod recorder;
mod reqwest_transport;
pub mod transport;

#[cfg(any(test, feature = "test-support"))]
pub use fake_transport::{FakeTransport, RecordedRequest, Responder};
#[cfg(any(test, feature = "test-support"))]
pub use recorder::RecordingTransport;
pub use reqwest_transport::{ReqwestTransport, TransportInitError};
pub use transport::{Method, OutboundRequest, RawResponse, Transport, TransportError};

#[cfg(test)]
#[path = "remote_tests.rs"]
mod tests;
