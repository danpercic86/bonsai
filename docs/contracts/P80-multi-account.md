# P80 — Multi-account forge (host default + per-repo override)

Status: contract (design-only). Supersedes the P79 single-token-per-host model
(`docs/contracts/P79-forge-account-management.md`) additively — no P79 IPC name is
removed, existing tokens keep working with zero re-auth.

## 0. Problem & locked decisions

Today a forge token is keyed by **host only** (`auth.rs`: keychain account = bare
host, service `com.bonsai.app`), so exactly one account exists per host across the
whole app. P80 adds:

- **Account model = host default + per-repo override** (user-locked). A host may
  hold several accounts; one is the host **default**; every repo inherits it; a
  repo may **pin** a specific account to override; a repo with no pin falls through
  to the host default (via owner-match then host default, §4).
- Forge-agnostic — everything keys on `host + kind + account identity`; applies to
  GitHub, GitLab, Bitbucket. Azure DevOps stays add-from-repo-only (carried-forward
  P79 OD-2) and its connect radio stays disabled.
- Bundle the token-guidance copy refresh for GitLab + Bitbucket (§6).

Invariant restated: **tokens live ONLY in the OS keychain.** settings.json stores
host/kind/login/avatar/accountId/keychainKey/defaults/overrides — **never a token.**

## 1. Account identity & keychain keying

### 1.1 `accountId` (stable identity, crosses IPC + persisted)

```
accountId(kind, host, login) =
    lower(kindStr(kind)) + ":" + lower(host) + ":" + lower(login)      // login known
    lower(kindStr(kind)) + ":" + lower(host)                           // login unknown (legacy)
```
`kindStr` = the camelCase wire string (`gitHub` | `gitLab` | `bitbucket` |
`azureDevOps`). Lowercased throughout ⇒ case-insensitive identity, matching the
existing `TokenStore` lowercasing. The two-part form is the **legacy/host-default
marker** used only for a pre-P80 token whose login was never validated.

### 1.2 `keychainKey` (the ACTUAL OS-keychain account name holding the token)

Decoupled from `accountId` so migration needs **zero keychain rewrites**:

- **New (P80) account:** `keychainKey == accountId` (three-part).
- **Migrated legacy account:** `keychainKey == host` (the bare pre-P80 key) — the
  existing keychain entry is found unchanged ⇒ zero re-auth.

At most one legacy (bare-host) account can exist per host (pre-P80 stored one token
per host), so a legacy `keychainKey` never collides with a P80 three-part key on the
same host. Token read for any account = `auth::global().get(&account.keychain_key)`.

Optional (nice-to-have, not required): when a legacy account is re-validated via
`forge_add_account`, rekey it — store under the new three-part key and delete the
bare-host entry, updating `keychain_key`. Skip if it complicates increment A.

## 2. Persistence model (settings.json — additive, all `#[serde(default)]`)

`SETTINGS_VERSION` stays `1` (additive fields, per the documented bar in
`settings.rs`). New Rust types live in a new module `settings/forge_accounts.rs`
(keep files under the ~500-line limit; `settings/forge_hosts.rs` stays as the
legacy read/migration source).

```rust
/// P80: one connected (or previously-connected) forge account. NEVER holds a token.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeAccountRecord {
    /// Stable identity "kind:host:login" (or "kind:host" if login unknown).
    pub account_id: String,
    /// Actual OS-keychain account key holding this account's token
    /// (== account_id for P80 accounts, == host for a migrated legacy account).
    /// NOT a token.
    pub keychain_key: String,
    pub host: String,                 // lowercased
    pub kind: bonsai_forge::ForgeKind,
    pub login: Option<String>,        // None until first successful validation
    pub avatar_url: Option<String>,   // best-effort display hint; never a token
}

/// P80: the default account for a host; repos inherit it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeHostDefault {
    pub host: String,        // lowercased
    pub account_id: String,
}

/// P80: a repo's pinned account override (OD-1: keyed by canonical workdir path,
/// deduped via `commands::same_repo_path`, mirroring `recent_repos`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoForgeOverride {
    pub repo_path: String,   // canonical repo workdir path
    pub account_id: String,
}
```

