//! `AzureDevOpsProvider::viewer()` — the P72 validate-then-identify contract
//! (`docs/contracts/P72-forge-connect-fixes.md` §A4/§3.1). Kept separate from
//! `mod_tests.rs` so neither file crosses the 500-line limit.

use super::testkit::*;
use super::*;

/// P72 case (a) + (l): validate on the repo endpoint, THEN identify — exactly
/// two requests, in that order, both with the Basic auth header.
#[test]
fn viewer_validates_repo_then_identifies_display_name() {
    let (p, seen) = provider_spy(
        Some("az-tok"),
        vec![
            (REPO_NEEDLE, 200, REPO_OK),
            (
                PROFILE_NEEDLE,
                200,
                r#"{ "displayName": "Ada Lovelace", "emailAddress": "ada@x" }"#,
            ),
        ],
    );
    let v = p.viewer().unwrap();
    assert_eq!(v.login, "Ada Lovelace");
    assert_eq!(v.avatar_url, None);

    let reqs = seen.lock().unwrap();
    assert_eq!(reqs.len(), 2, "validate + one best-effort identify");
    // 1. the SCOPE-VALIDATION probe: the repository object (Code scope).
    assert_eq!(
        reqs[0].url,
        "https://dev.azure.com/org/proj/_apis/git/repositories/repo?api-version=7.1"
    );
    // 2. the cross-host identity endpoint, api-versioned.
    assert_eq!(
        reqs[1].url,
        "https://app.vssps.visualstudio.com/_apis/profile/profiles/me?api-version=7.1"
    );
    // Basic auth (NOT Bearer / PRIVATE-TOKEN); plaintext PAT never on the wire.
    for req in reqs.iter() {
        let auth = req
            .headers
            .iter()
            .find(|(k, _)| k == "Authorization")
            .map(|(_, v)| v.as_str())
            .expect("Authorization header");
        assert!(auth.starts_with("Basic "), "auth: {auth}");
        assert!(!auth.contains("Bearer"), "auth: {auth}");
        assert!(!auth.contains("az-tok"), "PAT leaked into header: {auth}");
        assert!(
            !req.headers
                .iter()
                .any(|(k, _)| k.eq_ignore_ascii_case("PRIVATE-TOKEN")),
            "GitLab header on an Azure request"
        );
    }
}

/// P72 case (b): a profile payload with no name ⇒ an empty login, and NOTHING is
/// cached (an empty login must never look like a resolved identity).
#[test]
fn viewer_with_nameless_profile_has_empty_login_and_is_not_cached() {
    let host = "az-nameless.example";
    let (p, _seen) = provider_for(
        azure_target_on(host),
        Some("az-tok"),
        vec![(REPO_NEEDLE, 200, REPO_OK), (PROFILE_NEEDLE, 200, "{}")],
    );
    let v = p.viewer().unwrap();
    assert_eq!(v.login, "");
    assert!(auth::cached_viewer(host).is_none(), "empty login must not be cached");
}

/// **THE REGRESSION TEST for the reported bug (P72 case c).** A PAT scoped only
/// Code (Read & Write) reaches the repository endpoint but is 401'd by the
/// profile endpoint, which needs `vso.profile`. Before P72 that 401 failed the
/// whole connect and told the user their valid token was rejected. It must now
/// succeed with an empty login.
#[test]
fn viewer_succeeds_when_code_only_pat_is_401ed_by_the_profile_endpoint() {
    let host = "az-codeonly.example";
    let (p, seen) = provider_for(
        azure_target_on(host),
        Some("az-tok"),
        vec![(REPO_NEEDLE, 200, REPO_OK), (PROFILE_NEEDLE, 401, "{}")],
    );
    let v = p.viewer().expect("a Code-only PAT must connect");
    assert_eq!(v.login, "");
    assert_eq!(v.avatar_url, None);
    assert_eq!(seen.lock().unwrap().len(), 2);
    assert!(auth::cached_viewer(host).is_none(), "empty login must not be cached");
}

/// P72 case (h): identify swallows EVERY error class — a rate limit or a
/// transport failure must not fail a connect whose credentials just passed.
#[test]
fn viewer_swallows_rate_limit_and_transport_errors_while_identifying() {
    for profile_status in [429, 0 /* transport failure sentinel */] {
        let host = format!("az-identify-{profile_status}.example");
        let (p, seen) = provider_for(
            azure_target_on(&host),
            Some("az-tok"),
            vec![
                (REPO_NEEDLE, 200, REPO_OK),
                (PROFILE_NEEDLE, profile_status, "{}"),
            ],
        );
        let v = p.viewer().unwrap_or_else(|e| panic!("{profile_status}: {e:?}"));
        assert_eq!(v.login, "");
        assert_eq!(seen.lock().unwrap().len(), 2);
        assert!(auth::cached_viewer(&host).is_none());
    }
}

