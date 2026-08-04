# P42 — Packaging + Auto-Update — Architect Contract

Status: DESIGN. Implementer: `senior-dev`. Orchestrator commits.

## 1. Overview & goal

Two deliverables, priority in this order:

1. **Auto-update (centerpiece).** Integrate the Tauri v2 **updater plugin** so a released Bonsai
   can check for / download / install updates, with an in-app **"Check for updates"** control, an
   **update-available notification**, a **download→install→restart** flow with progress, and an
   **auto-check-on-launch** setting (prefs-gated, default OFF). No forced/silent auto-install.
2. **Packaging (secondary).** Per-OS release-installer bundle config (Windows NSIS+MSI, macOS
   dmg+app, Linux deb+AppImage), `createUpdaterArtifacts`, and **placeholders only** for
   code-signing (Windows Authenticode) + macOS notarization. Real certs/secrets are USER CHECKPOINT.

## 2. Invariants (non-negotiable)

- **INV-1 — Wrap the updater behind Bonsai's IPC-triple.** The React app NEVER imports
  `@tauri-apps/plugin-updater` / `@tauri-apps/plugin-process` directly. It only calls Bonsai
  `IpcApi` methods (`checkForUpdate`, `downloadAndInstallUpdate`, `relaunchApp`). The real impl in
  `src/ipc/tauri.ts` calls the plugins; `src/ipc/mock.ts` fakes them statefully. This is the same
  seam every other Bonsai feature uses and is what makes the whole update UI browser-harness
  verifiable (`pnpm dev:mock`, no Tauri present).
- **INV-2 — Harness-verifiable via a stateful mock seam.** `mock.ts` gates behaviour on a URL query
  read at module init (mirrors the existing `AI_OFF = query('ai') === 'off'` idiom):
  `?update=available` | `?update=none` | `?update=error`. `mock.ts` must always compile.
- **INV-3 — User-confirmed, never silent.** Auto-check may run on launch (setting), but download +
  install + relaunch are always explicit user actions. Default auto-check OFF.
- **INV-4 — Reuse the prefs store.** The auto-check flag is a new additive field on the existing
  `Settings` struct / `UiSettings` / `UiSettingsPatch` (`serde(default)`), not a new store.
- **INV-5 — No secrets in the repo.** Do NOT commit anything that signs or embeds a private key.
  The updater **public** key goes in `tauri.conf.json`; the private key + password are CI secrets.
- **INV-6 — File-size discipline.** New update UI lives in its own component files; the Settings
  "Updates" block is its own child component. Soft ~500-line limit.

## 3. Design decisions (with justification; USER CHECKPOINT flagged)

### D1 — Where the real updater logic lives: **JS plugin from `tauri.ts`, no custom Rust command**
The Tauri v2 updater flow is stateful JS-side: `check()` returns an `Update` handle that must be
held to call `.downloadAndInstall()`. Bridging that handle across a Rust command boundary means
re-implementing the update state machine and a handle registry in Rust for zero benefit. Therefore:
`tauri.ts` holds the `Update` object in a module-level variable between `checkForUpdate()` and
`downloadAndInstallUpdate()`, and Rust does **only** plugin registration + capabilities + the new
settings field. INV-1 is still satisfied because the React app talks solely to the `IpcApi`
wrapper. **Alternative (rejected):** thin `check_for_update`/`install_update` Rust commands holding
the handle in `AppState` — more code, more failure surface, no upside. Flag for orchestrator: if a
future need arises to drive updates headless from Rust (e.g. scheduler), revisit.

### D2 — Update endpoint: static `latest.json`, default GitHub Releases — **USER CHECKPOINT**
Recommend a static manifest served from the release host, defaulting to
`https://github.com/<OWNER>/<REPO>/releases/latest/download/latest.json`. The actual owner/repo/host
is the USER's to set. Put a clearly-marked placeholder in `tauri.conf.json` and document it in §9.

