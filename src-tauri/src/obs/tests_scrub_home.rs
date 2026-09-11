//! P91 §7.2 — home-directory masking (`obs/scrub_home.rs`).
//!
//! Every case drives the PURE [`mask_home_with`] with a **synthetic** home, so
//! the suite is portable: all three OS layouts are asserted on any host, and
//! there is no process-global to sequence (2026-09-11 — the home string travels
//! in `writer::WriterConfig::home_mask`, and `tests_writer` owns the wiring
//! proof that the writer actually passes it).
//!
//! The `home` argument must arrive folded, exactly as `normalize_home` produces
//! it, so the fixtures below are lowercase with `/` separators. The last section
//! covers [`super::home_resolve`], which produces those values from an
//! `app_config_dir()` when `home_dir()` fails.

use super::redact::{HOME_TOKEN, REDACTED_TOKEN};
use super::scrub::scrub_string;
use super::home_resolve::strip_app_containers;
use super::scrub_home::{mask_home_with, normalize_home};
use std::path::Path;

const WIN_HOME: &str = "c:/users/jane";
const MAC_HOME: &str = "/users/jane";
const LINUX_HOME: &str = "/home/jane";

// ---- the three layouts collapse identically ----

#[test]
fn all_three_os_layouts_collapse_to_the_same_token() {
    let cases = [
        (WIN_HOME, r"C:\Users\jane\Repos\bonsai", r"<home>\Repos\bonsai"),
        (MAC_HOME, "/Users/jane/Repos/bonsai", "<home>/Repos/bonsai"),
        (LINUX_HOME, "/home/jane/Repos/bonsai", "<home>/Repos/bonsai"),
    ];
    for (home, input, expected) in cases {
        assert_eq!(mask_home_with(input, home), expected, "home={home}");
    }
}

#[test]
fn everything_below_the_home_dir_survives_verbatim() {
    // The point of masking only the PREFIX: a raw log must stay debuggable, so
    // the repo name, the subdirectory and the file name are all kept.
    let input = r"C:\Users\jane\Repos\bonsai\src-tauri\src\obs\scrub.rs:128";
    assert_eq!(
        mask_home_with(input, WIN_HOME),
        r"<home>\Repos\bonsai\src-tauri\src\obs\scrub.rs:128"
    );
}

#[test]
fn the_account_name_is_gone_from_the_output() {
    for (home, input) in [
        (WIN_HOME, r"C:\Users\jane\x"),
        (MAC_HOME, "/Users/jane/x"),
        (LINUX_HOME, "/home/jane/x"),
    ] {
        let out = mask_home_with(input, home);
        assert!(!out.contains("jane"), "account name leaked: {out}");
        assert!(out.contains(HOME_TOKEN), "no placeholder: {out}");
    }
}

// ---- case and separator differences must not defeat the match ----

#[test]
fn case_differences_still_match() {
    // Windows and macOS are case-insensitive, so `c:\users\JANE` is the same
    // directory as `C:\Users\jane`.
    for input in [r"c:\users\JANE\x", r"C:\USERS\JANE\x", r"C:\Users\Jane\x"] {
        assert_eq!(mask_home_with(input, WIN_HOME), r"<home>\x", "input={input}");
    }
    assert_eq!(mask_home_with("/HOME/Jane/x", LINUX_HOME), "<home>/x");
}

#[test]
fn separator_differences_still_match() {
    // A Windows path arrives with either separator — including inside a URL.
    assert_eq!(mask_home_with("C:/Users/jane/x", WIN_HOME), "<home>/x");
    assert_eq!(mask_home_with(r"C:\Users\jane/x", WIN_HOME), "<home>/x");
    assert_eq!(
        mask_home_with("file:///C:/Users/jane/repo", WIN_HOME),
        "file:///<home>/repo"
    );
}

