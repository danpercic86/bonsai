//! P91 §7 redaction tests — the increment-1 acceptance criteria for the
//! `Redactor` (stable ordinals within a session, different ordinals across
//! sessions) and for the credential scrubber (every fixture pattern caught).

use super::redact::{scrub_string, scrub_value, Kind, Redactor, REDACTED_TOKEN};
use serde_json::json;

const SALT_A: [u8; 16] = [1; 16];
const SALT_B: [u8; 16] = [9; 16];

#[test]
fn ordinals_are_stable_within_a_session() {
    let r = Redactor::with_salt(SALT_A);
    let first = r.tag(Kind::Ref, "refs/heads/main");
    let other = r.tag(Kind::Ref, "refs/heads/feature");
    assert_eq!(r.tag(Kind::Ref, "refs/heads/main"), first);
    assert_eq!(r.tag(Kind::Ref, "refs/heads/feature"), other);
    assert_ne!(first, other, "distinct refs must get distinct ordinals");
    assert!(first.starts_with("ref#"), "unexpected tag shape: {first}");
}

#[test]
fn ordinal_namespaces_are_independent_per_kind() {
    let r = Redactor::with_salt(SALT_A);
    let a = r.tag(Kind::Ref, "same-string");
    let b = r.tag(Kind::Path, "same-string");
    assert!(a.starts_with("ref#") && b.starts_with("path#"));
}

/// §7.2's cross-session requirement: the SAME value, seen in the SAME order,
/// must not produce the same ordinal in a different session — otherwise two log
/// files could be cross-linked or dictionary-attacked.
#[test]
fn ordinals_differ_across_sessions() {
    let today = Redactor::with_salt(SALT_A);
    let tomorrow = Redactor::with_salt(SALT_B);
    assert_ne!(
        today.tag(Kind::Ref, "refs/heads/main"),
        tomorrow.tag(Kind::Ref, "refs/heads/main"),
    );
}

