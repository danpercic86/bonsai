//! CVSS v3.x base score (FIRST CVSS v3.1 specification §7.1 + Appendix A
//! `Roundup`), and the §5.1 score → severity band.
//!
//! Only the eight base metrics are read; temporal / environmental metrics
//! are ignored. A vector missing a base metric, repeating one, or carrying
//! an unknown value ⇒ `None`. v3.0 vectors are scored with the same
//! formula (the v3.1 changes are to Roundup precision only).

use crate::model::Severity;

/// FIRST v3.1 Appendix A: the smallest number, to one decimal, ≥ `x`,
/// computed on integers to avoid float artefacts.
fn roundup(x: f64) -> f64 {
    let int_input = (x * 100_000.0).round() as i64;
    if int_input % 10_000 == 0 {
        int_input as f64 / 100_000.0
    } else {
        ((int_input / 10_000) as f64 + 1.0) / 10.0
    }
}

#[derive(Default)]
struct Base {
    av: Option<f64>,
    ac: Option<f64>,
    pr: Option<char>,
    ui: Option<f64>,
    scope_changed: Option<bool>,
    c: Option<f64>,
    i: Option<f64>,
    a: Option<f64>,
}

fn cia(v: &str) -> Option<f64> {
    match v {
        "H" => Some(0.56),
        "L" => Some(0.22),
        "N" => Some(0.0),
        _ => None,
    }
}

fn set<T>(slot: &mut Option<T>, v: Option<T>) -> Option<()> {
    if slot.is_some() {
        return None;
    }
    *slot = Some(v?);
    Some(())
}

fn parse(vector: &str) -> Option<Base> {
    let mut parts = vector.trim().split('/');
    match parts.next()? {
        "CVSS:3.0" | "CVSS:3.1" => {}
        _ => return None,
    }
    let mut b = Base::default();
    for part in parts {
        let (k, v) = part.split_once(':')?;
        match k {
            "AV" => set(
                &mut b.av,
                match v {
                    "N" => Some(0.85),
                    "A" => Some(0.62),
                    "L" => Some(0.55),
                    "P" => Some(0.2),
                    _ => None,
                },
            )?,
            "AC" => set(
                &mut b.ac,
                match v {
                    "L" => Some(0.77),
                    "H" => Some(0.44),
                    _ => None,
                },
            )?,
            "PR" => set(
                &mut b.pr,
                match v {
                    "N" | "L" | "H" => v.chars().next(),
                    _ => None,
                },
            )?,
            "UI" => set(
                &mut b.ui,
                match v {
                    "N" => Some(0.85),
                    "R" => Some(0.62),
                    _ => None,
                },
            )?,
            "S" => set(
                &mut b.scope_changed,
                match v {
                    "U" => Some(false),
                    "C" => Some(true),
                    _ => None,
                },
            )?,
            "C" => set(&mut b.c, cia(v))?,
            "I" => set(&mut b.i, cia(v))?,
            "A" => set(&mut b.a, cia(v))?,
            // Temporal / environmental metrics: not part of the base score.
            _ => {}
        }
    }
    Some(b)
}

/// The CVSS v3.x base score of `vector`, or None if it is not a complete,
/// well-formed v3.0 / v3.1 base vector.
pub fn base_score_v3(vector: &str) -> Option<f32> {
    let b = parse(vector)?;
    let changed = b.scope_changed?;
    let pr = match (b.pr?, changed) {
        ('N', _) => 0.85,
        ('L', false) => 0.62,
        ('L', true) => 0.68,
        ('H', false) => 0.27,
        ('H', true) => 0.5,
        _ => return None,
    };
    let iss = 1.0 - (1.0 - b.c?) * (1.0 - b.i?) * (1.0 - b.a?);
    let impact = if changed {
        7.52 * (iss - 0.029) - 3.25 * (iss - 0.02).powi(15)
    } else {
        6.42 * iss
    };
    let exploitability = 8.22 * b.av? * b.ac? * pr * b.ui?;
    let score = if impact <= 0.0 {
        0.0
    } else if changed {
        roundup((1.08 * (impact + exploitability)).min(10.0))
    } else {
        roundup((impact + exploitability).min(10.0))
    };
    Some(score as f32)
}

/// §5.1 band: ≥ 9 Critical, ≥ 7 High, ≥ 4 Medium, > 0 Low, else Unknown.
pub fn band(score: f32) -> Severity {
    if score >= 9.0 {
        Severity::Critical
    } else if score >= 7.0 {
        Severity::High
    } else if score >= 4.0 {
        Severity::Medium
    } else if score > 0.0 {
        Severity::Low
    } else {
        Severity::Unknown
    }
}

#[cfg(test)]
#[path = "cvss_tests.rs"]
mod tests;
