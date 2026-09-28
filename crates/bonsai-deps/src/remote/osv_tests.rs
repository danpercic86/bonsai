use super::*;

#[test]
fn mint_id_accepts_osv_shapes() {
    for id in [
        "GHSA-jfh8-c2jp-5v3q",
        "RUSTSEC-2021-0001",
        "GO-2022-0001",
        "PYSEC-2021-1",
        "CVE-2021-44228",
        "a",
        "OSV:x.y_z",
    ] {
        assert_eq!(mint_id(id).as_ref().map(AdvisoryId::as_str), Some(id));
    }
    assert!(mint_id(&format!("A{}", "b".repeat(63))).is_some());
}

#[test]
fn mint_id_rejects_everything_else() {
    for id in [
        "",
        "-GHSA",
        ".x",
        "GHSA/../x",
        "a b",
        "a?b=c",
        "a#b",
        "a%2F",
        "é",
        "x\u{202E}",
    ] {
        assert!(mint_id(id).is_none(), "{id:?}");
    }
    assert!(mint_id(&format!("A{}", "b".repeat(64))).is_none());
}

#[test]
fn mint_page_token_bounds() {
    assert!(mint_page_token("abc/DEF+=").is_some());
    assert!(mint_page_token(&"x".repeat(1024)).is_some());
    assert!(mint_page_token(&"x".repeat(1025)).is_none());
    assert!(mint_page_token("").is_none());
    assert!(mint_page_token("a\nb").is_none());
    assert!(mint_page_token("a\u{7f}").is_none());
    assert!(mint_page_token("é").is_none());
    let t = mint_page_token("tok");
    assert_eq!(t.as_ref().map(PageToken::as_str), Some("tok"));
}