### D3 — Updater signing keypair: pubkey-in-config / private-key-in-CI-secret — **USER CHECKPOINT**
Tauri's updater signs artifacts with a **minisign** keypair, separate from OS code-signing.
Recommend: senior-dev generates a keypair (`pnpm tauri signer generate`), commits ONLY the
**public** key into `tauri.conf.json` `plugins.updater.pubkey`, and documents that the private key
+ password go into CI as `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The
private key is NEVER committed (INV-5). Orchestrator: confirm the pubkey-in-config /
private-key-in-secret split with the user.

### D4 — Auto-check on launch: **prefs-gated, default OFF**
Default OFF: avoids a surprise outbound network call on first launch before the user has opted in
(privacy) and keeps startup deterministic. A manual "Check for updates" button is always available.
When ON, App fires one `checkForUpdate()` after mount and shows the notification only if available.

### D5 — Install/restart UX: download-with-progress → confirm → install → prompt → relaunch
Flow: notification "vX available" → user opens `UpdateDialog` (shows version + notes) → "Download &
install" → progress bar (bytes) → on finish, "Restart now / Later" → `relaunchApp()` via
`tauri-plugin-process`. Windows `installMode`: **`passive`** (shows a minimal progress UI, no user
prompts, auto-closes) — smoothest for an app-driven update.

### D6 — Code-signing / notarization: **config placeholders only — USER CHECKPOINT**
Add empty/commented placeholders for Windows `certificateThumbprint` and macOS
`signingIdentity`/notarization env. Do NOT attempt to sign. Unsigned local `pnpm tauri build` must
still succeed and still emit updater artifacts (the AI gate).

### D7 — Error typing: add `updateFailed` to the `AppError` kind union
Updater failures are constructed in `tauri.ts`/`mock.ts` (the flow is JS-side). Add one kind,
`updateFailed`, for signature/parse/endpoint errors; genuine offline reuses existing `networkError`.

## 4. Config changes — `src-tauri/tauri.conf.json`

Add a top-level `plugins.updater` block and extend `bundle`:

```jsonc
{
  "bundle": {
    "active": true,
    "targets": "all",                 // NSIS+MSI (win), dmg+app (mac), deb+AppImage+rpm (linux)
    "createUpdaterArtifacts": true,   // emit *.sig + latest.json inputs for the updater
    "icon": [ /* unchanged */ ],
    "windows": {
      // PLACEHOLDER — USER CHECKPOINT (D6). Leave empty; do NOT invent a value.
      "certificateThumbprint": null,
      "nsis": { "installMode": "perMachine" }   // recommend; optional
    },
    "macOS": {
      // PLACEHOLDER — USER CHECKPOINT (D6): signingIdentity + notarization via env
      // (APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID) at build time. Leave unset.
    }
  },
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/OWNER/REPO/releases/latest/download/latest.json"  // PLACEHOLDER — D2
      ],
      "pubkey": "PLACEHOLDER_MINISIGN_PUBLIC_KEY",   // D3 — real pubkey committed by senior-dev
      "windows": { "installMode": "passive" }        // D5
    }
  }
}
```

`app.version` stays `0.1.0` (the release-cut/version-bump policy is out of scope for P42).

## 5. Rust changes

- **`src-tauri/Cargo.toml`** — add deps:
  ```toml
  tauri-plugin-updater = "2"
  tauri-plugin-process = "2"
  ```
- **`src-tauri/src/lib.rs`** — register both plugins alongside the existing `.plugin(...)` calls
  (updater is desktop-only; guard with `#[cfg(desktop)]` per Tauri convention if needed):
  ```rust
  .plugin(tauri_plugin_updater::Builder::new().build())
  .plugin(tauri_plugin_process::init())
  ```
  No new `invoke_handler` entries (D1). No custom updater command.
- **`src-tauri/capabilities/default.json`** — add plugin permissions (mirror `dialog:allow-open`):
  ```json
  "updater:default",
  "process:default"
  ```
  (`process:default` grants `relaunch`; `updater:default` grants `check`+`download_and_install`.)