Add to `Settings` (all `#[serde(default)]`, empty on a pre-P80 file):

```rust
pub forge_accounts: Vec<ForgeAccountRecord>,
pub forge_host_defaults: Vec<ForgeHostDefault>,
pub repo_forge_overrides: Vec<RepoForgeOverride>,
// `forge_hosts: Vec<ForgeHostRecord>` (P79) is RETAINED as the migration source
// (OD-5). Keep it mirrored for one release for rollback safety.
```

Helpers in `settings/forge_accounts.rs` (mirror the P79 upsert/remove style; each
returns `bool` where a hot path uses `update_if` to skip no-op writes):

```rust
pub fn upsert_forge_account(s: &mut Settings, rec: ForgeAccountRecord);
pub fn remove_forge_account(s: &mut Settings, account_id: &str);
pub fn set_host_default(s: &mut Settings, host: &str, account_id: &str);
pub fn clear_host_default(s: &mut Settings, host: &str);
pub fn set_repo_override(s: &mut Settings, repo_path: &str, account_id: &str);
pub fn clear_repo_override(s: &mut Settings, repo_path: &str);
/// Lazy P79→P80 migration; returns true if it changed anything (for `update_if`).
pub fn migrate_forge_hosts_to_accounts(s: &mut Settings) -> bool;
```

## 3. Migration (backward-compatible, lazy)

Run in `load_from` (in-memory, pure — no write) so every read sees the P80 shape,
then persist opportunistically on the next `update`/`update_if` (P79 backfill
pattern — never write-amplify a pure read):

```
migrate_forge_hosts_to_accounts(s):
  changed = false
  for h in s.forge_hosts:
    aid = accountId(h.kind, h.host, h.login)
    if s.forge_accounts has no account with host==h.host:      # not already migrated
      s.forge_accounts.push(ForgeAccountRecord{
        account_id: aid,
        keychain_key: h.host,          # legacy token lives under the bare host key
        host: h.host, kind: h.kind, login: h.login, avatar_url: None })
      if s.forge_host_defaults has no entry for h.host:
        s.forge_host_defaults.push({ host: h.host, account_id: aid })
      changed = true
  return changed
```

Keychain: **no keychain migration.** The migrated account's `keychain_key` is the
bare host, so `auth::global().get(host)` finds the existing token unchanged. A user
with one github.com token keeps working with zero re-auth, as that host's single
account and its default.

## 4. Resolution algorithm

The command layer (which has settings access) resolves the account; the pure crate
receives the resolved keychain key. `accounts_for_host` filters `forge_accounts` to
the host; `connected` is decided by keychain presence at call time (no network).
`owner` is the origin remote's owner/namespace, already parsed by the existing
crate `detect_provider` and surfaced as `ForgeTarget.owner` / `ForgeRepoContext.owner`
(handles https + ssh/scp forms, strips `.git`). NOTE: that parser lowercases the
HOST but PRESERVES owner case, so the owner-vs-login compare below must lowercase
both sides. No new URL/owner parser is needed — reuse `detect_provider`'s `owner`.

```
resolve_account(settings, repo_path, host, owner):
  accts = settings.forge_accounts where host == host
  if accts is empty: return { account: None, source: "none" }

  # 1. per-repo override (a MANUAL pin ALWAYS wins over an owner match)
  ov = settings.repo_forge_overrides find repo_path (same_repo_path compare)
  if ov and accts contains ov.account_id:
      return { account: that, source: "override" }
  # pinned account was deleted → DO NOT error; fall through

  # 2. owner match (login-based, NO API calls)
  if owner not empty:
      matches = accts where account.login is Some and lower(login) == lower(owner)
      if matches.len() == 1:
          return { account: matches[0], source: "ownerMatch" }
      # 0 or >1 owner matches → fall through (never error)

  # 3. host default
  d = settings.forge_host_defaults find host
  if d and accts contains d.account_id:
      return { account: that, source: "hostDefault" }

  # 4. single account
  if accts.len() == 1:
      return { account: accts[0], source: "single" }

  # 5. multiple accounts, no usable default (OD-4)
  return { account: accts[0], source: "hostDefault" }   # first (most-recent); UI nudges to pick a default
```

