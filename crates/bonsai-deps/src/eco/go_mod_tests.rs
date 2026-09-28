//! Go adapter tests (E2 Go row).

use std::time::{Duration, Instant};

use super::*;
use crate::fixtures::go::GO_MOD;

const DEFAULT: DepSource = DepSource::Registry(RegistryRef::Default);

fn try_parse(text: &str) -> Result<ParsedGroup, ParseError> {
    let files = FileSet::default();
    GoAdapter.parse(&ParseInput {
        manifest_path: "go.mod",
        manifest: text,
        lockfile: None,
        files: &files,
    })
}

fn dep<'a>(g: &'a ParsedGroup, name: &str) -> &'a Dependency {
    g.direct
        .iter()
        .find(|d| d.name == name)
        .unwrap_or_else(|| panic!("no dep {name}"))
}

#[test]
fn requires_split_into_direct_and_indirect() {
    let g = try_parse(GO_MOD).unwrap();
    let mut direct: Vec<&str> = g.direct.iter().map(|d| d.name.as_str()).collect();
    direct.sort_unstable();
    assert_eq!(
        direct,
        [
            "corp.invalid/zzsentinel/mod",
            "example.com/local",
            "example.com/quoted",
            "github.com/fork/replaced",
            "github.com/pkg/errors",
            "github.com/single/line",
        ]
    );
    let errors = dep(&g, "github.com/pkg/errors");
    assert_eq!(errors.source, DEFAULT);
    // Declared ⇒ resolved (MVS).
    assert_eq!(errors.declared.as_deref(), Some("v0.9.1"));
    assert_eq!(errors.resolved.as_deref(), Some("v0.9.1"));
    assert_eq!(
        dep(&g, "github.com/single/line").resolved.as_deref(),
        Some("v2.0.0+incompatible")
    );
    // Every versioned require is a lock entry; indirect ones only there.
    let entries: Vec<&str> = g.lock_entries.iter().map(|e| e.name.as_str()).collect();
    assert!(entries.contains(&"golang.org/x/text"));
    assert!(
        entries.contains(&"github.com/pinned/only"),
        "`// indirect; …`"
    );
    assert!(entries.contains(&"github.com/pkg/errors"));
    assert!(g.has_lockfile);
}

#[test]
fn replace_directives_apply() {
    let g = try_parse(GO_MOD).unwrap();
    // `=> ./local` ⇒ Path, never a version to send.
    let local = dep(&g, "example.com/local");
    assert_eq!(local.source, DepSource::Path);
    assert_eq!(local.resolved, None);
    assert!(g.lock_entries.iter().all(|e| e.name != "example.com/local"));
    // `X v1.0.0 => Y v1.0.1` ⇒ query Y.
    let fork = dep(&g, "github.com/fork/replaced");
    assert_eq!(fork.resolved.as_deref(), Some("v1.0.1"));
    assert_eq!(fork.declared.as_deref(), Some("v1.0.0"));
    // A versioned left side applies to that version only.
    let pinned = g
        .lock_entries
        .iter()
        .find(|e| e.name == "github.com/pinned/only")
        .unwrap();
    assert_eq!(
        (pinned.version.as_str(), &pinned.source),
        ("v1.0.0", &DEFAULT)
    );
}

#[test]
fn declared_names_is_the_module_path() {
    assert_eq!(GoAdapter.declared_names(GO_MOD), ["example.com/app"]);
    assert_eq!(
        GoAdapter.declared_names("module \"example.com/q\" // c\n"),
        ["example.com/q"]
    );
    assert!(GoAdapter.declared_names("require (\n").is_empty());
}

#[test]
fn old_go_versions_have_no_full_graph() {
    for (go, full) in [
        ("go 1.16\n", false),
        ("go 1.17\n", true),
        ("", false),
        ("go 2.0\n", false),
    ] {
        let g = try_parse(&format!("module m.example/x\n{go}")).unwrap();
        assert_eq!(g.has_lockfile, full, "{go:?}");
    }
    assert!(!GoAdapter.is_lockfile("go.sum"));
    assert!(GoAdapter
        .lockfile_for("go.mod", &FileSet::default())
        .is_none());
}

#[test]
fn syntax_errors_carry_the_line_only() {
    for (text, line) in [
        ("module m\nrequire (\n  a.example/b v1.0.0\n", 4),
        ("module m\nrequire a.example/b\n", 2),
        ("module m\nreplace a.example/b v1 v2\n", 2),
        ("module m\n\nrequire \"zz-sentinel-token v1\n", 3),
        ("module m\nreplace a => b c d\n", 2),
    ] {
        let e = try_parse(text).unwrap_err();
        assert_eq!(e.kind, ParseErrorKind::InvalidSyntax, "{text:?}");
        assert_eq!(e.line, Some(line), "{text:?}");
        assert!(!format!("{e:?}").contains("zz-sentinel"));
    }
}

#[test]
fn crlf_bom_and_inline_tokens() {
    let text = "\u{FEFF}module m.example/x\r\nrequire (\r\n\ta.example/b v1.0.0 // indirect\r\n\t`c.example/d` v2.0.0\r\n)\r\nreplace c.example/d=>../d\r\n";
    let g = try_parse(text).unwrap();
    assert_eq!(dep(&g, "c.example/d").source, DepSource::Path);
    assert!(g.direct.iter().all(|d| d.name != "a.example/b"));
    assert_eq!(g.lock_entries.len(), 1);
}

#[test]
fn a_large_go_mod_parses_linearly() {
    let mut text = String::from("module m.example/x\n\ngo 1.21\n\nrequire (\n");
    for i in 0..50_000 {
        text.push_str(&format!("\tgithub.com/o/r{i} v1.0.{i} // indirect\n"));
    }
    text.push_str(")\n");
    let t = Instant::now();
    let g = try_parse(&text).unwrap();
    assert_eq!(g.lock_entries.len(), 50_000);
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
}
