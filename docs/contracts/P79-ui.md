# P79 — Forge Account Management — UI Contract

Owner: ui-designer. Input contract: `docs/contracts/P79-forge-account-management.md`.
Scope: three user-approved surfaces — (1) connected-account header in the PR panel,
(2) expiry → reconnect state, (3) global Accounts settings section.

Design stance: **reuse, do not invent.** All three surfaces are built from existing patterns —
`forge-pr.css` (PR panel), `identity-menu.css` `.identity-avatar` (monogram), `SettingsGroup` /
`SettingsRow` / `SettingsEmpty` (settings), `ContextMenu` (overflow), `ConfirmDialog`
(destructive confirm), `.error-banner` (inline errors). **No new design token is introduced**
(see §7); ui-reference.md is unchanged (§8).

All copy strings below are literal. Sentence case, plain language, no raw libgit2 / HTTP text.

---

## 0. Shared: provider display (used by all three surfaces)

A `ForgeKind` must be shown without color as the sole carrier and without new icon assets. Use a
**2-letter provider badge** (text monogram), matching the house "letter-badge" precedent (A/M/D/U
status badges):

| kind          | badge | full label (accessible name / tooltip) |
|---------------|-------|-----------------------------------------|
| `gitHub`      | `GH`  | GitHub          |
| `gitLab`      | `GL`  | GitLab          |
| `bitbucket`   | `BB`  | Bitbucket       |
| `azureDevOps` | `AZ`  | Azure DevOps    |
| `unknown`     | `??`  | Unknown forge   |

New component **`src/components/ForgeProviderBadge.tsx`** (SRP, ~40 lines): renders
`<span className="forge-provider-badge" title={label} aria-label={label}>{badge}</span>`.
Class `.forge-provider-badge`: reuse `.pr-draft-tag` geometry (1px `--border`, radius 999px,
11px, `--text-2`, `--bg-2` bg, padding `1px 7px`, `font-family: var(--font-mono)`, letter-spacing
`0.04em`). No hue — provider is named by the letters, not a color.

### Avatar with monogram fallback

`avatarUrl` may be null. New component **`src/components/ForgeAvatar.tsx`** (SRP, ~45 lines).
Reuse the `.identity-avatar` visual (22px circle, `--bg-3`, 1px `--border`, `--text-1`, `grid;
place-items:center`) under a new class `.forge-avatar` (identical rules; separate class so the two
never entangle):

- `avatarUrl !== null`: `<img className="forge-avatar" src={avatarUrl} alt="" width=22 height=22>`
  with `onError` → fall back to the monogram span (swap once; never loop).
- `avatarUrl === null` (or image failed): `<span className="forge-avatar" aria-hidden="true">{initial}</span>`
  where `initial` = first Unicode code point of `login` uppercased, or `?` when login is null.
- Always `aria-hidden`/`alt=""` — the login text beside it is the accessible name; the avatar is
  decorative (never the sole carrier).

Compact vs cozy: avatar is **20px** in `compact`, **22px** in `cozy` (via a `data-density` hook on
the parent, mirroring existing density selectors). Provider badge unchanged across densities.

---

## 1. Piece 1 — Connected-account header in the PR panel

### 1.1 Placement & geometry

A compact bar rendered **once at the top of `.pr-panel`, above** the `list`/`detail`/`create`
bodies, shown **only when `ctx.viewer !== null`** (connected + warm). It does NOT render for
`connect` / `reauth` / `unsupported` / `loading` / `error` views (there is no identity to show).

