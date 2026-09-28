use super::*;

/// Vectors with scores from the FIRST CVSS v3.1 calculator / specification
/// examples.
#[test]
fn first_v31_vectors() {
    for (v, want) in [
        ("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H", 9.8),
        ("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:C/C:H/I:H/A:H", 10.0),
        ("CVSS:3.1/AV:N/AC:L/PR:L/UI:N/S:C/C:H/I:H/A:H", 9.9),
        ("CVSS:3.1/AV:N/AC:L/PR:H/UI:N/S:U/C:H/I:H/A:H", 7.2),
        ("CVSS:3.1/AV:N/AC:L/PR:N/UI:R/S:C/C:L/I:L/A:N", 6.1),
        ("CVSS:3.1/AV:L/AC:L/PR:L/UI:N/S:U/C:H/I:N/A:N", 5.5),
        ("CVSS:3.1/AV:N/AC:H/PR:N/UI:N/S:U/C:H/I:N/A:N", 5.9),
        ("CVSS:3.1/AV:A/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H", 8.8),
        ("CVSS:3.1/AV:L/AC:L/PR:N/UI:R/S:U/C:H/I:H/A:H", 7.8),
        ("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:L", 5.3),
        ("CVSS:3.1/AV:P/AC:H/PR:H/UI:R/S:U/C:N/I:N/A:N", 0.0),
        ("CVSS:3.0/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H", 9.8),
    ] {
        assert_eq!(base_score_v3(v), Some(want), "{v}");
    }
}

#[test]
fn temporal_metrics_are_ignored() {
    assert_eq!(
        base_score_v3("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H/E:U/RL:O/RC:C"),
        Some(9.8)
    );
}

#[test]
fn malformed_vectors_score_none() {
    for v in [
        "",
        "CVSS:2.0/AV:N/AC:L/Au:N/C:P/I:P/A:P",
        "AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H",
        "CVSS:4.0/AV:N/AC:L/AT:N/PR:N/UI:N/VC:H/VI:H/VA:H/SC:N/SI:N/SA:N",
        "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H",
        "CVSS:3.1/AV:X/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H",
        "CVSS:3.1/AV:N/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H",
        "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A",
    ] {
        assert_eq!(base_score_v3(v), None, "{v}");
    }
}

#[test]
fn bands() {
    assert_eq!(band(10.0), Severity::Critical);
    assert_eq!(band(9.0), Severity::Critical);
    assert_eq!(band(8.9), Severity::High);
    assert_eq!(band(7.0), Severity::High);
    assert_eq!(band(6.9), Severity::Medium);
    assert_eq!(band(4.0), Severity::Medium);
    assert_eq!(band(3.9), Severity::Low);
    assert_eq!(band(0.1), Severity::Low);
    assert_eq!(band(0.0), Severity::Unknown);
}
