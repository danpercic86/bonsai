# P69 — Settings redesign (UI contract)

Owner: `ui-designer`. Implementer: `senior-dev`. Status: contract.
Prerequisite reading: `docs/contracts/ui-reference.md` §2 (tokens/contrast), §3 (spacing/density),
§8 (states), §11 (pills). This contract adds `ui-reference.md` §12 "Settings surface" in the same
pass — that section is the durable extract; this file is the increment spec.

**Scope.** Restructure the Settings overlay from one 560px scrolling column of eleven flat sections
into a two-pane categorised modal; move identity switching into the header; standardise every
control; add per-row help and reset; close the effective-identity gap. No backend change, no new
IPC command, **no new design token**.

---

## 0. Decisions at a glance

| # | Decision | Why |
|---|---|---|
| D1 | Two-pane modal, 880 × min(660, 100vh−64) | Locked by the user. Rail is the only affordance that makes 11 sections findable. |
| D2 | 7 categories, single flat rail with two hairline dividers | Group *headings* inside a `role="tablist"` are an ARIA hazard; dividers + a `repo` pill carry the same meaning with no role juggling. |
| D3 | Search = **cross-category row results**, not rail filtering | The failure mode is "I know the setting, not the category". Filtering the rail still makes the user scan. |
| D4 | Switch = CSS over native `<input type="checkbox">`; segmented = CSS over native `<input type="radio">` | Keeps every `getByRole('checkbox'/'radio', {name})` query green and gets keyboard/AT for free. **No `role="switch"`, no `role="tablist"` on segments.** |
| D5 | Identity **switching** in the header; identity **CRUD** stays in Settings → Identities | The daily action gets a permanent home; the rare admin action does not earn a second modal type. |
| D6 | Header trigger reads the **effective** identity (local ?? global) and names its source | Today's local-only match shows nothing in the default harness state, which is also the common real state. |
| D7 | Applying a profile confirms **only** when it would overwrite a differing *local* identity | Writing into an empty slot destroys nothing; overwriting is a silent config change the user cannot see. |
| D8 | Per-row reset only. No "Reset all settings" in P69 | Per-row covers the real need; a global reset needs a confirm + a defaults contract that would grow `settings.rs` (at its ratchet ceiling). |
| D9 | `.header-toolbar` is extracted to `HeaderToolbar.tsx` | `App.tsx` is at 1114 and may not grow; extraction removes ~50 lines and adds ~1, so App.tsx **shrinks**. |
| D10 | Settings keeps **one geometry in both densities** | `ui-reference.md` §3 scopes `panelDensity` to the right panel + AI dock. Unchanged. |

---

## 1. Category taxonomy

### 1.1 The rail

```
General
Appearance
Commit graph
AI
Identities
────────────────────
Git config      [repo]
────────────────────
About
```

| Order | Id | Label | What belongs here |
|---|---|---|---|
| 1 | `general` | General | App-wide behaviour that is neither visual nor AI: background activity, external tools. |
| 2 | `appearance` | Appearance | How Bonsai's chrome looks. Nothing that changes what the canvas *contains*. |
| 3 | `graph` | Commit graph | Anything drawn on the history canvas — geometry, per-row detail, badges. |
| 4 | `ai` | AI | Every AI and MCP setting, in three sub-groups. |
| 5 | `identities` | Identities | The saved name/email/signing-key list. Editing only — switching lives in the header. |
| 6 | `git-config` | Git config `repo` | Raw Git configuration for the **open repository** (or your global file, via the scope switch). |
| 7 | `about` | About | Version, updates, the welcome tour. |

**Scope disambiguation** (deliverable 1). Two mechanisms, both non-colour:

1. The rail item `Git config` carries a **hueless status pill** (`ui-reference.md` §11) reading
   `repo` — 11px, `--text-2` over its own 12% tint, `border: 1px solid transparent`. No other rail
   item has a pill, and it is separated by hairline dividers above and below.
2. The Git config pane header carries a **scope switch** (segmented: `This repository` | `Global`)
   plus a scope line naming the actual file, so the answer is a fact and not an inference:
   - `This repository` → `Editing .git/config in {repoFolderName}` (12px `--text-2`, path in
     `title`).
   - `Global` → `Editing your global Git config (~/.gitconfig)`.

Every other category is global and says so once, in its pane subtitle.

### 1.2 No repo open, in `git-config`

The current bare sentence is replaced by an in-pane empty block (`SettingsEmpty.tsx`, §9). It is
**not** `EmptyState.tsx` — that component hard-codes the Bonsai hero mark, tagline, recents list and
three CTAs and is the app-level no-repo screen; embedding it in a 640px pane would be a hero section
inside a settings dialog. `SettingsEmpty` follows `ui-reference.md` §8's centred-pane idiom:

```
            No repository open

  Git config is stored per repository. Open one
  to view and edit it.

            [ Open repository… ]
```

- Container `padding: 24px 16px`, centred, `max-width: 40ch`.
- Title 13px/600 `--text-1`; body 12px `--text-2` (**not** `--text-3`); `gap: 8px`.
- One `btn-secondary` action, `Open repository…`, wired to App's existing `handleOpenRepository`.
  This needs a **new prop** `onOpenRepository(): void` on `SettingsPanel` — flagged in §11.

### 1.3 Coverage table — every control from the eleven current sections, mapped exactly once

