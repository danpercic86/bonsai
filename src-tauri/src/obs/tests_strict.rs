//! P91 §7.1 — strict-mode enforcement tests.
//!
//! These are the tests that make the header's own promise checkable: a `strict`
//! file must be safe to hand to a third party WITHOUT reading it first. They
//! deliberately drive the enforcement through records a hostile or buggy
//! producer could send, not through records the emit sites are known to produce.

use serde_json::json;

use super::redact::Redactor;
use super::strict::{enforce, redact_names};

fn r() -> Redactor {
    Redactor::with_salt([7; 16])
}

#[test]
fn absolute_paths_become_ordinals_keeping_only_the_extension() {
    let r = r();
    let out = redact_names(
        r"cannot open C:\Users\jane.doe\repos\acme-client\src\App.tsx",
        &r,
    );
    assert!(!out.contains("jane.doe"), "{out}");
    assert!(!out.contains("acme-client"), "{out}");
    assert!(out.contains("path#") && out.ends_with(".tsx"), "{out}");
}

#[test]
fn unix_paths_and_git_dirs_are_redacted_too() {
    let r = r();
    let out = redact_names("cannot open /home/jane/work/secret-project/.git", &r);
    assert!(
        !out.contains("secret-project") && !out.contains("jane"),
        "{out}"
    );
    assert!(out.contains("path#"), "{out}");
}

#[test]
fn ref_names_keep_their_kind_only() {
    let r = r();
    let out = redact_names("checkout refs/heads/feature/acme-billing failed", &r);
    assert!(!out.contains("acme-billing"), "{out}");
    assert!(out.contains("ref#") && out.contains("(branch)"), "{out}");
    assert!(redact_names("refs/tags/v1.2.3", &r).contains("(tag)"));
    assert!(redact_names("refs/remotes/origin/main", &r).contains("(remote)"));
}

#[test]
fn urls_keep_scheme_and_forge_kind_only() {
    let r = r();
    let out = redact_names("push to https://github.com/acme-corp/secret-repo.git", &r);
    assert!(
        !out.contains("acme-corp") && !out.contains("secret-repo"),
        "{out}"
    );
    assert!(
        out.contains("remote#") && out.contains("(https,github)"),
        "{out}"
    );
    // An scp-style remote is redacted too (as a path ordinal rather than a
    // remote one) — over-redaction is fine, a leaked host+repo is not.
    let scp = redact_names("git@gitlab.example.com:o/r.git", &r);
    assert!(!scp.contains("gitlab.example.com"), "{scp}");
    assert!(redact_names("ssh://git@dev.azure.com/org/proj/_git/r", &r).contains("(ssh,azure)"));
}

#[test]
fn the_same_name_gets_the_same_ordinal_every_time() {
    let r = r();
    let a = redact_names("refs/heads/main", &r);
    let b = redact_names("checkout refs/heads/main", &r);
    assert!(b.contains(a.trim()), "{a} vs {b}");
}

/// Over-redaction is the accepted failure direction, but ordinary words and
/// symbol names must survive or the log stops being readable.
#[test]
fn structural_fields_and_prose_are_left_alone() {
    let r = r();
    for s in [
        "get_graph",
        "Sidebar",
        "sidebar.branch.checkout",
        "and/or",
        "status|graph",
        "9f2a1c4d5e6f70819293a4b5c6d7e8f901234567",
    ] {
        assert_eq!(redact_names(s, &r), s, "over-redacted: {s}");
    }
}

/// MUST-FIX 1: a hostile / buggy producer sends `args` on an `ipc.call`. The
/// WRITER strips it — the record is never trusted.
#[test]
fn args_are_stripped_regardless_of_what_the_producer_sent() {
    let r = r();
    let mut v = json!({
        "kind": "ipc.call",
        "cmd": "get_graph",
        "argsHash": "deadbeef",
        "args": { "repoId": "C:/Users/jane/repos/acme", "branch": "refs/heads/secret" }
    });
    enforce(&mut v, &r);
    assert!(v.get("args").is_none(), "args survived strict mode: {v}");
    assert_eq!(v["cmd"], "get_graph", "structural fields are untouched");
    assert_eq!(v["argsHash"], "deadbeef");
}

/// MUST-FIX 3: an `AppError` Display string carrying a username and a repo name
/// must not reach a strict file verbatim.
#[test]
fn error_messages_are_scrubbed_of_paths_refs_and_urls() {
    let r = r();
    let mut v = json!({
        "kind": "error",
        "where": "repo.open",
        "message": r"cannot open C:\Users\jane.doe\repos\acme-client\.git while fetching https://github.com/acme-corp/private for refs/heads/release"
    });
    enforce(&mut v, &r);
    let msg = v["message"].as_str().expect("message");
    for leak in ["jane.doe", "acme-client", "acme-corp", "private", "release"] {
        assert!(!msg.contains(leak), "leaked {leak}: {msg}");
    }
    assert!(
        msg.contains("path#") && msg.contains("remote#") && msg.contains("ref#"),
        "{msg}"
    );
    assert_eq!(v["where"], "repo.open", "the emit site is a symbol, kept");
}

#[test]
fn enforcement_reaches_nested_arrays_and_objects() {
    let r = r();
    let mut v = json!({
        "kind": "refresh",
        "scope": "status",
        "paths": ["/home/jane/repos/acme/src/App.tsx", "refs/heads/wip"],
        "detail": { "repo": "C:/work/client-x/.git" }
    });
    enforce(&mut v, &r);
    let text = v.to_string();
    for leak in ["jane", "acme", "wip", "client-x"] {
        assert!(!text.contains(leak), "leaked {leak}: {text}");
    }
}