**Owner-match caveat (in-scope limit):** matching is purely `login == owner`, so an
org/group-owned repo (`some-org/repo`) will not match a personal login and correctly
falls through to the host default. Full org/namespace coverage is explicitly OUT OF
SCOPE for P80 — it would need per-account membership/namespace data and is deferred.

Token for any forge call = `auth::global().get(&resolved.account.keychain_key)`;
`None` account or `None` token ⇒ unauthenticated (existing `ForgeConnect` path).

### 4.1 Crate change (`bonsai-forge`)

`open()` must no longer read the token by bare host. Add:

```rust
/// Build a provider for `workdir` using an EXPLICIT keychain key (resolved by the
/// command layer). `keychain_key = None` ⇒ unauthenticated provider.
pub fn open_with_key(workdir: &Path, keychain_key: Option<&str>)
    -> Result<Box<dyn ForgeProvider>, AppError>;
```

`open()` is kept as a thin back-compat wrapper (`open_with_key(workdir,
Some(host))`) for any not-yet-migrated caller, but all `forge_*` commands route
through `open_with_key` with the resolved key. Add a command-layer helper (owner is
carried so `resolve_account` can do the owner match):

```rust
// commands/forge.rs — resolve once, reuse for every forge call in a command.
struct ForgeResolution { workdir: PathBuf, host: String, kind: ForgeKind, owner: String,
                         keychain_key: Option<String>,
                         account_id: Option<String>, source: AccountSource }
fn resolve_forge(state, settings_file, repo_id) -> Result<ForgeResolution, AppError>;
```

`set_token_for_host` / `clear_token_for_host` / `set_token` / `clear_token` in the
crate gain key-explicit variants (`*_with_key`) so the command layer controls which
account's keychain entry is written; validation logic (`validate_token`) is
unchanged.

## 5. IPC surface

All commands = request/response (no new events; the PR panel refetches on demand,
matching the P79 note). Rust `#[tauri::command]` in `commands/forge.rs`; TS mirror
in `src/ipc/types.ts` (`IpcApi`), wired in `src/ipc/tauri.ts`, mocked in
`src/ipc/mock/handlers/forge.ts`, fixtures in `src/ipc/fixtures/forge.ts`.

### 5.1 Changed DTOs

```rust
// bonsai-forge/src/types.rs — extend ForgeAccount (all camelCase on the wire).
pub struct ForgeAccount {
    pub account_id: String,        // NEW
    pub host: String,
    pub kind: ForgeKind,
    pub login: Option<String>,
    pub avatar_url: Option<String>,
    pub connected: bool,
    pub is_host_default: bool,     // NEW
}

// ForgeRepoContext — add the resolved-account fields.
pub struct ForgeRepoContext {
    /* …existing… */
    pub resolved_account_id: Option<String>,   // NEW
    pub account_source: AccountSource,         // NEW
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AccountSource { Override, OwnerMatch, HostDefault, Single, None }
```

TS mirror:

```ts
export interface ForgeAccount {
  accountId: string;
  host: string;
  kind: ForgeKind;
  login: string | null;
  avatarUrl: string | null;
  connected: boolean;
  isHostDefault: boolean;
}
export type AccountSource = 'override' | 'ownerMatch' | 'hostDefault' | 'single' | 'none';
// ForgeRepoContext gains: resolvedAccountId: string | null; accountSource: AccountSource;
```

Add camelCase wire-shape unit tests for the extended `ForgeAccount`,
`ForgeRepoContext`, and `AccountSource` (mirror existing tests in `types.rs`);
assert `AccountSource::OwnerMatch` serializes to `"ownerMatch"`.

### 5.2 Commands

Existing (behavior extended, names unchanged):

