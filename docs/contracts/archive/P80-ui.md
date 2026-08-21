# P80 increment B — Multi-account forge UI — UI Contract

Owner: ui-designer. Input contracts: `docs/contracts/P80-multi-account.md` (backend, increment A,
merged) and `docs/contracts/P79-ui.md` (the single-account surfaces this extends). Increment A IPC
is the input; do not redesign the data shape.

Design stance: **reuse, do not invent.** Every surface here is built from patterns that already
ship — `ForgeAccountHeader` / `ForgeAvatar` / `ForgeProviderBadge` (P79), `ContextMenu` with radio
rows + `header` + `detail` + `busy` (P69i — this is exactly the identity-menu switcher idiom, §12.6),
`SettingsGroup` / `SettingsRow` / `SettingsEmpty`, `ConfirmDialog`, `.error-banner`. **No new design
token.** ui-reference.md gets one small additive edit: the `AccountSource` label vocabulary is
catalogued so its microcopy cannot drift (§9 below).

All copy strings are literal. Sentence case, plain language, no raw libgit2 / HTTP text.

---

## 0. Data the UI consumes (from increment A — no new IPC)

- `ForgeRepoContext.resolvedAccountId: string | null` and `.accountSource: AccountSource`
  (`'override' | 'ownerMatch' | 'hostDefault' | 'single' | 'none'`).
- `ForgeAccount` now carries `accountId` + `isHostDefault`; `forgeListAccounts()` returns **all**
  accounts across **all** hosts. The PR-panel switcher filters to `ctx.host` client-side.
- Writes: `forgeSetRepoAccount(repoId, accountId | null)` (pin / clear override),
  `forgeSetHostDefault(host, accountId)`, `forgeAddAccount(host, kind, token)` (canonical; upserts by
  learned login — prefer over the `forgeSetTokenForHost` alias), `forgeRemoveAccount(accountId)`,
  `forgeSetToken(repoId, token)` (validates + adds + auto-pins the repo, OD-3),
  `forgeClearToken(repoId)` (clears the repo override only, OD-2 — equivalent to
  `forgeSetRepoAccount(repoId, null)`; prefer the latter as the explicit call).

### 0.1 Shared: `AccountSource` display vocabulary (new tiny module)

New file **`src/components/forgeAccountSource.ts`** (~25 lines, no JSX): a pure map so the same
strings back the switcher caption, its tooltip, and any Settings hint.

| `accountSource` | short caption (in-header) | tooltip / long form |
|---|---|---|
| `override`   | `Pinned to this repo`  | `Pinned to this repository. Other repositories on this host use the default.` |
| `ownerMatch` | `Matched by owner`     | `Chosen because its username matches this repository's owner.` |
| `hostDefault`| `Host default`         | `The default account for this host.` |
| `single`     | *(none — no caption)*  | *(no tooltip; only one account exists)* |
| `none`       | *(none — not connected)* | *(n/a; the connect view shows instead)* |

```ts
export function accountSourceCaption(s: AccountSource): string | null;  // null for single/none
export function accountSourceTooltip(s: AccountSource): string | null;
```

Caption text: `--text-3`, 11px, no color-only meaning (the words carry it). Never shown for `single`
(no clutter when there is nothing to disambiguate).

---

## 1. PR-panel account header — the account switcher

### 1.1 Behaviour split (design decision)

The header must do two distinct things without adding chrome: **show/switch which account this repo
uses** and **act on the active account's token**. Map them to the two controls the P79 header
already has:

- **Left group (avatar + login + host) becomes the switcher trigger** — but **only when the host has
  ≥2 accounts**. With 0–1 accounts it stays exactly the P79 static text (no switcher clutter, per the
  brief). The trigger opens a `ContextMenu` of accounts (the identity-menu idiom, §12.6).
- **Kebab (`⋯`) keeps token/override actions** (Change token; Reset to host default; Manage
  accounts…). It is unchanged geometry from P79.