| # | Today (section → control) | New home | Control after |
|---|---|---|---|
| 1 | Getting started → `Show welcome tour` button | About → Help | Button `Show tour` (row label `Welcome tour`) |
| 2 | Updates → `Current version` (read-only) | About → Version | Read-only value |
| 3 | Updates → `Check for updates` button + result line | About → Version | Button + inline result line (unchanged behaviour) |
| 4 | Updates → checkbox `Automatically check for updates on launch` | About → Version | **Switch** |
| 5 | Background jobs → section desc | General → Background activity | Split into per-row help (§8) |
| 6 | Background jobs → checkbox `Enable auto-fetch` | General → Background activity | **Switch**, relabelled `Auto-fetch from remotes` |
| 7 | Background jobs → slider `Interval` (fetch) | General → Background activity | `NumberSlider`, relabelled **`Fetch every`** |
| 8 | Background jobs → checkbox `Refresh status & health periodically` | General → Background activity | **Switch**, relabelled `Refresh status automatically` |
| 9 | Background jobs → slider `Interval` (health) | General → Background activity | `NumberSlider`, relabelled **`Refresh every`** |
| 10 | Graph → slider `Commit node size` | Commit graph → Geometry | `NumberSlider` + help |
| 11 | Graph → slider `Row height` (`#settings-graph-row`) | Commit graph → Geometry | `NumberSlider` — **id + label frozen** |
| 12 | Graph → slider `Lane width` | Commit graph → Geometry | `NumberSlider` + help |
| 13 | Graph → checkbox `Short SHA` | Commit graph → Row details | **Switch**, label frozen |
| 14 | Graph → checkbox `Author name` | Commit graph → Row details | **Switch**, label frozen |
| 15 | Graph → checkbox `Date` | Commit graph → Row details | **Switch**, label frozen |
| 16 | Graph → radio fieldset `Date basis` (Author/Committer) | Commit graph → Row details | **Segmented** over the same native radios |
| 17 | Graph → checkbox `Ahead/behind on branches` | Commit graph → Badges | **Switch**, label frozen |
| 18 | Graph → checkbox `Signature badge` | Commit graph → Badges | **Switch**, label frozen |
| 19 | Graph → checkbox `Compact rows` | Commit graph → Geometry | **Switch**, label frozen |
| 20 | Graph → Forge signals → `Show PR badges` | Commit graph → Badges | **Switch**, label frozen |
| 21 | Graph → Forge signals → `Show CI status` | Commit graph → Badges | **Switch**, label frozen |
| 22 | Appearance → button `Theme` | Appearance | **Segmented** `Dark` \| `Light` |
| 23 | Appearance → button `File lists` | Appearance | **Segmented** `Tree` \| `Flat` |
| 24 | Appearance → button `Panel density` | Appearance | **Segmented** `Cozy` \| `Compact` |
| 25 | Appearance → cross-reference note (`--text-3`) | Appearance | Row help on Panel density, **`--text-2`** |
| 26 | Git config → Hooks → `Run git hooks for this repository` | Git config → Hooks | **Switch** |
| 27 | Git config → row `Level` (Local \| Global) | Git config → **pane header scope switch** | Segmented, promoted out of the body |
| 28 | Git config → Identity → `user.name` | Git config → Identity | Text field (stacked row) |
| 29 | Git config → Identity → `user.email` | Git config → Identity | Text field (stacked row) |
| 30 | Git config → Advanced → Behaviour (curated enum/text keys) | Git config → Advanced (`<details>`) | Unchanged control kinds |
| 31 | Git config → Advanced → Custom keys (+ Remove / Add entry) | Git config → Advanced (`<details>`) | Unchanged control kinds |
| 32 | External tools → `Terminal command` + `Reset to auto-detect` | General → External tools | Text field (stacked) + **row `↺` reset** |
| 33 | External tools → `Editor command` + `Reset to auto-detect` | General → External tools | Text field (stacked) + **row `↺` reset** |
| 34 | Identity profiles → per-profile `Label` / `user.name` / `user.email` / `signing key` | Identities | Text fields in `IdentityProfileCard` |
| 35 | Identity profiles → `Apply to current repo` | Identities **and** header menu | Button `Use in this repository` / menu row |
| 36 | Identity profiles → `Delete` | Identities | Button + **inline two-step confirm** (§4.6) |
| 37 | Identity profiles → `Add profile` | Identities | Button `Add identity` |
| 38 | Identity profiles → `Active on this repo` badge | Identities **and** header menu | Hueless pill `in use`, now **effective**-based |
| 39 | Identity profiles → empty state | Identities | `SettingsEmpty` variant (§8) |
| 40 | Identity profiles → email warn + apply error + "Applied" flash | Identities | Unchanged mechanics, reworded (§8) |
| 41 | AI assistance → checkbox `Enable AI features` | AI → Assistance | **Switch** (consent flow unchanged) |
| 42 | AI assistance → radio fieldset `Conflict resolution` (2 options + hints) | AI → Assistance | **Stays a radio group** (each option needs a sentence) |
| 43 | AI assistance → CLI availability status line | AI → Assistance | Unchanged (`.settings-ai-status`) |
| 44 | AI runs → `Repository access` self-labelling button | AI → Runs | **Segmented** `Read-only` \| `No file access` |
| 45 | AI runs → checkbox `Stream AI output` | AI → Runs | **Switch** |
| 46 | AI runs → checkbox `Stream partial replies` | AI → Runs | **Switch** |
| 47 | AI runs → Limits → `Stop a run that goes quiet` + `After` (secs) | AI → Runs → Limits | **Switch** + `NumberSlider` |
| 48 | AI runs → Limits → `Stop a run after a fixed time` + `Limit` (secs) | AI → Runs → Limits | **Switch** + `NumberSlider` |
| 49 | AI runs → Limits → `Replies per run` (turns) | AI → Runs → Limits | `NumberSlider` |
| 50 | AI runs → Limits → `Set a spend limit per run` + `Limit` (USD) | AI → Runs → Limits | **Switch** + `NumberSlider` |
| 51 | AI runs → Bulk resolve → `Batch size` (KB) | AI → Runs → Bulk resolve | `NumberSlider` |
| 52 | AI runs → whole-fieldset disabled hint | AI → Runs | Moves to the **top** of the group (§5.4) |
| 53 | MCP → checkbox `Enable MCP server` | AI → AI access | **Switch** (consent flow + copy frozen) |
| 54 | MCP → checkbox `Allow AI to modify repositories` | AI → AI access | **Switch** (consent flow + copy frozen) |
| 55 | MCP → status line | AI → AI access | Unchanged |
| 56 | MCP → read-only `Server URL` + Copy | AI → AI access | Stacked row, unchanged |
| 57 | MCP → read-only `Bearer token` + Copy | AI → AI access | Stacked row, unchanged |
| 58 | MCP → `Register with Claude Code · Globally` (Add + Copy) | AI → AI access | Unchanged |
| 59 | MCP → `Register with Claude Code · This repository` (Add + Copy) | AI → AI access | Unchanged |

**58 of 59 controls keep their behaviour.** The one behavioural change is #38 (local-only → effective
identity, §4). Nothing is dropped.

---

## 2. Two-pane shell

### 2.1 Wireframe

```
 dialog-overlay (existing scrim, backdrop-mousedown closes)
 ┌────────────────────────────────────────────────────────────────────────────────┐
 │  Settings                                                                  ✕   │ 48px
 ├──────────────────────┬─────────────────────────────────────────────────────────┤
 │  General             │ ┌─────────────────────────────────────────────────────┐ │ 56px
 │  Appearance          │ │ 🔍  Search settings                                 │ │
 │  Commit graph        │ └─────────────────────────────────────────────────────┘ │
 │  AI                  ├─────────────────────────────────────────────────────────┤
 │  Identities          │                                                         │
 │ ──────────────────── │  Commit graph                                           │
 │ ▌Git config   [repo] │  How the history canvas is drawn. Applies to every repo. │
 │ ──────────────────── │                                                         │
 │  About               │  GEOMETRY                                               │
 │                      │  ┌──────────────────────────────────────────────────┐   │
 │                      │  │ Commit node size            [──●───]  [  4 ]  ↺  │   │
 │                      │  │ Radius of the dot on each commit.                │   │
 │                      │  ├──────────────────────────────────────────────────┤   │
 │                      │  │ Row height                  [───●──]  [ 28 ]     │   │
 │                      │  │ Vertical space per commit row.                   │   │
 │                      │  └──────────────────────────────────────────────────┘   │
 │                      │                                                         │
 │                      │  ROW DETAILS                                            │
 │                      │  ┌──────────────────────────────────────────────────┐   │
 │                      │  │ Short SHA                                 ( ●━)  │   │
 │                      │  │ Show the abbreviated commit id in each row.      │   │
 │                      │  ├──────────────────────────────────────────────────┤   │
 │                      │  │ Date basis            [ Author | Committer ]      │  │
 │                      │  └──────────────────────────────────────────────────┘   │
 │  200px               │                                        ~680px           │
 └──────────────────────┴─────────────────────────────────────────────────────────┘
                                    880px
```

### 2.2 Geometry (all values from the 4/8/12/16/24 scale)

| Element | Spec |
|---|---|
| Card | `width: 880px; max-width: calc(100vw - 48px); height: min(660px, calc(100vh - 64px)); display: grid; grid-template-columns: 200px 1fr; grid-template-rows: 48px 1fr; overflow: hidden;` background `--bg-1`, 1px `--border`, radius 6 |
| Header (row 1, spans 2 cols) | `height: 48px; padding: 0 8px 0 16px; display:flex; align-items:center; border-bottom: 1px solid var(--border);` title 15px/600 `--text-1`, `margin-right:auto`; close = existing `.btn-icon`, 32×32 |
| Rail (row 2, col 1) | `padding: 8px; overflow-y: auto; border-right: 1px solid var(--border);` background `--bg-1` |
| Rail item | `height: 32px; padding: 0 8px; gap: 8px; border-radius: 6px; font-size: 13px; color: var(--text-2);` items separated by `2px` gap |
| Rail divider | `height:1px; background: var(--border); margin: 8px 8px;` `aria-hidden` |
| Content col (row 2, col 2) | `display:grid; grid-template-rows: 56px 1fr; min-width:0;` background `--bg-0` |
| Search bar | `padding: 12px 16px; border-bottom: 1px solid var(--border);` input full-width, 32px tall |
| Pane | `padding: 16px 24px 24px; overflow-y: auto; scroll-behavior: auto;` |
| Pane header | title 15px/600 `--text-1`; subtitle 12px `--text-2`, `margin-top: 4px`; block `margin-bottom: 16px` |
| Group | `margin-bottom: 24px`; group title 11px uppercase, `letter-spacing:.08em`, `--text-3` (decorative — duplicates visible structure), `margin: 0 0 8px 0` |
| Row | see §5.1 |

**Overflow.** The card never scrolls as a whole. The rail and the pane scroll independently;
the header and search bar are fixed. At `height < 560px` the card takes `calc(100vh - 32px)` and
the pane's own scrollbar carries everything. At `width < 928px` the card takes
`calc(100vw - 48px)`; below **720px** the rail collapses to a horizontally-scrolling 40px strip
above the search bar (`grid-template-columns: 1fr; grid-template-rows: 48px 40px 56px 1fr`) —
same roles, same tab order. This is a Tauri desktop app, so the narrow case is an insurance policy,
not a target.

**Scroll reset.** Changing category sets `pane.scrollTop = 0` and moves focus to the pane
(`tabindex="-1"`) only when the change came from a keyboard activation in the rail; a mouse click
leaves focus on the rail item.

---

## 3. Search

### 3.1 Model — cross-category results (D3)

While the query is non-empty the pane replaces the selected category's content with a **results
list**: every matching row from **every** category, rendered with its real control (live and
editable in place), grouped under its category name.

