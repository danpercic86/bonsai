//! Unit tests for `search`: argv building, log-output parsing, cap/empty
//! guards, and wire shapes. Extracted verbatim from the former inline
//! `mod tests`; shared fixtures live in `test_support`.

use super::test_support::*;
use super::*;

#[test]
fn build_log_args_path_default() {
    let args = build_log_args(&q(SearchField::Path, "src/lib.rs"), 1000);
    let mut expected = base_args("1001");
    expected.extend(["-i", "--all", "--", "src/lib.rs"].map(String::from));
    assert_eq!(args, expected);
}

#[test]
fn build_log_args_content_literal_default() {
    let args = build_log_args(&q(SearchField::Content, "needle"), 1000);
    let mut expected = base_args("1001");
    expected.extend(["-i", "-Sneedle", "--all"].map(String::from));
    assert_eq!(args, expected);
}

#[test]
fn build_log_args_content_regex_case_sensitive() {
    let query = SearchQuery {
        text: "re.*x".to_string(),
        field: SearchField::Content,
        regex: true,
        case_sensitive: true,
        max_results: 0,
        scope_ref: None,
    };
    let args = build_log_args(&query, 1000);
    let mut expected = base_args("1001");
    // No -i (case-sensitive); -G flag (regex).
    expected.extend(["-Gre.*x", "--all"].map(String::from));
    assert_eq!(args, expected);
}

#[test]
fn build_log_args_scope_ref_overrides_all() {
    // An explicit scope rides behind `--end-of-options` (audit §2.6).
    let query = SearchQuery {
        scope_ref: Some("dev".to_string()),
        ..q(SearchField::Path, "f.txt")
    };
    let args = build_log_args(&query, 50);
    let mut expected = base_args("51");
    expected.extend(["-i", "--end-of-options", "dev", "--", "f.txt"].map(String::from));
    assert_eq!(args, expected);

    // Content mode: the pickaxe token (an OPTION) stays BEFORE the marker.
    let content = SearchQuery {
        scope_ref: Some("dev".to_string()),
        ..q(SearchField::Content, "needle")
    };
    let args = build_log_args(&content, 50);
    let mut expected = base_args("51");
    expected.extend(["-i", "-Sneedle", "--end-of-options", "dev"].map(String::from));
    assert_eq!(args, expected);
}

/// Audit §2.6: a scope named like an option is rejected in EVERY mode
/// before any git spawn (PanicRunner proves the shell modes) or revwalk.
#[test]
fn leading_dash_scope_ref_is_rejected() {
    for field in [
        SearchField::Path,
        SearchField::Content,
        SearchField::Message,
        SearchField::Author,
        SearchField::All,
    ] {
        let query = SearchQuery {
            scope_ref: Some("--output=C:/evil.bat".to_string()),
            ..q(field, "needle")
        };
        let err = search_commits(Path::new("."), &PanicRunner, &query)
            .expect_err("leading-dash scope must be rejected");
        match err {
            AppError::Other(m) => assert!(m.contains("invalid scope ref"), "got {m}"),
            other => panic!("expected Other(invalid scope ref), got {other:?}"),
        }
    }
}

#[test]
fn build_log_args_metachars_stay_one_token() {
    // A `;`/space-bearing pathspec is exactly ONE argv token — never split,
    // never a second command.
    let path_args = build_log_args(&q(SearchField::Path, "a b; rm -rf /"), 1000);
    assert_eq!(path_args.last().unwrap(), "a b; rm -rf /");
    assert_eq!(path_args[path_args.len() - 2], "--");

    // Content: the whole needle rides inside the single `-S…` token.
    let content_args = build_log_args(&q(SearchField::Content, "x; rm -rf /"), 1000);
    assert!(content_args.contains(&"-Sx; rm -rf /".to_string()));
}