```
┌ .pr-panel ─────────────────────────────────────────────┐
│ ┌ .forge-account-header ───────────────────────────┐   │  ← new, flex none
│ │ (o) octocat            github.com   [GH]     [⋯]  │   │
│ └───────────────────────────────────────────────────┘  │
│ ┌ PrList / PrDetailView / PrCreateForm ─────────────┐   │  ← existing body
│ │ …                                                  │   │
│ └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

- `.forge-account-header`: `flex: none; display:flex; align-items:center; gap:8px;
  padding:8px 12px; border-bottom:1px solid var(--border);` — height ~40px cozy, ~34px compact
  (padding `6px 12px` in compact). Matches `.pr-list-header`'s 12px horizontal rhythm exactly.
- Left group (flex 1, min-width 0): `ForgeAvatar` · login (`.forge-account-login`, 13px 600
  `--text-1`, ellipsis) · host (`.forge-account-host`, 11px `--text-3`, ellipsis, `flex:none`).
- Right group (flex none): `ForgeProviderBadge` · a kebab overflow **`⋯`** button.

**Overflow menu over two inline buttons (design decision):** the bar must not compete with the
list/detail below it, and it persists across both views, so a single kebab keeps chrome minimal
(restraint — the app already has ~150 commands). Reuse `ContextMenu`. Kebab button:
`className="btn-icon"`, `aria-label="Account actions"`, `aria-haspopup="menu"`, ≥24px hit target.

Menu items (reuse existing ContextMenu item styling):
1. **"Change token"** → `setConnectMode('change'); setView('connect')`.
2. **"Disconnect"** — `danger`-styled item → opens the ConfirmDialog (§1.3).

### 1.2 New component

**`src/components/ForgeAccountHeader.tsx`** (SRP, presentational, ~90 lines). Props:
```ts
interface ForgeAccountHeaderProps {
  viewer: ForgeViewer;          // login + avatarUrl (non-null; parent gates on ctx.viewer)
  host: string;
  kind: ForgeKind;
  onChangeToken(): void;
  onDisconnect(): void;         // parent opens the confirm; header only requests it
}
```
No IPC, no confirm state inside — the header requests actions; `PrPanel` owns the ConfirmDialog and
`connectMode`. Do NOT inline into `PrPanel.tsx` (SRP; PrPanel is already a large container).

### 1.3 Disconnect confirmation (destructive)

Reuse `ConfirmDialog` (default `danger` variant). Owned by `PrPanel`.

- Title: **`Disconnect from {host}?`**
- Body: `You're signed in as {login}. Disconnecting removes the saved token for {host} from your
  OS keychain. Pull requests and CI status will be unavailable until you reconnect.`
  (render `{login}` and `{host}` in `<span className="mono">`.)
- Confirm label: **`Disconnect`** (danger). Cancel: **`Cancel`** (focus lands on Cancel — the
  ConfirmDialog default; a stray Enter never disconnects).
- On confirm: `busy=true`, `ipc.forgeClearToken(repoId)` → on success `setView('connect');
  setConnectMode('connect'); setBootstrapTick(t=>t+1)` (header disappears, ForgeConnect returns).
  On error: keep the dialog, surface via the existing toast (`Could not disconnect: {msg}`).
- Not an "are you sure?" — it names the exact host + login and the consequence, and the token is
  re-addable (the wording says so), so no separate undo affordance is required.

### 1.4 Change-token flow

Non-destructive (backend overwrites only after the new token validates; a rejected token leaves the
old one in place). No confirm. `ForgeConnect` renders in `change` mode (§2). A Cancel in
`ForgeConnect` returns to the prior view — see §2.4.

### 1.5 States

| State | Header |
|-------|--------|
| connected + warm (`ctx.viewer !== null`) | header shown, avatar/login/host/badge/kebab |
| cold token (`viewer===null, authenticated===true`) | header hidden; panel goes to `list` optimistically (a later authFailed trips §3) |
| avatar 404 / offline image | monogram fallback (§0), no layout shift |
| very long login/host | each ellipsis-truncates with a `title` tooltip; badge + kebab never clipped |
| disconnect in flight | confirm's `Disconnect` shows `busy` (disabled); kebab menu closed |

### 1.6 A11y & keyboard

- Kebab: `aria-haspopup="menu"`, `aria-expanded`, opens ContextMenu (existing roving-focus +
  Esc-to-close + focus-restore behaviour). "Disconnect" item carries no color-only meaning — it is
  worded "Disconnect" and styled danger.
- Focus after disconnect confirm closes: returns to the token field in `ForgeConnect` (natural,
  since the view switches to `connect`).
- Header is not a heading element (it sits inside the scrollable panel); login is plain text.

---

## 2. Piece 2 — expiry → reconnect (ForgeConnect modes)

`ForgeConnect` gains **`mode: ConnectMode`** (`'connect' | 'change' | 'reauth'`). Only the
heading, sub-line, an optional banner, and the submit label change; the token field, the
per-provider scopes hint (P78/P64c — **must still render in every mode**), the keychain note, the
error banner, and the submit handler are unchanged.

### 2.1 Per-mode copy

| element | `connect` (first-time) | `change` | `reauth` (expired/revoked) |
|---------|------------------------|----------|-----------------------------|
| banner | — (none) | — (none) | **warning banner**, see §2.2 |
| heading | `Connect to {host}` | `Replace token for {login}` | `Reconnect to {host}` |
| sub-line | `Paste a personal access token to view and open pull requests for {owner}/{repo}.` | `Paste a new token to replace the one saved for {host}. The current token keeps working until the new one is validated.` | `Paste a new token to reconnect. Your access to {owner}/{repo} is paused until then.` |
| submit (idle) | `Connect` | `Replace token` | `Reconnect` |
| submit (busy) | `Connecting…` | `Replacing…` | `Reconnecting…` |

`{login}` falls back to `{host}` when the login is unknown. Scopes hint + "Create a token" link +
keychain note render identically in all three modes.

### 2.2 Re-auth warning banner (non-alarming)

Rendered above the heading **only in `reauth` mode**. New class `.forge-reauth-banner`:
```
border: 1px solid var(--warning);
background: color-mix(in srgb, var(--warning) 12%, transparent);
color: var(--text-1);
border-radius: 6px; padding: 8px 10px; font-size: 12px; line-height: 1.45;
display:flex; gap:8px; align-items:flex-start;
```
Contrast: body text is `--text-1` on the panel background (AA ≥4.5:1 in both themes — the tint is
only 12% so effective bg ≈ `--bg-1`); the `--warning` border on `--bg-1` is ≥3:1 in both themes
(matches the documented `.identity-avatar` unset ring). Meaning is carried by the **words**, not
the hue.

- `role="status"` (polite; not `alert` — it is expected, not an emergency), `aria-live="polite"`.
- Copy (literal): **`Your saved token for {host} expired or was revoked. Reconnect to keep viewing
  pull requests — your token stays saved until you replace it.`**
  Render `{host}` in `<span className="mono">`. Deliberately reassuring: it states the token is
  kept and exactly what to do next. No error red, no scary icon.
- Optional leading glyph: a small `⚠` in `--warning` (`aria-hidden`), redundant with the text.

### 2.3 Prop wiring

```ts
// ForgeConnect gains:
mode: ConnectMode;
login?: string | null;   // for the change/reauth copy; falls back to host
```
Heading/sub/submit/banner selected from a small `Record<ConnectMode, …>` map inside `ForgeConnect`
(mirrors the existing `CONNECT_HINTS` record so a new mode is a compile error until copy is
supplied). `onSubmit` unchanged — the parent passes the correct handler per mode.

### 2.4 PrPanel behaviour (states)

| trigger | result |
|---------|--------|
| any forge read/write rejects `authFailed` | `ipc.forgeInvalidateViewer(ctx.host)` (await), `setConnectMode('reauth')`, `setView('connect')`, `setBootstrapTick` (refetch context → `viewer:null`) |
| `reauth` submit → valid token | `forgeSetToken(repoId, token)` → `setConnectMode('connect')`, refetch, back to `list`, header re-appears |
| `reauth` submit → bad token | stay in `reauth`, inline `.error-banner` shows the message; banner still visible |
| `change` submit → valid | overwrite, back to `list` |
| `change` submit → bad | stay in `change`, inline error; old account still connected |
| Cancel in `change`/`reauth` | `change` → back to `list`; `reauth` → stay on `connect` (there is nothing to go back to — the panel is not authenticated-warm). Provide a **Cancel** button only in `change` mode; `reauth` has no Cancel (reconnect is the only forward path, matching first-connect which also has none). |
| `commit_statuses` authFailed (OD-3) | trips the same reauth path, but **suppress the extra toast** to avoid double-notifying (the reauth screen is the notification). |

Toast on entering reauth: none — the banner is the message (avoid a toast + banner double-hit).

---

## 3. Piece 3 — global Accounts settings section

### 3.1 Rail entry (catalog)

- Add `'accounts'` to `SettingsCategoryId`, placed **after `identities`** in `SETTINGS_CATEGORIES`.
- `SettingsCategory` entry:
  - `label`: **`Accounts`**
  - `subtitle`: **`Forge sign-ins used for pull requests and CI status.`**
  - `dividerBefore`: **false** (it groups naturally with Identities above it — both are "who you
    are" settings; the next hairline stays before `git-config`).
  - no `pill`.
- **Rail is text-only** — `SettingsRail` renders labels, not icons (confirmed in
  `SettingsRail.tsx`). So no icon is added; the "label + icon" request resolves to **label only**,
  consistent with every other rail entry. (If icons are ever added to the rail, that is a separate,
  app-wide change — do not special-case Accounts.)
- `CATEGORY_PAGES.accounts = { Page: AccountsCategory }` (new thin category wrapper, mirroring
  `IdentitiesCategory.tsx`).
- Catalog rows (per architect §6.1): a `group` row `accounts.list` (aggregate, `role="group"`,
  no `[data-setting-control]`) + an unconditional `button` row `accounts.add`
  (label **`Add a token for a host`**, keywords `forge github gitlab bitbucket sign in connect
  token account`).

### 3.2 Component tree

- **`src/components/settings/categories/AccountsCategory.tsx`** (~15 lines): reads nothing from
  context it doesn't need; renders `<SettingsAccountsSection />`.
- **`src/components/settings/SettingsAccountsSection.tsx`** (container, ~180 lines): owns
  `forgeListAccounts` fetch, list state, per-card change/disconnect, add-form open state, and the
  ConfirmDialog. Composes small children below.
- **`src/components/settings/SettingsAccountCard.tsx`** (~120 lines): one account row +
  inline change-token form.
- **`src/components/settings/SettingsAccountAddForm.tsx`** (~130 lines): the add-a-host form.

Split from the start (SRP / ~500-line rule); do not grow `SettingsShell`/`SettingsPanel`.

### 3.3 List layout

Reuse `SettingsGroup` as the container (group title **`Connected accounts`**). Each account is a
card reusing `.settings-profile` chrome (border, radius, padding) under `.settings-account` (same
rules; separate class):

```
┌ .settings-account (role=group, aria-labelledby) ──────────────┐
│ (o) octocat                    [GH]  ● Connected   [⋯ or btns] │  ← head row
│     github.com                                                 │  ← host sub-line
│  … inline "Change token" form appears here when armed …        │
└───────────────────────────────────────────────────────────────┘
```

- Head row: `ForgeAvatar` · login (`--text-1` 13px 600; when `login===null` show host as the
  title and omit the sub-line) · `ForgeProviderBadge` · **connected state chip** · actions.
- Host sub-line: `--text-3` 12px, mono, ellipsis + `title`.
- **Connected state chip** (color not sole carrier — dot + word):
  - `connected===true`: `.settings-account-state.is-connected` — `● Connected` (dot uses
    `--success`, text `--text-2`).
  - `connected===false`: `.settings-account-state.is-disconnected` — `○ Token missing` (dot uses
    `--text-3`), plus a help line: `The token for this host is no longer in your keychain. Add a
    new one or remove this entry.` This is the out-of-band-vanished case from architect §2.2.
- Actions: same **kebab overflow** as the PR header (consistency) — items **`Change token`** and
  **`Remove account`** (danger). On a narrow settings pane the kebab avoids two buttons wrapping.
  (When `connected===false`, the change item reads **`Add token`** and Remove stays.)

Density: card padding follows the existing `.settings-profile` values; avatar 20/22px per §0.

### 3.4 Per-card change token (inline)

Selecting `Change token` (or `Add token`) reveals an inline form **inside the card** (not a route
change — settings has no view stack). Reuse `.settings-config-field` inputs:
- One masked token field (`type=password`, autoComplete off), label **`Personal access token`**.
- The per-provider scopes hint for this card's `kind` (reuse the `CONNECT_HINTS` text — export it
  from `ForgeConnect` or a shared module so it is not duplicated) + "Create a token" link
  (IPC-routed via `openUrl`, same as `ForgeConnect`).
- Keychain note (reuse the `ForgeConnect` note copy).
- Buttons: **`Replace token`** (primary) / **`Cancel`** (secondary). Busy → `Replacing…`.
- Submit: `ipc.forgeSetTokenForHost(host, kind, token)` → on success collapse form + refetch list;
  on error inline `.error-banner`, keep form open, old token intact.

### 3.5 Add-a-token-for-a-host form (`SettingsAccountAddForm.tsx`)

Opened by the `accounts.add` button row **`Add a token for a host`** (below the list). A bordered
card (`.settings-account-add`, reuse `.settings-profile` chrome) with:

1. **Provider kind** — a segmented/radio group (reuse `SettingsSegmented` or a `<select>` styled as
   `.settings-config-field`): `GitHub` · `GitLab` · `Bitbucket` · `Azure DevOps`.
   - **Azure DevOps is disabled** (OD-2): the option is present but `disabled`, greyed, with an
     inline hint under the group: **`Azure DevOps accounts must be added from an open Azure DevOps
     repository — its sign-in needs a repository to verify against.`** Do not silently omit it (a
     hidden option looks like a missing feature); disable + explain. If Azure is somehow selected,
     the submit button is disabled with the same hint as a tooltip.
2. **Host** — text field (`.settings-config-field`), label **`Host`**, placeholder per kind
   (`github.com`, `gitlab.com`, `bitbucket.org`). Lowercase on submit (backend keys lowercased).
3. **Personal access token** — masked field + the selected kind's scopes hint + "Create a token"
   link + keychain note (same as §3.4).
4. Buttons: **`Add account`** (primary, disabled until host and token are non-empty and kind ≠
   azureDevOps) / **`Cancel`**. Busy → `Adding…`.
- Submit: `ipc.forgeSetTokenForHost(host, kind, token)` → success: collapse form, refetch list,
  toast `Connected to {host} as {login}.`; error inline `.error-banner` (`authFailed` →
  `That token was rejected. Check it has the required scopes and hasn't expired.`;
  `forgeUnsupported` → the Azure hint; `forgeRateLimited` → `The forge is rate-limiting requests —
  try again in a few minutes.`; `networkError` → `Couldn't reach {host}. Check your connection.`).