Rejected alternatives:
- *Filter the rail* — the user still has to click through categories to find the row; it answers
  "which category" but not "where is the setting".
- *Filter rows in the current page only* — fails the primary use case outright ("where is the AI
  spend limit?" requires already knowing it is under AI).
- *A result list that jumps and highlights* — two interactions (search, then jump) and a highlight
  that has to decay; editing in place is one interaction and no state to unwind.

### 3.2 Behaviour

- Debounce: none. Matching is a synchronous pass over a ~60-entry static index.
- Matching: case-insensitive substring over `label` + `help` + `keywords`, on the whitespace-split
  query with **all** terms required (AND). Not fuzzy — fuzzy over 60 rows produces confident
  nonsense.
- Result group header: category name, 11px uppercase `--text-3`, plus a right-aligned
  `Go to {Category}` text button (12px `--text-2`, `--text-1` on hover) that clears the query and
  selects that category.
- Highlight: the matched substrings in the **label** are wrapped in
  `<mark class="settings-match">` — `background: var(--selection); color: var(--text-1);
  border-radius: 2px; padding: 0 1px;`. Contrast `--text-1` on `--selection`: **9.4:1** dark /
  **13.3:1** light. Help text is **not** highlighted (too noisy at 12px).
- Rail while searching: each item shows a right-aligned match count (11px `--text-2`); zero-count
  items get `opacity: .5` but stay clickable — clicking any rail item **clears the query** and
  selects it. `aria-selected` still tracks the underlying selection.
- Live region: a visually-hidden `role="status" aria-live="polite"` announces
  `{n} settings match` / `No settings match` on each settled query.
- Esc: reuse `ListFilterInput`'s capture-phase Esc — first Esc clears a non-empty query, second Esc
  closes the dialog. No new mechanism.

### 3.3 Zero match

```
        No settings match “xyz”.

  Try a shorter word — for example graph, fetch,
  identity, or spend.

              [ Clear search ]
```
Title 13px `--text-1`; body 12px `--text-2`; `btn-secondary`. Same `SettingsEmpty` component as
§1.2, different props.

### 3.4 The index

`src/components/settings/settingsIndex.ts` — pure data, its own file (the fixture-table rule):

```
{ id, category, label, help, keywords }
```

`keywords` are never displayed. Supply them for every row whose label is not the word the user
would type. Minimum required set:

| Row | keywords |
|---|---|
| Auto-fetch from remotes | `background poll origin sync automatic` |
| Refresh status automatically | `health rescan watcher poll periodic` |
| Commit node size | `dot radius avatar graph` |
| Row height | `density spacing graph rows` |
| Lane width | `column spacing branches graph` |
| Short SHA | `hash oid abbreviated id` |
| Date basis | `author committer timestamp` |
| Ahead/behind on branches | `divergence upstream counts` |
| Signature badge | `gpg ssh signed verified` |
| Compact rows | `density graph small` |
| Show PR badges | `pull request forge github azure` |
| Show CI status | `checks pipeline build forge` |
| Theme | `dark light appearance colour color` |
| File lists | `tree flat folders nesting` |
| Panel density | `cozy compact spacing right panel` |
| Terminal command | `shell console external open in` |
| Editor command | `ide vscode external open in` |
| Enable AI features | `claude assistant consent` |
| Conflict resolution | `merge resolve autonomy propose` |
| Repository access | `sandbox files read-only permissions` |
| Stop a run that goes quiet | `idle timeout stall hang` |
| Stop a run after a fixed time | `wall clock timeout duration` |
| Replies per run | `turns iterations` |
| Set a spend limit per run | `budget cost usd dollars money` |
| Batch size | `bulk resolve chunk kb` |
| Enable MCP server | `model context protocol port tools` |
| Allow AI to modify repositories | `write mutation grant` |
| Run git hooks for this repository | `husky pre-commit hooks` |
| user.name / user.email | `identity author committer who am i` |
| Check for updates on launch | `updater version release auto` |
| Welcome tour | `onboarding getting started intro help` |

---

## 4. Header identity menu

### 4.1 Placement

`.header-toolbar`, **last** — to the right of the ⚙ Settings button, at the far right edge of the
40px header. This is the conventional account slot and it is the only toolbar position that does not
push an existing control.

**What it costs / earns.** It adds one 32px control to a five-control toolbar. It earns the slot by
(a) deleting a whole Settings section (Identity profiles' Apply flow) and (b) making a fact visible
that today is invisible until a commit *fails*: which identity this repository commits as.

**Visibility.** Rendered only when a repo is open (`activeRepo !== null`), matching the existing
🤖 / 📊 buttons. This is forced by the IPC: `getConfig(repoId, level)` requires a repo id, so with no
repo open Bonsai cannot read even the global identity. With no repo, CRUD remains reachable via
Settings → Identities and the new `Manage identities…` palette command.

### 4.2 Trigger appearance

A `.btn-icon`-sized 32×32 button containing a 22px circle.

- Circle: `border-radius: 999px; width:22px; height:22px; display:grid; place-items:center;
  font-size: 10px; font-weight: 600; letter-spacing: .02em;`
- **Initials**, not a glyph, for the two identified states: first letters of the first two
  whitespace-separated words of the effective `user.name`, uppercased, max 2 chars; one word → one
  char. Initials beat a generic person glyph because the whole point of the control is *which*
  identity.
- No emoji. The toolbar already carries 🤖 and 📊; a third pictograph would read as decoration.

| State | Circle | `aria-label` | `title` |
|---|---|---|---|
| 1 — matches a saved identity | `background: var(--bg-3); color: var(--text-1); border: 1px solid var(--border);` initials | `Commit identity: Work` | `Work\nAda Lovelace <work@bonsai.dev>\nFrom this repository's config` |
| 2 — identity, no match | same, but `border: 1px solid var(--text-3)` (dotted-off feel via border colour only) | `Commit identity: Mock Fixture User` | `Mock Fixture User <fixture@bonsai.dev>\nFrom your global Git config` |
| 3 — no identity at all | `background: var(--bg-2); color: var(--text-1); border: 1px solid var(--warning);` glyph `?` | `Commit identity not set` | `No name and email are set. Commits will fail until you set one.` |
| 4 — no repo open | control absent | — | — |
| loading (effective identity not yet read) | state-2 chrome, circle content `·` (middle dot), `aria-busy="true"`, `aria-label="Reading commit identity…"` | — | — |
| error (`getConfig` rejects) | renders as state 3, but `title` = `Bonsai couldn't read this repository's Git config.` and the menu header says the same | — | — |

State 3 is carried by the `?` **glyph** and by the accessible name, never by the `--warning` ring
alone. Ring contrast `--warning` on `--bg-1`: **7.3:1** dark / **4.5:1** light (§2) — clears the 3:1
graphics bar.

Button states: hover `background: var(--bg-2)`; open `background: var(--bg-3)` +
`aria-expanded="true"`; `:focus-visible` 2px `--accent`, 1px offset.

### 4.3 Menu

A `ContextMenu`, anchored with the house idiom
(`rect = trigger.getBoundingClientRect()` → `{x: rect.right, y: rect.bottom + 2}`); the existing
viewport clamp flips it leftward at the right edge, which is exactly what a far-right trigger needs.
Min width 240px, max 320px.

**Header block** (new `ContextMenuProps.header`, §4.4) — `padding: 8px 12px;
border-bottom: 1px solid var(--border);` non-interactive, `role="presentation"`:

```
COMMITTING AS                     ← 11px uppercase .08em --text-3
Ada Lovelace                      ← 13px/600 --text-1, ellipsis
work@bonsai.dev                   ← 12px --text-2, ellipsis
From this repository's config     ← 12px --text-2, margin-top 4px
```

State 3 replaces lines 2–4 with:
```
No commit identity set            ← 13px/600 --text-1
Commits will fail until you set a name and email.   ← 12px --text-2
```

**Items**, in order:

1. One row per saved profile.
   - `label` = profile label, or `userName` when the label is blank, or `Unnamed identity` when both
     are blank.
   - `detail` = `{userName} · {userEmail}` (single line, ellipsised).
   - `checked` = true iff the profile's trimmed `userName` **and** `userEmail` both equal the
     **effective** identity.
   - `onSelect` = apply (§4.5). Selecting the already-checked row is a no-op close — it is **not**
     disabled, because a checked-but-dead row is worse than a harmless one.
2. `Save “{name}” as an identity…` — present **only** when an effective identity exists and no
   profile matches it (state 2). Opens Settings → Identities with a draft profile pre-filled from
   the effective name/email, Label field focused.
3. `Set an identity…` — present **only** in state 3. Opens Settings → Git config with the Identity
   sub-group focused, i.e. the exact `configMissing` deep link (§4.7).
4. `Add an identity…` — present **only** when the profile list is empty and states 2/3 have not
   already contributed a row (i.e. state 1 is impossible here; this covers "identity set, list
   empty" together with item 2 — render item 2 in preference and omit this one). Practically: never
   render an empty menu.
5. `Manage identities…` — always last. Opens Settings → Identities.

No separator is introduced. `ContextMenu` has no separator concept today and adding one is a fourth
API change for a two-item tail; `Manage identities…`'s wording and terminal position carry the
distinction.

### 4.4 `ContextMenu` extensions (additive, three fields)

```
ContextMenuItem:
  checked?: boolean   // present ⇒ row renders role="menuitemradio" aria-checked={checked}
                      // and reserves a 16px leading check column (✓ in --text-1, else blank).
                      // Absent ⇒ role="menuitem", no column, byte-identical to today.
  detail?: string     // one secondary line under the label: 12px --text-2, ellipsis,
                      // never focusable. Row height grows 32 → 46px when present.

ContextMenuProps:
  header?: React.ReactNode  // rendered above the list inside .context-menu, role="presentation",
                            // excluded from keyboard navigation (focus queries already scope to
                            // .context-menu-item, so no change to the nav code).
```

`checked` uses `menuitemradio` rather than `menuitemcheckbox` because at most one identity is in
effect. The check column is a **glyph**, not a colour — the row's background is unchanged when
checked, so a checked row is legible in both themes and in high-contrast mode.

### 4.5 Switching — behaviour and confirmation (D7)

Applying writes `user.name`, `user.email` and (when the profile has one) `user.signingkey` to the
repo's **local** config.

| Precondition | Behaviour |
|---|---|
| No local identity (repo inherits global) | Apply immediately. Nothing is overwritten. |
| Local identity already equals the profile | No-op; close the menu. |
| Local identity exists and **differs** | `ConfirmDialog`, `confirmVariant='primary'` |

The confirm is not "Are you sure?" — it names both sides and the consequence:

- Title: `Change this repository's identity?`
- Body (`.dialog-body`): `This repository commits as Sam Carter <sam@old.dev>, set in its own Git
  config. Using Work replaces that with Ada Lovelace <work@bonsai.dev>.`
- Detail (`.dialog-body-detail`): `Commits you have already made are not changed. You can switch
  back at any time.`
- Confirm: `Change identity`. Cancel: `Cancel`.

`primary`, not `danger`: no work is destroyed and the action is fully reversible from the same menu.
The confirm exists because the overwritten value is otherwise invisible.

**In-flight & result.** The selected row shows the present-participle label `Applying…` and the menu
stays open with `aria-busy="true"` until the write settles (§8 of `ui-reference.md`: words, not
spinners). On success the menu closes and a toast fires:
`success` · `Now committing as Work in this repository.` — dedupe key `identity:{repoId}`.
On failure the menu closes and an `error` toast fires:
`Couldn't switch identity. ` + the backend message verbatim — dedupe key `identity:{repoId}`.

### 4.6 CRUD (D5)

`Manage identities…` opens **Settings → Identities** via a new
`SettingsPanel` prop `initialCategory?: SettingsCategoryId`.

Rejected: a dedicated `IdentityManagerDialog`. It would add a second modal type, a second focus
trap, and a second width escape hatch for a list a user edits perhaps twice a year — while Settings,
which already exists and is already reachable, is where every other rarely-edited list lives.
**Flagged for the orchestrator** (§12, A1) since the user's phrasing could also be read as wanting
CRUD fully out of Settings.

Delete, in Identities, is an **inline two-step** rather than a modal-over-modal:

- Resting: `Delete` (`btn-secondary`, `--danger` text).
- After the first click the row's action area becomes:
  `Delete “Work”?` (13px `--text-1`) · `[ Delete ]` (danger) · `[ Cancel ]`, with focus moved to
  `Cancel`, and a detail line `This only removes the saved identity. Your repository's Git config is
  not changed.` Esc cancels. Auto-cancels on blur out of the card.

### 4.7 Reconciliation with Settings → Git config → Identity, and the deep link

- Git config → Identity remains the raw editor for `user.name` / `user.email` at the selected scope.
  It gains one cross-reference line under the sub-group title, 12px `--text-2`:
  `Switching between saved identities is quicker from the identity button in the toolbar.`
- Identities → each card's `Use in this repository` is the same action as the menu row and goes
  through the same confirmation rule.
- After any successful apply, the Git-config view refetches if mounted, and the header trigger
  re-reads the effective identity. One shared hook (`useEffectiveIdentity`, §9) is the single source
  so the two surfaces can never disagree.
- **`configMissing` deep link.** `App.tsx:102-105` must now pass **both**
  `initialCategory='git-config'` and `configInitialFocus='identity'`. `SettingsShell` seeds its
  category state from `initialCategory ?? 'general'` on every `false → true` transition of `open`
  (not just first mount), so a second deep link in the same session still lands correctly.
  `SettingsGitConfigSection`'s existing scroll+focus effect is unchanged — it only mounts when its
  category is selected, which is now guaranteed.

### 4.8 Exact microcopy

| Key | String |
|---|---|
| Trigger a11y (identified) | `Commit identity: {label or name}` |
| Trigger a11y (unset) | `Commit identity not set` |
| Trigger a11y (loading) | `Reading commit identity…` |
| Menu header label | `Committing as` |
| Source — local | `From this repository's config` |
| Source — global | `From your global Git config` |
| Source — unreadable | `Bonsai couldn't read this repository's Git config.` |
| Unset title | `No commit identity set` |
| Unset body | `Commits will fail until you set a name and email.` |
| Save-as item | `Save “{name}” as an identity…` |
| Set item | `Set an identity…` |
| Add item | `Add an identity…` |
| Manage item | `Manage identities…` |
| Applying row label suffix | `Applying…` |
| Success toast | `Now committing as {label} in this repository.` |
| Failure toast prefix | `Couldn't switch identity. ` |
| Confirm title | `Change this repository's identity?` |
| Confirm body | `This repository commits as {oldName} <{oldEmail}>, set in its own Git config. Using {label} replaces that with {newName} <{newEmail}>.` |
| Confirm detail | `Commits you have already made are not changed. You can switch back at any time.` |
| Confirm button | `Change identity` |
| Unnamed profile fallback | `Unnamed identity` |

---

## 5. Control standards

### 5.1 Canonical row anatomy

```
grid-template-columns: 1fr auto 24px;   column-gap: 12px;   align-items: center;
padding: 10px 0;   min-height: 44px;
row 1: [ label            ] [ control ] [ ↺ ]
row 2: [ help text        ] (control spans both rows: grid-row: 1 / -1)
```

| Part | Spec |
|---|---|
| Label | 13px `--text-1`, `<label for>` or the radiogroup's `aria-labelledby` target. Never truncated — it wraps if it must. |
| Help | 12px `--text-2`, `margin-top: 2px`, `max-width: 56ch`. Carries `id="{rowId}-help"`, wired via `aria-describedby` on the control. **Never `--text-3`.** |
| Control | `justify-self: end; grid-row: 1 / -1;` |
| Reset `↺` | 24×24 `.btn-icon`, 12px glyph `↺`, `--text-3` resting / `--text-1` hover. `aria-label="Reset {label} to default"`, `title="Reset to default ({defaultLabel})"`. **Conditionally rendered** — only when the value ≠ default. The 24px grid column is always present, so showing/hiding it never shifts the row. |
| Separator | `.settings-row + .settings-row { border-top: 1px solid var(--border); }` |
| Stacked variant | `.settings-row--stacked`: label / help / control each on its own grid row, control `width: 100%`. For text fields, read-only value+Copy pairs, and any control wider than ~200px. |

**Density.** One geometry in both `cozy` and `compact` (D10, `ui-reference.md` §3). Row 44px min,
rail 32px, controls ≥24px, in both. State it in code as a comment so a later density pass does not
"fix" it.

### 5.2 Decision rule — which control

| The setting is… | Control | In this app |
|---|---|---|
| An independent on/off of one behaviour | **Switch** | all 15 current checkboxes |
| One of 2–3 exclusive values with short, self-explanatory labels | **Segmented** | Theme, File lists, Panel density, Date basis, Repository access, Git config scope |
| One of 2+ exclusive values where each option needs a sentence | **Radio group** (stacked, hint under each) | AI → Conflict resolution |
| A bounded number the user tunes by feel | **Slider + number** (`NumberSlider`) | all 8 numeric settings |
| Free text, a path, or an unbounded value | **Text field** (stacked row) | terminal/editor command, `user.*`, custom config keys |
| A one-shot action | **Button** | Check for updates, Show tour, Add identity, Copy |
| >3 exclusive values | *(none needed in P69)* — would be a `Combobox` |

### 5.3 Controls that are currently the wrong type

1. **`Theme`** (`SettingsAppearanceSection.tsx:35-37`) — a `btn-secondary` labelled with its own
   current value. A button labelled `Dark` reads as "make it dark", the opposite of what it does.
   → Segmented `Dark | Light`.
2. **`File lists`** (`:41-47`) — same defect. → Segmented `Tree | Flat`.
3. **`Panel density`** (`:55-61`) — same defect; the inline comment even concedes "the label is the
   affordance today". → Segmented `Cozy | Compact`. The D6 note about a future third value is
   *satisfied* by a segmented control, not blocked by it.
4. **`Repository access`** (`SettingsAiRunSection`) — a fourth self-labelling button, and the one
   where mislabelling is riskiest (it toggles between two permission levels).
   → Segmented `Read-only | No file access`.
5. **All 15 checkboxes** — a bare native checkbox reads as "select this item in a list", not "this
   feature is on". → Switch.
6. **`Date basis`** — currently a radio *fieldset* with a legend, for two one-word values. Correct
   semantics, wrong weight. → Segmented skin over the same radios.
7. **Two rows both labelled `Interval`** in Background jobs — two controls with the *same accessible
   name* in one dialog. A screen-reader user cannot tell them apart, and neither can a test.
   → `Fetch every` and `Refresh every`. **MUST-FIX regardless of the rest of P69.**

### 5.4 The AI whole-fieldset-disabled pattern — keep, but re-present

**Keep** the `<fieldset disabled={!aiActive}>`. It is the only mechanism that removes ten controls
from the tab order in one place and it maps exactly to the real dependency. `aria-disabled` +
manual `tabindex` juggling would be more code and less reliable.

Three presentation fixes:

1. The explanation moves from the **bottom** to the **top** of the group, immediately under the
   group title, as a 12px `--text-2` note carrying `id="ai-run-gate-note"`.
2. The `<fieldset>` gets `aria-describedby="ai-run-gate-note"` so the reason is announced when
   focus reaches the group, not after the user has already skipped past it.
3. Disabled presentation is `opacity: .55; cursor: not-allowed` — never a hue change, and switch
   knob *position* still distinguishes on from off while disabled.

Copy: the current string is `Turn on “Enable AI features” above to change these.` It is a dependency
hint, not consent copy, but "AI features" is one word away from the P68g consent surface — see §12
(A3) before rewording. Recommended replacement, **pending user sign-off**:
`These take effect once AI features are on.` No inline "turn it on" button — that would start the
consent flow from a place the security pass did not review.

### 5.5 Switch — visual spec

```
off:  ( ●━━━ )        on:  ( ━━━● )
```

| | Spec |
|---|---|
| Wrapper | `<label class="settings-switch">`: `position: relative; display:inline-flex; align-items:center; min-height: 24px; width: 36px; flex: none;` |
| Native input | `<input type="checkbox">`, `position:absolute; inset:0; width:100%; height:100%; opacity:0; margin:0; cursor:pointer;` — **the input is the whole 36×24 hit target**. Implicit `checkbox` role, native Space toggle, `getByRole('checkbox', {name})` unchanged. |
| Track | `width: 36px; height: 20px; border-radius: 999px;` off `background: var(--text-2)`; on `background: var(--accent)` |
| Knob | `width: 14px; height: 14px; border-radius: 999px; background: var(--bg-0);` `transform: translateX(3px)` off / `translateX(19px)` on |
| Motion | `transition: transform 120ms ease-out, background-color 120ms ease-out;` transform + colour only; disabled under `prefers-reduced-motion` |
| Hover | track `filter: brightness(1.08)` (works in both themes without a token) |
| Active | knob `width: 17px` (squash), same 120ms |
| `:focus-visible` | `.settings-switch:has(> input:focus-visible) .settings-switch-track { outline: 2px solid var(--accent); outline-offset: 2px; }` |
| Disabled | wrapper `opacity: .55; cursor: not-allowed` — knob position still readable |

**Non-colour carrier:** the knob's *position*. Never rely on the track hue.

**Contrast, both themes (all boundaries ≥3:1):**

| Pair | Dark | Light |
|---|---|---|
| off track `--text-2` vs pane `--bg-0` | **7.9:1** | **4.9:1** |
| on track `--accent` vs pane `--bg-0` | **5.6:1** | **4.7:1** |
| knob `--bg-0` vs off track `--text-2` | **7.9:1** | **4.9:1** |
| knob `--bg-0` vs on track `--accent` | **5.6:1** | **4.7:1** |

### 5.6 Segmented — visual spec

- Container: `<div role="radiogroup" aria-labelledby="{rowId}-label">`, `display:inline-flex;
  background: var(--bg-2); border: 1px solid var(--border); border-radius: 6px; padding: 2px;
  gap: 2px;`
- Segment: `<label class="settings-segment"><input type="radio" class="visually-hidden">
  <span>Dark</span></label>`. `min-height: 24px; padding: 0 10px; border-radius: 4px; font-size:
  12px; color: var(--text-2); border: 1px solid transparent;`
- Selected: `background: var(--selection); color: var(--text-1); font-weight: 600;
  border-color: var(--accent);`
- Hover (unselected): `background: var(--bg-3)`.
- `:focus-visible`: `.settings-segment:has(> input:focus-visible) { outline: 2px solid var(--accent);
  outline-offset: 1px; }`
- Disabled: container `opacity: .55`, inputs `disabled`.
- Max **3** segments. Beyond that, use a `Combobox`.

**Non-colour carriers:** the 600 weight, the 1px `--accent` border, and native `aria-checked`.
`--accent` border vs `--bg-2`: **4.4:1** dark / **4.1:1** light — clears 3:1.
Native radios ⇒ arrow-key navigation and `getByRole('radio', {name})` are free.

### 5.7 Reset-to-default (D8)

- **Per row only.** `↺` in the row's third column, rendered only when the value differs from
  default. This gives an exact, reversible, zero-confirmation escape from any bad knob.
- **No per-category reset** — a category reset is the same click count as 2–3 row resets in practice
  and is a mis-click that silently destroys several deliberate choices.
- **No global "Reset all settings"** in P69. It needs a destructive confirm and an authoritative
  defaults contract shared with Rust. Recommended as a small follow-up.
- Rows with no meaningful default get no `↺`: the four Git-config identity/custom-key fields, the
  MCP read-only fields, and every button row.

**Prerequisite.** `DEFAULT_UI_SETTINGS` exists only in `src/ipc/mock/persistence.ts:77` — mock-only.
Per-row reset needs it in production. Recommended: move the constant to
`src/settings/defaults.ts` and have `mock/persistence.ts` re-export it, so nothing else changes and
`src-tauri/src/settings.rs` is untouched. Flagged in §12 (A2).

---

## 6. All states, both themes

### 6.1 Rail item

| State | Spec |
|---|---|
| Default | `--text-2`, transparent bg |
| Hover | `background: var(--bg-2)`, `--text-1` |
| Selected | `background: var(--selection)`, `--text-1`, 600, `box-shadow: inset 2px 0 0 var(--accent)` — the accent bar is the non-colour-dependent shape carrier, since `--selection` vs `--bg-1` is only ~1.3:1 |
| Selected + hover | as selected; no change (avoids a "did I lose my place" flicker) |
| Active (pressed) | `background: var(--bg-3)` |
| `:focus-visible` | 2px `--accent` outline, 1px offset |
| Disabled | never — no rail item is ever disabled. `git-config` with no repo shows the §1.2 empty block instead, which is more informative than a dead tab. |
| Long label | none of the 7 labels can overflow 200px − 16px padding − pill; still set `text-overflow: ellipsis; white-space: nowrap; min-width: 0` and a `title` |

`--text-1` on `--selection`: **9.4:1** dark / **13.3:1** light. `--text-2` on `--bg-1`: **7.3:1** /
**7.4:1** (§2).

### 6.2 Pane

| State | Spec |
|---|---|
| Default | rows as §5.1 |
| Loading (Git config fetching) | three skeleton rows: `--bg-2` bars at 40% / 70% / 55% width, 12px tall, radius 6, `skeleton-pulse` 1.2s (§8). `aria-busy="true"` on the group. Under `prefers-reduced-motion` the pulse is suppressed (existing outstanding item — do **not** add a new animation that is not in the reduced-motion block). |
| Error (Git config read fails) | existing dismissible `.error-banner` at the top of the pane: `--danger` @12% bg, `--danger` text, 6px radius. Copy: `Couldn't read this repository's Git config.` + the backend message verbatim + a `Try again` button. |
| Empty (no repo, git-config) | §1.2 |
| Empty (no identities) | title `No identities yet`, body `Save the name and email you commit with, then switch between them from the toolbar.`, action `Add identity` |
| Empty (search, 0 hits) | §3.3 |
| Long text field value | `text-overflow: ellipsis` is wrong for an editable field — the input scrolls natively; `title` carries the full value; the stacked variant gives it full pane width |
| Long help text | wraps, capped at 56ch |
| Long repo path in the Git config scope line | single line, ellipsis, full path in `title` |
| Long profile label | card title ellipsises at the card width; full label in `title` |

### 6.3 Theme parity

Every surface above is expressed in tokens only; both themes are produced by the same rule set.
**No new `:root` / `[data-theme='light']` token is introduced by P69.** Any hardcoded hex that
appears in the implementation is a defect.

Contrast summary for the pairs P69 newly *creates*:

| Pair | Dark | Light | Bar |
|---|---|---|---|
| switch off track `--text-2` / `--bg-0` | 7.9:1 | 4.9:1 | 3:1 ✓ |
| switch on track `--accent` / `--bg-0` | 5.6:1 | 4.7:1 | 3:1 ✓ |
| switch knob `--bg-0` / track (both states) | 5.6–7.9:1 | 4.7–4.9:1 | 3:1 ✓ |
| segment selected border `--accent` / `--bg-2` | 4.4:1 | 4.1:1 | 3:1 ✓ |
| segment label `--text-1` / `--selection` | 9.4:1 | 13.3:1 | 4.5:1 ✓ |
| `<mark>` `--text-1` / `--selection` | 9.4:1 | 13.3:1 | 4.5:1 ✓ |
| rail selected `--text-1` / `--selection` | 9.4:1 | 13.3:1 | 4.5:1 ✓ |
| rail accent bar `--accent` / `--selection` | 2.6:1 | 3.6:1 | decorative only — meaning is carried by `aria-selected` + weight |
| identity ring `--warning` / `--bg-1` | 7.3:1 | 4.5:1 | 3:1 ✓ |
| help text `--text-2` / `--bg-0` | 7.9:1 | 4.9:1 | 4.5:1 ✓ |

### 6.4 Motion

- Switch knob transform + track colour: 120ms ease-out.
- Segment background/border: 120ms ease-out.
- Reset `↺` opacity on hover: 120ms.
- **Nothing else animates.** Category changes are instant; the pane never fades; the card never
  animates in. Rationale: the Settings modal sits over a canvas that may be laying out 20k rows —
  any transition that repaints a large area competes with it for the frame.
- All three transitions go into the existing `prefers-reduced-motion` block (`ui-reference.md` §9).

---

## 7. Accessibility & keyboard

### 7.1 Roles and names

| Element | Role / name |
|---|---|
| Card | `role="dialog" aria-modal="true" aria-labelledby="settings-title"` — the `<h2 id="settings-title">Settings</h2>` yields the same accessible name as today's `aria-label="Settings"`, so `getByRole('dialog', {name:'Settings'})` stays green. `aria-modal` is new. |
| Rail | `role="tablist" aria-orientation="vertical" aria-label="Settings categories"` |
| Rail item | `role="tab" aria-selected id="settings-tab-{id}" aria-controls="settings-pane"`. `git-config`'s accessible name is `Git config, repository` (the pill text is folded in via a visually-hidden span, not left as a decorative pill). |
| Divider | `<div aria-hidden="true">` — a presentational child of the tablist is transparent to AT, so the tablist still owns only tabs. |
| Pane | `role="tabpanel" id="settings-pane" tabindex="-1" aria-labelledby="settings-tab-{selected}"` |
| Search | `ListFilterInput` (`role="searchbox"`), `ariaLabel="Search settings"`, `placeholder="Search settings"` |
| Search results status | visually-hidden `role="status" aria-live="polite"` |
| Group | `<section aria-labelledby="{groupId}-title">` |
| Switch | native `checkbox`, labelled by the row label, `aria-describedby="{rowId}-help"` |
| Segmented | `role="radiogroup" aria-labelledby="{rowId}-label"`, native radios inside |
| Reset | `aria-label="Reset {label} to default"` |
| Identity trigger | `aria-haspopup="menu" aria-expanded` + `aria-label` per §4.2 |
| Identity menu rows | `role="menuitemradio" aria-checked` when `checked` is present |

### 7.2 Keyboard

| Key | Behaviour |
|---|---|
| `Ctrl/Cmd + ,` | Open Settings. **Verified free** (`App.tsx:790-838` binds only `Ctrl+O`, `Ctrl+Tab`, `Ctrl+W`, `?`). Must be registered **above** the `typing` guard at `App.tsx:821` so it works from the commit message box. No-op when Settings is already open (a shortcut that toggles a modal is surprising). |
| `Tab` | Pure DOM order: close ✕ → search → rail (one stop) → pane content. No `tabindex` juggling. Trapped in the card. |
| `Shift+Tab` | Reverse, wrapping. |
| `↑` / `↓` in the rail | Move + activate (automatic activation — panes are cheap and there is no async load per tab). Wraps. |
| `Home` / `End` in the rail | First / last category. |
| `→` from the rail | Move focus into the pane (`pane.focus()`), the tablist convention. |
| `Space` on a switch | Native toggle. |
| `←` / `→` in a segmented group | Native radio navigation. |
| `Esc` | 1st press: clears a non-empty search (existing `ListFilterInput` capture-phase handler). 2nd press: closes Settings (existing App overlay-Esc effect). |
| `Enter` | Activates the focused control. Never closes the dialog — there is no primary "OK"; settings apply live. |
| Backdrop mousedown | Closes (existing behaviour, unchanged). |

**Focus trap and restore.** `SettingsShell` stores `document.activeElement` on the `false → true`
transition and restores it on close (falling back to the `.settings-toggle` button, then `body`).
Initial focus goes to the **search input** — it is a text field, so no accidental activation, and it
is the fastest route for a user who already knows the setting's name. When opened via a deep link
(`initialCategory` set) initial focus goes to the **pane** instead, so the deep-linked section's own
focus effect (`configInitialFocus='identity'`) is not fighting the search box.

### 7.3 Command palette

Three new `PaletteAction`s in `App.tsx`'s `appCommands` (`group: 'action'`), replacing nothing:

| id | title | keywords |
|---|---|---|
| `app.settings` *(existing)* | `Open Settings` | extend with `identity graph appearance updates` |
| `app.identities` | `Manage identities…` | `profile user name email author committer signing` |
| `app.gitConfig` | `Open Git config…` | `settings local global user.name user.email hooks` |

Both new entries call `setSettingsOpen(true)` with the matching `initialCategory`.
`app.gitConfig` is `disabled` when no repo is open.

### 7.4 Hit targets

Rail item 32px · switch 36×24 · segment ≥24px · reset 24×24 · close 32×32 · identity trigger 32×32.
All ≥24px in both densities.

---

## 8. Microcopy pass

**FROZEN — do not touch** (P68g security pass, `docs/contracts/P68-security-audit.md`): every string
in `SettingsMcpSection.tsx`, the consent-facing strings in `SettingsAiSection.tsx`, and all three
consent `ConfirmDialog` bodies. Where those strings sit inside a redesigned row, the *layout*
changes and the *words* do not.

**FROZEN — test-bound** (see §11): `Row height`, `#settings-graph-row`, `Short SHA`, `Author name`,
`Date`, `Ahead/behind on branches`, `Signature badge`, `Compact rows`, `Show PR badges`,
`Show CI status`, and the header's `Switch to light theme` / `Switch to dark theme`.

| Where | Today | Becomes | Why |
|---|---|---|---|
| Background jobs | section title `Background jobs` | `Background activity` | "Jobs" is CI jargon here |
| Background jobs | `Enable auto-fetch` | `Auto-fetch from remotes` | "Enable X" is redundant next to a switch |
| Background jobs | help — *(none)* | `Fetches in the background for every open repository. Never pulls, pushes, or asks for credentials.` | the section desc, moved to the row it describes |
| Background jobs | `Interval` (fetch) | `Fetch every` | **duplicate accessible name** — MUST-FIX |
| Background jobs | `Refresh status & health periodically` | `Refresh status automatically` | shorter, and "periodically" is implied by the interval row below |
| Background jobs | help — *(none)* | `Re-reads the working directory and repository health on a timer.` | |
| Background jobs | `Interval` (health) | `Refresh every` | duplicate name |
| Appearance | note in `--text-3` (`:65-68`) | row help on Panel density, `--text-2`: `Affects the right panel and the AI dock. Graph row spacing is separate — see Commit graph → Compact rows.` | `--text-3` is decorative-only (§2); this is instructional |
| Commit graph | help — *(none)* | `Commit node size` → `Radius of the dot drawn on each commit.` · `Row height` → `Vertical space per commit row.` · `Lane width` → `Horizontal space between branch lanes.` · `Short SHA` → `Show the abbreviated commit id in each row.` · `Date basis` → `Which timestamp the Date column shows.` | eight sliders/toggles with no explanation at all |
| External tools | `Reset to auto-detect` button | the row `↺`, `title="Reset to default (auto-detect)"`; field placeholder `Auto-detect` | one reset idiom app-wide |
| External tools | help — *(none)* | `Leave empty to use your system default.` | the '' ⇒ auto-detect rule is currently invisible |
| Git config | `Level` + `Local | Global` | pane-header scope switch `Scope` + `This repository | Global`, plus the scope line (§1.1) | "Level" is libgit2's word, not the user's |
| Git config | `Open a repository to view and edit its Git config.` (bare) | the §1.2 empty block with an `Open repository…` action | a sentence that names a problem and offers no fix |
| Identities | section title `Identity profiles` | `Identities` | "profile" collides with git's `includeIf`/profile concepts |
| Identities | `Apply to current repo` | `Use in this repository` | "apply" reads like a form's Apply button |
| Identities | badge `Active on this repo` | hueless pill `in use` (§11 recipe) | lowercase row-pill convention; `--text-2` on its own 12% tint, **5.79:1** / **6.22:1** |
| Identities | `This does not look like an email address (missing @).` | `That doesn't look like an email address — it's missing an “@”.` | contractions + a real dash read as a person, not a validator |
| Identities | `No profiles yet. Add one to get started.` | title `No identities yet` + `Save the name and email you commit with, then switch between them from the toolbar.` | says what the feature is *for* |
| Identities | `Add profile` | `Add identity` | |
| Identities | `Delete` (bare) | two-step, §4.6 | destructive-action rule |
| About | section title `Getting started` | `Help` (inside the About category) | |
| About | `First-run tour` / `Show welcome tour` | label `Welcome tour`, help `A short guided tour of Bonsai's main panes.`, button `Show tour` | "first-run" is an implementation word |
| About | `Automatically check for updates on launch` | **unchanged label** + new help `Bonsai contacts the update server on startup. Turn this off to check only when you press Check for updates.` | discloses network egress; label left alone — see §12 (A4) |
| About | `Current version` | help `The version of Bonsai you are running.` | |
| AI runs | bottom hint `Turn on “Enable AI features” above to change these.` | top note `These take effect once AI features are on.` — **pending §12 (A3)** | position fix is unconditional; the rewording is not |

---

## 9. Component decomposition

New directory `src/components/settings/`. Every file is a single responsibility, well under 500.

### 9.1 Shell and primitives

| File | ~LOC | Responsibility |
|---|---|---|
| `settings/SettingsShell.tsx` | 190 | Overlay + card grid, focus trap/restore, category state (seeded from `initialCategory`), search query state, results-vs-category switch, scroll reset. Takes a `categories: SettingsCategory[]` array. |
| `settings/SettingsRail.tsx` | 120 | `role="tablist"`, roving tabindex, arrow/Home/End, dividers, per-category match counts, the `repo` pill. |
| `settings/SettingsSearchBar.tsx` | 60 | Wraps `ListFilterInput`; owns the placeholder/label and the live-region status text. |
| `settings/SettingsResults.tsx` | 130 | Cross-category result groups, `Go to {Category}` links, `<mark>` highlighting, zero-match. |
| `settings/SettingsRow.tsx` | 110 | The §5.1 anatomy, incl. the stacked variant and the conditional `↺`. |
| `settings/SettingsSwitch.tsx` | 55 | §5.5. Native checkbox inside. |
| `settings/SettingsSegmented.tsx` | 70 | §5.6. Native radios inside. |
| `settings/SettingsGroup.tsx` | 45 | Group title + hairline-separated children. |
| `settings/SettingsPaneHeader.tsx` | 50 | Category title, subtitle, optional trailing slot (the Git config scope switch). |
| `settings/SettingsEmpty.tsx` | 50 | The three in-pane empty variants (no repo / no identities / no search hits). |
| `settings/settingsIndex.ts` | 170 | The searchable row index (§3.4). Pure data. |
| `settings/types.ts` | 40 | `SettingsCategoryId`, `SettingsCategory`, `SettingsIndexEntry`. |

### 9.2 Category files

| File | ~LOC | Composes |
|---|---|---|
| `settings/categories/GeneralCategory.tsx` | 150 | Background activity rows (lifted out of `SettingsPanel.tsx`) + `SettingsExternalToolsSection` |
| `settings/categories/AppearanceCategory.tsx` | 80 | Replaces `SettingsAppearanceSection.tsx` (three self-labelling buttons → segmented) |
| `settings/categories/GraphCategory.tsx` | 60 | Wraps `SettingsGraphSection` (re-skinned) in three groups: Geometry / Row details / Badges |
| `settings/categories/AiCategory.tsx` | 90 | `SettingsAiSection` + `SettingsAiRunSection` + `SettingsMcpSection` under three group titles |
| `settings/categories/IdentitiesCategory.tsx` | 70 | `SettingsProfilesSection` + empty state + the draft-prefill entry point |
| `settings/categories/GitConfigCategory.tsx` | 90 | Scope switch (owns `level` state, lifted out of `SettingsGitConfigSection`) + `SettingsGitConfigSection` + `SettingsEmpty` |
| `settings/categories/AboutCategory.tsx` | 100 | Version + `SettingsUpdatesSection` + the welcome-tour row |

### 9.3 Identity surface

| File | ~LOC | Responsibility |
|---|---|---|
| `src/components/HeaderToolbar.tsx` | 100 | **New.** The whole `.header-toolbar` lifted out of `App.tsx:921-971` verbatim, plus the identity trigger. Net effect on `App.tsx`: **−50 / +1 lines** (D9). |
| `src/components/IdentityMenu.tsx` | 170 | Trigger + own open/anchor state + `ContextMenu` + apply + confirm. Lifts open state via `onMenuOpenChange` (the `TabStrip.tsx:35-37` precedent) because `App.tsx:802` early-returns global shortcuts while a menu is open. |
| `src/components/IdentityAvatar.tsx` | 45 | The 22px initials/`?` circle, its four states, initials derivation. |
| `src/hooks/useEffectiveIdentity.ts` | 90 | **The behavioural fix.** Reads `getConfig(repoId,'local')`, falls back to `getConfig(repoId,'global')`, returns `{name, email, source: 'local'\|'global'\|null, loading, error}`. Consumed by `IdentityMenu` **and** `SettingsProfilesSection` so the two can never disagree. |

### 9.4 Existing files — what changes

| File | Now | Action |
|---|---|---|
| `SettingsPanel.tsx` | 365 | Becomes the props façade + handlers; renders `SettingsShell` with the category array. → ~240 (≈120 of which is the props interface). |
| `SettingsGitConfigSection.tsx` | **436** | `level` state + the Level row move out to `GitConfigCategory`; the `<details>` Advanced block moves to `settings/GitConfigAdvanced.tsx` (~160). → ~230. **This is the required split — the file may not reach 500.** |
| `SettingsProfilesSection.tsx` | 281 | Per-profile card extracted to `settings/IdentityProfileCard.tsx` (~160); the local-only match logic is deleted in favour of `useEffectiveIdentity`. → ~140. |
| `SettingsAppearanceSection.tsx` | 71 | **Deleted**, replaced by `AppearanceCategory.tsx`. |
| `SettingsGraphSection.tsx` | — | Re-skinned: checkboxes → `SettingsSwitch`, radios → `SettingsSegmented`, rows → `SettingsRow`. **Labels frozen.** |
| `SettingsAiSection.tsx` | — | Re-skinned; radio group stays a radio group; **strings frozen**. |
| `SettingsAiRunSection.tsx` / `SettingsAiLimits.tsx` | — | Re-skinned; `Repository access` → segmented; gate note moves to the top. |
| `SettingsMcpSection.tsx` | — | Re-skinned (2 checkboxes → switches, rows → `SettingsRow` stacked). **Strings frozen.** |
| `SettingsUpdatesSection.tsx` | — | Re-skinned (1 checkbox → switch). |
| `SettingsHooksToggle.tsx` | — | Re-skinned (1 checkbox → switch). |
| `App.tsx` | 1114 | `-50` (toolbar extraction) `+1` (`<HeaderToolbar>`) `+~14` (Ctrl/Cmd+, binding, two palette entries, `initialCategory` on the configMissing path). **Net ≈ −35 → still under the ratchet.** |
| `ContextMenu.tsx` | 321 | +3 additive fields (§4.4), ~+35 lines → ~356. |

**Honest answer to "which of the 8 existing sections re-parent unmodified": none.** Every one carries
`.settings-checkbox` or `btn-secondary` markup that this contract replaces. The two with the smallest
diff are `SettingsUpdatesSection` (one checkbox) and `SettingsHooksToggle` (one checkbox). Pretending
otherwise would produce a redesign with two visual languages in one dialog.

### 9.5 Suggested increments

- **A — shell.** `SettingsShell` + rail + search bar + `SettingsRow` / `SettingsSwitch` /
  `SettingsSegmented` / `SettingsGroup` / `SettingsPaneHeader` / `SettingsEmpty` / types, and the
  General + Appearance + About categories. Search index seeded for those three.
- **B — repo scope.** `GitConfigCategory` + the `SettingsGitConfigSection` split + the
  `configMissing` deep link + `Ctrl/Cmd+,`.
- **C — identity.** `useEffectiveIdentity` + `HeaderToolbar` + `IdentityMenu` + `IdentityAvatar` +
  the `ContextMenu` extensions + `IdentitiesCategory` + the profiles split + palette entries.
- **D — graph & AI re-skin.** `GraphCategory`, `AiCategory`, re-skin the five AI/MCP/graph sections,
  full search index, `SettingsResults`.

---

## 10. Harness states

`pnpm dev` with `VITE_MOCK_IPC=1`. **The harness is headless: `requestAnimationFrame` never fires
and screenshots fail outright.** Everything below is verified by `read_page` / `get_page_text` /
a batched `javascript_tool` computed-style read.

| Fixture | Exists? | Verifies |
|---|---|---|
| default (`?` none) | yes | Shell geometry, rail, all 7 categories, search, both themes (`resize_window` colorScheme). Identity **state 2**: effective = global `Mock Fixture User <fixture@bonsai.dev>` (`fixtures/config.ts:47-53`), profiles `Work`/`Personal` (`mock/persistence.ts:118-132`) → no checked row, `Save “Mock Fixture User” as an identity…` present. |
| `?fixture=noconfig` | yes | Identity **state 3** (`?` glyph, warning ring, `Set an identity…`), and the `configMissing` deep link landing on Git config → Identity with the rail selecting `git-config`. |
| `?fixture=identitymatch` | **new** | Identity **state 1**: seed `local` `user.name = Ada Lovelace`, `user.email = work@bonsai.dev` **and** a profile with those exact values → one `menuitemradio` with `aria-checked="true"`, source line `From this repository's config`, and applying the *other* profile firing the §4.5 confirm (differing local identity). |
| no repo open (close all tabs) | yes | Identity **state 4** (trigger absent), Settings → Git config showing the §1.2 empty block, `app.gitConfig` palette entry disabled. |
| `?fixture=slowconfig` | **new** (or reuse the existing latency knob) | Git config skeleton rows, `aria-busy`, and the identity trigger's `·` loading circle + `aria-label="Reading commit identity…"`. |
| `?fixture=configerror` | **new** — `getConfig` rejects | Git config error banner with `Try again`; identity trigger falls back to state-3 chrome with the distinct "couldn't read" title. |
| `?fixture=longsettings` | **new** | Pathological content: a profile with a 120-char label and a 90-char email; `terminalCommand` of 300 chars; a repo workdir path of 200 chars; a custom config key `a.very.long.section.key.name…`. Verifies ellipsis + `title` in the menu rows, the card titles, the scope line, and that no row grows the card. |
| MCP running / stopped | yes | AI → AI access rows in both states. |
| `?update=` seam | yes | About → version result line. |
| profiles = `[]` | via `localStorage bonsai.mockUiSettings` | Identities empty state + `Add an identity…` menu row. |
| `aiConsented: true` seed | known harness step | AI category with the fieldset **enabled** — otherwise the ten run knobs are only visible disabled. |

### USER CHECKPOINT (not AI-verifiable)

1. **Any visual proof at all** — screenshots fail in this harness. The two-pane geometry, the switch
   and segmented rendering, and both themes must be eyeballed in `pnpm tauri dev`.
2. `Ctrl/Cmd + ,` on macOS — the binding is testable in jsdom, the absence of an OS/webview conflict
   is not.
3. Applying an identity actually writing `.git/config` (the mock writes to memory only).
4. Pane scroll feel and focus-ring rendering (rAF paused).
5. `:has()` support in the shipped WebView2 / WebKitGTK versions — used by the switch and segment
   focus rules. Provide a `:focus-within` fallback if the native check fails.

---

## 11. Frozen contract surface (do not rename)

- Input id `#settings-graph-row`.
- Accessible names `Row height`, `Switch to light theme`, `Switch to dark theme`.
- All graph toggle labels queried as `getByRole('checkbox', {name})` in
  `e2e/10-settings-persistence.spec.ts:44-46,71-73` and the vitest suites: `Short SHA`,
  `Author name`, `Date`, `Ahead/behind on branches`, `Signature badge`, `Compact rows`,
  `Show PR badges`, `Show CI status`.
- The **implicit `checkbox` role** of every toggle and the **implicit `radio` role** of every
  exclusive choice. The switch and segmented specs are CSS skins over native inputs precisely so
  these survive (D4). `role="switch"` is **not** used, deliberately — the test churn is not worth a
  role that AT already conveys through the visual and the label.
- `getByRole('dialog', {name: 'Settings'})` — preserved via `aria-labelledby` on the `<h2>`.

---

## 12. Flagged ambiguities — orchestrator decisions

**A1 — Where identity CRUD lives.** The user asked to "extract the profiles from settings page".
Switching definitively moves to the header. CRUD is my call and I put it in **Settings →
Identities** (D5): one modal type, one focus trap, reuses the deep-link mechanism P69 needs anyway,
and matches every desktop app's "manage accounts" placement. *Alternative:* a dedicated
`IdentityManagerDialog` opened from the menu, fully removing identities from Settings.
**Recommendation: Settings → Identities.** Ask the user if the intent was stronger.

**A2 — Production defaults constant.** `DEFAULT_UI_SETTINGS` exists only in
`src/ipc/mock/persistence.ts:77`; per-row reset needs it in production and `src-tauri/src/settings.rs`
is at its ratchet ceiling. **Recommendation: move the constant to `src/settings/defaults.ts` and
re-export it from `mock/persistence.ts`.** No Rust change, no ratchet growth. A tester item should
assert the TS defaults match `settings.rs`'s serde defaults, since nothing enforces it.

**A3 — Rewording the AI-runs gate hint.** `Turn on “Enable AI features” above to change these.` is a
dependency hint, not consent copy, but it names the P68g consent switch. **Recommendation: apply the
positional fix (bottom → top, `aria-describedby`) unconditionally, and hold the reword
(`These take effect once AI features are on.`) for the user's sign-off.** I have *not* specced an
inline "turn on AI features" button there — it would start the consent flow from a surface the
security pass did not review.

**A4 — The updates auto-check help line.** Adding
`Bonsai contacts the update server on startup…` discloses network egress the current UI does not
mention. It is strictly more honest, but it is a new privacy statement. **Recommendation: ship it,
and tell the user.** The label itself is left unchanged.

**A5 — "Reset all settings".** Out of scope for P69 (D8). Per-row reset covers the real need; a
global reset needs a destructive confirm and a shared defaults contract. **Recommendation: follow-up
milestone**, not a P69 add-on.

**A6 — Rail without group headings.** I dropped the `App` / `Repository` headings in favour of two
hairline dividers plus a `repo` pill (D2), because non-tab children inside a `role="tablist"` are an
ARIA hazard and three headings for seven items is heavy chrome. If the orchestrator wants visible
grouping, the fix is `role="tab"` items in separate `role="tablist"` groups with
`aria-label`s — more markup, no functional gain. **Recommendation: keep the dividers.**

**A7 — Increment sizing.** §9.5 splits P69 into four senior-dev passes. Increment D (the graph/AI
re-skin) touches five existing sections and is the one most likely to need a second review round.
**Recommendation: run A→B→C→D and commit each.**