Rationale: switching account is a *selection* (radio menu, matches the identity switcher the user
already knows); token/override actions are *commands* (kebab). Reusing both existing affordances
means no new top-level control and no new token.

### 1.2 Geometry (unchanged from P79 §1.1)

`.forge-account-header`: `flex none; display:flex; align-items:center; gap:8px; padding:8px 12px`
(cozy) / `6px 12px` (compact); `border-bottom:1px solid var(--border)`. Avatar 22px cozy / 20px
compact. Order: avatar · login · host · **source caption** · spacer · provider badge · kebab.

Add the source caption as a new muted inline element between host and the provider badge:
`.forge-account-source` (`flex:none; font-size:11px; color:var(--text-3); white-space:nowrap;
overflow:hidden; text-overflow:ellipsis; max-width:11ch`). Rendered only when
`accountSourceCaption(ctx.accountSource) !== null`. Its `title` is the long tooltip.

```
┌ .forge-account-header ─────────────────────────────────────────────┐
│ (o) octocat   github.com  · Pinned to this repo      [GH]      [⋯]  │
└─────────────────────────────────────────────────────────────────────┘
        └─────── switcher trigger (button when ≥2 accounts) ──┘
```

When the left group is a switcher trigger, prefix a small chevron `▾` (`aria-hidden`,
`.forge-account-caret`, `--text-3`, 10px) after the host so it reads as openable; omit it in the
static (≤1 account) case.

### 1.3 The switcher menu

Reuse `ContextMenu`, anchored the house way (`rect.right`, `rect.bottom + 2` off the trigger).

- **`header` block** (non-interactive): `Accounts on {host}`. If the host has ≥2 accounts and **no
  default is set** (OD-4 nudge, §4), append a second line in `--text-3`, 11px:
  `No default account set — pick one below or in Settings.`
- **One radio row per account on the host** (`checked` ⇒ `role="menuitemradio"`):
  - `label` = `login` (or `host` when login is null).
  - `checked` = `account.accountId === ctx.resolvedAccountId`.
  - `detail` (second line, 12px `--text-2`) = a suffix showing role, drawn from the account, not the
    live source: `Host default` when `account.isHostDefault`, else omitted. (The *why-active* caption
    lives in the header; the menu's detail states the account's standing.)
  - `onSelect` → `forgeSetRepoAccount(repoId, accountId)`. Selecting the already-resolved account is
    still allowed (it pins it explicitly — turns an `ownerMatch`/`hostDefault` into an `override`);
    that is the intended "make this stick" gesture.
- **Separator**, then two plain rows:
  - `Use host default` — enabled only when `ctx.accountSource === 'override'` (otherwise the repo is
    already inheriting; show it `disabled`). → `forgeSetRepoAccount(repoId, null)`.
  - `Add another account…` → routes into the connect view in a new **`add`** mode (§1.6) which, on
    success, auto-pins the new account to this repo (uses `forgeSetToken(repoId, token)`, OD-3).

### 1.4 Kebab menu (revised from P79)

Items (reuse `ContextMenu`):
1. `Change token` — replace the **active** account's token → connect view `change` mode (unchanged).
2. `Reset to host default` — shown only when `ctx.accountSource === 'override'`; **not** danger
   (nondestructive, instantly reversible). → `forgeSetRepoAccount(repoId, null)`, then
   `setBootstrapTick`. No ConfirmDialog.
