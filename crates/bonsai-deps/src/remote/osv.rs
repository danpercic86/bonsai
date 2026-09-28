//! OSV builders + DTOs (P125d). P125a: THE ONLY HOME of [`AdvisoryId`] and
//! [`PageToken`] `[MEDIUM-2]`. Neither has a public constructor, nor a
//! Deserialize / TryFrom / From impl: they are minted here, and only from
//! fields of a deserialized OSV response, so response data can build a URL
//! only through a value that passed the shape check (§7.5).

/// An OSV advisory id (`GHSA-…`, `RUSTSEC-…`, `GO-…`, `PYSEC-…`, …).
///
/// It cannot be built outside `remote::osv` — not with the tuple
/// constructor:
///
/// ```compile_fail,E0603
/// use bonsai_deps::remote::osv::AdvisoryId;
/// let _id = AdvisoryId("x".into());
/// ```
///
/// nor with a parser (there is none):
///
/// ```compile_fail,E0599
/// use bonsai_deps::remote::osv::AdvisoryId;
/// let _id = AdvisoryId::parse("x");
/// ```
///
/// nor through the private mint:
///
/// ```compile_fail,E0603
/// let _id = bonsai_deps::remote::osv::mint_id("GHSA-aaaa-bbbb-cccc");
/// ```
///
/// Stable rustdoc does not enforce the error codes above, so this positive
/// control proves the same paths resolve (the blocks fail on privacy, not
/// on a typo):
///
/// ```
/// use bonsai_deps::remote::osv::{AdvisoryId, PageToken};
/// fn _read(id: &AdvisoryId, t: &PageToken) -> (String, String) {
///     (id.as_str().to_string(), t.as_str().to_string())
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AdvisoryId(String);

/// An OSV `querybatch` `next_page_token`.
///
/// ```compile_fail,E0603
/// use bonsai_deps::remote::osv::PageToken;
/// let _t = PageToken("x".into());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageToken(String);

impl AdvisoryId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PageToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Max advisory-id length accepted by [`mint_id`] (`^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$`).
const MAX_ID_LEN: usize = 64;
/// Max page-token length accepted by [`mint_page_token`] (1 KiB).
const MAX_TOKEN_LEN: usize = 1024;

/// Call ONLY on a field of a deserialized OSV DTO. The DTOs arrive in P125d.
#[allow(dead_code)] // first caller: the P125d OSV response mapper
fn mint_id(raw: &str) -> Option<AdvisoryId> {
    let b = raw.as_bytes();
    let first_ok = b.first().is_some_and(|c| c.is_ascii_alphanumeric());
    let rest_ok = b
        .iter()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b':' | b'-'));
    (first_ok && rest_ok && b.len() <= MAX_ID_LEN).then(|| AdvisoryId(raw.to_string()))
}

/// Call ONLY on the `next_page_token` of a deserialized OSV response.
#[allow(dead_code)] // first caller: the P125d querybatch pager
fn mint_page_token(raw: &str) -> Option<PageToken> {
    let ok = !raw.is_empty()
        && raw.len() <= MAX_TOKEN_LEN
        && raw.bytes().all(|c| c.is_ascii_graphic() || c == b' ');
    ok.then(|| PageToken(raw.to_string()))
}

#[cfg(test)]
#[path = "osv_tests.rs"]
mod tests;