| Command | Signature | P80 change |
|---|---|---|
| `forge_repo_context` | `(repoId) -> ForgeRepoContext` | now runs `resolve_account` (incl. owner match); fills `resolvedAccountId` + `accountSource`; `authenticated`/`viewer` reflect the RESOLVED account's token/viewer |
| `forge_list_accounts` | `() -> Vec<ForgeAccount>` | returns `accountId` + `isHostDefault`; lists ALL accounts across hosts |
| `forge_list_prs`/`forge_get_pr`/`forge_create_pr`/`forge_list_review_comments`/`forge_commit_statuses` | unchanged sigs | route through resolved key via `open_with_key` |
| `forge_set_token` | `(repoId, token) -> ForgeViewer` | validates for origin host, learns login, **adds/updates that account** (three-part key) AND pins it as the repo override (OD-3) |
| `forge_clear_token` | `(repoId) -> ()` | **clears the repo override only** (OD-2), does NOT delete the account |
| `forge_invalidate_viewer` | `(host) -> ()` | unchanged |

New:

```
forge_add_account(host: string, kind: ForgeKind, token: string) -> ForgeViewer
  // Validate PAT (no repo), learn login, store under three-part keychain key,
  // upsert forge_accounts; if the host has no default, set this as default.
  // Azure DevOps ⇒ forgeUnsupported (carried-forward P79 OD-2). Supersedes
  // forge_set_token_for_host — KEEP the old name as a thin alias for back-compat.

forge_remove_account(accountId: string) -> ()
  // Delete the account's token by its keychain_key, remove from forge_accounts,
  // and clean references: if it was a host default, promote another account on
  // that host or clear the default; remove any repo overrides pointing to it.
  // Idempotent.

forge_set_host_default(host: string, accountId: string) -> ()
  // Set/replace the host's default account. Errors if accountId isn't on host.

forge_set_repo_account(repoId: string, accountId: string | null) -> ()
  // Pin (accountId) or clear (null ⇒ inherit: owner match → host default) the
  // repo override.
```

`forge_clear_token_for_host(host)` (P79) is retained: signs out ALL accounts on a
host (delete each account's keychain entry + records + defaults + overrides for that
host). Recommend the UI prefer per-account `forge_remove_account`.

Request/response stay compact (ids + small DTOs; no token ever returned).

### 5.3 Mock parity (`src/ipc/mock/handlers/forge.ts`)

The module-level `accounts` array becomes `ForgeAccountRecord`-shaped (accountId,
keychainKey modeled as the map key); add module-level `hostDefaults`
(`Record<host, accountId>`) and `repoOverrides` (`Record<repoId, accountId>`).
Implement `resolve_account` (incl. the owner-match step, parsing owner from the
fixture repo context) in JS for `forgeRepoContext`. New handlers `forgeAddAccount`,
`forgeRemoveAccount`, `forgeSetHostDefault`, `forgeSetRepoAccount`; keep
`forgeSetTokenForHost` as an alias to `forgeAddAccount`. Add a `?forge=multi`
sentinel seeding two github.com accounts (one host default; distinct logins) so the
harness exercises switching, owner match + override without a native window.
Preserve existing sentinels (`auth`/`off`/`expired`/`gitlab`/`bitbucket`/`azure`/
`unsupported`). `bad`-token rejection unchanged.

## 6. Token-guidance copy refresh (bundled — copy only, exact strings)

Edit `CONNECT_HINTS` in `src/components/ForgeConnect.tsx`. gitHub + azureDevOps +
unknown are UNCHANGED. Replace the two entries below verbatim:

```ts
  gitLab: {
    scopes:
      'Use a personal access token with the "api" scope. Read-only scopes such as "read_api" or "read_repository" are not enough to create merge requests.',
    url: 'https://gitlab.com/-/user_settings/personal_access_tokens',
    placeholder: 'glpat-…',
  },
  bitbucket: {
    scopes:
      'Use a repository or workspace access token with Pull requests (read and write). App passwords still work but Atlassian is retiring them during 2026, so prefer an access token.',
    url: 'https://support.atlassian.com/bitbucket-cloud/docs/create-a-repository-access-token/',
    placeholder: 'access token',
  },
```

## 7. Acceptance criteria (testable)