3. Separator, then `Manage accounts…` — opens Settings → Accounts (the app's settings-open action).
   This is where full sign-out lives; keeps removal out of the per-repo surface.

The P79 destructive **"Disconnect"** item and its ConfirmDialog are **removed** — see §2.

### 1.5 Loading / disabled / empty / error states

| State | Rendering |
|---|---|
| host has 1 account (`single`) | static header (P79), no chevron, no source caption, kebab has Change token + Manage accounts… only |
| host has ≥2 accounts | switcher trigger active; caption + chevron shown |
| a write in flight (pin / reset) | keep the menu open with `busy` (ContextMenu `aria-busy`); the header shows the optimistic new active account after `setBootstrapTick` resolves; trigger `aria-disabled` while busy |
| account list still loading (accounts fetched lazily on first menu open) | first trigger click opens the menu with a single non-interactive `header` line `Loading accounts…`; rows populate when `forgeListAccounts` resolves; on failure the header line becomes `Couldn't load accounts. {msg}` (no rows) |
| long login/host | login ellipsises (P79); menu rows ellipsise `label`; `detail`/`title` carry the full value |
| `accountSource === 'none'` | header not rendered at all (the connect view shows) — unchanged from P79 §1.5 |

Fetch strategy: to avoid a list round-trip on every repo open, the switcher fetches
`forgeListAccounts()` **lazily on first trigger open** and caches it in `PrPanel`; it refetches after
any add/remove/default write and on `bootstrapTick`. The header still renders instantly from
`ctx` alone.

### 1.6 New / changed components (PR side)

- **`src/components/ForgeAccountSwitcher.tsx`** (new, ~130 lines, presentational). Props:
  ```ts
  interface ForgeAccountSwitcherProps {
    host: string;
    activeLogin: string | null;      // ctx.viewer?.login
    activeAvatarUrl: string | null;
    kind: ForgeKind;
    accountSource: AccountSource;
    resolvedAccountId: string | null;
    accounts: ForgeAccount[] | null; // null = not yet loaded; filtered to host by parent or self
    accountsError: string | null;
    busy: boolean;
    onOpenMenu(): void;              // triggers lazy fetch
    onSelectAccount(accountId: string): void;
    onUseHostDefault(): void;
    onAddAnother(): void;
    onChangeToken(): void;
    onResetToDefault(): void;
    onManageAccounts(): void;
  }
  ```
  Renders the trigger (switcher or static), the source caption, both menus. No IPC, no confirm — it
  requests actions; `PrPanel` owns state, the account cache, and the writes.
- **`src/components/ForgeAccountHeader.tsx`** (changed): becomes a thin wrapper that composes
  `ForgeAccountSwitcher` (when accounts may exist) or the static P79 row. Keep it ≤120 lines; if it
  grows, fold the static row into the switcher's `single` branch and delete the split. Its P79 props
  are retained and extended with the switcher props above.
- **`src/components/PrPanel.tsx`** (changed, container): add `accounts` cache state + lazy fetch;
  wire the new handlers to the increment-A IPC; **remove** `confirmDisconnect`, `disconnecting`,
  `handleDisconnect`, and the Disconnect `ConfirmDialog`. Add `handleSelectAccount`,
  `handleUseHostDefault`, `handleAddAnother` (sets a new connect mode), `handleManageAccounts`.
  `ForgeConnect` gains the `add` mode (§1.6a).

### 1.6a `ForgeConnect` `add` mode (per-repo add-another)

Extend `ConnectMode` to `'connect' | 'change' | 'reauth' | 'add'`. `add` mode copy (adding a second
account for the current host, from the PR panel):

| element | `add` |
|---|---|
| banner | none |
| heading | `Add another account for {host}` |
| sub-line | `Paste a token for a different account. This repository will use the new account.` |
| submit idle / busy | `Add account` / `Adding…` |
| Cancel | shown (returns to `list`) |

On success `PrPanel` calls `forgeSetToken(repoId, token)` (validates, adds, auto-pins → the repo's
override becomes the new account, OD-3), then `setConnectMode('connect')` + `setBootstrapTick`. The
scopes hint / keychain note render identically (as in every mode).

### 1.7 A11y & keyboard (switcher)

- Trigger: `<button>` with `aria-haspopup="menu"`, `aria-expanded`, accessible name
  `Switch account (currently {login})`. ≥24px hit target (the left group already exceeds this).
- Menu: `ContextMenu` roving focus, Esc-close, focus-restore to the trigger (existing behaviour);
  radio rows announce `aria-checked`; the check column reserves for all rows (P69i).
- The source caption is decorative-adjacent text, not a control; the trigger's accessible name and
  the menu convey the state to AT. Never color-only.

---

## 2. "Disconnect" semantics (OD-2) — resolved copy

The P79 "Disconnect" wording is **retired** from the PR panel because `forgeClearToken(repoId)` no
longer signs out — it only unpins the repo. Full sign-out (keychain token deletion) lives **only** in
Settings → Accounts via `forgeRemoveAccount`.

- The unpin action is the kebab's **`Reset to host default`** (§1.4) — nondestructive, no confirm,
  no danger styling.
- To reach real sign-out, the kebab offers **`Manage accounts…`** → Settings → Accounts.
- No PR-panel ConfirmDialog remains for account actions. Removing a token is a deliberate,
  named, destructive act that only happens in Settings (§3.6).

This prevents the "Disconnect looked like sign-out but only unpinned" ambiguity: the per-repo control
now says exactly what it does (*reset this repo to the host default*), and the destructive control
(*remove account*) is elsewhere and clearly labelled.

---

## 3. Settings → Accounts — multi-account section

Restructures P79 §3 from a flat host list into **host groups**, each owning its accounts, its
default control, and an "add another account" affordance.

### 3.1 Structure

```
Connected accounts
┌ host group: github.com  [GH] ───────────────────────────────────────┐
│  (o) octocat        ◉ Default   ● Connected              [⋯]         │
│      github.com                                                       │
│  (a) alt-bot        ○ Default   ● Connected              [⋯]         │
│      github.com                                                       │
│  [ + Add another account to github.com ]                             │
└──────────────────────────────────────────────────────────────────────┘
┌ host group: gitlab.com  [GL] ──────────────────────────────────────┐
│  (m) me             (only account)  ● Connected          [⋯]        │
│  [ + Add another account to gitlab.com ]                            │
└─────────────────────────────────────────────────────────────────────┘
[ Add a token for a host ]     ← existing global add (new host)
```

- Group container: reuse `SettingsGroup` for the outer `Connected accounts` title; within it, one
  **`.settings-account-group`** block per host (new class; `border:1px solid var(--border);
  border-radius:8px; padding:8px; display:flex; flex-direction:column; gap:8px`). Its header row:
  `ForgeProviderBadge` + host name (`--text-2`, 12px, mono, ellipsis + `title`).
- Cards inside a group drop their own host sub-line duplication where redundant (host is on the
  group header); keep the per-card host sub-line only when `login === null` (identity unknown).

### 3.2 Host-default control (per account)

Each connected account card gets a **default radio** within its host group's `radiogroup`:

- Control: a native `<input type="radio">` styled as the app's radio (reuse the P79
  `.settings-account-kind` radio look), label **`Default`**, one group per host
  (`name="host-default-{host}"`). `checked = account.isHostDefault`.
