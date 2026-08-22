//! Unit tests for [`super`] (`settings.rs`): file persistence, recent repos,
//! atomic writes, concurrent updates, and pane-width clamping.
//!
//! Kept in a sibling file so `settings.rs` stays closer to the ~500-line soft
//! limit. Declared with `#[path]` as a child module of `settings`, so
//! `super::*` still reaches the private items without widening their
//! visibility (the `external_tests` / `session_drain_tests` convention).

use super::*;

pub(super) fn settings_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("settings.json")
}

/// save_to then load_from round-trips the exact struct (P1 §3.3.1).
#[test]
fn roundtrip() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let mut s = Settings::default();
    record_recent(&mut s, "D:\\Repos\\x", 1_753_660_800);
    record_recent(&mut s, "D:\\Repos\\y", 1_753_660_900);

    save_to(&file, &s).expect("save settings");
    let loaded = load_from(&file);
    assert_eq!(loaded, s);
    assert_eq!(loaded.version, SETTINGS_VERSION);
    assert_eq!(loaded.recent_repos[0].path, "D:\\Repos\\y");
}

/// A missing file degrades to defaults, never errors (P1 §3.3.2).
#[test]
fn missing_file_defaults() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let loaded = load_from(&settings_path(&dir));
    assert_eq!(loaded, Settings::default());
    assert!(loaded.recent_repos.is_empty());
}

/// Corrupt JSON degrades to defaults, never errors (P1 §3.3.2).
#[test]
fn corrupt_json_defaults() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    std::fs::write(&file, "{nope").expect("write corrupt file");
    assert_eq!(load_from(&file), Settings::default());
}

/// Insert 12 distinct paths -> capped at 10, newest first; re-inserting an
/// existing path in a different case moves it to the front, dedupes, and
/// updates last_opened (P1 §3.3.3).
#[test]
fn record_recent_upserts_and_caps() {
    let mut s = Settings::default();
    for i in 0..12 {
        record_recent(&mut s, &format!("D:\\Repos\\repo-{i}"), 1000 + i);
    }
    assert_eq!(s.recent_repos.len(), MAX_RECENT_REPOS);
    assert_eq!(s.recent_repos[0].path, "D:\\Repos\\repo-11");
    assert_eq!(s.recent_repos[9].path, "D:\\Repos\\repo-2");

    // Re-insert repo-5 with different case: moved to front, deduped,
    // last_opened stamped.
    record_recent(&mut s, "d:\\repos\\REPO-5", 2000);
    assert_eq!(s.recent_repos.len(), MAX_RECENT_REPOS);
    assert_eq!(s.recent_repos[0].path, "d:\\repos\\REPO-5");
    assert_eq!(s.recent_repos[0].last_opened, 2000);
    assert_eq!(
        s.recent_repos
            .iter()
            .filter(|r| r.path.eq_ignore_ascii_case("D:\\Repos\\repo-5"))
            .count(),
        1
    );
}

/// Dedupe now goes through `same_repo_path`, so two spellings that
/// `fs::canonicalize` to the SAME directory collapse to one entry even
/// though they are not string-equal in any casing — the old
/// `eq_ignore_ascii_case` kept them as two rows.
#[test]
fn record_recent_dedupes_by_canonical_path() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).expect("create repo dir");
    // Same directory, spelled with a redundant `.` component + trailing sep.
    let alias = dir.path().join(".").join("repo").join("");

    let mut s = Settings::default();
    record_recent(&mut s, &repo.display().to_string(), 1);
    record_recent(&mut s, &alias.display().to_string(), 2);

    assert_eq!(s.recent_repos.len(), 1, "one physical dir => one entry");
    assert_eq!(s.recent_repos[0].last_opened, 2, "stamp refreshed");
}

/// Two DIFFERENT real directories stay two entries (the case-sensitive-FS
/// regression this fix targets: `Repo` and `repo` are distinct on ext4).
#[test]
fn record_recent_keeps_distinct_real_dirs_apart() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let a = dir.path().join("alpha");
    let b = dir.path().join("beta");
    std::fs::create_dir(&a).expect("create a");
    std::fs::create_dir(&b).expect("create b");

    let mut s = Settings::default();
    record_recent(&mut s, &a.display().to_string(), 1);
    record_recent(&mut s, &b.display().to_string(), 2);
    assert_eq!(s.recent_repos.len(), 2, "distinct dirs stay distinct");
}

/// Fallback preserved: when NEITHER side canonicalizes (a recents entry
/// whose folder was deleted) the old ASCII-case-insensitive compare still
/// dedupes, so a stale entry is refreshed rather than duplicated.
#[test]
fn record_recent_falls_back_for_unresolvable_paths() {
    let mut s = Settings::default();
    record_recent(&mut s, "D:\\Gone\\Deleted-Repo", 1);
    record_recent(&mut s, "d:\\gone\\deleted-repo", 2);
    assert_eq!(s.recent_repos.len(), 1, "unresolvable paths use the string compare");
    assert_eq!(s.recent_repos[0].last_opened, 2);
}

