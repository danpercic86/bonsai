//! `[HIGH-2]` YAML bomb fixtures, generated (never a committed multi-MB file).

/// The classic "billion laughs": 9 levels of 9 aliases each.
pub(crate) fn alias_bomb() -> String {
    let mut s = String::from(
        "a0: &a0 [\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\",\"lol\"]\n",
    );
    for i in 1..10 {
        let p = i - 1;
        s.push_str(&format!(
            "a{i}: &a{i} [*a{p},*a{p},*a{p},*a{p},*a{p},*a{p},*a{p},*a{p},*a{p}]\n"
        ));
    }
    s
}

/// An anchor that is never used.
pub(crate) const ANCHOR_ONLY: &str = "lockfileVersion: '9.0'\nimporters: &imp\n  .: {}\n";

/// A lone alias with no anchor (the parser itself may reject it first).
pub(crate) const ALIAS_ONLY: &str = "a: 1\nb: *a\n";

/// `depth` nested block mappings: `k: \n  k: \n ...`.
pub(crate) fn block_nesting(depth: usize) -> String {
    let mut s = String::new();
    for i in 0..depth {
        s.push_str(&"  ".repeat(i));
        s.push_str("k:\n");
    }
    s.push_str(&"  ".repeat(depth));
    s.push_str("v\n");
    s
}

/// `depth` nested flow sequences `[[[…]]]`.
pub(crate) fn flow_nesting(depth: usize) -> String {
    format!("{}{}", "[".repeat(depth), "]".repeat(depth))
}

/// A flow sequence of `n` scalars (n + 1 nodes).
pub(crate) fn wide_seq(n: usize) -> String {
    let mut s = String::with_capacity(n * 2 + 2);
    s.push('[');
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        s.push('1');
    }
    s.push(']');
    s
}
