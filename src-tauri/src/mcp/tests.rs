use super::*;

#[test]
fn base64url_encodes_known_vectors() {
    // RFC 4648 test vectors, url-safe, no padding.
    assert_eq!(base64url_nopad(b""), "");
    assert_eq!(base64url_nopad(b"f"), "Zg");
    assert_eq!(base64url_nopad(b"fo"), "Zm8");
    assert_eq!(base64url_nopad(b"foo"), "Zm9v");
    assert_eq!(base64url_nopad(b"foob"), "Zm9vYg");
    assert_eq!(base64url_nopad(b"fooba"), "Zm9vYmE");
    assert_eq!(base64url_nopad(b"foobar"), "Zm9vYmFy");
    // url-safe alphabet uses '-' and '_' (0xfb,0xff -> "-_" region).
    assert_eq!(base64url_nopad(&[0xff, 0xff, 0xff]), "____");
    assert_eq!(base64url_nopad(&[0xfb, 0xff, 0xbf]), "-_-_");
}

#[test]
fn generated_token_is_43_chars_no_padding() {
    // 32 bytes -> ceil(32/3)*4 = 44 raw, minus 1 for the last partial group.
    let t = generate_token();
    assert_eq!(t.len(), 43, "32 bytes base64url (no pad) is 43 chars: {t}");
    assert!(!t.contains('='));
    assert!(t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
}

#[test]
fn two_generated_tokens_differ() {
    assert_ne!(generate_token(), generate_token());
}

#[test]
fn tool_count_reflects_write_gate() {
    assert_eq!(tool_count(false), 14);
    assert_eq!(tool_count(true), 34);
}

/// Audit §3.7: the persisted token is reused ONLY when the actual bound
/// port matches the persisted one; an ephemeral-port fallback rotates it.
#[test]
fn token_rotates_when_bound_port_differs_from_persisted() {
    let tok = "persisted-token".to_string();
    // Same port → reuse.
    assert_eq!(
        token_for_bound_port(Some(tok.clone()), Some(8765), 8765),
        tok
    );
    // Ephemeral fallback (different port) → fresh token.
    let rotated = token_for_bound_port(Some(tok.clone()), Some(8765), 49152);
    assert_ne!(rotated, tok);
    assert_eq!(rotated.len(), 43, "fresh CSPRNG token");
    // No persisted port (nothing registered yet) → keep the token.
    assert_eq!(token_for_bound_port(Some(tok.clone()), None, 49152), tok);
    // No persisted token at all → generate.
    assert_eq!(token_for_bound_port(None, Some(8765), 8765).len(), 43);
}