- **`src-tauri/src/settings.rs`** — add ONE additive field to `Settings` (mirror `ai_enabled`):
  ```rust
  /// Auto-check for updates on launch (P42 D4). Default false; additive
  /// `#[serde(default)]`; a legacy file without this key loads as `false`.
  pub auto_check_updates: bool,
  ```
  Add `auto_check_updates: false` to `Settings::default()`. No clamp needed. Add a serde
  round-trip + legacy-back-compat test (mirror `old_settings_file_without_ai_fields_loads_defaults`).
- **`src-tauri/src/commands.rs`** — surface the field in `get_ui_settings` / `set_ui_settings`:
  add `auto_check_updates: s.auto_check_updates` to both `UiSettings { .. }` literals, and handle
  `patch.auto_check_updates` in `apply_patch`. The `UiSettings`/`UiSettingsPatch` Rust structs
  (wherever defined) each gain the field, mirroring existing bool settings.

## 6. IPC surface (the triple)

### 6.1 TypeScript types — `src/ipc/types.ts`

```ts
/** Result of IpcApi.checkForUpdate (P42). `available` false ⇒ up to date;
 *  version/notes/date populated only when available. currentVersion is always set. */
export interface UpdateCheckResult {
  available: boolean;
  currentVersion: string;
  /** Target version when available, else null. */
  version: string | null;
  /** Release notes (may be markdown/plain), else null. */
  notes: string | null;
  /** Publish date string from the manifest, else null. */
  date: string | null;
}

/** Streamed progress of downloadAndInstallUpdate (P42). Bytes are cumulative. */
export interface UpdateProgress {
  phase: 'started' | 'downloading' | 'finished';
  downloadedBytes: number;
  /** Total size when the manifest/server provides it, else null. */
  contentLength: number | null;
}
```

Add `'updateFailed'` to the `AppError['kind']` union (D7). Add `autoCheckUpdates: boolean` to
`UiSettings` and `autoCheckUpdates?: boolean` to `UiSettingsPatch`. Export `UpdateCheckResult` and
`UpdateProgress` from `src/ipc/index.ts`.

### 6.2 `IpcApi` methods — `src/ipc/types.ts`

```ts
/** Check the configured endpoint for a newer release. Resolves with availability
 *  + version metadata. Rejects AppError (`networkError` offline/unreachable,
 *  `updateFailed` bad signature/manifest). No-op safe to call repeatedly. */
checkForUpdate(): Promise<UpdateCheckResult>;

/** Download + install the update discovered by the most recent checkForUpdate,
 *  streaming byte progress via `onProgress` (bridged through a Channel in the
 *  Tauri impl; invoked directly in the mock). Resolves when the installer has
 *  applied the update; the app must then call relaunchApp() to restart.
 *  Rejects `noOperationInProgress` if no update was found first,
 *  `networkError`/`updateFailed` on transfer/verify failure. */
downloadAndInstallUpdate(onProgress: (p: UpdateProgress) => void): Promise<void>;

/** Restart the app to complete a finished update (tauri-plugin-process). Never
 *  resolves in practice (process exits). In the mock it is a logged no-op. */
relaunchApp(): Promise<void>;
```

### 6.3 Real impl — `src/ipc/tauri.ts`

- `import { check } from '@tauri-apps/plugin-updater'; import { relaunch } from '@tauri-apps/plugin-process'; import { getVersion } from '@tauri-apps/api/app';`
- Module-level `let pendingUpdate: Update | null = null;` (D1).
- `checkForUpdate()`: `const u = await check(); pendingUpdate = u;` then map to `UpdateCheckResult`
  (`available: !!u`, `currentVersion: await getVersion()`, `version: u?.version ?? null`,
  `notes: u?.body ?? null`, `date: u?.date ?? null`). Wrap thrown errors into `AppError`
  (`networkError` vs `updateFailed`).
- `downloadAndInstallUpdate(onProgress)`: if `pendingUpdate` null → reject `noOperationInProgress`.
  Call `pendingUpdate.downloadAndInstall(evt => …)` translating Tauri `DownloadEvent`
  (`Started{contentLength}` → phase `started` + set total; `Progress{chunkLength}` → accumulate
  bytes, phase `downloading`; `Finished` → phase `finished`) into `UpdateProgress`. A `Channel` is
  NOT required here (the plugin callback is already in-process); keep the `onProgress` signature to
  match the mock and match INV-1.
- `relaunchApp()`: `return relaunch();`

### 6.4 Stateful mock — `src/ipc/mock.ts`

- Module init: `const UPDATE_MODE = query('update'); // 'available' | 'none' | 'error' | null`
  (default treated as `'none'`). Constant `MOCK_CURRENT_VERSION = '0.1.0'`, `MOCK_NEXT_VERSION = '0.2.0'`.
