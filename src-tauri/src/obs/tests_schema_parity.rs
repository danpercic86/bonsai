//! P91 §3 — the cross-language guard on the record-schema version.
//!
//! The chain this closes:
//!   [`record::OBS_SCHEMA_VERSION`] (the constant) → `writer.rs` stamps it into
//!   every `session` header, so it is the number that actually reaches disk →
//!   `src/ipc/types/obs.ts` re-declares it for the UI (and `src/obs/types.ts`
//!   re-exports that). The two sides share no build step, so nothing but this
//!   test makes moving one of them red.
//!
//! Direction matters: **Rust is authoritative.** A reader tells a v1 corpus from
//! a v2 one by the header Rust wrote, never by the TS literal — which today has
//! no consumers at all and is therefore pure documentation that can silently rot.
//! If this test fails, edit the TypeScript file to match; do NOT move the Rust
//! constant to match TypeScript. (The `= 1` this was born from drifted for six
//! days after the 2026-09-16 v1 → v2 bump, undetected, for exactly that reason.)
//!
//! Its own file rather than an addition to `record.rs` (503 lines, already over
//! the soft limit) or `tests_record.rs` (narrowly the `changedProps` three-state
//! serde contract): a cross-language parity guard is a separate concern, the same
//! reason `settings_defaults_parity_tests.rs` stands alone.
//!
//! `include_str!`, not `std::fs::read`: the test's cwd is not guaranteed under
//! `cargo nextest`, embedding at compile time turns a moved/renamed TS file into
//! a build error instead of a runtime skip, and Cargo tracks the file so editing
//! it rebuilds this test.

use super::record::OBS_SCHEMA_VERSION;

/// The TS mirror must declare the exact value Rust writes into the header.
#[test]
fn ts_mirror_matches_the_rust_schema_version() {
    const TS: &str = include_str!("../../../src/ipc/types/obs.ts");
    // Comment lines are stripped first: the TS doc block names this constant (and
    // this test), so a prose mention must never be able to satisfy the assertion.
    let code: String = TS
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let literal = format!("export const OBS_SCHEMA_VERSION = {OBS_SCHEMA_VERSION};");
    assert_eq!(
        code.matches(literal.as_str()).count(),
        1,
        "src/ipc/types/obs.ts must declare the schema version exactly once, as {literal:?} — \
         Rust owns this value (obs::record::OBS_SCHEMA_VERSION is what writer.rs emits into \
         every session header), so the fix is to edit the TypeScript file, NOT the Rust constant"
    );
}