#[test]
fn the_bare_home_dir_with_and_without_a_trailing_separator_matches() {
    assert_eq!(mask_home_with(r"C:\Users\jane", WIN_HOME), "<home>");
    assert_eq!(mask_home_with(r"C:\Users\jane\", WIN_HOME), r"<home>\");
    assert_eq!(mask_home_with("/home/jane", LINUX_HOME), "<home>");
}

// ---- boundary anchoring ----

#[test]
fn a_sibling_directory_with_the_same_prefix_is_not_masked() {
    // `/home/bobby` is a DIFFERENT account: masking it as `<home>by` would be
    // both wrong and misleading.
    for input in ["/home/janet/x", "/home/jane2/x", "/home/jane.bak/x", "/home/jane-old/x"] {
        assert_eq!(mask_home_with(input, LINUX_HOME), input, "input={input}");
    }
    assert_eq!(mask_home_with(r"C:\Users\janet\x", WIN_HOME), r"C:\Users\janet\x");
}

#[test]
fn a_match_ending_at_punctuation_or_whitespace_is_still_a_match() {
    assert_eq!(mask_home_with("cwd=/home/jane, ok", LINUX_HOME), "cwd=<home>, ok");
    assert_eq!(mask_home_with("\"/home/jane\"", LINUX_HOME), "\"<home>\"");
    assert_eq!(mask_home_with("at /home/jane during scan", LINUX_HOME), "at <home> during scan");
}

// ---- multiple occurrences / no-match ----

#[test]
fn every_occurrence_is_replaced() {
    assert_eq!(
        mask_home_with("/home/jane/a -> /home/jane/b", LINUX_HOME),
        "<home>/a -> <home>/b"
    );
}

#[test]
fn a_string_without_the_home_dir_is_returned_borrowed_and_unchanged() {
    for input in ["/opt/tools/code", "no paths here at all", "", "/home/", "/homely/jane"] {
        let out = mask_home_with(input, LINUX_HOME);
        assert_eq!(out, input, "input={input}");
        assert!(matches!(out, std::borrow::Cow::Borrowed(_)), "must not allocate: {input}");
    }
}

// ---- normalize_home: what is installable ----

#[test]
fn normalize_home_folds_case_and_separators_and_drops_the_trailing_one() {
    assert_eq!(normalize_home(Path::new(r"C:\Users\Jane\")).as_deref(), Some("c:/users/jane"));
    assert_eq!(normalize_home(Path::new("/Users/Jane")).as_deref(), Some("/users/jane"));
    assert_eq!(normalize_home(Path::new("/home/jane//")).as_deref(), Some("/home/jane"));
}

#[test]
fn a_root_only_home_is_refused_so_it_cannot_mask_every_path() {
    // `/` or `C:\` as "home" would turn every absolute path into `<home>/…`,
    // destroying the raw log's whole purpose.
    for root in ["/", "//", r"C:\", "C:/", ""] {
        assert!(normalize_home(Path::new(root)).is_none(), "root={root:?}");
    }
}

#[test]
fn the_shared_parent_of_all_accounts_is_refused() {
    // `C:\Users` as "home" would mask `C:\Users\jane\x` to `<home>\jane\x`: the
    // account name is still there, now behind a token claiming it is not. Worse
    // than no masking, so it is refused — this is what bounds a too-greedy
    // `home_resolve` candidate.
    for shared in [r"C:\Users", "C:/users/", "/Users", "/home", "/home/", r"D:\USERS"] {
        assert!(normalize_home(Path::new(shared)).is_none(), "shared={shared:?}");
    }
    // One segment deeper IS a home directory.
    assert!(normalize_home(Path::new(r"C:\Users\jane")).is_some());
    assert!(normalize_home(Path::new("/home/jane")).is_some());
}

// ---- interaction with the credential scrubber ----

#[test]
fn the_placeholder_survives_the_credential_pass_untouched() {
    // `<` / `>` are not word characters to the walker, and `home` is not a
    // sensitive key — so a masked path is not re-mangled.
    let masked = r"<home>\Repos\bonsai";
    assert_eq!(scrub_string(masked, None), masked);
}

#[test]
fn masking_does_not_rescue_a_credential_further_down_the_path() {
    // A token inside a path under the home dir must still be scrubbed: the home
    // pass runs FIRST, then every existing rule sees the shortened string.
    let out = scrub_string("/home/jane/log password=hunter2secret", Some(LINUX_HOME));
    assert!(out.starts_with(HOME_TOKEN), "home pass ran first: {out}");
    assert!(out.contains(REDACTED_TOKEN), "credential rule still fires: {out}");
    assert!(!out.contains("hunter2secret"), "{out}");
    assert!(!out.contains("jane"), "{out}");
}

#[test]
fn scrub_string_without_a_home_leaves_paths_alone() {
    // The `None` arm: a writer that could not resolve a home masks nothing —
    // which is exactly what its `homeMasking: false` header stamp says.
    let raw = "/home/jane/Repos/bonsai";
    assert_eq!(scrub_string(raw, None), raw);
}

// ---- home_resolve: the fail-closed fallback (auditor MUST-FIX 2026-09-11) ----

#[test]
fn the_config_dir_of_every_os_layout_strips_back_to_the_home_dir() {
    // The fallback when `home_dir()` fails. Pure and synthetic, so all three
    // layouts are checked on any host — which is why the walker folds `\` to `/`
    // instead of using `Path::parent` (that splits only on the HOST separator,
    // making the Windows case one opaque component on unix). The Windows
    // expectation is therefore `/`-folded; `normalize_home` folds again, so the
    // difference is invisible downstream.
    let cases = [
        (r"C:\Users\jane\AppData\Roaming\com.bonsai.app", "C:/Users/jane"),
        ("/Users/jane/Library/Application Support/com.bonsai.app", "/Users/jane"),
        ("/home/jane/.config/com.bonsai.app", "/home/jane"),
    ];
    for (config_dir, expected) in cases {
        assert_eq!(
            strip_app_containers(Path::new(config_dir)).as_deref(),
            Some(Path::new(expected)),
            "config_dir={config_dir}"
        );
    }
}

#[test]
fn an_unrecognised_container_stops_the_walk_instead_of_climbing_past_home() {
    // `XDG_CONFIG_HOME=/opt/cfg` — stopping early yields a useless candidate;
    // climbing on would yield `/opt`, then `/`. Under-masking is the safe
    // direction here, because over-stripping to a shared parent re-exposes the
    // account name (see `the_shared_parent_of_all_accounts_is_refused`).
    assert_eq!(
        strip_app_containers(Path::new("/opt/cfg/com.bonsai.app")).as_deref(),
        Some(Path::new("/opt/cfg"))
    );
    // And a candidate that IS the shared parent is refused downstream, so the
    // two gates compose: masking stays off rather than masking the wrong thing.
    let greedy = strip_app_containers(Path::new("/home/.config/com.bonsai.app"));
    assert_eq!(greedy.as_deref(), Some(Path::new("/home")));
    assert!(normalize_home(Path::new("/home")).is_none());
}

#[test]
fn a_config_dir_without_a_parent_yields_no_candidate() {
    for degenerate in ["", "/", "com.bonsai.app", "/com.bonsai.app", "C:/com.bonsai.app"] {
        assert!(strip_app_containers(Path::new(degenerate)).is_none(), "{degenerate:?}");
    }
}

#[test]
fn the_derived_candidate_feeds_normalize_home_and_masks_the_account_name() {
    // The two halves composed, which is what `resolve_home_mask` does: config dir
    // ⇒ candidate ⇒ folded home ⇒ the account name is gone from a raw path.
    let candidate = strip_app_containers(Path::new(r"C:\Users\jane\AppData\Roaming\com.bonsai.app"))
        .expect("candidate");
    let home = normalize_home(&candidate).expect("normalizable");
    assert_eq!(home, WIN_HOME);
    assert_eq!(mask_home_with(r"C:\Users\jane\Repos\bonsai", &home), r"<home>\Repos\bonsai");
}
