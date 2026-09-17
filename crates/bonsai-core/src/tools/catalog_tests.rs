//! Catalog-integrity tests (P112 AC8) and the label maps (AC12).
//!
//! These are whole-table invariants, asserted exhaustively rather than
//! spot-checked: the table is the one place a typo silently turns into "a tool
//! the picker can never offer", or — worse, for `CUSTOM_ID` — into a detected
//! row impersonating the browsed-path row.

use std::collections::{BTreeMap, BTreeSet};

use crate::external::TargetOs;

use super::catalog::{
    auto_rungs, entries_any_os, entries_for, find, find_for, AutoVia, Recipe, Rung, CATALOG,
    CUSTOM_ID, LEGACY_ALIASES,
};
use super::{label_map, ToolKind};

const OSES: [TargetOs; 3] = [TargetOs::Windows, TargetOs::MacOs, TargetOs::Linux];
const KINDS: [ToolKind; 2] = [ToolKind::Terminal, ToolKind::Editor];

#[test]
fn ids_are_unique_per_kind_and_os() {
    // Keyed by NAME rather than by the enum so `ToolKind` keeps exactly the
    // derives the contract specifies.
    let mut seen: BTreeSet<(&str, &str, &str)> = BTreeSet::new();
    for e in CATALOG {
        let key = (kind_name(e.kind), os_name(e.os), e.id);
        assert!(
            seen.insert(key),
            "duplicate catalog id {:?} for {:?} on {}",
            e.id,
            e.kind,
            os_name(e.os)
        );
    }
}

#[test]
fn no_catalog_id_equals_the_custom_pseudo_id() {
    // A collision would let a detected tool impersonate the browsed-path row —
    // the one row whose target is not from this table.
    for e in CATALOG {
        assert_ne!(e.id, CUSTOM_ID, "catalog id collides with CUSTOM_ID");
    }
}

#[test]
fn every_entry_has_a_label_a_program_and_at_least_one_rung() {
    for e in CATALOG {
        assert!(!e.id.is_empty(), "empty id");
        assert!(!e.label.is_empty(), "{} has no label", e.id);
        assert!(!e.program.is_empty(), "{} has no program", e.id);
        assert!(!e.rungs.is_empty(), "{} has no rungs", e.id);
    }
}

#[test]
fn built_in_is_an_entrys_only_rung() {
    // A rung after `BuiltIn` would be unreachable: `BuiltIn` always hits.
    for e in CATALOG {
        if e.rungs.iter().any(|r| matches!(r, Rung::BuiltIn)) {
            assert_eq!(e.rungs.len(), 1, "{} mixes BuiltIn with other rungs", e.id);
        }
    }
}

#[test]
fn mac_open_entries_are_mac_os_and_name_an_app() {
    for e in CATALOG {
        if e.recipe == Recipe::MacOpen {
            assert_eq!(e.os, TargetOs::MacOs, "{} is MacOpen but not macOS", e.id);
            assert!(e.app_name.is_some(), "{} is MacOpen with no app_name", e.id);
            // Such a row can only resolve through a `Bundle` rung, so "open" IS
            // the program that launches it.
            assert_eq!(e.program, "open", "{} is MacOpen with a program", e.id);
        }
    }
}

#[test]
fn bundle_rungs_only_appear_on_macos_rows() {
    for e in CATALOG {
        if e.rungs.iter().any(|r| matches!(r, Rung::Bundle { .. })) {
            assert_eq!(
                e.os,
                TargetOs::MacOs,
                "{} has a Bundle rung off macOS",
                e.id
            );
        }
    }
}

#[test]
fn the_catalog_has_exactly_the_rows_the_contract_lists() {
    // Pins the count two comments assert but nothing guarded:
    // `catalog_table.rs`'s macro doc ("rather than 35 spelled-out struct
    // literals") and `docs/contracts/P112-tool-catalog.md`'s table. A row added
    // here without a contract row (or vice versa) now fails instead of drifting.
    // Bump this deliberately, together with both.
    assert_eq!(CATALOG.len(), 35);
}

