# P79 — Forge Account Management

Status: contract (design only). Builds on P62–P64 forge integration.

Scope: three user-approved pieces —
1. Change/disconnect the connected account **in the PR panel**.
2. Token **expiry → reconnect** flow (keep the token, prompt to reconnect).
3. A global **Accounts** settings section listing all connected hosts (change/disconnect + add-for-host).

Invariants held: Rust owns all git/forge logic and token I/O; IPC carries compact precomputed
data (never a token back to the frontend); commands are request/response (no events/channels this
milestone); heavy git2/HTTP runs in `spawn_blocking`; every surface is mock-implementable.

---

## 0. What already exists (do not re-derive)

- `crates/bonsai-forge/src/auth.rs`: `TokenStore` (`get/set/delete/has`, host-keyed, lowercased) +
  viewer cache (`cache_viewer/cached_viewer/evict_viewer`). Token shared across repos on a host.
- `crates/bonsai-forge/src/lib.rs`: `open`, `set_token(workdir,&str)->ForgeViewer`,
  `clear_token(workdir)`, `validate_token`, `resolve_target` (private, **network-free** — reads
  `origin`), `build_provider`.
- `src-tauri/src/commands/forge.rs`: `forge_set_token(repo_id, token)`, `forge_clear_token(repo_id)`
  + the read commands.
- Persistence precedent: `src-tauri/src/settings.rs` — hand-rolled `settings.json`,
  `Settings` struct, `load_from`, `save_to` (atomic rename), `update(file, mutate)` serialized by
  `SETTINGS_IO`. `profiles: Vec<IdentityProfile>` is the precedent for a list field with
  `#[serde(default)]`.
- Auth-failure error: backend maps 401 / non-ratelimit 403 → `AppError::AuthFailed`; TS discriminant
  is `AppError.kind === 'authFailed'` (confirmed in `src/ipc/types.ts`).
- UI: `src/components/PrPanel.tsx` (View union at :28), `ForgeConnect.tsx`, settings catalog
  `src/components/settings/types.ts` (`SettingsCategoryId` at :11), `SettingsProfilesSection.tsx`
  as the list-managing-section precedent.

---

## 1. Persistence: the known-connected-hosts index

**Chosen approach: a new field on `settings.json`.** The OS keychain is not portably enumerable, so
Bonsai maintains its own index of hosts it has stored a token for. It lives in `settings.json`
(NOT the keychain) and stores **only host + provider kind + optional last-known login** — never a
token, never an avatar-that-could-leak, nothing secret. This reuses the existing atomic
`settings::update` cycle and adds no new capability surface.

### 1.1 New Rust type (`src-tauri/src/settings.rs`)

```rust
/// P79: one forge host Bonsai has stored a PAT for. The keychain is the store of
/// record for the token; this index only remembers WHICH hosts exist (the keychain
/// can't be enumerated portably) plus a display hint. NEVER holds the token.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeHostRecord {
    /// Lowercased host, e.g. "github.com". Keychain account key.
    pub host: String,
    /// Provider kind, so add-for-host / list can pick the right API without a repo.
    /// Serialized as `bonsai_forge::ForgeKind` camelCase ("gitHub" | ...).
    pub kind: bonsai_forge::ForgeKind,
    /// Last-known login for offline display (avatar is fetched fresh / from the
    /// viewer cache; not persisted). None until first successful validation.
    pub login: Option<String>,
}
```