#[test]
fn salt_is_hex_and_session_specific() {
    let a = Redactor::with_salt(SALT_A);
    assert_eq!(a.salt_hex().len(), 32);
    assert!(a.salt_hex().chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(Redactor::new().salt_hex(), Redactor::new().salt_hex());
}

#[test]
fn tag_path_keeps_only_the_extension() {
    let r = Redactor::with_salt(SALT_A);
    let tag = r.tag_path("C:/work/secret-project/src/App.tsx");
    assert!(tag.starts_with("path#") && tag.ends_with(".tsx"), "{tag}");
    assert!(!tag.contains("secret-project"));
    // A dotfile's name is not an extension.
    let dotfile = r.tag_path("/repo/.gitignore");
    assert!(!dotfile.contains('.'), "{dotfile}");
}

#[test]
fn args_hash_is_8_hex_stable_and_salt_dependent() {
    let a = Redactor::with_salt(SALT_A);
    let b = Redactor::with_salt(SALT_B);
    let h = a.hash_args(r#"{"repoId":"/x"}"#);
    assert_eq!(h.len(), 8);
    assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(
        h,
        a.hash_args(r#"{"repoId":"/x"}"#),
        "must be deterministic"
    );
    assert_ne!(h, a.hash_args(r#"{"repoId":"/y"}"#), "content must matter");
    assert_ne!(h, b.hash_args(r#"{"repoId":"/x"}"#), "salt must matter");
}

/// §12 row 1: "token scrubber catches every fixture pattern".
///
/// The list is written from the THREAT MODEL, not from the implementation: it
/// covers the credential shapes a Git client plausibly meets (GitLab and Azure
/// DevOps as much as GitHub), the two auth-header schemes, and the awkward
/// base64 case whose alphabet contains `/`. A fixture list that only restated
/// `TOKEN_PREFIXES` would prove nothing.
#[test]
fn scrubber_catches_every_credential_fixture() {
    let fixtures = [
        "ghp_1234567890abcdefghijklmnopqrstuvwxyzAB",
        "gho_1234567890abcdefghijklmnopqrstuvwxyzAB",
        "ghu_1234567890abcdefghijklmnopqrstuvwxyzAB",
        "ghs_1234567890abcdefghijklmnopqrstuvwxyzAB",
        "ghr_1234567890abcdefghijklmnopqrstuvwxyzAB",
        "github_pat_11ABCDEFG0abcdefghijkl_ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "xoxb-1234-5678-abcdefghijklmnop",
        "xoxp-1234-5678-abcdefghijklmnop",
        // GitLab: personal / deploy / runner tokens. 26 chars — below any
        // generic length floor, so ONLY the prefix rule can catch these.
        "glpat-abcdefghij1234567890",
        "gldt-abcdefghij1234567890",
        "glrt-abcdefghij1234567890",
        // npm automation token.
        "npm_abcdefghijklmnopqrstuvwxyz0123456789AB",
        // AWS access-key ids (20 chars).
        "AKIAIOSFODNN7EXAMPLE",
        "ASIAIOSFODNN7EXAMPLE",
        // Google API key.
        "AIzaSyA1234567890abcdefghijklmnopqrstuv",
        // OpenAI-style secret key.
        "sk-proj-abcdefghij1234567890abcdefghij",
        // Docker Hub PAT.
        "dckr_pat_abcdefghij1234567890",
        // Azure DevOps PAT shape: 52 base32 chars.
        "abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrst",
        // ... and the mixed-case/base64 Azure variant, which is NOT base32.
        "Zm9vYmFyMTIzNDU2Nzg5MFFXRVJUWXVpb3BBU0RGR0hqa2w0NTY3",
        // Base64 containing `/` and `+` — §7.2's "base64 PAT shape". The
        // slash-bearing case is the one an over-eager "it has a slash, it must be
        // a path" rule lets through.
        "aGVsbG8x/d29ybGQyMzQ1Njc4OTBhYmNkZWZnaGlqa2xtbm9w+cQ==",
        "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.payload",
        // Basic auth: only ~24 chars of base64, so the length floor cannot see
        // it — the `Basic` keyword is what establishes intent.
        "Authorization: Basic am9lOnN1cGVyc2VjcmV0MTIz",
        "https://user:sup3rs3cret@github.com/org/repo.git",
        "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaA==\n-----END OPENSSH PRIVATE KEY-----",
    ];
    for f in fixtures {
        let out = scrub_string(f, None);
        assert!(
            out.contains(REDACTED_TOKEN),
            "credential fixture survived scrubbing: {f} -> {out}"
        );
    }
    // And the secret bytes themselves are gone, not merely annotated.
    assert!(!scrub_string(fixtures[0], None).contains("1234567890abcdef"));
    assert!(!scrub_string("glpat-abcdefghij1234567890", None).contains("abcdefghij"));
    assert!(
        !scrub_string("Authorization: Basic am9lOnN1cGVyc2VjcmV0MTIz", None).contains("am9lOn")
    );
    assert!(
        !scrub_string("https://user:sup3rs3cret@github.com/org/repo.git", None)
            .contains("sup3rs3cret")
    );
}

/// The `Basic` keyword occurs in prose; swallowing the word after it there would
/// corrupt a message for no privacy gain. The shape guard is what separates the
/// two cases, so both directions are pinned.
#[test]
fn basic_only_swallows_credential_shaped_material() {
    let prose = "basic auth is disabled for this remote";
    assert_eq!(scrub_string(prose, None), prose);
    let header = "Basic dXNlcjpwYXNzd29yZDEyMw==";
    assert!(scrub_string(header, None).contains(REDACTED_TOKEN));
}

/// §7.2.1 increment-3 additions: a bare JWT (no `Bearer` keyword) and an
/// in-string `key=value`/`key: value` credential pair are both scrubbed, while a
/// 40-char SHA and a long real path stay untouched.
#[test]
fn increment3_layer_a_jwt_and_keyvalue() {
    // Bare JWT that lost its `Bearer ` keyword — the `.` disqualifies the opaque
    // shape, so only the `eyJ` rule catches it.
    let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NSJ9.SflKxwRJSMeKKF2QT4fwpM";
    let out = scrub_string(jwt, None);
    assert!(out.contains(REDACTED_TOKEN), "bare JWT survived: {out}");
    assert!(
        !out.contains("SflKxwRJSMeKKF2QT4fwpM"),
        "JWT signature leaked: {out}"
    );

    // Plain-text credential-helper lines: `password=…` and `password: …`.
    for line in [
        "protocol=https\nhost=github.com\nusername=joe\npassword=hunter2secret",
        "password: s3cr3t-value-here",
        "the token=abc123def456 was rejected",
    ] {
        let o = scrub_string(line, None);
        assert!(
            o.contains(REDACTED_TOKEN),
            "credential pair survived: {line} -> {o}"
        );
    }
    assert!(!scrub_string("password=hunter2secret", None).contains("hunter2secret"));

    // Negatives: a 40-char SHA and a long path have no sensitive key and must be
    // left alone (also pinned in `scrubber_keeps_what_the_contract_says_to_keep`).
    let sha = "9f2a1c4d5e6f70819293a4b5c6d7e8f901234567";
    assert_eq!(scrub_string(sha, None), sha);
    let path = "/home/developer/projects/bonsai/src/components/settings/categories";
    assert_eq!(scrub_string(path, None), path);
    // The `basic`-in-prose asymmetry must survive unchanged.
    let prose = "basic auth is disabled for this remote";
    assert_eq!(scrub_string(prose, None), prose);
}

#[test]
fn scrubber_keeps_what_the_contract_says_to_keep() {
    // §7.1: commit SHAs are retained — a 40-hex word is not a credential.
    let sha = "9f2a1c4d5e6f70819293a4b5c6d7e8f901234567";
    assert_eq!(scrub_string(sha, None), sha);
    // Ordinary prose, ref names, and plain URLs survive untouched.
    for s in [
        "refresh round 3 collapsed 2 traces",
        "refs/remotes/origin/feature/log-core",
        "https://github.com/org/repo.git",
        "src/components/Sidebar.tsx",
        // A long, deep path with no extension: the base64-vs-path discriminator
        // is segment length, and no realistic path segment reaches 24 chars.
        "/home/developer/projects/bonsai/src/components/settings/categories",
    ] {
        assert_eq!(scrub_string(s, None), s, "over-redacted: {s}");
    }
}

#[test]
fn sensitive_keys_lose_their_value_whatever_its_shape() {
    let mut v = json!({
        "cmd": "forge_set_token_for_host",
        "token": "plain-looking",
        "userPassword": "hunter2",
        "authHeader": "whatever",
        "nested": { "clientSecret": "s3cr3t", "keep": "visible" },
        "list": ["ghp_1234567890abcdefghijklmnopqrstuvwxyzAB", "fine"]
    });
    scrub_value(&mut v, None);
    assert_eq!(v["token"], json!(REDACTED_TOKEN));
    assert_eq!(v["userPassword"], json!(REDACTED_TOKEN));
    assert_eq!(v["authHeader"], json!(REDACTED_TOKEN));
    assert_eq!(v["nested"]["clientSecret"], json!(REDACTED_TOKEN));
    assert_eq!(v["nested"]["keep"], json!("visible"));
    assert_eq!(v["cmd"], json!("forge_set_token_for_host"));
    assert_eq!(v["list"][0], json!(REDACTED_TOKEN));
    assert_eq!(v["list"][1], json!("fine"));
}

// ---------------------------------------------------------- cross-side vectors

/// The salt the mock IPC layer reports (`src/ipc/mock/handlers/obs.ts`), so a
/// harness run and this test hash identically.
const SALT_MOCK: [u8; 16] = [
    0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
];

/// §7.2 — `argsHash` is the ONE token both sides compute identically, so these
/// vectors are the contract between `obs/redact.rs` and `src/obs/redact.ts`.
/// They are generated HERE and hardcoded in `src/obs/redact.test.ts`; a TS test
/// checking TS output would prove nothing.
///
/// Coverage is chosen for the places canonicalization silently diverges:
/// UTF-8 vs UTF-16 (the non-ASCII vector), integer-vs-float formatting (`1` is
/// canonical — `JSON.stringify(1.0)` is `"1"`, and the TS canonicalizer is the
/// authority for the wire text), key ordering, and nesting.
pub(crate) const ARGS_HASH_VECTORS: &[(&str, &str)] = &[
    ("[]", "2375e1e2"),
    ("[{}]", "c673fb57"),
    (r#"["/repo/one",{"path":"src/app.ts"}]"#, "05c8197b"),
    (r#"[{"a":1,"b":"zwei"}]"#, "2502ec22"),
    (r#"[{"msg":"héllo — ünïcode"}]"#, "885d657a"),
    (r#"[{"n":1,"f":1.5,"t":true,"z":null}]"#, "ddfacfa0"),
    (r#"[[1,2,[3,"x"]],"<fn>"]"#, "d2d06e48"),
];

#[test]
fn args_hash_cross_side_vectors() {
    let r = Redactor::with_salt(SALT_MOCK);
    for (canonical, expected) in ARGS_HASH_VECTORS {
        let got = r.hash_args(canonical);
        println!("VECTOR {canonical} => {got}");
        assert_eq!(&got, expected, "vector drift for {canonical}");
    }
}
