//! Tauri build script — it is also the only place next to `tauri.conf.json`
//! that can carry a comment, since that file is parsed as STRICT JSON
//! (no comments, and unknown keys are rejected by `deny_unknown_fields`).
//!
//! # P71 — `bundle.targets` omits `msi` on purpose
//!
//! NSIS is the ONE Windows artifact. The WiX/MSI relaunch custom action
//! (`LaunchApplication`, `Impersonate="yes"`) is executed by `msiexec.exe`'s own
//! process, so the relaunched app inherits msiexec's environment block instead
//! of the user's — a per-user Git/npm install on the User PATH becomes invisible
//! and every shell-out fails. NSIS relaunches via `nsis_tauri_utils::RunAsUser`,
//! which calls `CreateProcessWithTokenW` with `lpEnvironment = NULL`, i.e. an
//! environment built from the user's profile. See
//! `docs/contracts/P71-updater-relaunch-env.md` §1.2–§1.3.
//!
//! **Condition on any future return of the MSI artifact (D3):** it must come
//! back WITH (a) Authenticode signing (`bundle.windows.certificateThumbprint`
//! populated and a real signing pipeline) AND (b) the updater manifest still
//! pinned to NSIS (`updaterJsonPreferNsis: true` retained in
//! `.github/workflows/release.yml`, plus a test asserting `latest.json`'s
//! Windows URL ends in `-setup.exe`). Without (b), the relaunch defect
//! reappears silently the moment two Windows artifacts coexist.
//!
//! `targets` is filtered per host OS, so the list yields `nsis` on Windows,
//! `app` + `dmg` on macOS, and `deb`/`rpm`/`appimage` on Linux.

fn main() {
    tauri_build::build()
}
