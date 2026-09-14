//! P69 §3.2 — the RUST half of the settings defaults parity mechanism.
//!
//! The four-link chain this closes:
//!   `UiSettings` (TS interface) → `DEFAULT_UI_SETTINGS` (annotated literal, so a
//!   missing/misspelt key is a COMPILE error) → `src/settings/uiSettingsDefaults.json`
//!   (the checked-in oracle, pinned from the TS side by
//!   `src/settings/defaults.test.ts`) → Rust `Settings::default()` (pinned HERE).
//!
//! Direction matters: the oracle is generated/owned by the TypeScript side and Rust
//! is the side being checked. If this test fails, fix the Rust default (or add the
//! field to `UiSettings` + `ui_settings_of`) — do NOT edit the oracle to match Rust.
//!
//! Its own file rather than an addition to `settings_ui_tests.rs` (396 lines): that
//! file is one soft-limit-sized unit already, and this concern (a cross-language
//! oracle) is separate from its clamping/back-compat subject.
//!
//! Declared with `#[path]` from `lib.rs`, NOT as a child module of `settings` like
//! the sibling `settings_*_tests.rs` files: `settings.rs` is over the file-size
//! ratchet's limit and so may not grow by even the three-line `mod` declaration.
//! Nothing here needs `settings`-private items — it uses the public `Settings` plus
//! the `pub(crate)` `ui_settings_of` projection — so the crate-root home costs
//! nothing.

use crate::settings::Settings;
use serde_json::Value;
use std::collections::BTreeSet;

/// The oracle, embedded at COMPILE time.
///
/// `include_str!` is resolved relative to THIS source file, so the two `../` hops
/// land on the repo root (`src-tauri/src/../../src/...`). That is deliberate: a
/// runtime `std::fs::read` of a relative path would depend on the process's current
/// directory, which is not guaranteed under `cargo nextest` (each test binary runs
/// its own process) or under a workspace-root `cargo test`. Compile-time embedding
/// also makes a moved/renamed oracle a build error instead of a skipped test, and
/// Cargo tracks the file so editing it forces a rebuild.
const ORACLE_JSON: &str = include_str!("../../src/settings/uiSettingsDefaults.json");

/// Recursive, key-naming comparison of two JSON documents.
///
/// A bare `assert_eq!` on two 30-key blobs tells the next person nothing, so every
/// finding names its own dotted path and says which side is which. Missing keys,
/// unexpected keys and per-key value mismatches are reported distinctly, and all of
/// them are collected before failing so one run shows the whole drift.
fn collect_diffs(path: &str, rust: &Value, oracle: &Value, out: &mut Vec<String>) {
    let child_path = |key: &str| {
        if path.is_empty() {
            key.to_string()
        } else {
            format!("{path}.{key}")
        }
    };

    if let (Value::Object(r), Value::Object(o)) = (rust, oracle) {
        let keys: BTreeSet<&String> = r.keys().chain(o.keys()).collect();
        for key in keys {
            let child = child_path(key);
            match (r.get(key), o.get(key)) {
                (Some(rv), Some(ov)) => collect_diffs(&child, rv, ov, out),
                (Some(rv), None) => out.push(format!(
                    "UNEXPECTED key `{child}`: Rust serialises {rv:?}, the oracle has \
                     no such key — add it to the `UiSettings` TS interface + \
                     `DEFAULT_UI_SETTINGS` (which regenerates the oracle), or drop the \
                     field from Rust `UiSettings`"
                )),
                (None, Some(ov)) => out.push(format!(
                    "MISSING key `{child}`: the oracle expects {ov:?}, Rust serialises \
                     no such key — add the field to Rust `UiSettings` + `ui_settings_of`"
                )),
                (None, None) => {}
            }
        }
        return;
    }

    if rust == oracle {
        return;
    }

    let key = if path.is_empty() { "<root>" } else { path };
    let mut message = format!("VALUE mismatch at `{key}`: Rust = {rust:?}, oracle = {oracle:?}");
    // The `0` vs `0.0` trap: `serde_json` treats `Number::PosInt(0)` and
    // `Number::Float(0.0)` as different values, and `{:?}` is the only rendering
    // that shows it. Spell the fix out rather than leaving a baffling `0 != 0`.
    if let (Value::Number(a), Value::Number(b)) = (rust, oracle) {
        if a.as_f64() == b.as_f64() {
            message.push_str(
                " — the numbers are numerically equal but of different JSON number \
                 types (integer vs float). The Rust field type decides: an `f64` \
                 serialises as `0.0`, so the oracle must keep the decimal point. Do \
                 NOT 'tidy' it away",
            );
        }
    }
    out.push(message);
}

/// The serialised Rust default `UiSettings` deep-equals the checked-in oracle.
///
/// There is no `UiSettings::default()` — `UiSettings` is a projection, so the value
/// under test is `ui_settings_of(&Settings::default())`, i.e. exactly what
/// `get_ui_settings` returns for a fresh install.
#[test]
fn rust_default_ui_settings_match_the_shared_oracle() {
    let oracle: Value = serde_json::from_str(ORACLE_JSON).expect(
        "src/settings/uiSettingsDefaults.json parses as JSON (it is a checked-in \
         source file, not repo- or user-derived state)",
    );
    let rust = serde_json::to_value(crate::commands::ui_settings_of(&Settings::default()))
        .expect("UiSettings serialises (derived Serialize over plain scalars/enums/Vec)");

    // Compared as parsed `Value`s, never as text: key order and whitespace in the
    // oracle must not matter.
    let mut diffs = Vec::new();
    collect_diffs("", &rust, &oracle, &mut diffs);
    assert!(
        diffs.is_empty(),
        "Rust `Settings::default()` has drifted from the TS-owned oracle \
         (src/settings/uiSettingsDefaults.json) in {} place(s):\n  - {}",
        diffs.len(),
        diffs.join("\n  - "),
    );
}

/// Guards the `0` vs `0.0` trap at its source: `aiMaxBudgetUsd` is an `f64` in Rust,
/// so the oracle MUST carry a float. JavaScript has one number type, so the TS half
/// of the chain passes either way — only this assertion catches a "tidied" `0`,
/// which would otherwise surface as the baffling `0 != 0` failure above.
#[test]
fn oracle_keeps_float_typed_defaults_as_floats() {
    let oracle: Value =
        serde_json::from_str(ORACLE_JSON).expect("src/settings/uiSettingsDefaults.json parses");
    let budget = oracle
        .get("aiMaxBudgetUsd")
        .expect("oracle has an `aiMaxBudgetUsd` key");
    assert!(
        budget.is_f64(),
        "oracle `aiMaxBudgetUsd` must be written as a float (`0.0`), got {budget:?} — \
         the Rust field is an `f64` and `serde_json` does not equate `0` with `0.0`"
    );
}
