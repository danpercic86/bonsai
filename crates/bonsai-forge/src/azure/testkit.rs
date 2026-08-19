//! Shared offline test harness for the Azure provider tests
//! (`mod_tests.rs`, `viewer_tests.rs`). No network, no fixtures on disk.

use super::*;
use crate::http::{HttpRequest, HttpResponse};
use std::sync::{Arc, Mutex};

pub(crate) type Spy = Arc<Mutex<Vec<HttpRequest>>>;

/// A canned transport keyed by a URL substring, recording every request into
/// a shared [`Spy`]. Zero network.
pub(crate) struct FakeTransport {
    routes: Vec<(String, u16, String)>,
    seen: Spy,
}

impl FakeTransport {
    pub(crate) fn with_seen(routes: Vec<(&str, u16, &str)>, seen: Spy) -> Self {
        Self {
            routes: routes
                .into_iter()
                .map(|(m, s, b)| (m.to_string(), s, b.to_string()))
                .collect(),
            seen,
        }
    }
}

impl HttpTransport for FakeTransport {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, AppError> {
        self.seen.lock().unwrap().push(req.clone());
        let (status, body) = self
            .routes
            .iter()
            .find(|(needle, _, _)| req.url.contains(needle.as_str()))
            .map(|(_, s, b)| (*s, b.clone()))
            .unwrap_or_else(|| panic!("no fake route matched {}", req.url));
        // Sentinel: status 0 ⇒ the TRANSPORT itself fails (a network blip), which
        // is a different failure class from any HTTP status.
        if status == 0 {
            return Err(AppError::NetworkError("request failed (simulated)".to_string()));
        }
        Ok(HttpResponse {
            status,
            headers: vec![],
            body,
        })
    }
}

/// The repo-probe route needle. The `?` is LOAD-BEARING: without it the
/// substring also matches `/pullrequests…`, which shares `repo_base`.
pub(crate) const REPO_NEEDLE: &str = "/_apis/git/repositories/repo?";
/// The cross-host identity route needle.
pub(crate) const PROFILE_NEEDLE: &str = "/profile/profiles/me";
/// A repository object as Azure returns it from the probe endpoint.
pub(crate) const REPO_OK: &str = r#"{ "id": "r1", "name": "repo" }"#;
/// Azure's invalid-PAT answer: an HTML sign-in page, not JSON.
pub(crate) const SIGNIN_HTML: &str = "<html><title>Sign In</title>SIGNIN_MARKER</html>";

pub(crate) fn azure_target() -> ForgeTarget {
    azure_target_on("dev.azure.com")
}

/// Same Azure coords on an arbitrary `host`. Hosts only key the PROCESS-WIDE
/// viewer cache (`auth::cache_viewer`) — never a URL — so cache assertions use a
/// unique host per test to stay independent of test execution order.
pub(crate) fn azure_target_on(host: &str) -> ForgeTarget {
    ForgeTarget {
        kind: ForgeKind::AzureDevOps,
        host: host.to_string(),
        owner: "org".to_string(),
        repo: "repo".to_string(),
        project: Some("proj".to_string()),
        web_url: "https://dev.azure.com/org/proj/_git/repo".to_string(),
    }
}

/// A provider over `target` with a spy transport (the general form of
/// [`provider_spy`], used where the target is not the default).
pub(crate) fn provider_for(
    target: ForgeTarget,
    token: Option<&str>,
    routes: Vec<(&str, u16, &str)>,
) -> (AzureDevOpsProvider, Spy) {
    let seen: Spy = Arc::new(Mutex::new(Vec::new()));
    let transport = FakeTransport::with_seen(routes, Arc::clone(&seen));
    let p = AzureDevOpsProvider::new(target, token.map(str::to_string), Box::new(transport));
    (p, seen)
}

pub(crate) fn provider_spy(
    token: Option<&str>,
    routes: Vec<(&str, u16, &str)>,
) -> (AzureDevOpsProvider, Spy) {
    provider_for(azure_target(), token, routes)
}

pub(crate) fn provider(token: Option<&str>, routes: Vec<(&str, u16, &str)>) -> AzureDevOpsProvider {
    provider_spy(token, routes).0
}
