//! Bearer-token minting for the embedded MCP server (P16 §8.2): CSPRNG token
//! generation, base64url encoding, and the persisted-vs-bound-port rotation
//! rule (audit §3.7).

use rand::Rng;

/// Token to serve on the ACTUAL bound port (audit §3.7): reuse the persisted
/// token only when the server came up on the persisted port — the one the
/// user's `claude mcp add` registration targets. When bind fell back to a
/// fresh ephemeral port, a local process squatting the OLD persisted port
/// would receive the client's still-valid bearer token (and could replay it
/// against the new port) — so rotate. The status the frontend shows carries
/// port + token, making the required re-registration visible. A persisted
/// token WITHOUT a persisted port (nothing registered yet) is kept.
pub(crate) fn token_for_bound_port(
    persisted_token: Option<String>,
    persisted_port: Option<u16>,
    actual_port: u16,
) -> String {
    match (persisted_token, persisted_port) {
        (Some(t), Some(p)) if p == actual_port => t,
        (Some(t), None) => t,
        _ => generate_token(),
    }
}

/// 32 CSPRNG bytes, base64url (no padding) — ~256 bits (P16 §8.2).
pub(crate) fn generate_token() -> String {
    let mut buf = [0u8; 32];
    rand::rng().fill_bytes(&mut buf);
    base64url_nopad(&buf)
}

/// Minimal base64url (RFC 4648 §5) encoder, no padding. Avoids a base64 dep for
/// the single token-encoding use.
pub(crate) fn base64url_nopad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 63) as usize] as char);
        }
    }
    out
}