- `checkForUpdate()`: `await delay(400)`. `error` → throw `AppError{kind:'networkError'}`.
  `available` → `{ available:true, currentVersion:'0.1.0', version:'0.2.0', notes:'- Mock release notes\n- Harness fixture', date:'2026-08-04' }`. else `{ available:false, currentVersion:'0.1.0', version:null, notes:null, date:null }`. Set a module flag `mockUpdateReady` when available.
- `downloadAndInstallUpdate(onProgress)`: if not `mockUpdateReady` → throw `AppError{kind:'noOperationInProgress'}`. Emit `started` (contentLength e.g. 5_000_000), ~15 `downloading` ticks accumulating bytes with `await delay(120)`, then `finished`. (`?update=error` may also throw mid-download to exercise the failure path — implementer's discretion.)
- `relaunchApp()`: `console.info('[mock] relaunch'); return;` (no reload — keeps harness state).
- `getUiSettings`/`setUiSettings` mock: add `autoCheckUpdates` to `readUiSettings()` and the
  `setUiSettings` merge (mirror `aiEnabled`), reading from the mock settings store.

## 7. Frontend

### 7.1 New components (each its own file)

- **`src/components/UpdateNotification.tsx`** — small dismissible banner/toast shown when an update
  is available: "Bonsai vX is available" + "View" (opens dialog) + dismiss. Presentational; props:
  `{ version: string; onView(): void; onDismiss(): void }`.
- **`src/components/UpdateDialog.tsx`** — `role="dialog"` overlay mirroring the existing
  `.dialog-overlay`/ConfirmDialog idiom. Shows current→target version + notes, a
  "Download & install" button, a progress bar during download, and a "Restart now / Later" prompt on
  finish. Props: `{ open; state: UpdateUiState; onDownload(): void; onRestart(): void; onClose(): void }`.
- **`src/components/SettingsUpdatesSection.tsx`** — Settings "Updates" block (mirror
  `SettingsMcpSection.tsx`): current version line, "Check for updates" button + result text
  (checking / up to date / vX available / error), and an **auto-check-on-launch** toggle. Props:
  `{ currentVersion: string; autoCheckUpdates: boolean; onToggleAutoCheck(v: boolean): void;
     checkState: UpdateCheckUiState; onCheck(): void; onOpenDialog(): void }`. Rendered by
  `SettingsPanel.tsx` (which gains matching pass-through props — do NOT inline the block).

### 7.2 Orchestration — `src/hooks/useUpdateController.ts` (recommended)

Extract the update state machine into a hook so `App` stays slim. Exposes a small enum + actions:

```ts
type UpdateUiState =
  | { status: 'idle' }
  | { status: 'checking' }
  | { status: 'upToDate' }
  | { status: 'available'; info: UpdateCheckResult }
  | { status: 'downloading'; info: UpdateCheckResult; progress: UpdateProgress }
  | { status: 'readyToRestart'; info: UpdateCheckResult }
  | { status: 'error'; message: string };
```

Actions: `check()` (→ `ipc.checkForUpdate`), `download()` (→ `ipc.downloadAndInstallUpdate` with a
progress setter → `readyToRestart` on resolve), `restart()` (→ `ipc.relaunchApp`), `dismiss()`.
`App` renders `<UpdateNotification>` when `status==='available'` and `<UpdateDialog>` when opened,
and passes the check-state + toggle down to `SettingsUpdatesSection` via `SettingsPanel`.

### 7.3 Auto-check-on-launch wiring — `App`

On mount, read `autoCheckUpdates` from loaded `UiSettings`; if true, call the hook's `check()` once.
The toggle in `SettingsUpdatesSection` fires `onChange({ autoCheckUpdates })` through the existing
`UiSettingsPatch` debounced-persist path.

### 7.4 `package.json` deps

```
dependencies:    @tauri-apps/plugin-updater ^2, @tauri-apps/plugin-process ^2
```
(`@tauri-apps/api` already present for `getVersion`.)

## 8. Sub-increments

- **P42a — plumbing (config + plugins + IPC-triple + mock).** tauri.conf.json (`plugins.updater`,
  `createUpdaterArtifacts`, bundle targets + signing PLACEHOLDERS), Cargo.toml + lib.rs plugin
  registration, capabilities, settings `auto_check_updates` field + commands wiring + Rust test,
  package.json deps, `types.ts` types + `AppError` kind + `IpcApi` methods, `tauri.ts` real impl,
  `mock.ts` stateful seam + settings field, `index.ts` exports. Gate: `cargo check`/`clippy`,
  `tsc`, `pnpm build` clean; `pnpm dev:mock` boots.
- **P42b — update UI + settings + harness verification.** `UpdateNotification`, `UpdateDialog`,
  `SettingsUpdatesSection`, `useUpdateController`, `SettingsPanel` + `App` wiring, auto-check-on-
  launch. Gate: harness verification (§10) of available→notify→download→restart, up-to-date, error.

Packaging/signing config stays in P42a as marked placeholders (no signing attempted).

## 9. What the user must provide (USER CHECKPOINT inputs)

1. **Endpoint** (D2): the real release host / owner / repo for the `latest.json` URL.
2. **Updater private key** (D3): after senior-dev generates the keypair, the user stores the
   PRIVATE key + password as CI secrets (`TAURI_SIGNING_PRIVATE_KEY` / `_PASSWORD`). Public key is
   committed.
3. **Code-signing** (D6): Windows Authenticode cert (thumbprint) and Apple Developer ID +
   notarization credentials — for signed release builds only.

## 10. AI gate vs USER CHECKPOINT

**AI gate (orchestrator-verifiable alone):**
- (a) `cargo check` + `cargo clippy` clean; `tsc` + `pnpm build` clean; new settings Rust test green.
- (b) `pnpm tauri build` **UNSIGNED** succeeds and produces updater artifacts (bundle + `*.sig` /
  `latest.json` inputs) — run in background, poll the log, never conclude failure from a timeout.
- (c) Browser harness (`pnpm dev:mock`) via the mock seam:
  - `?update=available` → notification appears → open dialog → "Download & install" → progress bar
    advances → "Restart now" prompt (relaunch is a mock no-op).
  - `?update=none` → "Check for updates" reports up to date; no notification.
  - `?update=error` → check surfaces the error state cleanly (no crash).
  - Settings "Updates" section renders current version + auto-check toggle; toggle persists via the
    mock settings store.
  Verify frugally (targeted console/text reads + one final screenshot).

**USER CHECKPOINT (orchestrator must NOT self-pass):**
- The real endpoint URL, the updater private key → CI secret, and code-signing certs (§9).
- The full **signed** download→install→relaunch round-trip against a live endpoint and a real signed
  release (requires the native installed app + real secrets).
- Native installer smoke (NSIS/MSI/dmg/deb/AppImage installs and launches on each OS).

## 11. Open questions for the orchestrator

- **Q1 (D2/D3):** Confirm GitHub Releases as the default host and the pubkey-in-config /
  private-key-in-secret split before senior-dev generates the keypair.
- **Q2:** Should P42 also introduce a version-bump/release checklist doc, or is that a later
  milestone? Recommend: out of scope for P42 (config + flow only).
- **Q3 (D5):** Windows `installMode` `passive` vs `basicUi` — recommend `passive`; confirm no
  requirement to show the full installer UI on update.