Add to `Settings`:
```rust
/// P79: forge hosts with a stored PAT (known-hosts index; keychain is source of
/// record for the token). Additive `#[serde(default)]` ⇒ a pre-P79 file loads `[]`.
pub forge_hosts: Vec<ForgeHostRecord>,
```
Default: `Vec::new()`. **No `SETTINGS_VERSION` bump** (additive, mirrors `profiles`).

### 1.2 Sync rules (index ⟷ keychain)

- **On successful set-token** (both per-repo `forge_set_token` and host-based
  `forge_set_token_for_host`): after the crate validates+stores, upsert
  `ForgeHostRecord { host, kind, login: Some(viewer.login) }` (replace existing by host).
- **On clear-token** (both variants): remove the record for that host.
- **Never** write the index for a rejected token (set stores nothing → index unchanged).
- Index mutation happens at the **command layer** (it needs `AppState`/settings); the pure
  `bonsai-forge` crate stays Tauri-free.

### 1.3 Migration / backfill (eventual consistency)

A user upgrading from P62–P78 may hold keychain tokens with no index entry. We do NOT (cannot
portably) enumerate the keychain. **Lazy backfill:** whenever `forge_repo_context` resolves a host
whose token `TokenStore::has(host)` is true but which is absent from `forge_hosts`, upsert a record
`{ host, kind, login: cached_viewer(host).map(|v| v.login) }`. This makes the Accounts list
converge as the user opens their repos, with zero keychain enumeration and no network.

> **OD-1 (flagged):** lazy backfill only surfaces a host once its repo is opened this install.
> A host whose repo is never opened but whose token exists in the keychain stays invisible in the
> Accounts list (its token is still used when that repo is later opened). Recommendation: accept
> this — it is strictly better than the status quo and avoids non-portable enumeration. Confirm.

---

## 2. Backend: new / changed IPC commands

All commands mirror the house shape (`spawn_blocking`, join-error → `AppError::Other`). No events,
no channels. Tokens are never returned.

### 2.1 New crate functions (`crates/bonsai-forge/src/lib.rs`)

```rust
/// Network-free: resolve (lowercased host, kind) from the repo's `origin`, so the
/// command layer can key the known-hosts index after set/clear WITHOUT a second
/// network call. `NoRemote` if no origin; empty host for an unparseable origin.
pub fn resolve_forge_host(workdir: &Path) -> Result<(String, ForgeKind), AppError>;

/// Validate `token` against `host`/`kind` DIRECTLY (no repo), and on success store
/// it in the keychain keyed by `host` + warm the viewer cache. Builds a repo-less
/// `ForgeTarget { kind, host, owner:"", repo:"", project:None, web_url:"" }`.
/// GitHub/GitLab/Bitbucket validate via their identity endpoint (GET /user or
/// equivalent), which needs no owner/repo. Azure DevOps validates on a REPOSITORY
/// endpoint ⇒ cannot validate repo-less: returns `ForgeUnsupported` (see OD-2).
/// Rejected token ⇒ `AuthFailed`, stores nothing. Returns the viewer.
pub fn set_token_for_host(host: &str, kind: ForgeKind, token: &str)
    -> Result<ForgeViewer, AppError>;

/// Delete the token for `host` from the keychain and evict its cached viewer.
/// Idempotent. No repo / no network.
pub fn clear_token_for_host(host: &str) -> Result<(), AppError>;