/// The atomic write leaves no `*.tmp` behind (P1 §3.3.4) — the tmp name is
/// unique per write now, so scan the whole dir instead of one fixed name.
#[test]
fn atomic_write_leaves_no_tmp() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let mut s = Settings::default();
    record_recent(&mut s, "D:\\Repos\\x", 1);
    save_to(&file, &s).expect("save settings");

    assert!(file.exists());
    assert!(!any_tmp_left(dir.path()));

    // Overwriting an existing file is also atomic (rename replaces).
    record_recent(&mut s, "D:\\Repos\\y", 2);
    save_to(&file, &s).expect("save settings again");
    assert!(!any_tmp_left(dir.path()));
    assert_eq!(load_from(&file), s);
}

/// True when any `*.tmp` file remains in `dir`.
fn any_tmp_left(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .expect("read dir")
        .filter_map(Result::ok)
        .any(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
}

/// Audit §2.3 regression: N threads × M `update` cycles on DISJOINT fields
/// all survive — no lost update reverts another thread's field. Before the
/// process-wide `SETTINGS_IO` lock, concurrent load→mutate→save cycles let
/// the last rename win and silently dropped the other writers' fields.
#[test]
fn concurrent_updates_of_disjoint_fields_all_survive() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    save_to(&file, &Settings::default()).expect("seed settings");

    const ROUNDS: u32 = 25;
    let handles: Vec<std::thread::JoinHandle<()>> = (0..4)
        .map(|writer: u32| {
            let file = file.clone();
            std::thread::spawn(move || {
                for i in 0..ROUNDS {
                    update(&file, |s| match writer {
                        0 => s.pane_widths.sidebar = SIDEBAR_MIN + (i % 10),
                        1 => s.mcp_token = Some(format!("token-{i}")),
                        2 => s.editor_command = format!("editor-{i}"),
                        _ => s.active_repo = Some(format!("repo-{i}")),
                    })
                    .expect("update settings");
                }
            })
        })
        .collect();
    for h in handles {
        h.join().expect("writer thread");
    }

    let last = ROUNDS - 1;
    let loaded = load_from(&file);
    assert_eq!(loaded.pane_widths.sidebar, SIDEBAR_MIN + (last % 10));
    assert_eq!(loaded.mcp_token.as_deref(), Some(format!("token-{last}").as_str()));
    assert_eq!(loaded.editor_command, format!("editor-{last}"));
    assert_eq!(loaded.active_repo.as_deref(), Some(format!("repo-{last}").as_str()));
    assert!(!any_tmp_left(dir.path()));
}

/// save_to creates missing parent directories.
#[test]
fn save_creates_parent_dirs() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = dir.path().join("nested").join("deeper").join("settings.json");
    save_to(&file, &Settings::default()).expect("save into nested dir");
    assert_eq!(load_from(&file), Settings::default());
}

/// Below-min and above-max on each axis clamp to the documented bounds;
/// in-range values pass through unchanged (P2a contract §3.4.1).
#[test]
fn clamp_pane_widths_clamps_both_axes() {
    assert_eq!(
        clamp_pane_widths(PaneWidths {
            sidebar: 10,
            right_panel: 10,
        }),
        PaneWidths {
            sidebar: SIDEBAR_MIN,
            right_panel: RIGHT_PANEL_MIN,
        }
    );
    assert_eq!(
        clamp_pane_widths(PaneWidths {
            sidebar: 9999,
            right_panel: 9999,
        }),
        PaneWidths {
            sidebar: SIDEBAR_MAX,
            right_panel: RIGHT_PANEL_MAX,
        }
    );
    let in_range = PaneWidths {
        sidebar: 300,
        right_panel: 400,
    };
    assert_eq!(clamp_pane_widths(in_range), in_range);
}

// --- P82: color-coded identity profiles -------------------------------------

/// A profile with the required pre-P82 fields but no `color` key (a
/// settings.json written before P82).
fn legacy_profile_json() -> serde_json::Value {
    serde_json::json!({
        "id": "p-legacy",
        "label": "Work",
        "userName": "Alice",
        "userEmail": "alice@example.com",
        "signingKey": null,
    })
}

/// AC(a): a legacy profile without `color` deserializes to `Neutral` — the
/// field-level `#[serde(default)]` is what makes an old `settings.json` load.
#[test]
fn legacy_profile_without_color_deserializes_to_neutral() {
    let p: IdentityProfile =
        serde_json::from_value(legacy_profile_json()).expect("legacy profile must deserialize");
    assert_eq!(p.color, ProfileColor::Neutral);
    assert_eq!(p.id, "p-legacy");
    assert_eq!(p.signing_key, None);
}