- Selecting → `forgeSetHostDefault(host, accountId)` → refetch list. Optimistic: the radio flips
  immediately; on error revert + inline `.error-banner` in the group.
- A disconnected account (`connected === false`) cannot be made default — its radio is `disabled`
  with `title` `Add a token before making this the default.`
- Text label `Default` is the carrier; the radio dot is not the sole signal (word + control).

**Design note (flagged):** a host with exactly one account has a trivial default. Show the radio as
**`(only account)`** static text (not an interactive radio) in the single-account case to avoid a
pointless control — the account is implicitly the default. Recommendation: implement this
single-account simplification; it matches "restraint over addition."

### 3.3 Connected state chip (unchanged from P79 §3.3)

`● Connected` (`--success` dot, `--text-2` word) / `○ Token missing` (`--text-3` dot) with the P79
help line for the missing case. Word carries meaning, never color alone.

### 3.4 Per-card kebab (revised)

Items:
1. `Change token` / `Add token` (when disconnected) — inline form (P79 §3.4), now calling
   `forgeAddAccount(host, kind, token)` (canonical). Note: if the new token authenticates as a
   *different* login, this creates a second account rather than mutating this one — acceptable and
   expected; the list refetch shows both. State this in a `settings-row-help` line under the form:
   `Uses whichever account the token belongs to. A token for a different user adds a new account.`