/// Drop the cached viewer for `host` WITHOUT deleting the token (expiry flow —
/// keep the PAT, stop showing a "connected" identity). Pub wrapper over
/// `auth::evict_viewer`. Infallible.
pub fn invalidate_viewer(host: &str);
```

> **OD-2 (flagged):** Azure DevOps has no repo-less identity endpoint under the Code-scoped PAT
> (see P72 note in `lib.rs`), so **add-a-token-for-an-Azure-host-with-no-repo-open is not supported
> in P79** — `set_token_for_host` returns `ForgeUnsupported` for `azureDevOps`. Azure accounts are
> still fully manageable from within an open Azure repo (existing `forge_set_token`), and appear in
> the Accounts list via backfill. Recommendation: ship this limitation; the Accounts "Add" form
> disables/greys the Azure option with a "connect from the repo" hint. Confirm.

### 2.2 New Tauri commands (`src-tauri/src/commands/forge.rs`)

```rust
/// P79: all forge hosts Bonsai knows a token for (the settings index), each with
/// live `connected` (keychain presence) + cache-warm viewer identity. NO network.
/// Errors: `other`.
#[tauri::command]
pub async fn forge_list_accounts(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ForgeAccount>, AppError>;

/// P79: validate a pasted PAT against `host`/`kind` directly (no repo needed) and,
/// on success, store it + upsert the known-hosts index. Returns the viewer.
/// Errors: `authFailed` | `forgeUnsupported` | `forgeRateLimited` | `networkError`
/// | `other`.
#[tauri::command]
pub async fn forge_set_token_for_host(
    state: tauri::State<'_, AppState>,
    host: String,
    kind: ForgeKind,
    token: String,
) -> Result<ForgeViewer, AppError>;

/// P79: sign out a host globally — delete its PAT, evict its viewer, remove it
/// from the known-hosts index. Idempotent. Errors: `other`.
#[tauri::command]
pub async fn forge_clear_token_for_host(
    state: tauri::State<'_, AppState>,
    host: String,
) -> Result<(), AppError>;

/// P79: evict the cached viewer for `host` WITHOUT deleting the token (expiry
/// flow). Keeps the keychain entry + the index record; only stops surfacing a
/// warm "connected" identity so the panel routes to re-auth. Infallible. Errors: —.
#[tauri::command]
pub async fn forge_invalidate_viewer(
    state: tauri::State<'_, AppState>,
    host: String,
) -> Result<(), AppError>;
```

- `forge_list_accounts` reads `settings.forge_hosts`, and for each builds
  `ForgeAccount { host, kind, login: record.login.or(cached_viewer.login), avatarUrl:
  cached_viewer.avatar_url, connected: TokenStore::has(host) }`. A record with `connected:false`
  (token vanished from keychain out-of-band) is still listed so the user can re-add or remove it.
- `forge_set_token`/`forge_clear_token` (existing, per-repo) gain index upsert/remove via
  `resolve_forge_host` (in the same `spawn_blocking`, before returning) — **signatures unchanged.**

### 2.3 Existing commands: authFailed already surfaces

`forge_list_prs` / `forge_get_pr` / `forge_create_pr` / `forge_commit_statuses` already return
`AppError::AuthFailed` from the REST layer on a 401/403. No backend change needed for detection —
the frontend drives the expiry flow (§4).

---

## 3. Types

### 3.1 Rust (`crates/bonsai-forge/src/types.rs`)

```rust
/// P79: one connected (or previously-connected) forge account for the global
/// Accounts settings section. Login/avatar are best-effort display hints from the
/// process viewer cache + the persisted index; NEVER a token.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeAccount {
    pub host: String,
    pub kind: ForgeKind,
    /// Cache-warm or last-known login; None if never validated this install.
    pub login: Option<String>,
    /// Cache-warm avatar; None when the viewer isn't warm.
    pub avatar_url: Option<String>,
    /// A token is currently present in the keychain for `host` (no network).
    pub connected: bool,
}
```
Add a `forge_account_wire_shape_is_camel_case` test asserting keys
`["host","kind","login","avatarUrl","connected"]` (mirrors existing wire-shape tests).

### 3.2 TypeScript (`src/ipc/types.ts`, forge block ~:1815)

```ts
/** P79: one connected/known forge account for the Accounts settings section. */
export interface ForgeAccount {
  host: string;
  kind: ForgeKind;
  login: string | null;
  avatarUrl: string | null;
  connected: boolean;
}
```

IPC declarations (add to `IpcApi`, forge section ~:2712):
```ts
/** P79: all forge hosts with a stored/known token. No network. Rejects AppError
 *  (`other`). */
forgeListAccounts(): Promise<ForgeAccount[]>;
/** P79: validate + store a PAT for `host`/`kind` directly (no repo). Rejects
 *  AppError (`authFailed` | `forgeUnsupported` | `forgeRateLimited` |
 *  `networkError` | `other`). */