#[test]
fn every_app_paths_row_also_has_a_well_known_folder_rung() {
    // The premise `SCAN_REG_BUDGET`'s doc (`detect.rs`) rests on: a cold scan
    // can spend the whole 1.5 s registry budget on the PATH walk before the
    // first `AppPaths` rung runs, after which every one of them yields `None`.
    // That is only SAFE because a default install is still found by a
    // `WinFolder` rung — an `AppPaths`-ONLY row would silently become
    // undetectable on a cold boot. Host-independent on purpose: the host scan
    // test asserts per-row shape and never that a `Registry` rung resolved,
    // which is exactly why that starvation was silent.
    for e in CATALOG {
        if e.rungs.iter().any(|r| matches!(r, Rung::AppPaths { .. })) {
            assert!(
                e.rungs.iter().any(|r| matches!(r, Rung::WinFolder { .. })),
                "{} has an AppPaths rung with no WinFolder fallback: an \
                 exhausted SCAN_REG_BUDGET makes it undetectable",
                e.id
            );
        }
    }
}

#[test]
fn every_auto_rung_resolves_for_its_own_os() {
    for kind in KINDS {
        for os in OSES {
            for rung in auto_rungs(kind, os) {
                let entry = find_for(kind, rung.id, os).unwrap_or_else(|| {
                    panic!(
                        "auto rung {:?} has no {:?} row on {}",
                        rung.id,
                        kind,
                        os_name(os)
                    )
                });
                // `MacApp` launches `open -a <app_name>`, so a `None` there
                // would emit `open -a code` instead of the app name.
                if rung.via == AutoVia::MacApp {
                    assert!(
                        entry.app_name.is_some(),
                        "MacApp auto rung {:?} has no app_name",
                        rung.id
                    );
                }
            }
        }
    }
}

#[test]
fn the_auto_ladders_are_the_ones_the_hardcoded_ladders_used() {
    // A snapshot of AC9's expectation at the DATA level: `external.rs`'s
    // ladders are `wt -d` / `powershell` / `cmd /K`, `open -a Terminal`,
    // `gnome-terminal` / `konsole` / `x-terminal-emulator`, and VS Code then
    // Insiders (with the macOS app-name pair first, then the `code` stub).
    let listed = |kind, os| {
        auto_rungs(kind, os)
            .iter()
            .map(|r| (r.id, r.via))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        listed(ToolKind::Terminal, TargetOs::Windows),
        vec![
            ("windows-terminal", AutoVia::Name),
            ("powershell", AutoVia::Name),
            ("cmd", AutoVia::Name)
        ]
    );
    assert_eq!(
        listed(ToolKind::Terminal, TargetOs::MacOs),
        vec![("apple-terminal", AutoVia::MacApp)]
    );
    assert_eq!(
        listed(ToolKind::Terminal, TargetOs::Linux),
        vec![
            ("gnome-terminal", AutoVia::Name),
            ("konsole", AutoVia::Name),
            ("x-terminal-emulator", AutoVia::Name)
        ]
    );
    for os in [TargetOs::Windows, TargetOs::Linux] {
        assert_eq!(
            listed(ToolKind::Editor, os),
            vec![
                ("vscode", AutoVia::Name),
                ("vscode-insiders", AutoVia::Name)
            ]
        );
    }
    assert_eq!(
        listed(ToolKind::Editor, TargetOs::MacOs),
        vec![
            ("vscode", AutoVia::MacApp),
            ("vscode-insiders", AutoVia::MacApp),
            ("vscode", AutoVia::Name)
        ]
    );
}

#[test]
fn every_legacy_alias_target_exists_in_the_catalog() {
    for ((kind, legacy), target) in LEGACY_ALIASES {
        assert!(!target.is_empty(), "alias {legacy} has an empty target");
        assert_ne!(*target, CUSTOM_ID, "alias {legacy} targets CUSTOM_ID");
        assert!(
            find(*kind, target).is_some(),
            "legacy alias {legacy} targets unknown id {target}"
        );
    }
}

#[test]
fn legacy_aliases_are_normalised_and_unique_per_kind() {
    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    for ((kind, legacy), _) in LEGACY_ALIASES {
        assert_eq!(
            *legacy,
            legacy.to_lowercase(),
            "alias {legacy} is not lowercase, so the exact lookup can never hit"
        );
        assert!(
            seen.insert((kind_name(*kind), legacy)),
            "duplicate alias {legacy}"
        );
    }
}

// ---- the lookups -------------------------------------------------------------