2. `Make default` — present only when `!account.isHostDefault` **and** `connected` **and** the host
   has ≥2 accounts (redundant with the radio, but discoverable from the kebab; omit if it clutters —
   the radio is the primary control). Recommendation: **omit** the kebab duplicate; keep only the
   radio. Listed here as the considered alternative.
3. `Remove account` (danger) → `forgeRemoveAccount(accountId)` confirm (§3.6).

### 3.5 Add another account to a host

Each host group ends with a secondary button **`Add another account to {host}`**
(`.btn-secondary settings-toggle-btn`). It reveals the existing `SettingsAccountAddForm` **with the
host + kind pre-filled and locked** to that group (host field read-only, kind fixed to the group's
kind — you are adding to a known host). On success: collapse, refetch, toast
`Added {login} to {host}.` The global **`Add a token for a host`** button (P79) stays below the
groups for adding a *new* host.

`SettingsAccountAddForm` gains optional props `lockedHost?: string` and `lockedKind?: ForgeKind`;
when present the kind radiogroup and host field render read-only (kind shown as a static badge + host
as disabled text) and the form calls `forgeAddAccount`.

### 3.6 Remove-account confirm (destructive — now per account, with fallback warning)

Reuse `ConfirmDialog` (danger), owned by `SettingsAccountsSection`. Targets `accountId`.

- Title: **`Remove {login-or-host}?`**
- Body: `This deletes the saved token for {login} on {host} from your OS keychain.`
  Then, **conditionally**, a fallback line built from what removal will affect:
  - if this account is the host default: ` It's the default for {host}; another account will become
    the default, or {host} will have none.`
  - if repos are pinned to it (see §3.6a): ` {N} repository/repositories pinned to it will fall back
    to the host default.`
  - closing: ` This can't be undone — you'll need a new token to sign in again.`
  Render `{login}` / `{host}` in `<span className="mono">`.
- Confirm: **`Remove`** (danger). On confirm → `forgeRemoveAccount(accountId)` → refetch.
- No undo affordance (the keychain token is gone); the copy states it plainly. Not a bare "Are you
  sure?" — it names the exact account and every consequence.

### 3.6a Per-repo override surfacing (pragmatic scope)

The brief asks to surface/manage per-repo overrides "if practical." Full override management needs a
repo→override listing IPC that increment A does **not** expose (`repo_forge_overrides` is not
returned by any read command). **Recommendation (flagged for orchestrator):**

