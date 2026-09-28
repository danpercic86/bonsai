//! `[HIGH-2]` TOML nesting bombs for the §4.2 pre-scan tests.

/// `a = [[[…]]]` with `depth` nested arrays.
pub(crate) fn nested_arrays(depth: usize) -> String {
    format!("a = {}{}\n", "[".repeat(depth), "]".repeat(depth))
}

/// `a = {b = {b = …}}` with `depth` nested inline tables.
pub(crate) fn nested_inline_tables(depth: usize) -> String {
    let mut s = String::from("a = ");
    for _ in 0..depth {
        s.push_str("{b = ");
    }
    s.push('1');
    s.push_str(&"}".repeat(depth));
    s.push('\n');
    s
}

/// `a.a.a…a = 1`: `dots` + 1 nested tables from a dotted key.
pub(crate) fn dotted_key(dots: usize) -> String {
    format!("{} = 1\n", vec!["a"; dots + 1].join("."))
}

/// `[a.a.…a]`: a table header with `dots` + 1 segments.
pub(crate) fn dotted_header(dots: usize) -> String {
    format!("[{}]\nx = 1\n", vec!["a"; dots + 1].join("."))
}

/// A Cargo.toml whose dependency table hides an unclosed bracket bomb.
pub(crate) fn cargo_manifest_bomb(depth: usize) -> String {
    format!(
        "[package]\nname = \"bomb\"\n\n[dependencies]\nserde = \"1\"\nx = {{ features = {} }}\n",
        "[".repeat(depth)
    )
}