#[test]
fn find_for_is_exact_os_which_is_what_an_os_explicit_caller_needs() {
    // The mac and Windows `vscode` rows share an id, but only the mac one has
    // an `app_name`. A host-relative lookup would hand a Windows host the
    // Windows row while building the macOS ladder — which is why the auto
    // ladder must use `find_for`, not `find`.
    let mac = find_for(ToolKind::Editor, "vscode", TargetOs::MacOs).expect("mac vscode row");
    assert_eq!(mac.app_name, Some("Visual Studio Code"));
    let win = find_for(ToolKind::Editor, "vscode", TargetOs::Windows).expect("win vscode row");
    assert_eq!(win.app_name, None);
    assert_eq!(
        find_for(ToolKind::Editor, "vscode", TargetOs::Linux).map(|e| e.os),
        Some(TargetOs::Linux)
    );
    assert_eq!(
        find_for(ToolKind::Terminal, "windows-terminal", TargetOs::Linux),
        None
    );
}

#[test]
fn find_falls_back_to_any_os_so_a_synced_setting_still_resolves() {
    // Host-OS row first, then any OS: a settings file synced from another
    // machine must still be nameable and keepable.
    for (kind, id) in [
        (ToolKind::Terminal, "apple-terminal"),
        (ToolKind::Terminal, "gnome-terminal"),
        (ToolKind::Terminal, "windows-terminal"),
        (ToolKind::Editor, "notepadpp"),
        (ToolKind::Editor, "kate"),
        (ToolKind::Editor, "zed"),
    ] {
        assert!(find(kind, id).is_some(), "{id} must resolve on any host");
    }
    assert_eq!(find(ToolKind::Terminal, ""), None);
    assert_eq!(find(ToolKind::Terminal, "no-such-tool"), None);
    // Kind-scoped: an editor id is not a terminal id.
    assert_eq!(find(ToolKind::Terminal, "vscode"), None);
    assert_eq!(find(ToolKind::Editor, "cmd"), None);
}

#[test]
fn entries_for_is_kind_and_os_scoped() {
    for kind in KINDS {
        for os in OSES {
            for e in entries_for(kind, os) {
                assert_eq!(e.kind, kind);
                assert_eq!(e.os, os);
            }
        }
    }
    // Every row is reachable through some (kind, os) pair.
    let reachable: usize = KINDS
        .iter()
        .flat_map(|k| OSES.iter().map(move |o| entries_for(*k, *o).count()))
        .sum();
    assert_eq!(reachable, CATALOG.len());
}

// ---- AC12: the label maps ----------------------------------------------------

#[test]
fn label_map_covers_every_id_of_that_kind_on_every_os() {
    for kind in KINDS {
        let map = label_map(kind);
        for e in entries_any_os(kind) {
            assert_eq!(
                map.get(e.id).map(String::as_str),
                Some(e.label),
                "label_map({kind:?}) is missing or mislabels {}",
                e.id
            );
        }
        // And contains nothing else — ids only, no invented rows.
        assert_eq!(
            map.len(),
            entries_any_os(kind)
                .map(|e| e.id)
                .collect::<BTreeSet<_>>()
                .len()
        );
    }
}

#[test]
fn label_map_is_well_defined_across_oses() {
    // A macOS-authored settings file must render on Windows, so an id shared
    // between OSes must not carry two different labels.
    for kind in KINDS {
        let mut by_id: BTreeMap<&str, &str> = BTreeMap::new();
        for e in entries_any_os(kind) {
            if let Some(prev) = by_id.insert(e.id, e.label) {
                assert_eq!(
                    prev, e.label,
                    "id {} has two labels across OSes ({prev} / {})",
                    e.id, e.label
                );
            }
        }
    }
}

#[test]
fn the_label_maps_do_not_contain_the_custom_pseudo_id() {
    // The browsed row's label is derived from its path by the backend, never
    // taken from a map.
    for kind in KINDS {
        assert!(!label_map(kind).contains_key(CUSTOM_ID));
    }
}

fn kind_name(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Terminal => "terminal",
        ToolKind::Editor => "editor",
    }
}

fn os_name(os: TargetOs) -> &'static str {
    match os {
        TargetOs::Windows => "windows",
        TargetOs::MacOs => "macos",
        TargetOs::Linux => "linux",
    }
}
