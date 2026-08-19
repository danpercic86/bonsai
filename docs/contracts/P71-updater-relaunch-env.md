# P71 — Auto-update relaunch inherits the installer's environment

Make the post-auto-update process have the **same environment it would have if launched from the
Start menu**. Today it does not: the Windows MSI updater path relaunches `bonsai.exe` as a child of
`msiexec.exe` with msiexec's environment block.

References read (@ `71236f8` + P70 working tree): `src-tauri/tauri.conf.json`,
`.github/workflows/release.yml`, `src-tauri/src/lib.rs` (plugin registration),
`src/ipc/tauri.ts` §update, `src/hooks/useUpdateController.ts`,
`crates/bonsai-core/src/{gitbin.rs,gitbin_preflight.rs,procutil.rs,external.rs,ai/mod.rs}`,
`crates/bonsai-forge/src/http.rs`, `Cargo.lock` (tauri 2.11.5, tauri-plugin-updater 2.10.1),
`pnpm-lock.yaml` (@tauri-apps/cli 2.11.4). Upstream sources read: Tauri v2 updater
`plugins/updater/src/{updater.rs,config.rs}`, bundler `nsis/installer.nsi`, bundler `msi/main.wxs`,
`nsis-tauri-utils` `crates/nsis-process`, MSDN `CreateProcessWithTokenW`, tauri-action README.
House pattern: `P70-git-resolution.md`, `P42-packaging-autoupdate.md`.

**Root cause is already verified on the affected machine — do not re-derive.** `Win32_Process`
showed `C:\Program Files\Bonsai\bonsai.exe` with `ParentProcessId` = an `msiexec.exe`; a
separately-launched build parented to `explorer.exe` behaved correctly. This contract explains
*why*, and designs the fix.

**Not urgent.** P70 already rescues `git` specifically. P71 is the general fix — see §2, the blast
radius P70 does *not* cover.

*Decisions §9 Q-1 … Q-4 closed by the orchestrator 2026-08-19: R1 approved as the primary fix; MSI
dropped entirely; R2 approved as a second increment with tightened scope; `perMachine` retained;
Q-4 rewritten as a FOR-USER item (§10).*

---

## 0. Key decisions

**D1 — The fix is `R1`: move the Windows update channel to the NSIS artifact only.** *(APPROVED
2026-08-19.)* The MSI's relaunch is broken *by construction* (§1.2); the NSIS relaunch is correct
*by construction* (§1.3). This is a config + workflow change with **zero code, zero IPC, zero UI**
change. Rejected alternatives in §4.

**D2 — Keep `restart_after_install` at its default (`true`).** Under NSIS this routes the relaunch
through `nsis_tauri_utils::RunAsUser`, which is exactly the mechanism that produces a correct
environment. Disabling it does *not* buy an in-app "relaunch" prompt (§4, R3) because the app
process is already dead by then.

**D3 — Drop `msi` from `bundle.targets` entirely; do not merely de-prioritise it.** *(APPROVED
2026-08-19.)* Two Windows installers with different install *and relaunch* semantics is the root
cause. Keeping the MSI as a "manual download only" artifact preserves the footgun — a manually
MSI-installed client still auto-updates through a foreign installer — for a use case (GPO/enterprise
deployment) that is not viable today anyway, because it needs Authenticode signing first.
`updaterJsonPreferNsis: true` is set **anyway** as belt-and-braces so re-enabling `msi` later cannot
silently re-break the update channel.