forgeSetTokenForHost(host: string, kind: ForgeKind, token: string): Promise<ForgeViewer>;
/** P79: delete a host's token + remove it from the index. Idempotent. */
forgeClearTokenForHost(host: string): Promise<void>;
/** P79: evict a host's cached viewer WITHOUT deleting the token (expiry flow). */
forgeInvalidateViewer(host: string): Promise<void>;
```

### 3.3 IPC wiring (`src/ipc/tauri.ts`, forge block ~:1067)

```ts
forgeListAccounts() { return invoke<ForgeAccount[]>('forge_list_accounts'); },
forgeSetTokenForHost(host, kind, token) {
  return invoke<ForgeViewer>('forge_set_token_for_host', { host, kind, token });
},
forgeClearTokenForHost(host) { return invoke<void>('forge_clear_token_for_host', { host }); },
forgeInvalidateViewer(host) { return invoke<void>('forge_invalidate_viewer', { host }); },
```
Arg keys are camelCase matching the Rust param names (`host`, `kind`, `token`).

---

## 4. Expiry → reconnect flow (piece 2)

Policy (user-chosen): **keep the token, do not auto-delete; prompt to reconnect.**

Sequence, driven entirely by the frontend in `PrPanel.tsx`:

1. Any forge read/write (`forgeListPrs` / `forgeGetPr` / `forgeCreatePr` / `forgeCommitStatuses`)
   rejects with `AppError.kind === 'authFailed'`.
2. PrPanel's shared error handler detects that kind, reads `ctx.host`, and calls
   `ipc.forgeInvalidateViewer(ctx.host)` (fire-and-forget; awaited before re-render).
3. It sets `connectMode = 'reauth'` and `view = 'connect'`, and re-fetches `forgeRepoContext`
   (which now reports `viewer: null`, `authenticated: true` — token still present but not warm).
4. `ForgeConnect` renders in **reauth** mode: copy differs from first-time connect —
   e.g. header "Reconnect to {host}", body "Your saved token for {login ?? host} was rejected —
   it may have expired or been revoked. Paste a new token to reconnect." A first-time connect
   (`connectMode==='connect'`) keeps the existing "Connect to {host}" copy.
5. Submitting calls `forgeSetToken(repoId, token)` (overwrites the keychain entry, re-warms the
   viewer, re-upserts the index) → on success `connectMode='connect'`, refetch context, back to
   `list`.

Distinguishing expiry from first-connect: the `connectMode` state (`'connect' | 'change' | 'reauth'`),
NOT the View union. Only the copy differs between `'reauth'` and `'change'`; both overwrite.
`commit_statuses` failing with authFailed uses the same handler (it must not silently no-op the
whole panel — recommend it also trips reauth since a rejected token invalidates all forge calls).

---

## 5. PR-panel change/disconnect (piece 1)

Keep the `View` union values, **add one**: `'reauth'` is NOT added to `View` (it's `connectMode`);
instead add a small connected-account header rendered ABOVE `list`/`detail`. New state:

```ts
type ConnectMode = 'connect' | 'change' | 'reauth';
const [connectMode, setConnectMode] = useState<ConnectMode>('connect');
```

- **New component `src/components/ForgeAccountHeader.tsx`** (SRP — do NOT inline into PrPanel):
  a compact row shown when `ctx.viewer` is non-null (connected + warm), rendered above `PrList`
  and `PrDetailView`. Shows avatar + `viewer.login` + host, and two actions:
  - **Change token** → `setConnectMode('change'); setView('connect')`. `ForgeConnect` in `change`
    mode ("Replace token for {login}"); submit calls `forgeSetToken(repoId, token)` (overwrites).
    A Cancel returns to `list`.
  - **Disconnect** → confirm dialog (destructive; reuse the app's confirm pattern), then
    `forgeClearToken(repoId)` → `setView('connect'); setConnectMode('connect')`, refetch context.
- `ForgeConnect` gains a `mode: ConnectMode` prop selecting header/body/submit-label copy; its
  submit handler is unchanged (parent passes the right onSubmit).
- When `ctx.viewer` is null but `ctx.authenticated` is true (token present, viewer cold, e.g. after
  restart), the panel proceeds to `list` optimistically; a subsequent authFailed trips §4.

Confirmation: **Disconnect requires explicit confirm** (matches the destructive-op guardrail).
Change token does not (it's non-destructive — a rejected new token leaves the old one in place;
note the backend `set_token` only overwrites AFTER validation).

---

## 6. Global Accounts settings section (piece 3)

### 6.1 Catalog

- Add `'accounts'` to `SettingsCategoryId` in `src/components/settings/types.ts`.
- Add a `SettingsCategory` entry (label "Accounts", subtitle e.g. "Forge sign-ins used for pull
  requests and CI status", placed after `identities`; set `dividerBefore` as the design dictates —
  defer exact placement/copy to `ui-designer`).
- The section is an aggregate `control: 'group'` row (like Identities/Profiles) — its per-account
  rows are runtime-generated and not individually catalogued (AM-4b precedent). Add a
  `SettingsRowRequirement`-free `group` row id `accounts.list` plus an unconditional
  `accounts.add` button row.

### 6.2 New component `src/components/settings/SettingsAccountsSection.tsx`

Precedent: `SettingsProfilesSection.tsx`. Responsibilities:
- On mount, `ipc.forgeListAccounts()` → render one card per `ForgeAccount` (avatar/login/host,
  `connected` badge, provider kind).
- Per card: **Change token** (inline form → `forgeSetTokenForHost(host, kind, token)`) and
  **Disconnect** (confirm → `forgeClearTokenForHost(host)` then refetch list).
- **Add account:** a form taking host + provider kind + token → `forgeSetTokenForHost`. The kind
  selector disables `azureDevOps` with the OD-2 hint. On success, refetch list.
- All of this is repo-independent (no `repoId`).

Keep the section file focused; if the add-form grows, split it into
`SettingsAccountAddForm.tsx`. Do not append to `SettingsShell`/`SettingsPanel`; wire the new
category via the existing `CATEGORY_PAGES` map.

---

## 7. Mock IPC (`src/ipc/mock/handlers/forge.ts` + fixtures)

Keep everything offline and sentinel-aware. Maintain a module-level accounts map so the browser
harness exercises the full lifecycle:

```ts
// Seeded from ?forge=auth (one github.com account, viewer warm) else empty.
let accounts: ForgeAccount[] = /* seeded */;
```
- `forgeListAccounts()` → return `accounts` (clone).
- `forgeSetTokenForHost(host, kind, token)` → reject `authFailed` when `token.includes('bad')`;
  reject `forgeUnsupported` when `kind === 'azureDevOps'` (mirror OD-2); else upsert
  `{host, kind, login: FORGE_VIEWER.login, avatarUrl: FORGE_VIEWER.avatarUrl, connected:true}` and
  return `FORGE_VIEWER`.
- `forgeClearTokenForHost(host)` → remove from `accounts`.
- `forgeInvalidateViewer(host)` → keep the account but set its `viewer`-warmth off; for the mock,
  flip the existing module `authenticated`/viewer so `forgeRepoContext` returns `viewer:null,
  authenticated:true` (drives the reauth path).
- Existing `forgeSetToken`/`forgeClearToken` also upsert/remove the current host in `accounts`
  (keep the two views consistent in the harness).
- New sentinel (recommend): `?forge=expired` → the first `forgeListPrs` throws `authFailed` once,
  so the harness can exercise the §4 reauth flow without real tokens.

Add a `ForgeAccount` fixture to `src/fixtures/forge.ts`.

---

## 8. File-size / SRP notes

- New UI lives in its OWN files: `ForgeAccountHeader.tsx`, `settings/SettingsAccountsSection.tsx`
  (+ optional `SettingsAccountAddForm.tsx`). Do NOT grow `PrPanel.tsx` beyond adding the
  `connectMode` state, the header mount, and the authFailed handler; extract any new render body.
- Backend: the four new commands go in the existing `commands/forge.rs`; the four crate fns in
  `bonsai-forge/src/lib.rs`; `ForgeAccount` in `types.rs`; `ForgeHostRecord` + the `Settings`
  field in `settings.rs`. No file should cross ~500 lines from this work.

---

## 9. Acceptance criteria

### Piece 1 — PR-panel change/disconnect
- When `ctx.viewer` is non-null, `ForgeAccountHeader` shows avatar + login + host above list/detail.
- "Change token" routes to `ForgeConnect` in `change` mode; a valid token overwrites via
  `forgeSetToken` and returns to `list` with the header refreshed.
- "Disconnect" prompts a confirm; on confirm calls `forgeClearToken`, returns to `connect`, and
  the header is gone. A rejected new token during Change leaves the old account connected.

### Piece 2 — expiry → reconnect
- A forge call returning `authFailed` calls `forgeInvalidateViewer(host)`, routes to `connect` in
  `reauth` mode with expiry-specific copy, and the token is NOT deleted (keychain entry survives).
- Submitting a fresh valid token re-warms the viewer and returns to `list`; submitting another bad
  token stays in reauth with an inline error.

### Piece 3 — Accounts settings
- `forge_list_accounts` returns one entry per known host with correct `connected` + best-effort
  login/avatar, no network.
- Add-for-host validates + stores + appears in the list; `azureDevOps` add is disabled with the
  OD-2 hint; change/disconnect per card work and refetch.
- The known-hosts index in `settings.json` is upserted on every set (per-repo and host-based) and
  removed on every clear; a pre-P79 `settings.json` loads with `forge_hosts: []`; backfill adds a
  host when its repo is opened while a token exists.
- Wire-shape test passes for `ForgeAccount`; a settings round-trip test proves `forge_hosts`
  survives load→save and defaults empty on a legacy file.

---

## 10. AI-gate vs USER CHECKPOINT

**AI-gate (orchestrator-verifiable):**
- `cargo test` — `ForgeAccount` wire-shape, settings `forge_hosts` default/round-trip, index
  upsert/remove logic (with the fake keychain), `resolve_forge_host` (network-free), and the
  `set_token_for_host` unsupported-Azure / auth-failed paths via the canned transport.
- `tsc` + vitest — mock handlers, PrPanel authFailed→reauth transition, Accounts section render.
- Browser harness (`VITE_MOCK_IPC=1`): `?forge=auth` shows the account header + change/disconnect;
  `?forge=expired` drives the reauth flow; the Accounts settings section lists/adds/removes.

**USER CHECKPOINT (native only — cannot be AI-verified):**
- Real keychain read/write/delete across the three commands (native `keyring`).
- Real PAT validation against github.com / gitlab.com / bitbucket.org (network + real token).
- Add-a-token-for-a-host-with-no-repo-open against a real forge.
- Cross-restart persistence of `forge_hosts` and that a token stored pre-P79 backfills into the
  Accounts list on opening its repo.

---

## 11. Open decisions for the orchestrator

- **OD-1** — lazy backfill only, no keychain enumeration (a host whose repo is never opened stays
  hidden). Recommended: accept.
- **OD-2** — Azure DevOps add-for-host-without-a-repo is unsupported (`ForgeUnsupported`); Azure is
  still manageable from within its repo. Recommended: accept.
- **OD-3** — should a `forgeCommitStatuses` authFailed trip the full reauth flow, or fail quietly
  (statuses are a background enrichment)? Recommended: trip reauth (a rejected token breaks every
  forge call), but suppress the extra toast to avoid double-notifying. Confirm.
