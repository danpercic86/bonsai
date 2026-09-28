//! `[HIGH-2]` XML bomb fixtures, generated (never a committed multi-MB file).

/// The classic "billion laughs": 10 entities, each 10 × the previous one.
pub(crate) fn billion_laughs() -> String {
    let mut s = String::from("<?xml version=\"1.0\"?>\n<!DOCTYPE lolz [\n <!ENTITY lol \"lol\">\n");
    for i in 1..10 {
        let prev = if i == 1 {
            "lol".to_string()
        } else {
            format!("lol{}", i - 1)
        };
        let refs = format!("&{prev};").repeat(10);
        s.push_str(&format!(" <!ENTITY lol{i} \"{refs}\">\n"));
    }
    s.push_str("]>\n<configuration><packageSources><add key=\"x\" value=\"&lol9;\"/></packageSources></configuration>\n");
    s
}

/// An external entity (XXE) pointing at a local file.
pub(crate) const XXE: &str = "<?xml version=\"1.0\"?>\n<!DOCTYPE c [<!ENTITY xxe SYSTEM \"file:///etc/passwd\">]>\n<configuration><packageSources><add key=\"x\" value=\"&xxe;\"/></packageSources></configuration>\n";

/// An entity reference with no DTD at all (undeclared).
pub(crate) const UNDECLARED_ENTITY: &str =
    "<configuration><packageSources><add key=\"x\" value=\"https://&corp;/\"/></packageSources></configuration>";

/// The same in element text.
pub(crate) const UNDECLARED_ENTITY_TEXT: &str =
    "<Project><PropertyGroup><RestoreSources>https://&corp;/</RestoreSources></PropertyGroup></Project>";

/// `depth` nested elements `<a><a>…</a></a>`.
pub(crate) fn deep_nesting(depth: usize) -> String {
    format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth))
}

/// One element carrying an attribute of `len` bytes.
pub(crate) fn huge_attribute(len: usize) -> String {
    format!(
        "<configuration><packageSources><add key=\"x\" value=\"https://h.invalid/{}\"/></packageSources></configuration>",
        "a".repeat(len)
    )
}

/// `n` sibling empty elements (n + 2 events).
pub(crate) fn wide(n: usize) -> String {
    format!("<r>{}</r>", "<a/>".repeat(n))
}