- **In scope now:** the removal confirmation's fallback line states repos will fall back. Because we
  can't enumerate them without a new IPC, word it generically: `Any repository pinned to this account
  will fall back to the host default.` (drop the `{N}` count).
- **Out of scope / follow-up:** a dedicated "Repositories using this account" list. If the
  orchestrator wants it, it needs a new `forge_list_repo_overrides() -> [{repoPath, accountId}]`
  read command from the architect. I recommend deferring — the fallback is already safe and
  non-lossy (resolution never errors on a dangling pin, backend §4/criterion 5).

### 3.7 No-default nudge (OD-4)

Appears **in Settings**, inside a host group, when the group has **≥2 connected accounts and none is
`isHostDefault`**:

- A `SettingsRow`-styled inline hint at the top of the group's account list (reuse the
  `settings-group-note` / caveat styling referenced in ui-reference §12.2), `role="note"`:
  **`Pick a default account for {host}. Repositories with no pinned account will use it.`**
- Non-blocking, always visible while the condition holds (it reflects a real config gap, not a
  transient message). **Not dismissible** — dismissing would hide an unresolved setting; selecting
  any Default radio resolves and removes it naturally. (Flagged: the brief mentions
  "dismissibility"; my recommendation is a self-resolving nudge over a dismiss button, because a
  dismissed-but-unresolved default silently returns to OD-4's "resolve to first" behaviour with no
  reminder. If the orchestrator prefers dismiss, make it session-only via component state.)
- A subtler echo appears in the PR-panel switcher menu header (§1.3) so the user can fix it in place.

### 3.8 States (Settings)

| State | Rendering |
|---|---|
| loading | `SkeletonRows` (P79) |
| empty | `SettingsEmpty` — title `No accounts connected`, body per P79 §3.8 |
| populated | host groups, each with its accounts + default control + add-another |
| single account on a host | `(only account)` static in place of the radio (§3.2) |
| ≥2 accounts, no default | OD-4 nudge shown (§3.7) |
| list fetch error | inline `.error-banner` + `Retry` (P79) |
| disconnected account | `Token missing` chip + help line; default radio disabled |
| add-another form open | form expands at the group's end; group's add button hidden while open |
| long host/login | ellipsis + `title` throughout |

### 3.9 Components (Settings side)

- **`src/components/settings/SettingsAccountsSection.tsx`** (changed, container ~180 lines): fetch,
  **group accounts by host** (stable order: host of the current repo first if known, else
  alphabetical), own the Remove confirm + add-another open state. Composes host groups.
- **`src/components/settings/SettingsAccountHostGroup.tsx`** (new, ~120 lines, presentational): one
  host's badge+title, the OD-4 nudge, its `radiogroup` of account cards, and the "Add another
  account to {host}" button + inline add form slot. Props: `host`, `kind`, `accounts` (this host),
  `onSetDefault(accountId)`, `onRequestRemove(account)`, `onChanged`, `onOpenUrl`.
- **`src/components/settings/SettingsAccountCard.tsx`** (changed): add the `Default` radio /
  `(only account)` static, per §3.2; kebab `Remove account` now → `forgeRemoveAccount(accountId)`;
  change-token form calls `forgeAddAccount`; add the "adds a new account" help line (§3.4). Props
  gain `isOnlyOnHost: boolean`, `onSetDefault(): void`.
- **`src/components/settings/SettingsAccountAddForm.tsx`** (changed): add `lockedHost?`,
  `lockedKind?`; call `forgeAddAccount`.
- **`src/components/settings/catalog/accounts.ts`** (unchanged): the one catalogued row stays
  `accounts.add`. (Per-account/per-group controls remain runtime-generated, uncatalogued — the
  `SettingsProfilesSection` precedent.)

Split from the start; do not grow `SettingsShell` / `SettingsPanel`. Each file stays under the
~500-line soft limit.

### 3.10 A11y (Settings)

- Host group: `<section role="group" aria-labelledby={group-title-id}>`; the default radios form one
  `radiogroup` per host (`aria-label` `Default account for {host}`).
- Radio `Default`: label is the accessible carrier; disabled state has `title`/`aria-describedby`
  pointing at the reason.
- OD-4 nudge: `role="note"`, referenced by the radiogroup via `aria-describedby` so AT users hear why
  the group needs attention.
- Kebab menus, token fields, ConfirmDialog focus: unchanged from P79 §3.9 (roving focus, Esc,
  restore, `type=password` never prefilled, focus rings 2px `--accent` `:focus-visible`).
- All hit targets ≥24px.

---

## 4. The no-default nudge — summary

Two placements, one condition (`≥2 connected accounts on the host, none default`):
1. **Settings host group** (primary, §3.7) — `role="note"`, self-resolving, not dismissible.
2. **PR-panel switcher menu header** (echo, §1.3) — one `--text-3` line so it is fixable in flow.
Copy: `Pick a default account for {host}. Repositories with no pinned account will use it.`
(Settings) and `No default account set — pick one below or in Settings.` (switcher header).

---

## 5. GitLab + Bitbucket connect-copy refresh (§6, copy only — FINAL)

Edit `CONNECT_HINTS` in `src/components/ForgeConnect.tsx`. `gitHub`, `azureDevOps`, and `unknown`
are **unchanged** (verbatim P78). Replace exactly these two entries:

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

These strings are reused verbatim by the Settings change/add forms (they read `CONNECT_HINTS`), so
the refresh lands everywhere in one edit.

---

## 6. Motion, themes, densities

- **Motion:** inline form reveal (add-another / change) ≤120ms `opacity` + `translateY(-2px)`
  ease-out, gated by `prefers-reduced-motion: reduce`. Menus use the existing `ContextMenu` motion.
  No motion on the header, switcher trigger, or graph.
- **Themes:** dark + light both covered by existing tokens only — `--bg-1/2/3`, `--border`,
  `--text-1/2/3`, `--accent`, `--success`, `--warning`, `--danger`. No literal hex.
  - Source caption `--text-3` on `.forge-account-header` bg (`--bg-2`): decorative-adjacent muted
    text; still clears AA ≥4.5:1 in both themes (same pairing as `.forge-account-host`, shipped).
  - `.settings-account-group` `--border` edge on `--bg-1`: ≥3:1 both themes (same as every settings
    card border).
- **Densities:** header padding + avatar per P79 §6 (`8px 12px`/22px cozy, `6px 12px`/20px compact);
  host-group padding constant `8px`; font sizes unchanged across densities.

---

## 7. New CSS classes (no new tokens)

In `src/styles/forge-pr.css`: `.forge-account-source`, `.forge-account-caret`.
Alongside the `.settings-account*` block: `.settings-account-group`,
`.settings-account-group-head`, `.settings-account-default` (the radio wrapper),
`.settings-account-default--static` (the `(only account)` text). Each defined purely with existing
tokens (§6). **No `:root` / `[data-theme='light']` additions.**

---

## 8. Harness / mock states to verify (VITE_MOCK_IPC=1)

Increment A ships `?forge=multi` (two github.com accounts, one default, distinct logins). Verify:
- `?forge=multi` PR panel: switcher trigger shows (≥2 accounts), caption reflects `accountSource`;
  opening the menu lists both with a checked radio; selecting the other pins it → caption flips to
  `Pinned to this repo`; `Use host default` enabled after an override, clears it back to
  `ownerMatch`/`hostDefault`; `Add another account…` opens connect `add` mode.
- OD-4: a `?forge=multi` variant with **no** host default → switcher header + Settings nudge both
  show; picking a Default radio clears both.
- Settings → Accounts: host groups render; Default radio switches default; per-card Remove confirm
  names the account + fallback line; add-another form pre-locks host/kind; single-account host shows
  `(only account)`; long login/host ellipsise with tooltip.
- Copy: `CONNECT_HINTS.gitLab` / `.bitbucket` equal §5; GitHub unchanged.

USER CHECKPOINT (native only): real keychain multi-account add/remove; real PAT validation for two
distinct logins on one host; per-repo pin persisting across restart; owner-match resolution against a
real remote; removing an account and confirming pinned repos fall back live.

---

## 9. ui-reference.md update (this pass)

One additive edit only: append to **§12.6** (or a new §12.7 "Forge accounts") a short catalogue of
the **`AccountSource` label vocabulary** (the §0.1 table above) and a one-line note that the PR-panel
account switcher reuses the identity-menu `ContextMenu` radio idiom (`checked` + `header` + `detail` +
`busy`), so future forge surfaces use the same strings and pattern. No token table change; no
geometry change.
