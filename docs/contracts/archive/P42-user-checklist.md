# P42 — Packaging + Auto-Update — USER CHECKPOINT Checklist

> The orchestrator has verified the P42 **AI gate** (Rust tests + clippy clean, `tsc` + `pnpm build`
> clean, mock-seam harness flows, updater private key NOT committed). The items below **cannot** be
> self-verified by the orchestrator — they require real secrets, a live release host, and the native
> installed app. Do NOT mark P42 fully done until Part B passes end-to-end.
>
> References: `docs/contracts/P42-packaging-autoupdate.md` §9 (user inputs) and §10 (AI gate vs
> USER CHECKPOINT).

---

## Part A — What the USER must provide before releases work

These are the placeholders shipped by P42 that only the user can fill in. Nothing here can be
committed by the AI (secrets / private keys / org-specific values).

### A1 — Update endpoint (D2)
- [ ] Replace the placeholder in `src-tauri/tauri.conf.json` → `plugins.updater.endpoints`:
      `https://github.com/OWNER/REPO/releases/latest/download/latest.json`.
      Substitute your real GitHub `OWNER/REPO`, **or** point it at any host that serves a Tauri v2
      `latest.json` manifest.
- [ ] Confirm the host is reachable over HTTPS from an end-user machine (no auth wall on the
      manifest / artifact URLs).

### A2 — Production updater signing keypair (D3, INV-5)
The committed key in `plugins.updater.pubkey` is a **DEV** key (its private half lives only at the
gitignored `.tauri/updater-dev.key` and must never ship). For real releases, generate a
**production** keypair:
- [ ] `pnpm tauri signer generate -w .tauri/updater-prod.key` (keep this file OUT of git — the
      `.tauri/` dir is already gitignored).
- [ ] Replace `plugins.updater.pubkey` in `tauri.conf.json` with the **production public key**
      printed by the generator.
- [ ] Store the **private key** + its password as CI secrets:
      `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
      The private key is NEVER committed.
- [ ] Sanity: the pubkey embedded in the shipped build must match the private key that signs
      `latest.json` / the `*.sig` artifacts, or every client will reject the update as a bad
      signature.

### A3 — OS code-signing credentials (D6)
Unsigned builds install with OS warnings and macOS Gatekeeper will block them; signing is required
for a smooth production update. These are placeholders (`null`/unset) in P42:
- [ ] **Windows (Authenticode):** set `bundle.windows.certificateThumbprint` in `tauri.conf.json`
      to your code-signing cert thumbprint (cert installed in the machine/user store), or wire the
      equivalent CI signing step.
- [ ] **macOS (Developer ID + notarization):** set the signing identity and provide notarization
      credentials at build time via env: `APPLE_ID`, `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID`
      (and `APPLE_SIGNING_IDENTITY` / keychain as applicable).
- [ ] **Linux:** no code-signing required; deb/AppImage ship unsigned (optional GPG repo signing is
      out of scope for P42).

---

## Part B — Native verification steps (USER CHECKPOINT — run on the target OS)

Do these on each OS you ship (Windows / macOS / Linux). They require the native app; the browser
harness cannot cover them.

### B1 — Build produces installers + updater artifacts
- [ ] Run `pnpm tauri build` (the orchestrator runs the unsigned AI-gate build; here run it with the
      Part A signing config in place).
- [ ] Confirm per-OS installers are produced: Windows NSIS `.exe` + MSI; macOS `.dmg` + `.app`;
      Linux `.deb` + `.AppImage`.
- [ ] Confirm **updater artifacts** exist because `bundle.createUpdaterArtifacts: true`: each
      updater target has a matching **`.sig`** file, and you can assemble a `latest.json` referencing
      the artifact URL + signature + version.

### B2 — Publish a higher version to the endpoint
- [ ] Bump `app.version` in `tauri.conf.json` (e.g. `0.1.0` → `0.2.0`) and build that newer version,
      signed with the production key.
- [ ] Upload the new installer/updater artifact + a `latest.json` (pointing at the newer version and
      its `.sig`) to the endpoint from A1.

### B3 — Full SIGNED update round-trip (the core checkpoint)
Install the **older** signed build (the one whose endpoint sees a newer `latest.json`), then:
- [ ] Open Settings → **Updates** section shows the current version.
- [ ] Click **Check for updates** → an **"vX is available"** banner/notification appears.
- [ ] Open the update dialog → it shows current → target version + release notes.
- [ ] Click **Download & install** → a progress bar advances (bytes).
- [ ] On finish → the **Restart now / Later** prompt appears.
- [ ] Click **Restart now** → the app relaunches and now reports the **new** version.
      (This exercises the real minisign signature verification — a wrong pubkey/key here fails.)

### B4 — Auto-check-on-launch toggle
- [ ] In Settings → Updates, enable **Auto-check for updates on launch**; confirm it persists.
- [ ] Fully quit and relaunch → with a newer version live at the endpoint, the availability
      notification appears automatically shortly after launch (no manual click).
- [ ] Disable the toggle → relaunch → no automatic outbound update check occurs (default-OFF, D4).

### B5 — Up-to-date and error/offline cases
- [ ] With the endpoint's `latest.json` at the same version as the installed app → **Check for
      updates** reports "up to date"; no notification, no crash.
- [ ] With the machine offline (or the endpoint URL unreachable) → **Check for updates** surfaces a
      clean error state (a network/update error message), no crash, app stays usable.

### B6 — Installer smoke per OS
- [ ] Each produced installer (NSIS, MSI, dmg, deb, AppImage) installs and launches the app on a
      clean machine/VM for its OS.

---

## Sign-off
- [ ] Part A inputs supplied (endpoint, production keypair → CI secrets, code-signing certs).
- [ ] Part B verified on Windows.
- [ ] Part B verified on macOS.
- [ ] Part B verified on Linux.

Only when Part A and all applicable Part B rows pass is P42 fully done (the AI gate alone is not
sufficient — a shipped auto-update depends on real secrets + a live endpoint).
