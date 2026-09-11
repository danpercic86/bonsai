//! Unit tests for [`super`] (`external_cmd.rs`) — audit MEDIUM-2 / LOW-1.
//!
//! Declared with `#[path]` as a child module so `super::*` reaches the private
//! character predicates.
//!
//! The absolute-path cases use a REAL file under [`crate::testutil::scratch_dir`]
//! rather than a literal: `Path::new("/Applications/x").is_absolute()` is
//! `false` on Windows, so a hard-coded POSIX literal would silently test the
//! bare-name branch on half the CI matrix.

use super::*;

const LABEL: &str = "Editor command";

/// `Err` for every input, with the category-only message contract checked once
/// here rather than in every case.
fn assert_refused(value: &str) {
    let err = validate_command_setting(value, LABEL)
        .expect_err(&format!("must be refused: {value:?}"));
    assert!(matches!(err, AppError::ExternalToolFailed(_)), "wrong variant for {value:?}");
    let msg = err.to_string();
    assert!(msg.contains(LABEL), "message names the setting: {msg}");
    // LOW-2's rule: never echo the attacker-supplied value back into a toast.
    // (`trim()`ed non-empty values only — a substring check on "" is useless.)
    let trimmed = value.trim();
    if !trimmed.is_empty() {
        assert!(!msg.contains(trimmed), "message must not echo the value: {msg}");
    }
}

// ---- accepted: empty ⇒ auto-detect ----

#[test]
fn empty_and_whitespace_only_are_valid_and_mean_auto_detect() {
    // Both settings ship empty; that must stay a valid configuration.
    for v in ["", "   ", "\t "] {
        assert!(validate_command_setting(v, LABEL).is_ok(), "{v:?} must be accepted");
    }
}

// ---- accepted: bare program name (PATH resolution) ----

#[test]
fn bare_program_names_are_accepted() {
    // A user typing `code` or `wt` relies on PATH resolution — this must keep
    // working, so the rule is NOT absolute-only.
    for v in ["code", "wt", "code-insiders", "notepad++.exe", "nvim", "sublime_text", "g++"] {
        assert!(validate_command_setting(v, LABEL).is_ok(), "{v:?} must be accepted");
    }
}

#[test]
fn surrounding_whitespace_is_trimmed_not_rejected() {
    assert!(validate_command_setting("  code  ", LABEL).is_ok());
}

// ---- accepted: absolute path to an existing executable ----

#[test]
fn absolute_path_to_an_existing_file_is_accepted() {
    let dir = crate::testutil::scratch_dir();
    let exe = dir.path().join("my-editor.exe");
    std::fs::write(&exe, b"stub").expect("write stub");
    let value = exe.display().to_string();
    assert!(Path::new(&value).is_absolute(), "fixture must be host-absolute");
    assert!(validate_command_setting(&value, LABEL).is_ok(), "portable editor must be accepted");
}

#[test]
fn absolute_path_with_spaces_and_parens_is_accepted() {
    // `C:\Program Files (x86)\Notepad++\notepad++.exe` — a real install path.
    // Space and `()` are therefore allowed in the ABSOLUTE branch only.
    let dir = crate::testutil::scratch_dir();
    let sub = dir.path().join("Program Files (x86)");
    std::fs::create_dir_all(&sub).expect("create dir");
    let exe = sub.join("np p.exe");
    std::fs::write(&exe, b"stub").expect("write stub");
    assert!(validate_command_setting(&exe.display().to_string(), LABEL).is_ok());
}

#[test]
fn absolute_path_that_does_not_exist_is_refused() {
    let dir = crate::testutil::scratch_dir();
    let missing = dir.path().join("nope.exe");
    assert_refused(&missing.display().to_string());
}

#[test]
fn absolute_path_to_a_directory_is_refused() {
    // A directory is not a program; `is_file()` is the gate, not `exists()`.
    let dir = crate::testutil::scratch_dir();
    assert_refused(&dir.path().display().to_string());
}