### 3.6 Remove-account confirm (destructive)

Reuse `ConfirmDialog` (danger), owned by `SettingsAccountsSection`:
- Title: **`Remove {host}?`**
- Body: `This deletes the saved token for {host}{login ? " (" + login + ")" : ""} from your OS
  keychain. Any repository on {host} will need a new token to view pull requests.` (`{host}` in
  `.mono`.)
- Confirm: **`Remove`** (danger). On confirm → `ipc.forgeClearTokenForHost(host)` → refetch list.
- Cannot be undone by Bonsai (the token itself is gone from the keychain) — wording states the
  consequence; re-adding requires a fresh token, which the copy implies.

### 3.7 States

| State | Rendering |
|-------|-----------|
| loading | `SkeletonRows` (reuse from `CommitPanel`) inside the group, or a single `.pane-empty` "Loading accounts…"; prefer skeleton for consistency |
| empty (no accounts) | `SettingsEmpty` (existing) — see §3.8 |
| populated | one `SettingsAccountCard` per `ForgeAccount` |
| list fetch error | inline `.error-banner` with a `Retry` button (mirror PrPanel's error view): `Couldn't load your accounts. {msg}` |
| connected:false entry | card shown with `Token missing` chip + help line (§3.3) so the user can remove/re-add |
| add form open | add card expanded below the list; the `Add` button row hidden or toggled |
| long host/login | ellipsis + `title` in card head and sub-line |

### 3.8 Empty state

`SettingsEmpty` with:
- Title: **`No accounts connected`**
- Body: **`Connect a forge account to view and open pull requests and see CI status. You can also
  connect from a repository's Pull requests tab.`**
- The **`Add a token for a host`** button remains visible below (it is unconditional).

### 3.9 A11y & keyboard

- Each card is `role="group" aria-labelledby={login-or-host title id}` (mirrors
  `IdentityProfileCard`).
- Kebab menus: `aria-haspopup="menu"`, ContextMenu roving focus, Esc close, focus restore to the
  kebab.
- State chip: the dot is `aria-hidden`; the word (`Connected` / `Token missing`) is the accessible
  carrier (never color alone).
- Provider kind group: `role="radiogroup"` with an accessible group label `Provider`; the disabled
  Azure option is `aria-disabled` with the hint referenced via `aria-describedby`.
- Token fields: `type=password`, `autoComplete="off"`, never prefilled/echoed (matches
  `ForgeConnect`); the value never leaves the controlled input.
- Focus: opening an inline change form focuses its token field; closing returns focus to the kebab.
  ConfirmDialog handles its own focus (Cancel) + Esc + restore.
- All hit targets ≥24px; focus rings are the app default (2px `--accent`, 1px offset,
  `:focus-visible`).

---

## 4. Motion

- Inline change/add form reveal: none required; if desired, a ≤120ms `opacity`+`transform:
  translateY(-2px)` ease-out, gated by `prefers-reduced-motion: reduce` → no transform. No motion
  on the PR header or graph.

---

## 5. Both themes

Every surface uses only `--bg-1/2/3`, `--border`, `--text-1/2/3`, `--accent`, `--success`,
`--warning`, `--danger` — all defined for `:root` (dark) and `[data-theme='light']`. No literal
hex. Contrast checked:
- Reauth banner text `--text-1` on `--bg-1` (12% warning tint): AA ≥4.5:1 both themes.
- `--warning` border edge on `--bg-1`: ≥3:1 both themes (reuses the documented `.identity-avatar`
  unset-ring pairing).
- Provider badge `--text-2` on `--bg-2`: AA ≥4.5:1 both themes (same as `.pr-draft-tag`, already
  shipped).
- Connected dot `--success` / disconnected `--text-3`: decorative (word carries meaning), but both
  clear ≥3:1 as graphics.

## 6. Densities

| element | cozy | compact |
|---------|------|---------|
| `.forge-account-header` padding | `8px 12px` | `6px 12px` |
| `ForgeAvatar` | 22px | 20px |
| `.settings-account` card padding | inherits `.settings-profile` | inherits `.settings-profile` |

Font sizes unchanged across densities (matches the rest of the app; density drives padding/heights,
not type).

---

## 7. New CSS classes (no new tokens)

All in existing stylesheets — put PR-panel classes in `src/styles/forge-pr.css`, settings classes
alongside the `.settings-profile` block (wherever it currently lives). New classes:
`.forge-account-header`, `.forge-account-login`, `.forge-account-host`, `.forge-avatar`,
`.forge-provider-badge`, `.forge-reauth-banner`, `.settings-account`, `.settings-account-state`
(`.is-connected` / `.is-disconnected`), `.settings-account-add`. Each is defined purely with
existing tokens (see §5). **No `:root` / `[data-theme='light']` token additions.**

---

## 8. ui-reference.md

**No change required.** No new token is introduced; the monogram/avatar pattern is already
documented (`.identity-avatar`, ui-reference §12.6) and the new `.forge-avatar` /
`.forge-provider-badge` are variations built from existing tokens. If the reviewer wants the forge
account/avatar pattern catalogued for reuse, that is a follow-up NIT, not a blocker.

---

## 9. Harness / mock states to verify (VITE_MOCK_IPC=1)

Per architect §7:
- `?forge=auth` → PR panel shows `ForgeAccountHeader` (avatar + login + host + badge + kebab);
  Change/Disconnect exercised; disconnect confirm returns to `ForgeConnect`.
- `?forge=expired` → first `forgeListPrs` throws `authFailed` once → reauth banner + reconnect copy;
  a good token returns to list, a `bad` token stays in reauth with inline error.
- Settings → Accounts: list renders from `forgeListAccounts`; add form (host + kind + token) adds a
  card; `azureDevOps` disabled with hint; a `bad` token → inline authFailed error; change/remove
  per card; empty state when accounts cleared; `connected:false` entry shows Token-missing chip.
- Long-content case: seed a fixture account with a long host/login to verify ellipsis + tooltip.

USER CHECKPOINT (native only): real keychain add/remove, real PAT validation against
github.com/gitlab.com/bitbucket.org, add-for-host with no repo open, cross-restart persistence of
`forge_hosts`, backfill on opening a pre-P79 repo.