/// AC(a): a whole `Settings` blob whose `profiles[*]` omit `color` still loads
/// (container-level default does NOT cover a missing field on a Vec element).
#[test]
fn legacy_settings_blob_with_colorless_profile_loads() {
    let blob = serde_json::json!({
        "version": SETTINGS_VERSION,
        "profiles": [ legacy_profile_json() ],
    });
    let s: Settings = serde_json::from_value(blob).expect("settings with legacy profile");
    assert_eq!(s.profiles.len(), 1);
    assert_eq!(s.profiles[0].color, ProfileColor::Neutral);
}

/// AC(b): every ProfileColor variant round-trips as its camelCase tag.
#[test]
fn profile_color_wire_shape_is_camel_case_all_variants() {
    let cases = [
        (ProfileColor::Neutral, "neutral"),
        (ProfileColor::Slate, "slate"),
        (ProfileColor::Blue, "blue"),
        (ProfileColor::Teal, "teal"),
        (ProfileColor::Green, "green"),
        (ProfileColor::Amber, "amber"),
        (ProfileColor::Orange, "orange"),
        (ProfileColor::Purple, "purple"),
        (ProfileColor::Pink, "pink"),
    ];
    for (variant, wire) in cases {
        let json = serde_json::to_value(variant).expect("serialize color");
        assert_eq!(json, serde_json::json!(wire), "{variant:?} serializes to {wire}");
        let back: ProfileColor =
            serde_json::from_value(serde_json::json!(wire)).expect("deserialize color");
        assert_eq!(back, variant, "{wire} round-trips to {variant:?}");
    }
}

/// AC(b): a full `IdentityProfile { color: Blue }` round-trips, and the JSON
/// carries `"color":"blue"`.
#[test]
fn identity_profile_with_color_roundtrips() {
    let original = IdentityProfile {
        id: "p1".to_string(),
        label: "Work".to_string(),
        user_name: "Alice".to_string(),
        user_email: "alice@example.com".to_string(),
        signing_key: Some("ABCDEF".to_string()),
        color: ProfileColor::Blue,
    };
    let json = serde_json::to_value(&original).expect("serialize profile");
    assert_eq!(json["color"], serde_json::json!("blue"));
    let back: IdentityProfile = serde_json::from_value(json).expect("deserialize profile");
    assert_eq!(back, original);
}

/// AC(a): a full `Settings` carrying colored profiles survives save_to/load_from
/// on disk unchanged (the color rides the whole-array persist path).
#[test]
fn colored_profiles_survive_save_load_roundtrip() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let s = Settings {
        profiles: vec![
            IdentityProfile {
                id: "w".to_string(),
                label: "Work".to_string(),
                user_name: "Alice".to_string(),
                user_email: "alice@work.com".to_string(),
                signing_key: None,
                color: ProfileColor::Blue,
            },
            IdentityProfile {
                id: "p".to_string(),
                label: "Personal".to_string(),
                user_name: "Alice".to_string(),
                user_email: "alice@home.com".to_string(),
                signing_key: None,
                color: ProfileColor::Green,
            },
        ],
        ..Settings::default()
    };
    save_to(&file, &s).expect("save settings");
    let loaded = load_from(&file);
    assert_eq!(loaded.profiles, s.profiles);
}

// --- git-hook disclosure ack (per-repo) -------------------------------------

/// `set_hooks_ack` is idempotent (re-acking never grows the list),
/// `hooks_ack_contains` reflects membership, and the whole thing survives
/// save_to/load_from on disk.
#[test]
fn hooks_ack_roundtrip() {
    let mut s = Settings::default();
    assert!(!hooks_ack_contains(&s, "D:\\Repos\\x"));

    set_hooks_ack(&mut s, "D:\\Repos\\x");
    assert!(hooks_ack_contains(&s, "D:\\Repos\\x"));
    assert_eq!(s.hooks_ack_repos.len(), 1);

    // Idempotent: the same path (and, for unresolvable temp paths, an ASCII-case
    // variant — the `same_repo_path` string fallback) does not push a duplicate.
    set_hooks_ack(&mut s, "D:\\Repos\\x");
    set_hooks_ack(&mut s, "d:\\repos\\x");
    assert_eq!(s.hooks_ack_repos.len(), 1, "re-ack must not grow the list");

    set_hooks_ack(&mut s, "D:\\Repos\\y");
    assert_eq!(s.hooks_ack_repos.len(), 2);
    assert!(hooks_ack_contains(&s, "D:\\Repos\\y"));

    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    save_to(&file, &s).expect("save settings");
    let loaded = load_from(&file);
    assert_eq!(loaded.hooks_ack_repos, s.hooks_ack_repos);
    assert!(hooks_ack_contains(&loaded, "D:\\Repos\\x"));
}

/// Additive-load: a pre-existing settings.json with NO `hooksAckRepos` key loads
/// `[]` (the container-level `#[serde(default)]`; no version bump).
#[test]
fn settings_without_hooks_ack_repos_loads_empty() {
    let blob = serde_json::json!({
        "version": SETTINGS_VERSION,
        "recentRepos": [],
    });
    let s: Settings = serde_json::from_value(blob).expect("legacy settings must deserialize");
    assert!(s.hooks_ack_repos.is_empty());
}
