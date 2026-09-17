//! P112 **AC18** — `pick_external_tool`'s write semantics, and the AC16
//! refusal table at the command layer.
//!
//! Everything here goes through [`super::commit_browsed_tool`], the seam that
//! starts where the native dialog ends: `Some(path)` is "the user confirmed",
//! and not calling it at all is "the user cancelled". The dialog itself is a
//! USER CHECKPOINT (UC6) and is deliberately absent.
//!
//! ## What runs on which runner (AMEND-5 item 6's rule, applied here)
//!
//! The fixture programs are real files under a `TempDir` — which is
//! `C:\…\Temp\…` on Windows but `/tmp/…` or `/var/folders/…` on the other two CI
//! legs (`ci.yml` runs `nextest --workspace` on all three). The ABSOLUTENESS
//! rule inside `validate_custom_program` is os-EXPLICIT: for `TargetOs::Windows`
//! it demands a drive letter or a UNC head (`tools/custom.rs:180-200`), so a
//! `/tmp` fixture validated as Windows is refused on a unix host before any
//! extension is looked at.
//!
//! The accept cases below therefore drive `TargetOs::host()` and build a fixture
//! the host's own rules accept — `.exe` on Windows (DEC-1), mode `0o755` on unix
//! (the execute bit). All four then run, and run genuinely, on all three
//! runners.
//!
//! An earlier version of this paragraph claimed the opposite: that naming the
//! fixtures `*.exe` made `TargetOs::Windows` acceptable on any host, because
//! `os` is a parameter. Neither clause held — the `.exe` rule is only REACHED
//! once absoluteness passes, the same mechanism
//! `tools::custom_tests::on_a_windows_host_the_unix_execute_bit_rule_cannot_be_evaluated`
//! documents from the other side.
//!
//! **One case stays host-bound: the refusal test's DEC-1 rows.** They keep an
//! explicit `TargetOs::Windows`, because "`payload.cmd` EXISTS and is still
//! refused" is the security-relevant row and Windows is the gate host. On
//! Linux/macOS that same call is still refused and the test still passes — but
//! by the absoluteness rule, so what goes unasserted there is DEC-1 itself, not
//! the command-layer property. That property (refuse ⇒ write nothing, echo
//! nothing) is carried on every leg by the same test's `TargetOs::host()` half.

use std::path::{Path, PathBuf};

use bonsai_core::error::AppError;
use bonsai_core::external::TargetOs;
use bonsai_core::tools::{ToolKind, ToolSource, CUSTOM_ID};

use super::commit_browsed_tool;
use crate::settings::{self, Settings};

/// A temp dir plus a settings file inside it, seeded with the defaults.
fn seeded() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = dir.path().join("settings.json");
    settings::save_to(&file, &Settings::default()).expect("seed settings");
    (dir, file)
}

/// A real program file that the **host's** rule set accepts: the `.exe`
/// extension Windows requires (DEC-1), plus the execute bit unix requires.
///
/// The `.exe` name is kept on unix too — that arm ignores extensions, and
/// `file_stem` is `<stem>` either way, which is what the label assertion reads.
fn program(dir: &Path, stem: &str) -> PathBuf {
    let p = dir.join(format!("{stem}.exe"));
    std::fs::write(&p, b"MZ").expect("write fixture program");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755))
            .expect("chmod the fixture program");
    }
    p
}

/// AC18 confirm: BOTH `custom_*_path` and `*_tool = "custom"` land, and the
/// returned row is the browsed row the picker lists.
#[test]
fn a_confirmed_pick_writes_both_the_path_and_the_selection() {
    for kind in [ToolKind::Terminal, ToolKind::Editor] {
        let (dir, file) = seeded();
        let prog = program(dir.path(), "Portable Editor");

        let row = commit_browsed_tool(&file, kind, &prog, TargetOs::host()).expect("accepted");
        assert_eq!(row.id, CUSTOM_ID);
        assert_eq!(row.kind, kind);
        assert_eq!(row.source, ToolSource::Custom);
        assert_eq!(
            row.label, "Portable Editor",
            "the label is the backend-derived stem"
        );
        assert_eq!(
            row.detail,
            prog.to_string_lossy(),
            "detail is the resolved path"
        );
        assert!(row.present);

        let s = settings::load_from(&file);
        let (tool, path) = match kind {
            ToolKind::Terminal => (&s.terminal_tool, &s.custom_terminal_path),
            ToolKind::Editor => (&s.editor_tool, &s.custom_editor_path),
        };
        assert_eq!(tool, CUSTOM_ID, "the selection is written too");
        assert_eq!(path, &prog.to_string_lossy(), "the path is written");
        // The OTHER kind is untouched — one pick writes one slot.
        let (other_tool, other_path) = match kind {
            ToolKind::Terminal => (&s.editor_tool, &s.custom_editor_path),
            ToolKind::Editor => (&s.terminal_tool, &s.custom_terminal_path),
        };
        assert_eq!(other_tool, "");
        assert_eq!(other_path, "");
    }
}

