//! P34 stash-scope matrix for the NATIVE scopes (`all` / `allWithUntracked`)
//! plus the scope-union wire mapping. Extracted verbatim from the former
//! inline `mod tests`; the `staged` fold rows live in `tests_staged`, shared
//! fixtures in `test_support`.

use super::test_support::*;
use super::*;

// ======================================================= P34 stash scopes
// Behavioral matrix for StashScope (all / allWithUntracked / staged), the new
// data-safety-critical `staged` path (FOLD semantics — orchestrator override:
// mixed staged+unstaged files are folded WHOLE, not rejected), and the
// stacking / rm --cached regression guards. Each test asserts BOTH the
// CreateStashResult/outcome AND the resulting repo state (index, worktree,
// stash stack). Fixtures reuse the s9_* helpers above.

// ---- AC-9: wire mapping of the scope union --------------------------------

#[test]
fn p34_scope_deserializes_from_camel_case() {
    use serde_json::{from_value, json};
    assert_eq!(
        from_value::<StashScope>(json!("all")).expect("all"),
        StashScope::All
    );
    assert_eq!(
        from_value::<StashScope>(json!("allWithUntracked")).expect("allWithUntracked"),
        StashScope::AllWithUntracked
    );
    assert_eq!(
        from_value::<StashScope>(json!("staged")).expect("staged"),
        StashScope::Staged
    );
    assert!(
        from_value::<StashScope>(json!("bogus")).is_err(),
        "unknown scope must not deserialize"
    );
}

// ---- Case 1: `All` == old DEFAULT behavior --------------------------------

#[test]
fn p34_all_stashes_tracked_leaves_untracked() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n"), ("b.txt", "b-base\n")]);

    // staged modify (a), unstaged modify (b), untracked (u).
    std::fs::write(d.join("a.txt"), "a-staged\n").expect("edit a");
    p34_stage(d, &["a.txt"]);
    std::fs::write(d.join("b.txt"), "b-unstaged\n").expect("edit b");
    std::fs::write(d.join("u.txt"), "untracked\n").expect("write u");

    let res = create_stash(d, None, StashScope::All).expect("create_stash all");
    assert!(res.created, "tracked changes must stash");

    // Tracked worktree changes gone; untracked survives.
    assert_eq!(s9_read(d, "a.txt"), "base\n", "staged a reverted");
    assert_eq!(s9_read(d, "b.txt"), "b-base\n", "unstaged b reverted");
    assert!(d.join("u.txt").exists(), "untracked left in place");
    p34_assert_index_clean(d);
    assert_eq!(list_stashes(d).expect("list").len(), 1, "one entry");
}

// ---- Case 2: `AllWithUntracked` also captures untracked -------------------

#[test]
fn p34_all_with_untracked_captures_untracked() {
    let dir = crate::testutil::scratch_dir();
    let d = dir.path();
    s9_init(d);
    s9_commit(d, "base", &[("a.txt", "base\n")]);

    std::fs::write(d.join("a.txt"), "a-staged\n").expect("edit a");
    p34_stage(d, &["a.txt"]);
    std::fs::write(d.join("u.txt"), "untracked\n").expect("write u");

    let res = create_stash(d, None, StashScope::AllWithUntracked).expect("create_stash");
    assert!(res.created);
    assert_eq!(s9_read(d, "a.txt"), "base\n", "tracked reverted");
    assert!(
        !d.join("u.txt").exists(),
        "allWithUntracked must sweep the untracked file"
    );
    p34_assert_index_clean(d);
    assert_eq!(list_stashes(d).expect("list").len(), 1);

    // Round-trip restores the untracked file too.
    let outcome = pop_stash(d, 0, false, None).expect("pop");
    assert_eq!(outcome, ApplyStashOutcome::Applied);
    assert!(d.join("u.txt").exists(), "untracked restored on pop");
    assert_eq!(s9_read(d, "u.txt"), "untracked\n");
    assert_eq!(list_stashes(d).expect("list").len(), 0, "clean pop drops");
}

// ---- Case 3: nothing to stash, per scope ----------------------------------

#[test]
fn p34_nothing_to_stash_each_scope() {
    for scope in [
        StashScope::All,
        StashScope::AllWithUntracked,
        StashScope::Staged,
    ] {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);
        let head = s9_head_oid(d);

        let res = create_stash(d, None, scope).unwrap_or_else(|e| panic!("{scope:?}: {e:?}"));
        assert!(!res.created, "{scope:?}: clean tree -> created:false");
        assert_eq!(
            list_stashes(d).expect("list").len(),
            0,
            "{scope:?}: no entry pushed"
        );
        assert_eq!(s9_head_oid(d), head, "{scope:?}: HEAD unchanged");
        assert_eq!(
            s9_read(d, "a.txt"),
            "base\n",
            "{scope:?}: worktree unchanged"
        );
    }
}
