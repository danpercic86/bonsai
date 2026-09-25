//! P121b — the cross-language guard on the `repo-changed` reason strings.
//!
//! [`super::repo::repo_changed_reason::ALL`] is the authoritative list of wire
//! values Rust emits in `RepoChangedPayload.reason`; `src/ipc/types/common.ts`
//! mirrors it as `REPO_CHANGED_REASONS`, which the frontend router
//! (`useRepoChangeSubscription.ts`) is tested against. The two sides share no
//! build step, so nothing but this test makes adding a reason on one side only
//! go red — and a reason missing from the TS router silently falls into its
//! `default:` full refresh (the exact P121b cascade).
//!
//! **Rust is authoritative.** If this fails, edit the TypeScript file to match.
//! Same pattern (and the same `include_str!` rationale) as
//! `obs/tests_schema_parity.rs`.

use super::repo::repo_changed_reason;

#[test]
fn ts_mirror_matches_the_rust_repo_changed_reasons() {
    const TS: &str = include_str!("../../../src/ipc/types/common.ts");
    // Strip comment lines first so a prose mention can never satisfy the check,
    // then drop ALL whitespace so formatter line-wrapping is irrelevant.
    let code: String = TS
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
        })
        .collect::<String>()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        // A Prettier-wrapped array gains a trailing comma; normalise it away.
        .replace(",]", "]");
    let items: Vec<String> = repo_changed_reason::ALL
        .iter()
        .map(|r| format!("'{r}'"))
        .collect();
    let literal = format!(
        "exportconstREPO_CHANGED_REASONS=[{}]asconst;",
        items.join(",")
    );
    assert_eq!(
        code.matches(literal.as_str()).count(),
        1,
        "src/ipc/types/common.ts must declare REPO_CHANGED_REASONS exactly once, as \
         `export const REPO_CHANGED_REASONS = [{}] as const;` (whitespace-insensitive) — Rust \
         owns this list (commands::repo::repo_changed_reason::ALL is what the emit sites send), \
         so the fix is to edit the TypeScript file, NOT the Rust constants",
        items.join(", ")
    );
}