#[test]
fn absolute_path_carrying_shell_syntax_is_refused_before_touching_the_fs() {
    let dir = crate::testutil::scratch_dir();
    let exe = dir.path().join("ok.exe");
    std::fs::write(&exe, b"stub").expect("write stub");
    // Same existing file, with syntax appended inside the one token.
    for suffix in ["&calc", "|calc", ";calc", "^", "$X", "`id`", "{x}", "%TEMP%", "\"", "'"] {
        assert_refused(&format!("{}{suffix}", exe.display()));
    }
}

// ---- refused: embedded arguments ----

#[test]
fn embedded_arguments_are_refused() {
    // THE vector MEDIUM-2 is about: a bare name plus args is arbitrary local
    // execution (`powershell -c …`), so whitespace-separated tokens go.
    for v in [
        "powershell -NoProfile -Command calc",
        "code --wait",
        "wt -d {path}",
        "code {path}",
        "\"my editor\" x",
        "code\tx",
    ] {
        assert_refused(v);
    }
}

// ---- refused: shell metacharacters in a bare name ----

#[test]
fn shell_metacharacters_in_a_bare_name_are_refused() {
    for v in [
        "code&calc", "code|calc", "code;calc", "code<x", "code>x", "code^x", "code$x", "code`x`",
        "code(x)", "code{x}", "code'x'", "code\"x\"", "code%x%", "code*", "code?",
    ] {
        assert_refused(v);
    }
}

// ---- refused: control characters / newlines / bidi ----

#[test]
fn interior_control_characters_are_refused() {
    // A program name cannot contain them, and a newline is how one setting
    // pretends to be two lines of anything that later reads the file.
    for v in ["co\nde", "co\rde", "code\u{0}", "code\u{7}", "code\u{7f}x"] {
        assert_refused(v);
    }
}

#[test]
fn bidi_overrides_and_isolates_are_refused() {
    // Same set `ai::stream::strip_control_chars` strips: a value whose rendering
    // disagrees with what launches is never legitimate.
    for v in ["co\u{202e}de", "\u{200f}code", "co\u{2066}de", "code\u{202a}"] {
        assert_refused(v);
    }
}

#[test]
fn surrounding_newlines_are_trimmed_like_any_whitespace() {
    // A pasted value carries a trailing newline; `trim()` runs BEFORE the shape
    // check, so this is `code` — and `program_spec` runs the SAME trim, so both
    // sides see a byte-identical string (asserted in `external_tests.rs`; what
    // the OS finally resolves can still differ — see the module's Residual).
    assert!(validate_command_setting("code\n", LABEL).is_ok());
    assert!(validate_command_setting("\tcode\r\n", LABEL).is_ok());
}

// ---- refused: paths that are neither shape ----

#[test]
fn relative_paths_with_separators_are_refused() {
    // `./code` and `tools\code.exe` resolve against a cwd we do not control —
    // exactly the primitive LOW-1 removes from the launch itself.
    for v in ["./code", "../code", "code/../code", "tools\\code.exe", ".\\code.exe", "sub/dir/code"]
    {
        assert_refused(v);
    }
}

#[test]
fn unc_paths_are_refused() {
    // Absolute on Windows and fetched over SMB by `is_file()`, so it is
    // rejected before the filesystem is touched.
    for v in ["\\\\server\\share\\evil.exe", "//server/share/evil.exe"] {
        assert_refused(v);
    }
}

#[test]
fn an_over_long_value_is_refused() {
    assert_refused(&"c".repeat(MAX_LEN + 1));
}

// ---- LOW-1: the launch-neutral cwd ----

#[test]
fn safe_cwd_is_an_existing_directory_that_is_not_the_repo() {
    let cwd = safe_cwd();
    assert!(cwd.is_dir(), "safe_cwd must exist: {}", cwd.display());
    // The test binary's own directory — never a repo working tree, which is the
    // whole point of LOW-1. `"."` would mean the process cwd leaked back in; the
    // documented fallback is `temp_dir()`, which is also never a repo.
    assert_ne!(cwd, PathBuf::from("."), "the cwd must never be the process cwd");
}