/// AC18 refusal: a path that fails validation writes **nothing** — not the path,
/// and not the selection — and the error never echoes the path.
///
/// DEC-1 is the load-bearing row: `payload.cmd` / `payload.bat` / `payload.ps1`
/// EXIST here and are still refused under `os = Windows`, because the CHECK, not
/// the dialog filter, is the gate. **That attribution holds on a Windows runner
/// only** — see the module doc: elsewhere the same paths are refused one rule
/// earlier, for not being Windows-absolute. The second loop is the half that
/// proves the command-layer property under the host's own rules on every runner.
#[test]
fn a_refused_path_mutates_nothing_and_never_echoes_it() {
    let (dir, file) = seeded();
    let before = std::fs::read_to_string(&file).expect("read settings");

    let refused = |path: &Path, os: TargetOs| {
        let err =
            commit_browsed_tool(&file, ToolKind::Editor, path, os).expect_err("must be refused");
        assert!(
            matches!(err, AppError::ExternalToolFailed(_)),
            "wrong variant for {path:?}"
        );
        let msg = err.to_string();
        assert!(
            !msg.contains(&*path.to_string_lossy()),
            "category-only: the refusal must not echo {path:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&file).expect("read settings"),
            before,
            "nothing may be written for {path:?}"
        );
    };

    // DEC-1: these three EXIST and are refused anyway.
    for name in ["payload.cmd", "payload.bat", "payload.ps1"] {
        let p = dir.path().join(name);
        std::fs::write(&p, b"@echo off").expect("write fixture");
        refused(&p, TargetOs::Windows);
    }

    // Host-OS: a path that does not exist at all, and a plain directory. Both
    // fail `is_file` under every `os` value (a bare directory is not a bundle
    // either — no `Contents/Info.plist`), so these two carry "refuse ⇒ write
    // nothing, echo nothing" on all three CI legs.
    for path in [dir.path().join("nope.exe"), dir.path().to_path_buf()] {
        refused(&path, TargetOs::host());
    }
}

/// AC18's concurrency half: a pick and an unrelated settings write, interleaved
/// 20×, lose neither — because the pick mutates through `settings::update`
/// (load→mutate→save under the process-wide `SETTINGS_IO` mutex) instead of a
/// bare `load_from` + `save_to`, whose last rename would drop the other writer.
///
/// **What it discriminates, stated exactly** (corrected 2026-09-15; the message
/// below used to claim "both fields in ONE update cycle", which this test does
/// NOT show): a two-cycle implementation — two sequential `settings::update`
/// calls, one per field — takes the mutex twice, reaches the identical final
/// state, and passes this test unchanged. The regression it catches is the lost
/// update, not the cycle count.
///
/// The single cycle is a STRUCTURAL fact instead: `commit_browsed_tool` contains
/// exactly one `settings::update` call, with the validation ahead of it. Its
/// only observable consequence — that no reader ever sees `editor_tool ==
/// "custom"` beside an empty `custom_editor_path` — is transient during the
/// FIRST pick alone (every later round rewrites identical values), so a polling
/// thread could miss the regression while claiming to pin it. Left unasserted
/// deliberately rather than asserted weakly.
#[test]
fn a_pick_racing_an_unrelated_settings_write_loses_neither() {
    let (dir, file) = seeded();
    let prog = program(dir.path(), "racer");

    const ROUNDS: u32 = 20;
    let picker = {
        let file = file.clone();
        let prog = prog.clone();
        std::thread::spawn(move || {
            for _ in 0..ROUNDS {
                commit_browsed_tool(&file, ToolKind::Editor, &prog, TargetOs::host())
                    .expect("pick accepted");
            }
        })
    };
    let dragger = {
        let file = file.clone();
        std::thread::spawn(move || {
            for i in 0..ROUNDS {
                settings::update(&file, |s| s.pane_widths.sidebar = 200 + (i % 10))
                    .expect("pane width save");
            }
        })
    };
    picker.join().expect("picker thread");
    dragger.join().expect("dragger thread");

    let s = settings::load_from(&file);
    assert_eq!(s.editor_tool, CUSTOM_ID, "the selection survived the race");
    assert_eq!(
        s.custom_editor_path,
        prog.to_string_lossy(),
        "the path survived the race — the pick mutates through settings::update, not load+save"
    );
    assert_eq!(
        s.pane_widths.sidebar,
        200 + ((ROUNDS - 1) % 10),
        "the other writer survived too"
    );
}

/// §5.1 reversibility (AC18's last clause): reverting to Auto-detect is the
/// ordinary `{ editorTool: "" }` patch and leaves `custom_editor_path` INTACT,
/// so re-selecting `"custom"` restores the tool. There is no "forget the path"
/// command to write here, by design.
#[test]
fn reverting_to_auto_detect_keeps_the_browsed_path_so_the_pick_is_restorable() {
    let (dir, file) = seeded();
    let prog = program(dir.path(), "restorable");
    commit_browsed_tool(&file, ToolKind::Editor, &prog, TargetOs::host()).expect("accepted");

    // The revert is a plain settings write, exactly what `apply_patch` performs.
    settings::update(&file, |s| s.editor_tool = String::new()).expect("revert");
    let s = settings::load_from(&file);
    assert_eq!(s.editor_tool, "");
    assert_eq!(
        s.custom_editor_path,
        prog.to_string_lossy(),
        "the path survives the revert"
    );

    settings::update(&file, |s| s.editor_tool = CUSTOM_ID.to_string()).expect("re-select");
    let s = settings::load_from(&file);
    assert_eq!(s.editor_tool, CUSTOM_ID);
    assert_eq!(s.custom_editor_path, prog.to_string_lossy());
}

/// Re-picking overwrites the stored path in place — one writer, one code path.
#[test]
fn a_second_pick_replaces_the_first() {
    let (dir, file) = seeded();
    let first = program(dir.path(), "first");
    let second = program(dir.path(), "second");

    commit_browsed_tool(&file, ToolKind::Terminal, &first, TargetOs::host()).expect("first");
    commit_browsed_tool(&file, ToolKind::Terminal, &second, TargetOs::host()).expect("second");

    let s = settings::load_from(&file);
    assert_eq!(s.custom_terminal_path, second.to_string_lossy());
    assert_eq!(s.terminal_tool, CUSTOM_ID);
}
