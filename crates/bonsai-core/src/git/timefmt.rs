//! Tiny date-formatting helpers shared by the AI grounding features — no chrono
//! dependency. Promoted out of `ai_explain.rs` (P53 OQ8) so `ai_line.rs` can
//! render the `YYYY-MM-DD` author date in its blame-why grounding without
//! duplicating the civil-from-days conversion.

/// Formats an epoch-seconds timestamp as `YYYY-MM-DD` (UTC). Civil-from-days
/// algorithm (Howard Hinnant) — no chrono dependency.
pub fn epoch_to_ymd(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sanity for the no-chrono civil-date conversion (moved with the fn from
    /// `ai_explain.rs`).
    #[test]
    fn epoch_to_ymd_known_dates() {
        assert_eq!(epoch_to_ymd(0), "1970-01-01");
        assert_eq!(epoch_to_ymd(1_767_225_600), "2026-01-01"); // 2026-01-01T00:00:00Z
        assert_eq!(epoch_to_ymd(951_782_400), "2000-02-29"); // leap day
    }
}
