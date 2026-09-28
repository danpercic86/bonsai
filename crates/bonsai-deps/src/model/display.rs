//! `clean_display` (contract §7.5, `[LOW-3]`): strip Unicode Cc + Cf, then
//! truncate to `max` chars.

/// Limit for every repo- or response-derived display string (`name`,
/// `declared`, `current`, `latest`, `manifest_path`, `lockfile_path`,
/// `ParseError.file`, `registry_host`).
pub const MAX_DISPLAY: usize = 256;
/// Limit for an advisory `summary`.
pub const MAX_SUMMARY_DISPLAY: usize = 300;
/// Limit for advisory ids and aliases.
pub const MAX_ID_DISPLAY: usize = 64;

/// Unicode general category Cf (format characters), Unicode 16. Inclusive
/// ranges, ascending. This covers the bidi overrides U+202A–202E and isolates
/// U+2066–2069, zero-width characters and the BOM. std has no general-category
/// API, and the contract names no Unicode crate, so the table lives here.
const CF_RANGES: &[(u32, u32)] = &[
    (0x00AD, 0x00AD),
    (0x0600, 0x0605),
    (0x061C, 0x061C),
    (0x06DD, 0x06DD),
    (0x070F, 0x070F),
    (0x0890, 0x0891),
    (0x08E2, 0x08E2),
    (0x180E, 0x180E),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x2064),
    (0x2066, 0x206F),
    (0xFEFF, 0xFEFF),
    (0xFFF9, 0xFFFB),
    (0x110BD, 0x110BD),
    (0x110CD, 0x110CD),
    (0x13430, 0x1343F),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0001, 0xE0001),
    (0xE0020, 0xE007F),
];

fn is_format_char(c: char) -> bool {
    let cp = c as u32;
    CF_RANGES.iter().any(|&(lo, hi)| (lo..=hi).contains(&cp))
}

/// Strips Cc (`char::is_control`) and Cf characters, then keeps at most `max`
/// chars (not bytes).
pub fn clean_display(s: &str, max: usize) -> String {
    s.chars()
        .filter(|&c| !c.is_control() && !is_format_char(c))
        .take(max)
        .collect()
}