#[test]
fn effective_cap_clamps_and_defaults() {
    assert_eq!(effective_cap(&q(SearchField::Message, "x")), MAX_SEARCH_RESULTS);
    let small = SearchQuery {
        max_results: 50,
        ..q(SearchField::Message, "x")
    };
    assert_eq!(effective_cap(&small), 50);
    let over = SearchQuery {
        max_results: 5000,
        ..q(SearchField::Message, "x")
    };
    assert_eq!(effective_cap(&over), MAX_SEARCH_RESULTS);
}
#[test]
fn parse_log_output_fills_fields_content() {
    let a = "a".repeat(40);
    let b = "b".repeat(40);
    let stdout = format!(
        "{}\n{}\n",
        record(&a, "add feature", "Ada", "1000"),
        record(&b, "fix bug", "Grace", "2000"),
    );
    let (matches, truncated) = parse_log_output(&stdout, 1000, SearchField::Content, "feat");
    assert!(!truncated);
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].oid, a);
    assert_eq!(matches[0].summary, "add feature");
    assert_eq!(matches[0].author_name, "Ada");
    assert_eq!(matches[0].author_ts, 1000);
    assert_eq!(matches[0].matched, MatchedField::Content);
    assert_eq!(matches[0].snippet, None);
}

#[test]
fn parse_log_output_path_sets_snippet() {
    let a = "a".repeat(40);
    let stdout = format!("{}\n", record(&a, "touch it", "Ada", "1000"));
    let (matches, _) = parse_log_output(&stdout, 1000, SearchField::Path, "src/x.rs");
    assert_eq!(matches[0].matched, MatchedField::Path);
    assert_eq!(matches[0].snippet.as_deref(), Some("src/x.rs"));
}

#[test]
fn parse_log_output_truncates_at_cap_plus_one() {
    let mut stdout = String::new();
    for i in 0..3 {
        let oid = format!("{i:040x}");
        stdout.push_str(&record(&oid, "s", "Ada", "1"));
        stdout.push('\n');
    }
    let (matches, truncated) = parse_log_output(&stdout, 2, SearchField::Content, "s");
    assert!(truncated, "3 records with cap 2 ⇒ truncated");
    assert_eq!(matches.len(), 2);
}

// ---------------------------------------------------------- empty / cap (git2)

#[test]
fn empty_text_returns_ok_without_running_git() {
    // Whitespace text short-circuits BEFORE any subprocess (PanicRunner proves it).
    let out = search_commits(Path::new("."), &PanicRunner, &q(SearchField::Content, "   "))
        .expect("empty ⇒ Ok");
    assert!(out.matches.is_empty());
    assert!(!out.truncated);
}

// ---------------------------------------------------------- wire shapes

#[test]
fn search_results_wire_shape_camel_case() {
    let results = SearchResults {
        matches: vec![SearchMatch {
            oid: "a".repeat(40),
            summary: "hi".to_string(),
            author_name: "Ada".to_string(),
            author_ts: 1234,
            matched: MatchedField::Message,
            snippet: None,
        }],
        truncated: true,
    };
    let v = serde_json::to_value(&results).expect("json");
    let m = &v["matches"][0];
    assert_eq!(m["authorName"], "Ada");
    assert_eq!(m["authorTs"], 1234);
    assert_eq!(m["matched"], "message");
    // snippet omitted when None (skip_serializing_if).
    assert!(m.get("snippet").is_none());
    assert_eq!(v["truncated"], true);

    // Path snippet present + camelCase matched.
    let path_v = serde_json::to_value(SearchMatch {
        oid: "b".repeat(40),
        summary: "s".to_string(),
        author_name: "Grace".to_string(),
        author_ts: 1,
        matched: MatchedField::Path,
        snippet: Some("src/x.rs".to_string()),
    })
    .expect("json");
    assert_eq!(path_v["matched"], "path");
    assert_eq!(path_v["snippet"], "src/x.rs");
}

#[test]
fn search_query_deserializes_with_defaults() {
    // Only text+field required; the rest default (regex/case false, cap 0, no scope).
    let query: SearchQuery =
        serde_json::from_value(serde_json::json!({ "text": "hi", "field": "author" }))
            .expect("deserialize");
    assert_eq!(query.text, "hi");
    assert_eq!(query.field, SearchField::Author);
    assert!(!query.regex);
    assert!(!query.case_sensitive);
    assert_eq!(query.max_results, 0);
    assert_eq!(query.scope_ref, None);
}
