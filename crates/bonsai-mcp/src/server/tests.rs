//! Unit tests for the per-session selection state (P16a). Extracted verbatim
//! from the former inline `mod tests`.

use super::*;

fn open(id: &str) -> OpenRepo {
    OpenRepo {
        repo_id: id.to_string(),
        path: PathBuf::from(format!("/repo/{id}")),
    }
}

/// A session over open tabs `a` and `b`, seeded with `seed`.
fn session(seed: Option<&str>) -> SessionRepos {
    let repos = vec![open("a"), open("b")];
    SessionRepos::new(
        seed.map(str::to_string),
        Box::new(move || repos.clone()),
    )
}

#[test]
fn resolve_workdir_none_selected_is_no_repo() {
    let s = session(None);
    assert!(matches!(s.resolve_workdir(), Err(AppError::NoRepo)));
}

#[test]
fn resolve_workdir_selected_present_returns_that_path() {
    let s = session(Some("b"));
    assert_eq!(
        s.resolve_workdir().expect("selected tab is open"),
        PathBuf::from("/repo/b")
    );
}

#[test]
fn resolve_workdir_selected_but_closed_is_no_repo() {
    // Seed selects `b`, but the open set no longer contains it (tab closed).
    let repos = vec![open("a")];
    let s = SessionRepos::new(Some("b".to_string()), Box::new(move || repos.clone()));
    assert!(matches!(s.resolve_workdir(), Err(AppError::NoRepo)));
}

#[test]
fn select_unknown_id_is_invalid_name() {
    let s = session(None);
    assert!(matches!(s.select("nope"), Err(AppError::InvalidName(_))));
}

#[test]
fn select_known_id_then_resolve_returns_right_path() {
    let s = session(None);
    s.select("a").expect("`a` is an open tab");
    assert_eq!(
        s.resolve_workdir().expect("just-selected tab resolves"),
        PathBuf::from("/repo/a")
    );
}

// ------------------------------------------------ parse_resolution units

#[test]
fn parse_resolution_accepts_the_three_valid_variants() {
    use bonsai_core::git::conflict::ConflictResolution;
    assert!(matches!(
        parse_resolution("ours"),
        Ok(ConflictResolution::Ours)
    ));
    assert!(matches!(
        parse_resolution("theirs"),
        Ok(ConflictResolution::Theirs)
    ));
    assert!(matches!(
        parse_resolution("markResolved"),
        Ok(ConflictResolution::MarkResolved)
    ));
}

#[test]
fn parse_resolution_rejects_unknown_and_wrong_case() {
    // Unknown token → invalidName carrying the offending value.
    match parse_resolution("bogus") {
        Err(AppError::InvalidName(m)) => assert!(m.contains("bogus"), "msg: {m}"),
        other => panic!("expected InvalidName, got {other:?}"),
    }
    // camelCase is exact — snake_case / other casings are rejected.
    assert!(parse_resolution("mark_resolved").is_err());
    assert!(parse_resolution("Ours").is_err());
    assert!(parse_resolution("").is_err());
}

// -------------------------------------------------------- err_result units

/// `err_result` must set `is_error=true`, carry the `{ kind, message }`
/// discriminant in `structured_content`, and render a single
/// `"<kind>: <message>"` text block (no duplicated payload).
#[test]
fn err_result_preserves_kind_and_message_and_flags_error() {
    let r = err_result(AppError::EmptyMessage);
    assert_eq!(r.is_error, Some(true));
    let sc = r.structured_content.expect("structured content present");
    assert_eq!(sc.get("kind").and_then(|v| v.as_str()), Some("emptyMessage"));
    assert!(sc.get("message").and_then(|v| v.as_str()).is_some());
    assert_eq!(r.content.len(), 1, "exactly one text block");
}

#[test]
fn err_result_text_block_is_kind_colon_message() {
    let r = err_result(AppError::InvalidName("bad/name".to_string()));
    let sc = r.structured_content.clone().expect("structured content");
    let kind = sc.get("kind").and_then(|v| v.as_str()).unwrap();
    let msg = sc.get("message").and_then(|v| v.as_str()).unwrap();
    let text = &r.content[0].as_text().expect("text block").text;
    assert_eq!(*text, format!("{kind}: {msg}"));
    assert_eq!(kind, "invalidName");
}

// ------------------------------------------------------ ok_json / summary

#[test]
fn ok_json_puts_full_payload_in_structured_and_compact_text() {
    let value = serde_json::json!({
        "nodes": [1, 2, 3],
        "edges": [],
        "headIndex": 0,
    });
    let r = ok_json(&value);
    assert_eq!(r.is_error, Some(false));
    // Full payload survives in structured_content.
    assert_eq!(r.structured_content.as_ref(), Some(&value));
    // The text block is a compact key summary, NOT the full JSON echo.
    let text = &r.content[0].as_text().expect("text block").text;
    assert!(text.starts_with('{') && text.contains("nodes"), "summary: {text}");
    assert!(
        text.len() < value.to_string().len(),
        "compact summary must be shorter than the full payload"
    );
}

#[test]
fn compact_summary_shapes() {
    use serde_json::json;
    assert_eq!(compact_summary(&json!([1, 2, 3, 4])), "[4 items]");
    assert_eq!(compact_summary(&json!([])), "[0 items]");
    assert_eq!(compact_summary(&json!(null)), "null");
    assert_eq!(compact_summary(&json!({"a": 1, "b": 2})), "{a, b}");
    // > MAX_KEYS keys are truncated with an ellipsis marker.
    let big: serde_json::Value = (0..20)
        .map(|i| (format!("k{i:02}"), json!(i)))
        .collect::<serde_json::Map<_, _>>()
        .into();
    let s = compact_summary(&big);
    assert!(s.ends_with(", …}"), "expected truncation marker: {s}");
}