/// P72 case (d): a 401 on the VALIDATION probe is a real credential failure —
/// one request only, identify never attempted, nothing cached.
#[test]
fn viewer_401_on_repo_probe_is_auth_failed_and_stops_before_identify() {
    let host = "az-401.example";
    let (p, seen) = provider_for(
        azure_target_on(host),
        Some("bad"),
        vec![(REPO_NEEDLE, 401, "{}"), (PROFILE_NEEDLE, 200, REPO_OK)],
    );
    let err = p.viewer().unwrap_err();
    match &err {
        AppError::AuthFailed(m) => {
            assert!(m.contains("Code (Read & Write)"), "message: {m}");
            assert!(!m.contains("bad"), "PAT leaked: {m}");
        }
        other => panic!("expected AuthFailed, got {other:?}"),
    }
    assert_eq!(seen.lock().unwrap().len(), 1, "identify is never attempted");
    assert!(auth::cached_viewer(host).is_none());
}

/// P72 case (e): a 404 names the coords instead of the useless bare "not found".
#[test]
fn viewer_404_on_repo_probe_names_org_project_and_repo() {
    let (p, _seen) = provider_spy(Some("az-tok"), vec![(REPO_NEEDLE, 404, "{}")]);
    match p.viewer().unwrap_err() {
        AppError::ForgeApi(m) => {
            // Assert the triple as ONE unit. Asserting "org"/"proj"/"repo"
            // separately is vacuous: each is a substring of the message's own
            // static tail ("organization", "project", "repository"), so those
            // assertions passed even when the coords were never interpolated.
            assert!(m.contains("org/proj/repo"), "message: {m}");
            // The status-derived message is PRESERVED and the hint appended, so
            // the bare text is a prefix rather than being replaced (SF-2).
            assert!(m.starts_with("not found"), "message: {m}");
            assert_ne!(m, "not found", "the coords hint must be appended");
        }
        other => panic!("expected ForgeApi, got {other:?}"),
    }
}

/// P72 case (f): Azure's `203` + HTML sign-in page is an AUTH failure, not a
/// "malformed response" — and the HTML never reaches the message.
#[test]
fn viewer_203_signin_page_is_auth_failed_not_malformed() {
    let (p, _seen) = provider_spy(Some("bad"), vec![(REPO_NEEDLE, 203, SIGNIN_HTML)]);
    match p.viewer().unwrap_err() {
        AppError::AuthFailed(m) => {
            assert!(!m.contains("malformed"), "message: {m}");
            assert!(!m.contains("SIGNIN_MARKER"), "body echoed into message: {m}");
            assert!(m.contains("203"), "message: {m}");
        }
        other => panic!("expected AuthFailed, got {other:?}"),
    }
}

/// P72 case (g): a `200` carrying HTML cannot read as a successful validation.
#[test]
fn viewer_200_with_html_body_fails_the_repo_probe() {
    let (p, _seen) = provider_spy(Some("az-tok"), vec![(REPO_NEEDLE, 200, SIGNIN_HTML)]);
    assert!(matches!(p.viewer(), Err(AppError::ForgeApi(_))));
}

/// P72 case (i): no token ⇒ `ForgeAuthRequired` before any request is issued.
#[test]
fn viewer_requires_token() {
    let (p, seen) = provider_spy(None, vec![(REPO_NEEDLE, 200, REPO_OK)]);
    assert!(matches!(p.viewer(), Err(AppError::ForgeAuthRequired(_))));
    assert!(seen.lock().unwrap().is_empty(), "no request before the auth check");
}

/// P72 case (j) — the §A5 taxonomy delta: validating against a repo endpoint
/// makes org/project/repo mandatory, so an Azure target with no project is now
/// `ForgeUnsupported` instead of a profile-endpoint attempt.
#[test]
fn viewer_without_project_is_unsupported_with_no_request() {
    let mut target = azure_target_on("az-noproject.example");
    target.project = None;
    let (p, seen) = provider_for(target, Some("az-tok"), vec![(REPO_NEEDLE, 200, REPO_OK)]);
    assert!(matches!(p.viewer(), Err(AppError::ForgeUnsupported(_))));
    assert!(seen.lock().unwrap().is_empty());
}

