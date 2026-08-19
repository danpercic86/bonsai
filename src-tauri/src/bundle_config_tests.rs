//! Guards on `src-tauri/tauri.conf.json` that must stay true for the P71
//! update channel to work.
//!
//! `tauri.conf.json` is strict JSON — it cannot carry a comment — so the
//! rationale lives in `build.rs` and here. Prose is what nobody reads while
//! editing a config file; a red test is not. This is the class of check that
//! would have caught the original defect: `"targets": "all"` silently emitted a
//! WiX/MSI artifact whose relaunch custom action runs inside `msiexec.exe`, so
//! the updated app inherited msiexec's environment block instead of the user's
//! (`docs/contracts/P71-updater-relaunch-env.md` §1.2, D3, M-1).

use serde_json::Value;

/// Parse the shipped `tauri.conf.json`. Failing to read or parse it is itself a
/// finding, so this asserts rather than skipping.
fn tauri_conf() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json");
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("tauri.conf.json must be readable at {path}: {e}"));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("tauri.conf.json must be valid JSON: {e}"))
}

fn bundle_targets(conf: &Value) -> Vec<String> {
    let targets = conf
        .get("bundle")
        .and_then(|b| b.get("targets"))
        .unwrap_or_else(|| panic!("bundle.targets must be present"));
    let list = targets.as_array().unwrap_or_else(|| {
        panic!(
            "bundle.targets must be an explicit ARRAY, not {targets}. \
             P71/D3: `\"all\"` re-enables the WiX/MSI artifact, whose relaunch \
             custom action runs inside msiexec.exe and hands the updated app \
             msiexec's environment block — the P71 root cause."
        )
    });
    list.iter()
        .map(|t| {
            t.as_str()
                .unwrap_or_else(|| panic!("every bundle.targets entry must be a string"))
                .to_string()
        })
        .collect()
}

#[test]
fn bundle_targets_never_include_msi() {
    let conf = tauri_conf();
    let targets = bundle_targets(&conf);
    assert!(
        !targets.iter().any(|t| t.eq_ignore_ascii_case("msi")),
        "bundle.targets must NOT contain \"msi\" (found {targets:?}).\n\
         P71/D3: NSIS is the ONE Windows artifact. The WiX/MSI relaunch custom \
         action (`LaunchApplication`, Impersonate=\"yes\") is executed by \
         msiexec.exe's own process, so the relaunched app inherits msiexec's \
         environment block instead of the user's: every directory that lives \
         only in HKCU\\Environment\\Path (a per-user Git, %APPDATA%\\npm, a \
         per-user editor) disappears and every shell-out fails.\n\
         If MSI must come back for enterprise deployment it returns ONLY with \
         (a) Authenticode signing and (b) updaterJsonPreferNsis: true retained \
         in .github/workflows/release.yml. See \
         docs/contracts/P71-updater-relaunch-env.md §1.2 and D3."
    );
    assert!(
        targets.iter().any(|t| t == "nsis"),
        "bundle.targets must still contain \"nsis\" — it is the only Windows \
         installer, and the only one whose relaunch (RunAsUser => \
         CreateProcessWithTokenW with lpEnvironment = NULL) builds the \
         environment from the user's profile. Found {targets:?}."
    );
}

#[test]
fn windows_updater_install_mode_is_still_passive() {
    // NSIS maps "passive" to /P and, with restart_after_install, to /R — the
    // pair that routes the relaunch through `nsis_tauri_utils::RunAsUser`
    // (contract §1.3, D2).
    let conf = tauri_conf();
    let mode = conf
        .get("plugins")
        .and_then(|p| p.get("updater"))
        .and_then(|u| u.get("windows"))
        .and_then(|w| w.get("installMode"))
        .and_then(Value::as_str);
    assert_eq!(
        mode,
        Some("passive"),
        "plugins.updater.windows.installMode must stay \"passive\" (M-1)"
    );
}