1. **Migration/zero-re-auth:** a settings.json with a P79 `forge_hosts` github.com
   record (token under bare-host keychain key) loads with one github.com account
   that is the host default, `connected == true`, and `forge_list_prs` succeeds
   using the existing token — no re-auth, no keychain rewrite. (Rust unit test with
   a fake keychain seeded under the bare host key.)
2. **Two accounts coexist:** `forge_add_account` twice on github.com (distinct
   logins) yields two `forge_accounts` with distinct three-part keychain keys; both
   `connected`.
3. **Per-repo override resolves:** with host default = A, `forge_set_repo_account(repoB, B)`
   ⇒ `forge_repo_context(repoB).resolvedAccountId == B`, `accountSource == "override"`;
   repoA (no override) resolves to A with `"hostDefault"`.
4. **Owner match:** with a `danpercic86` github.com account present (and no repo
   override), both `danpercic86/bonsai` and `danpercic86/play` resolve to that
   account with `accountSource == "ownerMatch"`; `randomuser/repo` does NOT resolve
   to it (falls to host default/single/none). Case-insensitive (`DanPercic86/bonsai`
   still matches). A manual per-repo override for `danpercic86/bonsai` pointing at a
   different account WINS over the owner match (`accountSource == "override"`).
5. **Deleted pin falls back:** override points at B, then `forge_remove_account(B)`
   ⇒ `forge_repo_context(repoB)` resolves via owner match/host default, NEVER
   errors, and the dangling override is cleaned.
6. **No token in settings.json:** after any set/add/remove flow, the serialized
   settings.json contains no PAT substring (assert on the JSON text).
7. **Copy:** `CONNECT_HINTS.gitLab`/`.bitbucket` equal the §6 strings; gitHub
   unchanged.
8. **Mock parity:** `?forge=multi` renders account switching in the browser harness;
   `tsc` + vitest green; `IpcApi` and the mock satisfy each other.

## 8. Decomposition (senior-dev sub-increments)

- **A — backend account model (no UI; no ui-designer).**
  Settings shapes + `settings/forge_accounts.rs` helpers + lazy migration; crate
  `open_with_key` + `*_with_key` auth variants + extended `ForgeAccount`/
  `ForgeRepoContext`/`AccountSource` DTOs (incl. `OwnerMatch`) + wire-shape tests;
  command-layer `resolve_forge` (owner-aware) + all `forge_*` commands rerouted; new
  commands (`forge_add_account`, `forge_remove_account`, `forge_set_host_default`,
  `forge_set_repo_account`) + aliases; TS `IpcApi` types + `tauri.ts` wiring; mock
  handlers + `?forge=multi` fixtures. Gate: cargo + tsc + vitest green, criteria
  1–6,8.
- **B — account-management UI (needs `ui-designer` FIRST → `docs/contracts/P80-ui.md`).**
  Account switcher in the PR panel / `ForgeAccountHeader`, host-default control +
  per-repo override control in `SettingsAccountsSection`, add/remove-account flow,
  showing `accountSource` (incl. an "auto-matched by owner" hint) in the repo
  context. Consumes only increment-A IPC.
- **B1 — copy refresh (§6).** Pure strings; can fold into B (ui-designer owns copy)
  or ship standalone. Criterion 7.

## 9. Open decisions (RESOLVED by user 2026-08-21 — kept for the record)

- **OD-1 → settings.json:** per-repo override stored in settings.json keyed by
  canonical workdir path (consistent with `recent_repos`, mock-friendly).
- **OD-2 → clear override only:** `forge_clear_token(repoId)` clears the repo's
  override; the account stays connected. Deletion is `forge_remove_account`.
- **OD-3 → auto-pin on connect:** `forge_set_token(repoId, …)` pins the newly-added
  account as that repo's override.
- **OD-4 → first + nudge:** multiple accounts on a host with no default resolve to
  the first (most-recent) and the UI nudges the user to pick a default.
- **OD-5 → keep one release:** keep mirroring the legacy `forge_hosts` index for
  rollback safety.
- **OD-6 → Azure disabled:** Azure DevOps stays add-from-repo-only, connect radio
  disabled (carried from P79).