> **Condition on any future return of the MSI artifact.** If MSI comes back for enterprise
> deployment, it must return with **both** (a) Authenticode signing
> (`bundle.windows.certificateThumbprint` populated and a real signing pipeline) **and** (b) the
> updater manifest still explicitly pinned to NSIS (`updaterJsonPreferNsis: true` retained, and a
> test asserting `latest.json`'s Windows URL ends in `-setup.exe`). Without (b) the relaunch defect
> documented in §1.2 silently reappears the moment two Windows artifacts coexist.

**D4 — `nsis.installMode` stays `perMachine`.** *(APPROVED 2026-08-19, out of scope.)* It has no
bearing on correctness (§1.3 covers both the elevated and unelevated `RunAsUser` branches).
Switching to `currentUser` would remove the UAC prompt from every update, but it is a product
decision about install scope and would force a one-time reinstall for every existing client.

**D5 — Ship the `R2` PATH-rehydration backstop as a SECOND increment, after R1 lands.** *(APPROVED
2026-08-19.)* The decisive argument is not defence-in-depth: **R1 does nothing for clients already
installed via MSI** — including the user who reported this. Their environment stays foreign until
they reinstall. R2 is what repairs them *in place*. Scope is tightened in §5 and is deliberately
narrow: it is a **PATH patch, not an environment repair**, and it does **not** make R1 optional.

**D6 — Nothing in this milestone touches `.tauri/updater-prod.key`, the pubkey in
`tauri.conf.json`, or the signing secrets.** The artifact *selection* changes; the trust chain does
not. Both NSIS and MSI updater artifacts are already signed by the same key
(`createUpdaterArtifacts: true` emits a `.sig` per artifact), so pinning the manifest to the NSIS
one requires no key operation of any kind.

---

## 1. Evidence

### 1.1 Q1 — Which artifact does the updater select when both exist?

**Not the updater's choice at all. It is whatever the release workflow writes into `latest.json`,
and today that is the MSI — by silent default.**

- The client only ever sees ONE Windows URL. `latest.json` has a single
  `platforms["windows-x86_64"] = { signature, url }` entry. `tauri-plugin-updater` downloads that
  URL and branches on the file extension (`.msi` → msiexec, `.exe` → NSIS). It has no artifact
  preference logic.
- `tauri-apps/tauri-action` generates that manifest (`includeUpdaterJson: true`, release.yml:135).
  Its README input:
  > `updaterJsonPreferNsis` — Whether the action will use the NSIS (setup.exe) or WiX (.msi)
  > bundles for the updater JSON if both types exist. **default: false (for legacy reasons)**
- `.github/workflows/release.yml` does **not** set it. `bundle.targets: "all"` produces both.
  Therefore v1.0.0's `latest.json` points at the **MSI**.

**Conclusion: the MSI was picked incidentally, by an upstream legacy default we never overrode.**
Pinning the manifest to NSIS is one line and is the core of the fix.

*Verification step for the implementer (no build needed):*
`curl -sL https://github.com/danpercic86/bonsai/releases/latest/download/latest.json` and read
`.platforms["windows-x86_64"].url` — it must currently end in `.msi`. This is the "before" datum.

### 1.2 Q2a — Why the MSI relaunch is broken

`tauri-plugin-updater` 2.10.1, Windows MSI branch:

```rust
let mut installer_args: Vec<&OsStr> = vec![OsStr::new("/i"), path.as_os_str()];
installer_args.extend(install_mode.msiexec_args().iter().map(OsStr::new)); // passive => "/passive"
installer_args.push(OsStr::new("/promptrestart"));
if self.context.restart_after_install {                 // default: true
    msi_current_exe_arg = format!("LAUNCHAPPARGS=\"{}\"", ...);
    installer_args.extend(install_mode.msi_restart_after_install_args()); // "AUTOLAUNCHAPP=True"
    installer_args.push(OsStr::new(&msi_current_exe_arg));
}
// ... ShellExecuteW(null, "open", msiexec, parameters, null, SW_SHOW) ...
std::process::exit(0);
```

`AUTOLAUNCHAPP=True` fires this custom action in the bundler's `msi/main.wxs`:

```xml
<CustomAction Id="LaunchApplication" Impersonate="yes" FileKey="Path"
              ExeCommand="[LAUNCHAPPARGS]" Return="asyncNoWait" />
<Custom Action="LaunchApplication" After="InstallFinalize">AUTOLAUNCHAPP AND NOT Installed</Custom>
```

A `FileKey`+`ExeCommand` custom action is executed **by msiexec's own process**. `Impersonate="yes"`
gets the *security token* right (it runs as the installing user) but says nothing about the
**environment block** — the new process inherits msiexec's, which is not the interactive user's.
This is exactly the observed `ParentProcessId = msiexec.exe`. There is no configuration knob that
fixes it; the only escape is a forked WiX template (§4, R4).

### 1.3 Q2b — Does NSIS have the same problem? **No — and for a documented reason.**

NSIS branch of the same function:

```rust
installer_args.extend(install_mode.nsis_args());                     // passive => "/P"
installer_args.push(OsStr::new("/UPDATE"));
if self.context.restart_after_install {
    installer_args.extend(install_mode.nsis_restart_after_install_args()); // passive => "/R"
    installer_args.push(OsStr::new("/ARGS"));
    installer_args.extend(nsis_current_exe_arg);
}
```

Bundler `nsis/installer.nsi`:

```nsis
Function .onInstSuccess
  ${If} $PassiveMode = 1
  ${OrIf} ${Silent}
    ${GetOptions} $CMDLINE "/R" $R0
    ${IfNot} ${Errors}
      ${GetOptions} $CMDLINE "/ARGS" $R0
      nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" "$R0"
    ${EndIf}
  ${EndIf}
FunctionEnd
```

`nsis-tauri-utils` `RunAsUser` (crates/nsis-process): when elevated it takes the **shell window's
process token** (i.e. `explorer.exe`'s), `DuplicateTokenEx`es it, and calls:

```rust
CreateProcessWithTokenW(*handle_new_token, 0, program_wide.as_ptr(),
                        command_line_wide.as_mut_ptr(), 0,
                        ptr::null(),   // <-- lpEnvironment
                        ptr::null(), &startup_info, &mut process_info)
```

MSDN, `CreateProcessWithTokenW`, `lpEnvironment`:

> A pointer to an environment block for the new process. **If this parameter is NULL, the new
> process uses an environment created from the profile of the user** specified by *lpUsername*.

So the relaunched app gets an environment block **built from the user's profile** (HKLM
`Session Manager\Environment` + `HKCU\Environment`), not inherited from the installer. That is the
same construction a Start-menu launch resolves to, and in one respect it is *fresher*: it re-reads
the registry rather than reusing explorer's block from logon time.

Two residual caveats the acceptance criteria must cover empirically:

- MSDN also warns `CreateProcessWithTokenW` does not `LOGON_WITH_PROFILE` by default, so
  `HKEY_CURRENT_USER` "may not produce results consistent with a normal interactive logon". In our
  case the user *is* interactively logged on (explorer is running as them, hive already mounted in
  `HKEY_USERS`), so `HKCU\Environment` is readable. Believed fine; **must be probed** (§7 C-1).
- Unelevated fallback: if `CreateProcessWithTokenW` is skipped (installer not elevated) or fails
  with `ERROR_ELEVATION_REQUIRED`, `RunAsUser` falls back to `ShellExecuteW`, making the app a child
  of the installer — which is itself a child of the *old, correctly-launched* app. Also fine.
- Session-only variables (something exported in a shell before launching) are NOT reproduced by
  either mechanism — but they are not reproduced by a Start-menu launch either, so this still meets
  the stated bar.

### 1.4 Q3 — Stop auto-relaunching altogether?

**Not viable as specified, and worse UX for no extra correctness.** On Windows the plugin calls
`std::process::exit(0)` immediately after `ShellExecuteW` launches the installer. The app is dead
*before* the install even begins. Consequences:

- The existing `readyToRestart` → **Restart** flow in `useUpdateController.ts` is **unreachable on
  Windows today** (it is the macOS/Linux path, where the plugin does not exit and `relaunch()` is
  required). See §11 — this must not be mistaken for dead code. A "Bonsai has been updated —
  relaunch" prompt therefore cannot be shown *after* the install; there is no process to show it in.
- The only achievable variant is a *pre*-install warning ("Bonsai will close; reopen it from the
  Start menu when the installer finishes") plus `restart_after_install(false)`. That leaves the user
  staring at a passive installer bar with no app, requires a `ui-designer` pass, new copy, new
  states, and a platform fork in the update UI — to reach a correctness level R1 reaches for free.

Rejected. See §4 R3.

### 1.5 Q4 — Relaunch with a correct environment directly?

That is precisely what R1 *is*: `RunAsUser` is the vendor-supported "hand the relaunch a
user-profile environment" mechanism, already implemented, already tested upstream, already wired to
`installMode: "passive"`. Building our own (fork `main.wxs` so the launch CA runs
`explorer.exe "<path>"`, or ship a launcher shim) means owning a bundler template fork that must be
re-synced on every Tauri upgrade, with no CI signal when upstream drifts. Rejected — §4 R4.

---

## 2. Blast radius — what a foreign environment actually breaks

Every item below is a real, present code path. Only item 1 is covered by P70.

| # | Surface | Env dependency | Anchor | P70? | R2? |
|---|---|---|---|---|---|
| 1 | `git` executable resolution | `PATH`, `PATHEXT`, `LOCALAPPDATA`, `ProgramFiles`, `ProgramW6432`, `ProgramFiles(x86)`, `SystemRoot`, `BONSAI_GIT_BIN` | `gitbin.rs:106,134,152,228-240,266-333` | **YES** — registry + well-known rungs rescue it | yes |
| 2 | Everything `git` itself spawns: credential helpers (`git-credential-manager.exe`), `ssh`, `git-lfs`, diff/merge tools, `sh.exe` | child `PATH` = process `PATH` (+ bin dir only when git came from a NON-PATH rung) | `gitbin.rs:396-404` | NO | yes |
| 3 | P49 external integrations — terminal (`wt.exe`), file manager, editor (`code`) | `PATH`/`PATHEXT` via `procutil::resolve_program` | `procutil.rs:22-23`, `external.rs:146,236,262,276` | NO | yes |
| 4 | AI CLI (P53–P57): `claude` resolution. The npm global shim `claude.cmd` lives in `%APPDATA%\npm` — a **User-PATH-only** directory, i.e. the identical failure class as the reported Git bug | `CLAUDE_BIN_ENV`, `PATH`, `PATHEXT` | `ai/mod.rs:329`, `procutil.rs` | NO | yes |
| 5 | libgit2 global/system config discovery → `user.name`/`user.email` (commit identity, P40), `credential.helper`, `http.proxy`, `safe.directory`, `core.autocrlf`, `gpg.format`/`gpg.program`/`commit.gpgsign`, `url.*.insteadOf` | `USERPROFILE`, `HOME`, `XDG_CONFIG_HOME` | all of `git2` | NO | **NO** |
| 6 | SSH auth: agent-backed `Cred::ssh_key_from_agent`, on-disk key discovery | `SSH_AUTH_SOCK` (unix), `HOME`/`USERPROFILE`, `GIT_SSH`, `GIT_SSH_COMMAND` | `git/remote.rs` | NO | **NO** |
| 7 | Forge/PR HTTP (P62–P64) — reqwest's default proxy detection reads env proxies; a corporate proxy set only in `HKCU\Environment` disappears | `HTTPS_PROXY`, `HTTP_PROXY`, `ALL_PROXY`, `NO_PROXY` | `bonsai-forge/src/http.rs:135` | NO | **NO** |
| 8 | Signing + verify (P58): `gpg` / `ssh-keygen` lookup, keyring location | `PATH`, `GNUPGHOME`, `%APPDATA%\gnupg` | `git/signing.rs` | NO | partial (`PATH` only) |
| 9 | Git hooks execution (P59) — hook scripts are shell scripts that call `node`/`npx`/`python` off `PATH` (Husky) | `PATH` | `git/hooks.rs` | NO | yes |
| 10 | Per-child env hardening call sites — they *add/remove* vars but otherwise inherit the process block wholesale | inherited block | `git/exec.rs:89-91`, `git/remote.rs:223-225`, `git/search.rs:141`, `git/undo.rs:444`, `scheduler.rs:737-738` | NO | partial |
| 11 | User escape hatches / diagnostics silently invisible | `BONSAI_GIT_BIN`, `BONSAI_GIT_TIMEOUT` (`git/timeout.rs:69`), `BONSAI_REQUIRE_GIT_STRICT` | various | NO | **NO** |
| 12 | Scratch/temp files | `TEMP`, `TMP` via `std::env::temp_dir()` | various | NO | **NO** |
| 13 | Settings + derived data location | `app_config_dir()` / `app_data_dir()` | `settings.rs:614`, `commands/shared.rs:117` | **Believed safe** — Tauri resolves these through `SHGetKnownFolderPath`, which is **token**-derived, and the MSI CA impersonates the user. Included because a silently-wrong profile here means settings loss, so it must be spot-checked once (§7 C-1). | n/a |

The three misleading auth toasts the user saw were item 2 (`git credential fill` couldn't spawn)
downstream of item 1. Items 4–9 would have failed the same way had the user reached them.

The `R2?` column is the honest scope of the §5 backstop: it repairs `PATH` and nothing else. Items
5, 6, 7, 11, 12 stay broken under R2 alone. **That is why R1 is the fix and R2 is the patch.**

---

## 3. R1 — the fix (increment 1)

Three edits. **No Rust, no TypeScript, no IPC, no UI, no mock-layer change.**

### 3.1 `src-tauri/tauri.conf.json`

Replace `"targets": "all"` with an explicit list that omits `msi`:

```jsonc
"bundle": {
  "active": true,
  // P71: NSIS is the ONE Windows artifact. The WiX/MSI relaunch custom action
  // runs the app as a child of msiexec.exe, so it inherits msiexec's environment
  // block instead of the user's (docs/contracts/P71-updater-relaunch-env.md §1.2).
  // If MSI ever returns for enterprise deployment it must come back WITH
  // Authenticode signing AND with updaterJsonPreferNsis: true retained (D3).
  "targets": ["nsis", "app", "dmg", "deb", "rpm", "appimage"],
  "createUpdaterArtifacts": true,
  // ... icon, windows.* unchanged; nsis.installMode stays "perMachine" (D4)
}
```

Bundler behaviour to rely on: `targets` is filtered per host OS, so the Windows job produces `nsis`
only, macOS produces `app` + `dmg`, Linux produces `deb`/`rpm`/`appimage`.
`plugins.updater.windows.installMode` stays `"passive"` — it maps to NSIS `/P` and, with
`restart_after_install`, to `/R` (verified in `updater/src/config.rs`).

### 3.2 `.github/workflows/release.yml`

Add one input to the `tauri-apps/tauri-action@v0` step (belt-and-braces per D3 — it makes the
manifest correct even if `msi` is ever re-added to `targets`):

```yaml
        with:
          # ... existing inputs unchanged ...
          includeUpdaterJson: true
          updaterJsonPreferNsis: true   # P71: never point latest.json at the .msi
          args: ${{ matrix.args }}
```

### 3.3 Documentation edits (part of increment 1)

- `.github/workflows/release.yml` header comment: state that the Windows artifact is NSIS only, and
  why (one line + a pointer to this contract).
- `README.md` / install docs: if an MSI download is advertised anywhere, remove it.
- `src/hooks/useUpdateController.ts` — add the §11 platform note as a doc comment on `restart()`.
  This is a **comment-only** edit, and it is required: §11 exists precisely because a future cleanup
  pass will grep the Windows path, conclude the state is unreachable, and delete a live macOS/Linux
  code path.

### 3.4 What explicitly does NOT change

- `plugins.updater.pubkey`, the endpoint URL, `TAURI_SIGNING_*` secrets, `.tauri/updater-prod.key`.
- `src-tauri/src/lib.rs` plugin registration (`tauri_plugin_updater::Builder::new().build()`).
- `src/ipc/tauri.ts` update wrappers, `src/ipc/types.ts` (`UpdateCheckResult`, `UpdateProgress`),
  `src/ipc/mock/handlers/update.ts`, `UpdateDialog.tsx`, and every behavioural line of
  `useUpdateController.ts`.
- The `?update=` browser-harness seam from P42.

**IPC surface delta: none.** The mock layer (`VITE_MOCK_IPC=1`) is unaffected and stays
behaviourally identical; the existing `?update=available|error|...` fixtures continue to drive the
dialog.

---

## 4. Rejected alternatives

**R3 — Stop auto-relaunching; prompt the user to relaunch.** Rejected: impossible as stated (the
process exits before the install starts, §1.4), so it degrades to a *pre*-install warning + a
manual Start-menu relaunch. Costs a `ui-designer` pass, new copy/states, a Windows-vs-macOS fork in
the update flow, and a worse UX — to reach a correctness level R1 already reaches.

**R4 — Fork the WiX template so the launch custom action goes through `explorer.exe`.** Rejected:
Tauri does support `bundle.windows.wix.template`, but adopting it means owning a copy of upstream's
`main.wxs` forever, re-syncing on every Tauri bump, with no CI signal when it drifts. It
reimplements, worse, what NSIS's `RunAsUser` already does correctly.

**R5 — Ship a launcher shim (`bonsai-launcher.exe`) that re-execs the real binary with a rebuilt
environment.** Rejected: adds a second signed binary, a second entry in Task Manager, breaks
single-instance/`Win32_Process` reasoning, and complicates the shortcut/uninstall story — all to
work around an installer we can simply stop using.

**R2 as the *primary* fix.** Rejected as primary: it repairs `PATH` only (see the `R2?` column in
§2) and cannot restore `USERPROFILE`/`HOME`, `SSH_AUTH_SOCK`, proxy vars or `TEMP`. Retained as
increment 2 — §5.

---

## 5. R2 — PATH rehydration backstop (increment 2, approved)

### 5.1 Why it exists

**R1 does nothing for clients already installed via MSI.** Their app keeps launching with a foreign
environment after every future update *until they reinstall from the NSIS artifact*. That includes
the user who reported this bug. R2 repairs those clients **in place**, on the next launch, with no
reinstall. It also protects against any future regression that reintroduces a foreign-environment
launch path.

### 5.2 What R2 is NOT — read this before implementing

R2 is a **`PATH` patch, not an environment repair.** It does **not** restore:

- `USERPROFILE` / `HOME` / `XDG_CONFIG_HOME` → so a wrong global git config (identity,
  `credential.helper`, `safe.directory`, signing config) stays wrong. **Blast radius #5.**
- `SSH_AUTH_SOCK`, `GIT_SSH`, `GIT_SSH_COMMAND` → agent-backed SSH auth stays broken. **#6.**
- `HTTPS_PROXY` / `HTTP_PROXY` / `ALL_PROXY` / `NO_PROXY` → forge/PR calls behind a corporate proxy
  stay broken. **#7.**
- `TEMP` / `TMP` → scratch files may still land in a system temp dir. **#12.**
- `BONSAI_GIT_BIN`, `BONSAI_GIT_TIMEOUT`, `BONSAI_REQUIRE_GIT_STRICT` → user overrides set in
  `HKCU\Environment` stay invisible. **#11.**

**R2 therefore does not make R1 optional and must never be described as "the fix".** R1 is the fix.

### 5.3 Design

Reuses P70's D2 pattern exactly — shell out to `%SystemRoot%\System32\reg.exe` **by absolute path**
(the whole premise is that `PATH` may be unusable), with `CREATE_NO_WINDOW`, defensive parsing, and
every failure silently skipped. **No new crate dependency.** Reuses P70's `GitEnv`-style trait
injection so every pure function is hermetically testable on any host OS with **zero `std::env`
mutation in tests**.

New file `crates/bonsai-core/src/winenv.rs` (~90 lines logic + ~80 lines tests in
`winenv_tests.rs`; both well under the 500-line limit).

```rust
/// Injection seam (mirrors `gitbin::GitEnv`): registry reads + process env.
/// Production impl shells `reg.exe`; tests supply a table-backed fake.
pub trait WinEnv {
    /// `reg query <key> /v <value>` → the raw value string, or `None` on any
    /// failure (missing key, non-zero exit, unparseable output).
    fn registry_string(&self, key: &str, value: &str) -> Option<String>;
    fn var(&self, key: &str) -> Option<String>;
}

/// Outcome of a rehydration attempt. Diagnostic only — never crosses IPC.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PathRehydration {
    /// `true` iff the process PATH was actually replaced.
    pub applied: bool,
    /// Directories present in the registry PATH but absent from the process
    /// PATH — the concrete evidence of a foreign environment. Empty when
    /// `applied` is false.
    pub added: Vec<String>,
}

/// Expand `%NAME%` references in a REG_EXPAND_SZ value against `env`.
/// Unknown names expand to empty. An unterminated `%` is left literal.
/// Never panics.
pub fn expand_percent_vars(raw: &str, env: &dyn WinEnv) -> String;

/// Compute the repaired PATH.
///
/// - Comparison: case-insensitive, after trimming trailing `\` and `/` and
///   surrounding whitespace. Empty segments are ignored.
/// - Missing entries are **PREPENDED**, in registry order (system entries
///   before user entries), ahead of the existing process PATH.
/// - The existing process PATH is copied through **verbatim**: never
///   reordered, never deduplicated, never dropped.
/// - Returns `None` when nothing is missing, so the caller skips `set_var`.
pub fn merge_path(system_path: &str, user_path: &str, process_path: &str)
    -> Option<(String, Vec<String>)>;

/// Read both registry PATHs, merge, apply. Silent no-op returning
/// `PathRehydration::default()` on non-Windows, on any registry read failure,
/// on malformed values, and when nothing is missing.
pub fn rehydrate_path(env: &dyn WinEnv) -> PathRehydration;

/// Production entry point. MUST be the FIRST statement of `bonsai::run()`.
pub fn rehydrate_path_once() -> PathRehydration;
```

Registry sources, read in this order (system first, so system entries land ahead of user entries):

| Scope | Key | Value |
|---|---|---|
| system | `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment` | `Path` |
| user | `HKCU\Environment` | `Path` |

Call site — `src-tauri/src/lib.rs`, first line of `run()`:

```rust
pub fn run() {
    // P71: repair a PATH inherited from an installer BEFORE anything spawns a
    // child or caches a resolution. MUST precede every thread spawn:
    // `std::env::set_var` is only sound single-threaded.
    let _ = bonsai_core::winenv::rehydrate_path_once();
    bonsai_core::git::relax_odb_hash_verification();
    tauri::Builder::default()
    // ...
}
```

### 5.4 Hard constraints for the implementer

1. **Prepend missing entries only; never replace, reorder, or deduplicate the existing PATH.** The
   inherited PATH is copied through byte-for-byte after the prepended segment.
2. **Never write to the parent, user, or machine environment. Never persist anything.** The only
   mutation is `set_var("PATH", …)` on this process. No `SetEnvironmentVariable` on the registry, no
   `WM_SETTINGCHANGE` broadcast, no file writes.
3. **`set_var` runs before any thread is spawned** — before `relax_odb_hash_verification()`, before
   the Tauri builder, before the async runtime. Rust 2024 marks `set_var` `unsafe` precisely for
   this; document the single-threaded precondition at the call site.
4. **Must run before `gitbin`'s process-lifetime cache is populated**, so the P70 ladder sees the
   repaired PATH and reports `source: "path"` (§7 C-1 depends on this ordering).
5. **A malformed or unreadable registry value is a silent no-op, never an exception.** `reg.exe`
   missing, non-zero exit, unexpected output shape, non-Windows host → `PathRehydration::default()`.
   Nothing is logged at error level; at most one debug line.
6. **No IPC command, no event, no channel, no UI, no mock-layer change.** `PathRehydration` is
   diagnostic-only and does not cross the IPC boundary.
7. **Tests mutate no process state.** Every assertion goes through the injected `WinEnv`; the only
   test that may touch the real environment is a single smoke test asserting
   `rehydrate_path_once()` does not panic.

### 5.5 Known trade-off — prepend vs append (route to security-auditor)

Prepending is what the orchestrator specified and it is the behaviour that actually rescues the
reported case: the user's per-user Git lives in a User-PATH directory that must win. The trade-off
is that a directory recovered from the registry now takes precedence over the inherited PATH, so if
the inherited PATH contained a *different* `git.exe`/`code.exe` earlier in the order, the winner
changes. Two mitigations are already in the design: only **missing** entries are added (an entry
already present keeps its original position), and the sources are the user's own HKLM/HKCU
`Environment` — i.e. exactly what a Start-menu launch would have resolved. Called out explicitly in
§8 item 6 so `security-auditor` evaluates it as a potential PATH-precedence concern rather than
discovering it cold.

---

## 6. Surface summary

| Layer | R1 (increment 1) | R2 (increment 2) |
|---|---|---|
| Tauri commands | none | none |
| Events | none | none |
| Channels | none | none |
| TS types (`src/ipc/types.ts`) | none | none |
| Mock IPC (`src/ipc/mock*`) | none | none |
| Rust source | none | `crates/bonsai-core/src/winenv.rs` + `winenv_tests.rs` (new), `mod winenv;` in `bonsai-core/src/lib.rs`, 1 line in `src-tauri/src/lib.rs` |
| Config | `src-tauri/tauri.conf.json` → `bundle.targets` | none |
| Workflow | `.github/workflows/release.yml` → `updaterJsonPreferNsis: true` | none |
| Docs/comments | release.yml header, README, `useUpdateController.ts` doc comment (§11) | module docs |
| UI | none — **no `ui-designer` pass required** | none |

---

## 7. Acceptance criteria

### Machine-verifiable (orchestrator AI gate)

- **M-1** `src-tauri/tauri.conf.json`: `bundle.targets` is an array and does **not** contain `msi`;
  `plugins.updater.windows.installMode` is still `"passive"`; `plugins.updater.pubkey` and
  `endpoints` are **byte-identical** to `71236f8`.
- **M-2** `.github/workflows/release.yml` contains `updaterJsonPreferNsis: true` inside the
  `tauri-apps/tauri-action@v0` `with:` block, and the `TAURI_SIGNING_*` env block is unchanged.
- **M-3** For increment 1, `git diff --stat` touches **only** `src-tauri/tauri.conf.json`,
  `.github/workflows/release.yml`, `README.md`/docs, and the comment-only edit to
  `src/hooks/useUpdateController.ts`. Zero behavioural changes under `src/` or `crates/`.
- **M-4** Existing suites stay green and unchanged in count (`cargo test --workspace`, `vitest`,
  `pnpm exec tsc --noEmit`, e2e). Increment 1 changes no behaviour, so any delta is a regression.
- **M-5** Browser harness unaffected: `pnpm dev` with `VITE_MOCK_IPC=1` and `?update=available`
  still drives the update dialog through all states. No mock-layer edits.
- **M-6** Windows bundling smoke: `pnpm tauri build` on Windows emits **exactly one** installer
  under `src-tauri/target/release/bundle/` (`nsis/Bonsai_<v>_x64-setup.exe`) plus its `.sig`, and
  **no** `msi/` directory. *(Long-running — background it and poll the log; never conclude failure
  from a tool timeout.)*
- **M-7** After the next tagged release: `curl -sL <endpoint>/latest.json | jq -r
  '.platforms["windows-x86_64"].url'` ends with `-setup.exe`, not `.msi`; and the release has no
  `.msi` asset.
- **M-8 (R2)** Unit tests for `expand_percent_vars` / `merge_path` / `rehydrate_path` over the
  injected `WinEnv`, running on any host OS, mutating no process state. Must cover:
  entry already present → no-op, `applied: false`;
  missing user entry → **prepended**, listed in `added`;
  ordering → system entries precede user entries, and both precede the untouched process PATH;
  the existing process PATH is reproduced **verbatim** (no reorder, no dedupe, no drop);
  trailing `\`/`/` and case differences compare equal;
  `%SystemRoot%` / `%USERPROFILE%` expansion, and an unknown `%VAR%` → empty;
  malformed / missing / non-zero-exit `reg.exe` output → `PathRehydration::default()`.
- **M-9 (R2)** `crates/bonsai-core` dependency set is unchanged (no registry crate added) — assert
  against `Cargo.toml`.

### USER CHECKPOINT (native — orchestrator must NOT self-declare)

- **C-1 — the acceptance instrument: `GitAvailability.source` must read `path`.**
  This is the single check that distinguishes **fixed** from **merely masked**. P70's preflight
  reports which rung of the resolver ladder produced `git`; that rung is an exact PATH-health
  oracle. After taking an auto-update, open the Git availability re-check:
  - `source === "path"` → the process PATH is the user's. **P71 worked.**
  - `source === "registry"` or `"wellKnown"` → git was found, but only because P70's fallback
    ladder went looking for it. **The environment is still foreign and P70 is covering for it** —
    every non-git surface in §2 is still exposed. P71 has *not* worked.

  It is also the fastest way for the user to self-diagnose without a debugger or Process Explorer:
  one field in the app tells them whether their PATH is real.

  Full C-1 procedure — install v1.0.0 from the **NSIS** `-setup.exe`, publish/serve a v1.0.1 test
  build, take the update, then in the relaunched app:
  1. Git availability re-check → `source` must be **`path`** (above).
  2. AI availability check → `claude` must resolve (blast radius #4, `%APPDATA%\npm` is
     User-PATH-only).
  3. **Open in terminal / file manager / editor** (P49) must all launch.
  4. A commit must pick up the correct `user.name`/`user.email` (blast radius #5 → `USERPROFILE`;
     note R2 would *not* have fixed this — only R1 does).
  5. Settings written before the update are still present after it (blast radius #13).
  6. `Get-CimInstance Win32_Process -Filter "Name='bonsai.exe'" | Select ProcessId,ParentProcessId,Path`
     — record the parent for the record; correctness is judged by 1–5, not by the parent PID.
- **C-2 — R2 repairs an MSI-installed client in place.** On a machine still running the
  MSI-installed build (i.e. a foreign environment), launch a build containing R2 and confirm C-1
  step 1 flips from `registry`/`wellKnown` to **`path`** without any reinstall. This is R2's whole
  reason for existing (§5.1). Also confirm C-1 steps 2–3 now pass, and record that step 4 (identity
  from `USERPROFILE`) may still fail — that is expected and is why R1 is still required.
- **C-3 — UAC behaviour.** `perMachine` means the update prompts for elevation. Confirm: accepting
  completes the update; **declining leaves the running app alive with an error state** (the plugin
  returns `Err` from `ShellExecuteW` before `exit(0)`), not a half-updated install.
- **C-4 — signed round-trip.** The v1.0.1 test release must be signed with the same prod key and
  verify on the client. Confirms D6: artifact selection changed, trust chain did not.
- **C-5 — macOS/Linux regression.** The update flow on at least one non-Windows platform still
  reaches `readyToRestart` and the **Restart** button still relaunches (that path is macOS/Linux
  only — §11 — and must not have been disturbed).

---

## 8. What `security-auditor` must review

R1 changes the install/trust path, so an audit pass is required before the release tag. Scope:

1. **Trust chain unchanged.** Diff-confirm `plugins.updater.pubkey`, `endpoints`, and the workflow's
   `TAURI_SIGNING_PRIVATE_KEY*` wiring are untouched, and that the NSIS artifact is signed by the
   same key and its `.sig` is the one referenced by `latest.json`.
2. **Manifest integrity.** Confirm `updaterJsonPreferNsis: true` changes only *which signed
   artifact* is referenced — never how the signature is produced or verified — and that the
   published `latest.json` `signature` field corresponds to the `-setup.exe` it points at (a
   mismatched pair would be a silent update-channel break, or worse a downgrade vector).
3. **Elevation surface.** NSIS `perMachine` + `passive` + `/R`: assess the elevated-installer →
   `CreateProcessWithTokenW`(shell token) relaunch. Specifically: does relaunching from an elevated
   installer via a duplicated explorer token ever yield an app running with **more** privilege than
   a Start-menu launch? (Expected: no — it drops to the shell's non-elevated token; confirm.)
4. **Install-location integrity.** Both installers write to `C:\Program Files\Bonsai`. Confirm
   dropping the MSI cannot leave a partially-uninstalled product, a stale service/scheduled task, or
   a writable-by-user directory under Program Files.
5. **Unsigned-binary posture.** v1.0.0 shipped unsigned (`certificateThumbprint: null`). Restate the
   residual risk of an unsigned NSIS installer being fetched over HTTPS and executed elevated, and
   whether the minisign updater signature is a sufficient mitigation for the auto-update path
   (it does not help a *manual* download).
6. **(R2) PATH precedence — explicitly requested review, see §5.5.** R2 **prepends** recovered
   registry entries ahead of the inherited PATH. Evaluate whether this introduces a PATH-precedence
   or binary-shadowing concern: can a value in `HKCU\Environment\Path` (writable by the user, and by
   anything running as the user) cause Bonsai to launch a different `git.exe` / `code.exe` /
   `claude.cmd` than it otherwise would? Weigh against the fact that the same registry value already
   governs every normally-launched process on the machine.
7. **(R2) Registry read path.** `reg.exe` invoked by absolute `%SystemRoot%` path with
   `CREATE_NO_WINDOW`; output parsed defensively; `%VAR%` expansion cannot recurse or expand
   unbounded; no registry *writes*, no environment broadcast, no persistence (§5.4 constraints 2
   and 5).

---

## 9. Decisions — closed

| # | Question | Decision (2026-08-19) |
|---|---|---|
| Q-1 | Drop the MSI artifact, or keep it as a manual download? | **Drop entirely.** Recorded as D3, with the conditions required for any future return (signing **and** a pinned NSIS updater). |
| Q-2 | Ship the R2 PATH-rehydration backstop? | **Yes, as increment 2**, scoped per §5. Decisive argument: R1 does nothing for already-MSI-installed clients; R2 repairs them in place. |
| Q-3 | `perMachine` vs `currentUser` NSIS install mode? | **Leave `perMachine`.** Not a correctness issue; out of scope. |
| Q-4 | Migration for the reporting user's MSI install? | **Manual uninstall + reinstall from the NSIS `-setup.exe`** rather than relying on an untested passive-mode WiX→NSIS migration. Written up as a FOR-USER item — §10. |

---

## 10. FOR-USER — one-time reinstall on the affected machine

**Not an implementation step.** This is the affected machine's owner's call, and the orchestrator is
surfacing it separately.

**What:** uninstall the MSI-installed Bonsai v1.0.0 and reinstall once from the NSIS
`Bonsai_1.0.0_x64-setup.exe` (or straight to the first post-P71 release).

**Steps:**
1. Settings → Apps → *Bonsai* → **Uninstall** (this is the WiX/MSI product entry).
2. Download `Bonsai_<version>_x64-setup.exe` from the GitHub release.
3. Run it (accept the UAC prompt — the install is `perMachine`).
4. Launch Bonsai and check the Git availability panel: `source` should read **`path`** (§7 C-1).

**Why not just wait for the next auto-update?** Because the update would arrive as an NSIS installer
onto a WiX-installed product. Tauri's NSIS script does detect a prior WiX install and uninstall it
first, but that migration has never been exercised in **passive** mode, and a failure mid-migration
leaves the machine with no working app. With an installed base of roughly one, a deliberate two-
minute reinstall is strictly safer than betting on it.

**Note:** R2 (§5) will repair the `PATH` on this machine in place without a reinstall, but only
`PATH` — the global-git-config, proxy and SSH-agent exposures (§5.2) persist until the reinstall.
The reinstall is still the real fix.

---

## 11. Durable note — `readyToRestart` / **Restart** is live on macOS + Linux

**Do not delete `UpdateUiState.readyToRestart`, `UpdateController.restart()`, `ipc.relaunchApp()`,
the `@tauri-apps/plugin-process` dependency, or the dialog's Restart button.**

On **Windows** they are unreachable: `tauri-plugin-updater` calls `std::process::exit(0)`
immediately after handing the installer to `ShellExecuteW`, so `downloadAndInstall()` never returns
and the state machine never advances past `downloading`. The installer performs the relaunch (§1.3).

On **macOS and Linux** they are the *only* relaunch path: the plugin replaces the bundle/AppImage
in place, returns normally, and the app must call `relaunch()` itself.

A cleanup pass that greps only the Windows behaviour will conclude this is dead code and remove a
live path on two platforms. §3.3 requires this note to be mirrored as a doc comment on
`UpdateController.restart()` in `src/hooks/useUpdateController.ts`, which is where that grep lands.
Covered by acceptance criterion **C-5**.
