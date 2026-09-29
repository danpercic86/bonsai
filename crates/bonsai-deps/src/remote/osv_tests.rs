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

fn dto(id: &str) -> VulnRefDto {
    VulnRefDto {
        id: id.into(),
        modified: "m".into(),
    }
}

#[test]
fn mint_refs_counts_dropped() {
    // P125h §1: 2 good + 1 bad ⇒ 2 refs, dropped 1.
    let (refs, dropped) = mint_refs(vec![dto("GHSA-a"), dto("../x"), dto("RUSTSEC-1")]);
    let ids: Vec<&str> = refs.iter().map(|r| r.id.as_str()).collect();
    assert_eq!((ids, dropped), (vec!["GHSA-a", "RUSTSEC-1"], 1));
    assert_eq!(mint_refs(Vec::new()).1, 0);
}

#[test]
fn pages_accumulate_dropped() {
    use crate::model::Ecosystem;
    use crate::remote::{FakeTransport, RawResponse};
    use std::sync::atomic::{AtomicU32, Ordering};
    let n = AtomicU32::new(0);
    let t = FakeTransport::new().with_responder(Box::new(move |_| {
        let body: &[u8] = if n.fetch_add(1, Ordering::SeqCst) == 0 {
            br#"{"results":[{"vulns":[{"id":"a b"},{"id":"GHSA-1"}],"next_page_token":"t"},{"vulns":[{"id":"GHSA-2"}]}]}"#
        } else {
            br#"{"results":[{"vulns":[{"id":"x/y"},{"id":"GHSA-3"}]}]}"#
        };
        Some(Ok(RawResponse {
            status: 200,
            body: body.to_vec(),
        }))
    }));
    let q = [
        crate::test_pkg::public(Ecosystem::Npm, "a", Some("1.0.0")),
        crate::test_pkg::public(Ecosystem::Npm, "b", Some("1.0.0")),
    ];
    let hits = query_chunk(&t, &q, &|| false).unwrap_or_else(|e| panic!("{e:?}"));
    let got: Vec<(usize, u32)> = hits.iter().map(|h| (h.refs.len(), h.dropped)).collect();
    assert_eq!(got, [(2, 2), (1, 0)]);
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
