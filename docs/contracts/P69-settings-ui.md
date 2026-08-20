# P69 — Settings redesign (UI contract)

Owner: `ui-designer`. Implementer: `senior-dev`.
Prerequisite reading: `docs/contracts/ui-reference.md` §2 (tokens/contrast), §3 (spacing/density),
§8 (states), §11 (pills). This contract adds `ui-reference.md` §12 "Settings surface" in the same
pass — that section is the durable extract; this file is the increment spec.

**Scope.** Restructure the Settings overlay from one 560px scrolling column of eleven flat sections
into a two-pane categorised modal; move identity switching into the header; standardise every
control; add per-row help and reset; close the effective-identity gap. No backend change, no new
IPC command, **no new design token**.

**Status: shipped.** Eleven increments, P69a–P69k (code head `a13b729`). This file was amended in
**P69l** so it describes what actually shipped rather than what was proposed; every amendment is
listed in §0.1 and marked `[P69l]` inline. Anything without a marker shipped as specced. Two items
are specced but **not implemented** and are labelled `[NOT IMPLEMENTED]`: the help-text highlight
fallback (§3.2) and the draft-divergence hint behaviour (§13).

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
| D11 `[P69l]` | Rail arrows use **manual activation** (arrows move focus, Enter/Space select), and there is **no focus trap** | Automatic activation would fire a `getConfig` round-trip every time focus passed over Git config. The trap is the architect's D-4: no dialog in this codebase has one, so adding it here alone would be the inconsistency. Focus *restore* ships. |

### 0.1 Post-ship amendments (P69l)

| # | Section | Amendment |
|---|---|---|
| 1 | §2.2 | The card is **one** 3-row grid (`48px 56px 1fr`) with every cell placed explicitly; there is no nested content-column grid. Category change does **not** move focus. |
| 2 | §3 | Matching is restricted to **renderable** rows; the index lives in `settingsCatalog.ts` + `catalog/*.ts`, not a separate `settingsIndex.ts`. Two Git-config blocks self-filter, and the Advanced `<details>` is forced open while searching. |
| 3 | §3.2 | Results group header is `--text-2`, not `--text-3`. Rail zero-count items are **not** dimmed — emphasis is inverted instead. Count colour ruling: `--text-1` upheld. Live-region singular is `1 setting matches`. Count semantics are per catalog **entry**. Help-text highlight fallback specced, not implemented. |
| 4 | §4.4 | `ContextMenu` gained **four** additive fields, not three: `busy` on the props as well. |
| 5 | §7.1 | Rail accessible name gains a `, {n} match(es)` suffix while a query is active; the `repo` scope is folded in with `aria-label`, not a visually-hidden span. Icon-only `Copy` buttons get object-naming `aria-label`s. |
| 6 | §7.2 | Manual rail activation; no focus trap; deep-linked opens set no initial focus of their own. |
| 7 | §7.4 | `Go to {Category}` added to the hit-target table (`min-height: 24px`). |
| 8 | §9.1 / §9.4 | File-table paths corrected to what shipped. |
| 9 | §13 | `P69c-draft-feedback-ui.md` folded in here verbatim-in-substance; that file is now a pointer stub. |

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
| Card `[P69l]` | `width: 880px; max-width: calc(100vw - 48px); height: min(660px, calc(100vh - 64px)); display: grid; grid-template-columns: 200px 1fr; grid-template-rows: 48px 56px 1fr; overflow: hidden;` background `--bg-1`, 1px `--border`, radius 6. **One grid, three rows** — header / search bar / pane — with every cell placed explicitly. There is no nested content-column grid. |
| Header (row 1, spans 2 cols) | `height: 48px; padding: 0 8px 0 16px; display:flex; align-items:center; border-bottom: 1px solid var(--border);` title 15px/600 `--text-1`, `margin-right:auto`; close = existing `.btn-icon`, 32×32 |
| Rail (col 1, `grid-row: 2 / span 2`) `[P69l]` | `padding: 8px; overflow-y: auto; border-right: 1px solid var(--border);` background `--bg-1`. The rail spans the search row and the pane row, so the search bar sits beside it, not above it. |
| Rail item | `height: 32px; padding: 0 8px; gap: 8px; border-radius: 6px; font-size: 13px; color: var(--text-2);` items separated by `2px` gap |
| Rail divider | `height:1px; background: var(--border); margin: 8px 8px;` `aria-hidden` |
| Search bar (col 2, row 2) | `padding: 12px 16px; border-bottom: 1px solid var(--border);` background `--bg-0`; input full-width, 32px tall. **Precedes the rail in DOM order** — explicit grid placement is what buys the ✕ → search → rail → pane tab order without `tabindex` juggling. |
| Pane (col 2, row 3) | `padding: 16px 24px 24px; overflow-y: auto; scroll-behavior: auto;` background `--bg-0` |
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

**Scroll reset `[P69l]`.** Changing category, and clearing a query from the rail, sets
`pane.scrollTop = 0`. **Focus is never moved by a category change** — a mouse click leaves it on the
rail item and a keyboard activation leaves it on the same item, which is what manual activation
(D11) requires: moving focus into the pane on activation would make `↑`/`↓` unusable as a browsing
gesture. The pane keeps `tabindex="-1"` because `→` still hands focus to it deliberately.

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

**Mechanism `[P69l]`.** There is no second renderer for the ~60 rows. Each hit category's own page is
mounted inside a `SettingsSearchContext`, and every `SettingsRow` whose id is not in the hit set
removes itself. Consequences that are part of the contract, not incidental:

- **Rows stamped outside `SettingsRow` must self-filter.** Two Git-config blocks are stamped in
  `GitConfigAdvanced.tsx` (`git-config.behaviour`, `git-config.custom-keys`); without their own
  `useSettingsRowVisible` calls, any git-config hit rendered the entire Advanced form — both blocks
  plus the add row — as a "result". The `Advanced` `<details>` is the *group*, so it also disappears
  when neither block survived.
- **The Advanced `<details>` is forced `open` while a query is running** (`open={searching ? true :
  undefined}` — uncontrolled otherwise, so the user's own open/closed state stays theirs). A hit
  inside a collapsed disclosure is an invisible result.
- **Only renderable rows can match.** `searchSettings` filters through
  `settingsAvailability.ts` first: a row whose precondition fails is not in the DOM, so matching it
  would report a count nobody can see and hand the result list a category whose block renders empty.
  One list feeds the status line, the rail counts and the results, so the three cannot disagree.
- `HeaderTrailing` is rendered inside a result block too, because exactly one catalogued row lives
  in a pane header rather than in a group (`git-config.scope`). It self-filters through the same
  context.

### 3.2 Behaviour

- Debounce: none. Matching is a synchronous pass over a ~60-entry static index.
- Matching: case-insensitive substring over `label` + `help` + `keywords`, on the whitespace-split
  query with **all** terms required (AND). Not fuzzy — fuzzy over 60 rows produces confident
  nonsense.
- Result group header: category name, 11px uppercase 600, `letter-spacing: .08em`, **`--text-2`**
  `[P69l]` — plus a right-aligned `Go to {Category}` text button (12px `--text-2`, `--text-1` on
  hover) that clears the query and selects that category. The header was specced `--text-3`; it
  could not stay. `--text-3` is decorative-only (§2 of `ui-reference.md`: 3.38:1 on `--bg-1`,
  ≈3.2:1 on `--bg-0` in light), and in a result list this heading is the **primary wayfinder** —
  it is the only thing telling the user which category a row belongs to. `--text-2` on `--bg-0` is
  **7.9:1** dark / **4.9:1** light, so 11px text clears AA. The uppercase + 600 weight keeps it
  distinct from the 400-weight button beside it. The group-title `--text-3` inside a *category page*
  is unaffected: there the title duplicates visible structure.
- Highlight: the matched substrings in the **label** are wrapped in
  `<mark class="settings-match">` — `background: var(--selection); color: var(--text-1);
  border-radius: 2px; padding: 0 1px;`. Contrast `--text-1` on `--selection`: **9.4:1** dark /
  **13.3:1** light. Overlapping ranges from different terms are merged before wrapping, so
  `set spend` over `Set a spend limit per run` yields two marks and never a nested pair.
- **Help-text highlight fallback `[P69l]` `[NOT IMPLEMENTED]`** — see §3.2.1. Today a query that
  matched only `keywords` or only `help` produces **zero** `<mark>` elements, including `graph`,
  which §3.3's own copy recommends as the example query: it returns 5 hits whose labels read
  `Row height`, `Lane width`, `Compact rows`… and nothing is pointed at.
- **Rail while searching `[P69l]`.** Each item shows a right-aligned count (11px, tabular numerals).
  **Emphasis is inverted, not dimmed.** The specced `opacity: .5` on zero-count items was removed: it
  put a deliberately-still-clickable control at **2.87:1** dark / **2.36:1** light, and because
  clicking a zero-count item is *how the user leaves the search*, the WCAG 1.4.3 inactive-component
  exemption does not apply to it. What shipped instead:

  | Rail item, query active | Label + count colour | Digit |
  |---|---|---|
  | has matches (`.is-hit`) | `--text-1` (**13.5:1** dark / **15.4:1** light on `--bg-1`) | `N` |
  | no matches | `--text-2` (**7.3:1** / **7.4:1**) — the resting colour, unchanged | `0` |

  Every item stays clickable, and clicking any of them clears the query and selects it.
  `aria-selected` still tracks the underlying selection, so the selected category is still marked
  even when it has zero hits.
- **Count colour — ruling `[P69l]`.** The design review asked for the count in `--accent`; the
  implementation shipped `--text-1`. **`--text-1` is upheld; no rework.** `--accent` is not usable
  here: on the `--selection` fill of a *selected* rail item it measures **2.6:1** dark / **3.6:1**
  light (§6.3 of this contract, measured in the P69 pass) — the implementer's independent read of
  **3.51:1** / **3.74:1** agrees on the verdict — i.e. it fails the 4.5:1 text bar in both themes
  and is *worse* on that case than the `opacity: .5` it was replacing. Since the count is a number
  the user reads, not a graphic, 4.5:1 applies. `--text-1` clears it everywhere, including on the
  selection fill (**9.4:1** / **13.3:1**).
  **What now carries hit vs no-hit**, given both states differ only by `--text-2` vs `--text-1`:
  1. **The digit itself** — `0` vs `N`. A literal value, not a colour, so WCAG 1.4.1 is satisfied
     without the lift.
  2. **A whole-item luminance step** — label *and* count move together from `--text-2` to
     `--text-1`, roughly a 2× contrast-ratio jump against `--bg-1`. It reads as mass, not hue, so it
     survives greyscale and both themes.
  3. **The accessible name suffix** (§7.1), which is the AT-side equivalent of the lift.
  This is sufficient. It is, however, deliberately quiet, and quietness is the one thing a headless
  harness cannot judge — **USER CHECKPOINT**: if the lift reads as too subtle in the native window,
  the correct strengthening is to render **nothing** for a zero-count item instead of `0`, making
  presence-vs-absence of the digit the primary carrier. Do not reach for `--accent`, a hue, a bold
  weight (600 is the selection carrier), or a badge fill.
- **Count semantics `[P69l]`.** Rail counts and the status line count **catalog entries**, not
  rendered instances. A `repeats: 'perProfile'` row is one entry and many rows: with three saved
  identities, `user.name` contributes **1** to the count while **3** rows render. This is
  defensible — the count answers "how many *settings* match", which is the question the query asks,
  and counting instances would make the number swing when a user adds an identity — but it must be
  stated rather than left incidental. Anyone changing it must change the status line, the rail and
  the result blocks together.
- Live region `[P69l]`: a visually-hidden `role="status" aria-live="polite"` announces
  `{n} settings match` / `1 setting matches` / `No settings match` on each settled query. The
  singular is `1 setting matches`, not `1 setting matched` or `1 match` — it keeps the sentence shape
  identical across all three cases so the announcement is predictable, and the noun stays plural-free
  while the verb agrees with `1 setting`.
- Esc: reuse `ListFilterInput`'s capture-phase Esc — first Esc clears a non-empty query, second Esc
  closes the dialog. No new mechanism.

#### 3.2.1 Help-text highlight fallback `[NOT IMPLEMENTED]`

For a later increment. The problem: `keywords` are never displayed and `help` is not highlighted, so
a query that matched neither label produces a result list with no visible reason for any of its rows.
`graph` is the worst case and it is the query our own empty-state copy recommends.

**Rule.** Per row, label first, help only as a fallback — never both, so the noise stays bounded:

1. In `SettingsRow.tsx`, while `search !== null`, compute the label's match ranges.
2. If the label produced **≥ 1** range: highlight the label (today's behaviour) and render `help`
   as plain text. Unchanged.
3. If the label produced **0** ranges **and** the row has catalog `help`: render the help paragraph
   through `highlightTerms(help, search.terms)` and leave the label plain.
4. If neither the label nor the help contains any term — a `keywords`-only match, e.g. `origin`
   hitting `Auto-fetch from remotes` — **render no mark at all**. The row still appears. Do not
   invent a "matched on keywords" badge, a tooltip, or a synthesised reason line: `keywords` is an
   internal index field, exposing it would leak vocabulary the user did not type, and the row's
   presence is already the answer.
5. **Single-character terms are excluded from the fallback.** A one-char term marks nearly every
   word of a 56ch sentence; short labels never reach that density. Labels keep highlighting at any
   term length (unchanged); the help fallback applies only to terms of length ≥ 2.

**Element and styling.** The existing `.settings-row-help` `<p>`, with the existing
`<mark class="settings-match">` — no new element, no new class, no new token. Inside a 12px
`--text-2` paragraph the mark's `color: var(--text-1)` reads as a slight lift on a `--selection`
ground; contrast is the same measured pair (**9.4:1** dark / **13.3:1** light) and it is a stronger
pass than the surrounding help text.

**Helper.** `settingsHighlight.tsx` keeps `highlightTerms` byte-identical and gains one pure export,
`hasHighlight(text, terms): boolean` (true iff `matchRanges` is non-empty). `SettingsRow` calls it
once for the label to choose the branch. Do not change `highlightTerms`'s return type — several call
sites rely on it returning the plain string when nothing matches.

**A11y.** `<mark>` needs no ARIA; most screen readers ignore it, and the row's accessible name and
description are unchanged either way. Never add `aria-label` to a mark.

**Copy.** §3.3's zero-match copy does **not** change. `graph` is still a good example query — with
this fallback it becomes a *better* one, because all five hits then show why they matched.

**Harness check.** `?` default fixture, open Settings, type `graph`, then count
`document.querySelectorAll('mark.settings-match').length` — expect ≥ 5 (today: 0). Then type
`row height` and confirm the marks are on the **label**, not duplicated into the help line.

### 3.3 Zero match

```
        No settings match “xyz”.

  Try a shorter word — for example graph, fetch,
  identity, or spend.

              [ Clear search ]
```
Title 13px `--text-1`; body 12px `--text-2`; `btn-secondary`. Same `SettingsEmpty` component as
§1.2, different props. The query is quoted verbatim with curly quotes, trimmed.

### 3.4 The index `[P69l]`

The searchable index is **pure data in its own modules** — the fixture-table rule — and it is the
same catalog that feeds every row's label, help, reset descriptor and id:

```
src/components/settings/settingsCatalog.ts   — SETTINGS_CATEGORIES, SETTINGS_INDEX, searchSettings, id helpers
src/components/settings/catalog/{general,appearance,graph,ai,repo,about,reset}.ts — the entries
```

Entry shape: `{ id, category, label, help, keywords, … }`. `keywords` are never displayed. Supply
them for every row whose label is not the word the user would type. A row that carries a stateful
`.settings-row-note` instead of catalog `help` (§5.1) **must** compensate in `keywords` — that is the
only place its vocabulary can live. Minimum required set:

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

No separator is introduced. `ContextMenu` has no separator concept today and adding one is a further
API change for a two-item tail; `Manage identities…`'s wording and terminal position carry the
distinction.

### 4.4 `ContextMenu` extensions (additive, **four** fields) `[P69l]`

```
ContextMenuItem:
  checked?: boolean   // present ⇒ row renders role="menuitemradio" aria-checked={checked}
                      // and reserves a 16px leading check column (✓ in --text-1, else blank).
                      // Absent ⇒ role="menuitem", no column, byte-identical to today.
  detail?: string     // one secondary line under the label: 12px --text-2, ellipsis,
                      // never focusable. Row height grows 32 → 46px when present.
  busy?: boolean      // the row whose write is settling: present-participle label + the menu
                      // stays open (§4.5).

ContextMenuProps:
  header?: React.ReactNode  // rendered above the list inside .context-menu, role="presentation",
                            // excluded from keyboard navigation (focus queries already scope to
                            // .context-menu-item, so no change to the nav code).
  busy?: boolean            // aria-busy on the menu root, for a menu that deliberately stays open
                            // while an activated row's write settles (§4.5).
```

The **check column belongs to the list, not the row**: it is reserved whenever *any* item in that
list declares `checked`, so a plain row in the same menu (`Manage identities…`) stays aligned with
the labelled ones instead of hanging 24px to its left.

`checked` uses `menuitemradio` rather than `menuitemcheckbox` because at most one identity is in
effect. The check column is a **glyph**, not a colour — the row's background is unchanged when
checked, so a checked row is legible in both themes and in high-contrast mode.

All four fields are additive: absent ⇒ byte-identical rendering to before.

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
model, and a second width escape hatch for a list a user edits perhaps twice a year — while
Settings, which already exists and is already reachable, is where every other rarely-edited list
lives. **Flagged for the orchestrator** (§12, A1) since the user's phrasing could also be read as
wanting CRUD fully out of Settings.

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
- **`configMissing` deep link.** `App.tsx` passes **both** `initialCategory='git-config'` and
  `configInitialFocus='identity'`. `SettingsShell` keys the deep link on a **monotonic request
  sequence**, not on an `open` transition, so a second deep link in the same session still lands
  (Settings already open, already on Git config, second failed commit). A request that names no
  category — the plain ⚙ click — only clears state and must never yank the user off the category
  they are reading. A deep link also clears any live query, so the result list cannot sit in front
  of the category it just selected.

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
| Help slot | `.settings-row-help-slot` holds the static help paragraph **and** the §13 draft hint. On slider rows (`.settings-row--slider`) it reserves `min-height: 18px` so nothing below the row moves when the hint replaces the help text mid-typing. Exactly one of the two is visible at a time. |

**Exactly one help line per row.** `.settings-row-help` is the static sentence, owned by the catalog.
A row whose explanation must track the live value uses `.settings-row-note` instead — identical
typography, same cell, emitted by the call site because the catalog is static data — and then carries
**no** catalog `help` at all. Never both, and everything visible in the help cell is in the control's
`aria-describedby`.

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
   knob *position* still distinguishes on from off while disabled. The dim lives on
   `.settings-row.is-disabled`, never on the `<fieldset>` (that would dim the sentence explaining
   the dim), and **never nests** — see `ui-reference.md` §2's dimming budget and §12.3.3.

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
- Disabled: container `opacity: .55`, inputs `disabled` — reset to `opacity: 1` inside
  `.settings-row.is-disabled`, on a selector that cannot tie on specificity (`:has()` takes its most
  specific argument's weight, so the naive override ties and the later rule wins).
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
| Searching, has matches `[P69l]` | `.is-hit` → label **and** count `--text-1`; count is the digit `N`, 11px tabular numerals, right-aligned, `aria-hidden` (the number reaches AT through the accessible name, §7.1) |
| Searching, no matches `[P69l]` | resting `--text-2`, count `0`. **Not dimmed, not disabled** — clicking it is how the user leaves the search |
| Disabled | never — no rail item is ever disabled. `git-config` with no repo shows the §1.2 empty block instead, which is more informative than a dead tab. |
| Long label | none of the 7 labels can overflow 200px − 16px padding − pill; still set `text-overflow: ellipsis; white-space: nowrap; min-width: 0` and a `title`. The count is `flex: none`, so it is never the thing that ellipsises |

`--text-1` on `--selection`: **9.4:1** dark / **13.3:1** light. `--text-2` on `--bg-1`: **7.3:1** /
**7.4:1**; `--text-1` on `--bg-1`: **13.5:1** / **15.4:1** (§2).

### 6.2 Pane

| State | Spec |
|---|---|
| Default | rows as §5.1 |
| Loading (Git config fetching) | three skeleton rows: `--bg-2` bars at 40% / 70% / 55% width, 12px tall, radius 6, `skeleton-pulse` 1.2s (§8). `aria-busy="true"` on the group. Under `prefers-reduced-motion` the pulse is suppressed (existing outstanding item — do **not** add a new animation that is not in the reduced-motion block). |
| Error (Git config read fails) | existing dismissible `.error-banner` at the top of the pane: `--danger` @12% bg, `--danger` text, 6px radius. Copy: `Couldn't read this repository's Git config.` + the backend message verbatim + a `Try again` button. |
| Empty (no repo, git-config) | §1.2 |
| Empty (no identities) | title `No identities yet`, body `Save the name and email you commit with, then switch between them from the toolbar.`, action `Add identity` |
| Empty (search, 0 hits) | §3.3 |
| Search results | §3.1: one `<section>` per hit category, `margin-top: 8px` between groups, header row `display:flex; align-items:center; gap:12px; margin-bottom:8px` (centre-aligned, not baseline, because `Go to` is a 24px target) |
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
| rail hit count `--text-1` / `--bg-1` `[P69l]` | 13.5:1 | 15.4:1 | 4.5:1 ✓ |
| rail hit count `--text-1` / `--selection` `[P69l]` | 9.4:1 | 13.3:1 | 4.5:1 ✓ |
| results group header `--text-2` / `--bg-0` `[P69l]` | 7.9:1 | 4.9:1 | 4.5:1 ✓ |
| identity ring `--warning` / `--bg-1` | 7.3:1 | 4.5:1 | 3:1 ✓ |
| help text `--text-2` / `--bg-0` | 7.9:1 | 4.9:1 | 4.5:1 ✓ |
| `--warning` 1px ring / `--bg-2` (§13) | 6.4:1 | 4.2:1 | 3:1 ✓ |

**Token-level finding `[P69l]`** — recorded here and in `ui-reference.md` §2 because it is not
P69-specific: `color: var(--accent)` on text is a house-wide pattern (~30 call sites) and is fine on
`--bg-0` / `--bg-1` / `--bg-2`, but it **fails the 4.5:1 text bar on a `--selection` fill in both
themes** (2.6:1 dark / 3.6:1 light — independently re-measured at 3.51 / 3.74 in the P69k pass; both
readings fail). Any accent-coloured *text* that can end up inside a selected row is therefore a
latent defect. Not audited in P69l; a candidate for the next a11y pass.

### 6.4 Motion

- Switch knob transform + track colour: 120ms ease-out.
- Segment background/border: 120ms ease-out.
- Reset `↺` opacity on hover: 120ms.
- §13's draft hint: opacity 120ms + input `border-color` 120ms, on appear only.
- **Nothing else animates.** Category changes are instant; the pane never fades; the card never
  animates in; search results appear without transition. Rationale: the Settings modal sits over a
  canvas that may be laying out 20k rows — any transition that repaints a large area competes with
  it for the frame.
- All of the above go into the existing `prefers-reduced-motion` block (`ui-reference.md` §9).

---

## 7. Accessibility & keyboard

### 7.1 Roles and names

| Element | Role / name |
|---|---|
| Card | `role="dialog" aria-modal="true" aria-labelledby="settings-title"` — the `<h2 id="settings-title">Settings</h2>` yields the same accessible name as today's `aria-label="Settings"`, so `getByRole('dialog', {name:'Settings'})` stays green. `aria-modal` is new. |
| Rail | `role="tablist" aria-orientation="vertical" aria-label="Settings categories"` |
| Rail item | `role="tab" aria-selected id="settings-tab-{id}" aria-controls="settings-pane"`, named by an explicit **`aria-label`** — see the template below |
| Divider | `<div aria-hidden="true">` — a presentational child of the tablist is transparent to AT, so the tablist still owns only tabs. |
| Pane | `role="tabpanel" id="settings-pane" tabindex="-1" aria-labelledby="settings-tab-{selected}"` |
| Search | `ListFilterInput` (`role="searchbox"`), `ariaLabel="Search settings"`, `placeholder="Search settings"` |
| Search results status | visually-hidden `role="status" aria-live="polite"` |
| Result group | `<section aria-labelledby="settings-results-{category}">` |
| Group | `<section aria-labelledby="{groupId}-title">` |
| Switch | native `checkbox`, labelled by the row label, `aria-describedby="{rowId}-help"` |
| Segmented | `role="radiogroup" aria-labelledby="{rowId}-label"`, native radios inside |
| Reset | `aria-label="Reset {label} to default"` |
| Identity trigger | `aria-haspopup="menu" aria-expanded` + `aria-label` per §4.2 |
| Identity menu rows | `role="menuitemradio" aria-checked` when `checked` is present |

**Rail accessible-name template `[P69l]`.**

```
{label}[, repository][, {n} match|matches]
```

- The `, repository` segment is present only on `git-config`, and it exists because the visible
  `repo` pill is `aria-hidden` — a purely visual pill leaves AT users without the qualifier.
- It is an explicit **`aria-label`**, not a visually-hidden sibling span: name computation joins
  sibling nodes with a space, which would produce `Git config , repository`.
- The `, {n} match` / `, {n} matches` suffix is present **only while a query is active** (counts
  non-null). Names outside a search are exactly what §11 froze.
- **This is deliberate, not a violation of the frozen-name rule.** The visible count is
  `aria-hidden`, so without the suffix an AT user gets the bare `12 settings match` status line and
  no per-category breakdown — they lose the entire signal the `--text-1` lift gives a sighted user.
  A changing accessible name on a control the user is *not* focused on is the lesser cost.
- **Warning for tests and for anyone querying the rail:** mid-search,
  `getByRole('tab', { name: 'Commit graph' })` **does not match** — it must be
  `getByRole('tab', { name: 'Commit graph, 5 matches' })` (or a regex). This already bit the e2e
  spec once. Prefer `name: /^Commit graph/` in any test that spans both states.

**Icon-only / repeated-label buttons `[P69l]`.** Where a surface has several buttons whose visible
text is the same verb, each gets an `aria-label` that **names the object**, prefixed with the visible
word so speech-input users can still say what they see, and it is **never derived from a sibling
node** (a name that depends on layout breaks the moment the row is re-ordered or the sibling is
truncated). The four shipped MCP names:

| Visible | `aria-label` |
|---|---|
| `Copy` | `Copy server URL` |
| `Copy` | `Copy bearer token` |
| `Copy` | `Copy command to register globally` |
| `Copy` | `Copy command to register for this repository` |

Visible text stays `Copy` — the MCP strings are frozen by §8.

### 7.2 Keyboard

| Key | Behaviour |
|---|---|
| `Ctrl/Cmd + ,` | Open Settings. **Verified free** (`App.tsx:790-838` binds only `Ctrl+O`, `Ctrl+Tab`, `Ctrl+W`, `?`). Must be registered **above** the `typing` guard at `App.tsx:821` so it works from the commit message box. No-op when Settings is already open (a shortcut that toggles a modal is surprising). |
| `Tab` `[P69l]` | Pure DOM order: close ✕ → search → rail (one stop) → pane content. No `tabindex` juggling. **Not trapped** — see the focus note below. |
| `Shift+Tab` | Reverse. |
| `↑` / `↓` in the rail `[P69l]` | **Move focus only** (manual activation, D11). Wraps. `Enter`/`Space` selects the focused category. |
| `Home` / `End` in the rail | Move focus to first / last category. |
| `→` from the rail | Move focus into the pane (`pane.focus()`), the tablist convention. |
| `Space` on a switch | Native toggle. |
| `←` / `→` in a segmented group | Native radio navigation. |
| `Esc` | 1st press: clears a non-empty search (existing `ListFilterInput` capture-phase handler). 2nd press: closes Settings (existing App overlay-Esc effect). |
| `Enter` | Activates the focused control. Never closes the dialog — there is no primary "OK"; settings apply live. |
| Backdrop mousedown | Closes (existing behaviour, unchanged). |

**Manual activation, and why `[P69l]`.** Automatic activation (the APG's default for cheap panels)
would fire a `getConfig` IPC round-trip every time focus passed *over* `Git config` on the way to
`About`. Arrow-browsing a rail must not perform I/O. The roving tabindex therefore follows **focus**,
not selection — binding it to the selected id would throw the arrow position away every time the
user tabbed out and back.

**Focus restore, and the absence of a trap `[P69l]`.** `SettingsShell` stores
`document.activeElement` when it mounts and restores it on unmount, falling back to the
`.settings-toggle` button and then `<body>`. There is **no focus trap**: this codebase has no shared
trap utility and no other dialog has one (architect's shell contract D-4), so adding one to Settings
alone would create an inconsistency, and a wrong-by-half trap is worse than none. Anyone adding one
must add it to every dialog in the same pass. Initial focus goes to the **search input** — a text
field, so no accidental activation, and the fastest route for a user who knows the setting's name.
A **deep-linked** open (`initialCategory` given, or `configInitialFocus === 'identity'`) sets no
initial focus of its own: it leaves focus placement to the target section's own effect, which focuses
`user.name`. A search box that grabbed focus there would silently defeat the commit-error linkage.

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

| Control | Size |
|---|---|
| Rail item | 32px tall, full rail width |
| Switch | 36×24 (the invisible `<input>` *is* the target) |
| Segment | ≥24px tall |
| Reset `↺` | 24×24 |
| Close ✕ | 32×32 |
| Identity trigger | 32×32 |
| `Go to {Category}` `[P69l]` | `min-height: 24px; padding: 0 6px; margin-right: -6px`, `display: inline-flex; align-items: center`. It shipped as a 96×16px text button — below the floor. The negative margin keeps the *text* flush with the pane's right edge while the padding grows the box, so the fix costs no alignment (`ui-reference.md` §3.1: the box grows, the glyph does not). `:focus-visible` 2px `--accent`, 1px offset, `border-radius: 4px`. |

All ≥24px in both densities.

---

## 8. Microcopy pass

**FROZEN — do not touch** (P68g security pass, `docs/contracts/P68-security-audit.md`): every string
in `SettingsMcpSection.tsx`, the consent-facing strings in `SettingsAiSection.tsx`, and all three
consent `ConfirmDialog` bodies. Where those strings sit inside a redesigned row, the *layout*
changes and the *words* do not. Adding an `aria-label` to an icon-only button in a frozen section is
not a string change (§7.1) — the visible words are untouched.

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
| Search `[P69l]` | — | status line `{n} settings match` / `1 setting matches` / `No settings match`; results header `Go to {Category}`; zero-match block per §3.3 | one sentence shape across all three counts |

---

## 9. Component decomposition

New directory `src/components/settings/`. Every file is a single responsibility, well under 500.

### 9.1 Shell and primitives

| File | ~LOC | Responsibility |
|---|---|---|
| `settings/SettingsShell.tsx` | 190 | Overlay + card grid, focus **restore** (no trap — D11), category state (seeded from `initialCategory` via the request-sequence rule), search query state, results-vs-category switch, scroll reset. |
| `settings/SettingsRail.tsx` | 145 | `role="tablist"`, roving tabindex on focus, arrows/Home/End with **manual activation**, dividers, per-category match counts, the `repo` pill, the §7.1 name template. |
| `settings/SettingsSearchBar.tsx` | 60 | Wraps `ListFilterInput`; owns the placeholder/label and the live-region status text. |
| `settings/SettingsResults.tsx` | 130 | Cross-category result groups, `Go to {Category}` buttons, the `SettingsSearchContext` provider, zero-match. |
| `settings/SettingsSearchContext.ts` | 25 | `{ terms, visible, category }` — the context every row self-filters against. |
| `settings/settingsHighlight.tsx` | 65 | `highlightTerms` — merged non-overlapping ranges → `<mark class="settings-match">`. Pure, testable, no component state. |
| `settings/SettingsRow.tsx` | 160 | The §5.1 anatomy: stacked variant, conditional `↺`, help slot, label highlighting, self-filtering. |
| `settings/SettingsSwitch.tsx` | 55 | §5.5. Native checkbox inside. |
| `settings/SettingsSegmented.tsx` | 70 | §5.6. Native radios inside. |
| `settings/SettingsGroup.tsx` | 45 | Group title + hairline-separated children; hides itself when no child survived a search. |
| `settings/SettingsPaneHeader.tsx` | 50 | Category title, subtitle, optional trailing slot (the Git config scope switch). |
| `settings/SettingsEmpty.tsx` | 50 | The three in-pane empty variants (no repo / no identities / no search hits). |
| `settings/settingsCatalog.ts` + `settings/catalog/*.ts` | 90 + 6 files | The row catalog and the search index — one source for label / help / keywords / reset / availability (§3.4). Pure data plus `searchSettings`. |
| `settings/settingsAvailability.ts` | 40 | `isRowAvailable` — the renderable-rows gate the search shares with the pages. |
| `settings/types.ts` | 40 | `SettingsCategoryId`, `SettingsCategory`, `SettingsIndexEntry`, `SettingsRowId`. |

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
| `SettingsGitConfigSection.tsx` | **436** | `level` state + the Level row move out to `GitConfigCategory`; the `<details>` Advanced block moves to `settings/GitConfigAdvanced.tsx` (~260, incl. the §3.1 self-filter). → ~230. **This is the required split — the file may not reach 500.** |
| `SettingsProfilesSection.tsx` | 281 | Per-profile card extracted to `settings/IdentityProfileCard.tsx` (~160); the local-only match logic is deleted in favour of `useEffectiveIdentity`. → ~140. |
| `SettingsAppearanceSection.tsx` | 71 | **Deleted**, replaced by `AppearanceCategory.tsx`. |
| `SettingsGraphSection.tsx` | — | Re-skinned: checkboxes → `SettingsSwitch`, radios → `SettingsSegmented`, rows → `SettingsRow`. **Labels frozen.** |
| `SettingsAiSection.tsx` | — | Re-skinned; radio group stays a radio group; **strings frozen**. |
| `SettingsAiRunSection.tsx` / `SettingsAiLimits.tsx` | — | Re-skinned; `Repository access` → segmented; gate note moves to the top. |
| `SettingsMcpSection.tsx` | — | Re-skinned (2 checkboxes → switches, rows → `SettingsRow` stacked). **Strings frozen**; the four `Copy` buttons gain object-naming `aria-label`s (§7.1). |
| `SettingsUpdatesSection.tsx` | — | Re-skinned (1 checkbox → switch). |
| `SettingsHooksToggle.tsx` | — | Re-skinned (1 checkbox → switch). |
| `App.tsx` | 1114 | `-50` (toolbar extraction) `+1` (`<HeaderToolbar>`) `+~14` (Ctrl/Cmd+, binding, two palette entries, `initialCategory` on the configMissing path). **Net ≈ −35 → still under the ratchet.** |
| `ContextMenu.tsx` | 321 | +4 additive fields (§4.4), ~+40 lines. |
| `src/styles/settings-shell.css` / `settings-primitives.css` | — | The shell (card grid, rail, search, results, pane, scope) and the primitives (row, switch, segmented, text, reset, hint). Split out of `styles.css` in P69g — do not add settings CSS back to `styles.css`. |

**Honest answer to "which of the 8 existing sections re-parent unmodified": none.** Every one carries
`.settings-checkbox` or `btn-secondary` markup that this contract replaces. The two with the smallest
diff are `SettingsUpdatesSection` (one checkbox) and `SettingsHooksToggle` (one checkbox). Pretending
otherwise would produce a redesign with two visual languages in one dialog.

### 9.5 Increments as shipped

- **A — shell** (P69a/b/g): `SettingsShell` + rail + primitives + General/Appearance/About.
- **B — repo scope** (P69h): `GitConfigCategory`, the `SettingsGitConfigSection` split,
  `GitConfigAdvanced`, the `configMissing` deep link, `Ctrl/Cmd+,`.
- **C — identity** (P69i/j): `useEffectiveIdentity`, `HeaderToolbar`, `IdentityMenu`,
  `IdentityAvatar`, the `ContextMenu` extensions, `IdentitiesCategory`, the profiles split, palette
  entries.
- **D — search** (P69k): `SettingsSearchBar`, `SettingsResults`, `SettingsSearchContext`,
  `settingsHighlight`, rail counts, the self-filter, `searchSettings`.
- P69c/d/e/f are the numeric-input and catalog-coverage passes; §13 is P69c's outstanding half.

---

## 10. Harness states

`pnpm dev` with `VITE_MOCK_IPC=1`. **The harness is headless: `requestAnimationFrame` never fires
and screenshots fail outright.** Everything below is verified by `read_page` / `get_page_text` /
a batched `javascript_tool` computed-style read.

| Fixture | Exists? | Verifies |
|---|---|---|
| default (`?` none) | yes | Shell geometry, rail, all 7 categories, search, both themes (`resize_window` colorScheme). Identity **state 2**: effective = global `Mock Fixture User <fixture@bonsai.dev>` (`fixtures/config.ts:47-53`), profiles `Work`/`Personal` (`mock/persistence.ts:118-132`) → no checked row, `Save “Mock Fixture User” as an identity…` present. |
| `?fixture=noconfig` | yes | Identity **state 3** (`?` glyph, warning ring, `Set an identity…`), and the `configMissing` deep link landing on Git config → Identity with the rail selecting `git-config`. |
| `?fixture=identitymatch` | yes | Identity **state 1**: `local` `user.name = Ada Lovelace`, `user.email = work@bonsai.dev` **and** a matching profile → one `menuitemradio` with `aria-checked="true"`, source line `From this repository's config`, and applying the *other* profile firing the §4.5 confirm. |
| no repo open (close all tabs) | yes | Identity **state 4** (trigger absent), Settings → Git config showing the §1.2 empty block, `app.gitConfig` palette entry disabled, and **no git-config rows in any search result** (§3.1 availability gate). |
| `?fixture=slowconfig` | yes | Git config skeleton rows, `aria-busy`, and the identity trigger's `·` loading circle + `aria-label="Reading commit identity…"`. |
| `?fixture=configerror` | yes | Git config error banner with `Try again`; identity trigger falls back to state-3 chrome with the distinct "couldn't read" title. |
| `?fixture=longsettings` | yes | Pathological content: a profile with a 120-char label and a 90-char email; `terminalCommand` of 300 chars; a repo workdir path of 200 chars; a custom config key `a.very.long.section.key.name…`. Verifies ellipsis + `title` in the menu rows, the card titles, the scope line, and that no row grows the card. |
| MCP running / stopped | yes | AI → AI access rows in both states. |
| `?update=` seam | yes | About → version result line. |
| profiles = `[]` | via `localStorage bonsai.mockUiSettings` | Identities empty state + `Add an identity…` menu row. |
| `aiConsented: true` seed | known harness step | AI category with the fieldset **enabled** — otherwise the ten run knobs are only visible disabled. |

**Search-specific checks `[P69l]`** (no new fixture needed):

| Check | How |
|---|---|
| Rail emphasis + counts | Type `graph`; read `.settings-rail-item.is-hit` count and each `.settings-rail-count` text. Expect a non-zero digit on hit items, `0` elsewhere, and **no** `opacity` other than `1` on any item. |
| Rail names mid-search | `read_page` the tablist and confirm every tab name ends in `, N match` / `, N matches`. |
| Results header colour | Batched computed-style read of `.settings-results-title` → `--text-2`'s value, in both `colorScheme`s. |
| Go-to hit target | `getBoundingClientRect().height` of `.settings-results-goto` ≥ 24. |
| Availability gate | Close all repos, search `hooks` → no `git-config` group and no count on the Git config tab. |
| Advanced disclosure | Search `custom` with a repo open → the `Advanced` `<details>` has `open`, and only the matching block renders. |
| Per-entry counts | With 3 identities, search `user.name` → rail count `1`, three rendered rows. |
| Highlight (after §3.2.1) | `document.querySelectorAll('mark.settings-match').length` for `graph` — expect 0 today, ≥5 once the fallback lands. |

### USER CHECKPOINT (not AI-verifiable)

1. **Any visual proof at all** — screenshots fail in this harness. The two-pane geometry, the switch
   and segmented rendering, and both themes must be eyeballed in `pnpm tauri dev`.
2. `Ctrl/Cmd + ,` on macOS — the binding is testable in jsdom, the absence of an OS/webview conflict
   is not.
3. Applying an identity actually writing `.git/config` (the mock writes to memory only).
4. Pane scroll feel and focus-ring rendering (rAF paused).
5. `:has()` support in the shipped WebView2 / WebKitGTK versions — used by the switch and segment
   focus rules. Provide a `:focus-within` fallback if the native check fails.
6. `[P69l]` **Whether the rail's `--text-2` → `--text-1` hit emphasis reads at a glance.** If it does
   not, the fix is to render nothing instead of `0` on zero-count items (§3.2) — not a hue.
7. `[P69l]` §13's 600 ms feel, its ring weight, and its 120 ms fade (rAF paused).

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
- **One documented exception `[P69l]`:** rail tab names carry a match-count suffix while a query is
  active (§7.1). Query the rail with a prefix regex if a test spans both states.

---

## 12. Flagged ambiguities — orchestrator decisions

**A1 — Where identity CRUD lives.** The user asked to "extract the profiles from settings page".
Switching definitively moves to the header. CRUD is my call and I put it in **Settings →
Identities** (D5): one modal type, one focus model, reuses the deep-link mechanism P69 needs anyway,
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

**A7 — Increment sizing.** §9.5 records what shipped: four functional passes plus the numeric and
catalog-coverage passes.

**A8 `[P69l]` — Two open items, both specced and unimplemented.** §3.2.1 (help-text highlight
fallback) and §13 (draft-divergence hint behaviour: the classifier, the hook and the
`NumberSlider`/USD-field wiring; the CSS, the reserved slot and `SettingsRow`'s `hint` prop already
shipped). Both are small, independent, and safe to land in either order.
**Recommendation: one follow-up increment carrying both.**

**A9 `[P69l]` — Accent-as-text on a selection fill.** §6.3's token-level finding: ~30 `color:
var(--accent)` text sites app-wide, and the token fails AA over `--selection` in both themes.
**Recommendation: a scoped a11y sweep in a later pass** — not P69's to fix, but it should not be
discovered a third time.

---

## 13. Draft feedback for out-of-range / blank slider entries

> Folded in from `docs/contracts/P69c-draft-feedback-ui.md` (P69l); that file is now a pointer stub.
> `[NOT IMPLEMENTED]` — what shipped is the CSS (`.settings-draft-hint`), the reserved 18px help
> slot on `.settings-row--slider`, and `SettingsRow`'s `hint` prop. The classifier, the hook, the
> `aria-invalid` wiring and the copy below are still to be built (§12 A8).
>
> The input model is **settled**: draft display + clamped commit per keystroke stays exactly as
> committed (`src/components/NumberSlider.tsx`, `P69-settings-shell.md` OQ-1). Nothing here reverts
> to snap-back or to commit-on-blur.

### 13.1 The problem, stated exactly

Since P69c the number input shows a **draft string** while it is focused, and the setting holds the
**clamped, rounded** commit of that string. Five drafts diverge from the setting:

| Draft class | Example (Row height, 24–48 px, currently 32) | Setting holds | Detect |
|---|---|---|---|
| **above** | `128` | `48` | numeric, `n > max` |
| **below** | `6` | `24` | numeric, `n < min` |
| **rounded** | `24.7` | `25` | numeric, in range, `Math.round(n) !== n` |
| **blank** | `` (cleared) | `32` — unchanged, by design (`P68g §6.1` acceptance 5, pinned by test) | `raw.trim() === ''` |
| **not a number** | `-`, `e`, `.` | `32` — unchanged | `Number.isNaN(Number(raw))` |

Today all five render identically to a valid draft: default border, no text, nothing announced. The
field silently contradicts the setting until blur. That is the gap.

**Not a divergence, and must not warn:** `06` (Number 6 === value 6), `6e1` (=== 60), `32.0`, or any
draft whose numeric value already equals the committed value. Compare **numerically**, never by
string, or leading zeros produce a hint on every normal edit.

**Canonical predicate.** One derived value drives border, hint, `aria-invalid`, and the timer:

```
kind = null | 'above' | 'below' | 'rounded' | 'blank'    // 'blank' also covers not-a-number
```

`null` whenever `draft === null` (not editing), or the draft is numeric and `Number(draft) === value`.

### 13.2 The affordance

**Chosen: a `--warning` inner ring on the number input + one line of plain text in the row's help
slot.** The two are a single state — neither ever renders without the other, so colour is never the
carrier (`ui-reference.md` §7; the A/M/D/U/R badges are the precedent). The sentence is the
non-colour carrier and it is strictly more informative than a glyph would be.

**Rejected alternatives.** A `--danger` treatment: nothing is broken and nothing will be lost — the
setting is already at a legal value — so `--danger` overstates it and would collide with the error
language used for failed operations. An icon-only marker: a 56px-wide input has no room, and a bare
glyph cannot say *what the value will become*, which is the whole point. A tooltip: invisible to
keyboard users mid-typing, and hover is the wrong trigger for a typing state. No extra glyph goes in
front of the sentence — a warning triangle before a 12px line under a 56px input is noise, and the
border already carries the "look here".

#### 13.2.1 Geometry

| Property | Value |
|---|---|
| Hint element | `<p class="settings-draft-hint" id="{id}-draft" aria-live="polite">`, **always rendered**, empty when `kind === null`. Passed to `SettingsRow` as `hint` |
| Type | 12px / `line-height: 16px`, `color: var(--text-2)`, `margin: 2px 0 0` — identical to settings help text (§5.1), deliberately |
| Slot | The row's help cell. `.settings-row--slider > .settings-row-help-slot { min-height: 18px }` for every slider row, so nothing below the row moves while typing |
| Help text | Hidden while the hint is non-empty — `.settings-row-help-slot:has(.settings-draft-hint:not(:empty)) > .settings-row-help { display: none }`. One line occupies the slot at a time |
| Input ring | `.settings-number[aria-invalid='true'] { border-color: var(--warning); box-shadow: inset 0 0 0 1px var(--warning); }` — reads as 2px with **no box-model change**, so the 56×28 field does not resize |
| Ordering | That rule must be declared **after** `.settings-number:focus` (equal specificity, source order decides). The field is always focused when a draft exists, so the warning must beat the focus border recolour — see §13.7 N1 |
| Wrapping | Hint strings are bounded (longest = `Too high — will be set to 86400 seconds.`, 40 chars) and always fit one line in the 56ch help slot. No wrap or ellipsis handling needed |

#### 13.2.2 Contrast — both themes

| Pair | Dark | Light | Bar |
|---|---|---|---|
| `--warning` ring vs `--bg-2` (input fill) | **6.4:1** | **4.2:1** | ≥3:1 graphics ✔ |
| `--warning` ring vs `--bg-0` (pane) | **7.9:1** | **4.9:1** | ≥3:1 ✔ |
| Hint `--text-2` on `--bg-0` | **7.9:1** | **4.9:1** | ≥4.5:1 text ✔ |

Measured 2026-08-19. The hint is **not** `--warning` text: §2 bars hue-as-text over its own tint, and
`--text-2` is what every other settings hint uses — consistency beats a second signal.

### 13.3 Timing — the crux

> **One-sentence rule: an above-maximum draft warns on the keystroke that causes it; every other
> divergence warns only after 600 ms with no further keystroke in the field.**

| Kind | Delay | Why |
|---|---|---|
| `above` | **0 ms — immediate** | For a non-negative integer field, appending a digit is monotonically increasing. Once the draft exceeds `max`, **no continuation of typing can make it valid** — only deleting or replacing can, and both clear the state at once. It is a terminal error, not a transient one, so there is nothing to wait for and waiting only delays the truth. |
| `below` | **600 ms idle** | Every valid entry passes through below-minimum prefixes: with `min = 24`, `30` passes through `3`; with `min = 60`, `600` passes through `6` and `60`. Warning on those punishes ordinary typing — exactly the failure this spec must not create. |
| `rounded` | **600 ms idle** | `24.` is a normal step toward `24.7`, and `24.7` toward `24.75` — the user is mid-number. |
| `blank` | **600 ms idle** | Select-all-then-type passes through empty on nearly every re-entry. An instant "Empty" on every edit would be the loudest possible false positive. |

**Why 600 ms.** Comfortable numeric typing runs 150–250 ms per digit; 600 ms is >2× that, so
uninterrupted entry never trips it, while a user who has stopped sees the hint ~0.6 s later — well
inside the ~1 s+ it takes to look away. It is also twice the app's established 300 ms `notify`
debounce, so it reads as the same family of "the user has stopped" delay. Ship it as one named
constant (`DRAFT_HINT_DELAY_MS`); anything under ~400 ms starts catching ordinary two-digit entry.

**Timer rules.**

1. The timer is **re-armed on every keystroke** in the number input (`onChange`), and only there.
2. It is **cleared** on blur, on `Enter`, on the draft becoming non-divergent, on the range input
   being used, on the external-value resync (`NumberSlider` rule (c)), and on unmount.
3. **Once visible, it stays visible** while any divergence holds, and changes text immediately if
   the kind changes (`below` → `above` when a digit is appended). The delay is armed once per entry
   into the warning state, **never re-armed while already showing** — a hint that flickered off and
   on per keystroke would be worse than no hint.
4. Going from divergent to valid **hides instantly** (no delay, no fade-out). A stale warning is a
   lie; hiding is never the annoying direction.
5. `above` fires immediately even if a `below`/`blank` timer is pending: the pending timer is
   cleared and the hint shows.
6. Blur/Enter drop the draft (existing P69c behaviour) and therefore the hint, in the same render.
   **The affordance can never outlive the draft it describes** — this is why no "stranded warning"
   state exists and why nothing needs to be reconciled on close.

#### 13.3.1 Motion

`transition: opacity 120ms ease-out` on the hint and `border-color 120ms ease-out` on the input, on
appear only; hide is instant. Opacity + a 1px border colour on a 56px box — no layout, no transform,
nothing that can contend with the canvas render budget. Both go to `none` inside the existing
`prefers-reduced-motion` block (`ui-reference.md` §9); the hint then appears instantly, which is
correct, not degraded.

### 13.4 Microcopy — implement verbatim

`unitSuffix = unit ? ' ' + unit : ''`. `value` is the prop, i.e. exactly what the field will show
when the user leaves it.

| Kind | String | Example (Row height, 24–48 px, at 32) |
|---|---|---|
| `above` | `Too high — will be set to {max}{unitSuffix}.` | `Too high — will be set to 48 px.` |
| `below` | `Too low — will be set to {min}{unitSuffix}.` | `Too low — will be set to 24 px.` |
| `rounded` | `Whole numbers only — will be set to {value}{unitSuffix}.` | `Whole numbers only — will be set to 25 px.` |
| `blank` (empty **or** not a number) | `No value — stays at {value}{unitSuffix}.` | `No value — stays at 32 px.` |

Each string says what is wrong *and* what the value will be, in that order, in ≤40 characters.
Sentence case, em-dash, no jargon, no libgit2 or DOM vocabulary, and no subject — naming the setting
would produce `Fetch every will be 300 seconds.`, which is worse than saying nothing. The row label
sits 16px above the sentence and is the control's accessible name, so context is never missing.

Empty and not-a-number share one string deliberately: to the user both are "I have not typed a
number yet", and `type="number"` blanks most non-numeric input anyway, so the distinction is
invisible far more often than it is real.

### 13.5 States — both themes, both densities

One geometry in `cozy` and `compact` (D10). Every row below is identical in both: hint 12px/16px,
slot 18px, input 56×28, ring 1px inset.

| State | Number input | Hint slot | `aria-invalid` |
|---|---|---|---|
| Default (no draft) | `--border` 1px, `--bg-2` fill | help text, `--text-2` | absent |
| Focused, draft valid | current focus treatment (see N1) | help text | absent |
| Focused, draft divergent, timer pending | unchanged from focused | help text (unchanged) | absent |
| Focused, draft divergent, showing | `--warning` border + inset ring; **focus outline unchanged** | hint sentence, help hidden | `true` |
| Hover | hover border, if any | unchanged | — |
| Hover **+** showing | warning ring **wins** over hover and over the focus border recolour | hint | `true` |
| Disabled | wrapper `.is-disabled` dim, inputs disabled | help text | absent |
| Blank draft, showing | warning ring; the field itself is visibly empty — that emptiness is its own second carrier | `No value — stays at 32 px.` | `true` |
| Long draft (`999999999999`) | value scrolls inside the 56px field (native `type=number`); ring unchanged | `Too high — will be set to 48 px.` | `true` |

**Disabled transition.** If `disabled` flips true while a hint is showing (the AI fieldset gate,
§5.4), the draft, the timer, and the hint are all dropped in that render. A disabled control must
never display a warning the user cannot act on.

**Theme parity.** Identical treatment in dark and light; only the two token values change. The ring
clears 3:1 in both (§13.2.2) and the hint clears 4.5:1 in both.

### 13.6 Accessibility

| Concern | Spec |
|---|---|
| Announcement | The hint `<p>` is **permanently in the DOM** with `aria-live="polite"` and empty text when idle, so the live region is registered before content ever arrives. Never `display:none` the hint element — hide the **help** paragraph instead. |
| Why a live region is safe here | It only ever fires when the user has stopped typing for 600 ms, or on the single terminal above-max keystroke. Rule 3 (no re-arm while showing) means the text does not change per keystroke, so there is no repeat chatter. |
| `aria-invalid` | `true` exactly when the hint is showing; **removed**, not `"false"`, otherwise. It is the single flag driving CSS, so the visual and the semantic can never disagree. |
| `aria-describedby` | **Composes, never clobbers**: `[describedBy, '{id}-draft'].filter(Boolean).join(' ')`, help id first so the explanation is announced before the warning. The existing `describedBy` prop (`P68g §1.6`) keeps working unchanged, including when it carries several ids. When the help paragraph is hidden it drops out of the accessible description on its own — no conditional id juggling. |
| Frozen surface | Input `id` (`#settings-graph-row`) and the range's `aria-label={label}` (`Row height`) are **untouched**. No role changes. `getByLabelText('Row height')` and every existing query still resolve. |
| Hit targets | Nothing new is interactive. The hint is text; it is not focusable, not clickable, and adds no tab stop. |
| Focus ring | The warning uses `border-color` + `inset box-shadow` only. The `:focus-visible` **outline** (2px `--accent`, 1px offset) is never overridden, so a keyboard user keeps the ring at all times. |
| Colour alone | Never. Ring and sentence are one state; the blank case additionally shows a visibly empty field. |
| Command palette | No entry. This is feedback, not an action. |

### 13.7 Component decomposition

| File | New/changed | Contents | Est. |
|---|---|---|---|
| `src/settings/draftHint.ts` | **new** | Pure, no React: `classifyDraft({draft, value, min, max, integer})` → `kind`, and `draftHintText(kind, {min, max, value, unit})` → the §13.4 string. Directly unit-testable, and the strings live in one place. | ~55 |
| `src/settings/useDraftHint.ts` | **new** | The hook: owns the 600 ms timer, the "already showing" latch, and cleanup. Returns `{ kind, text, ariaInvalid, describedBy, onDraftInput(raw), clear() }`. | ~70 |
| `src/components/NumberSlider.tsx` | changed | Calls the hook from the existing `onChange`/`onBlur`/`onKeyDown`/resync paths, spreads `aria-invalid` + composed `aria-describedby`, renders the hint `<p>` into `SettingsRow`'s `hint` prop. | +~20 |
| `src/components/SettingsAiLimits.tsx` | changed | The hand-rolled USD field calls the **same** hook — see §13.8. | +~12 |
| `src/styles/settings-primitives.css` | **already shipped** | `.settings-draft-hint`, the `:has()` help-hiding rule, the slider slot's `min-height`. Still missing: `.settings-number[aria-invalid='true']` and the reduced-motion additions. | +~10 |

`src/settings/` already exists (`ranges.ts`), so no new directory. The classifier is deliberately
outside `src/components/` so a non-component (`SettingsAiLimits`'s USD input) can import it without
depending on a component module.

**N1 — fix while you are here.** `.settings-number:focus` currently only recolours its border
(`P68g §7-N1`). Since the field is *always* focused when a draft exists, that focus border and this
warning border compete for the same 1px and the order of two equal-specificity rules becomes
load-bearing. Replace it with a real `:focus-visible` outline (2px `--accent`, offset 1px, per §2)
and the conflict disappears structurally.

### 13.8 Scope

Applies to **every settings slider**, with no per-call-site opt-out: Commit graph → Geometry (commit
node size, row height, lane width), General → Background activity (`Fetch every`, `Refresh every`),
AI → Runs → Limits (idle timeout, hard cap, replies per run), AI → Runs → Bulk resolve (batch size).
Eight controls, one behaviour.

**`SettingsAiLimits`'s USD field: yes, the same treatment.** It hand-rolls its own draft today
(`P68g §1.3` control 7 — `NumberSlider` rounds to integers, so USD could not use it) and is
scheduled to fold onto `NumberSlider` later. It has the same divergence, so it gets the same hook
now; folding it later then removes code instead of needing a second pass. Two adjustments:

- Its bounds are 0.5–100 USD, so `above`/`below`/`blank` apply verbatim
  (`Too high — will be set to 100 USD.`).
- The `rounded` kind is **not** enabled for it: it is a `step 0.5` decimal field, so `integer: false`
  is passed and fractional drafts are legal. Do **not** invent a step-rounding message — if the
  control ever starts snapping to the step, that is a separate spec.

**Not in scope:** the range inputs (they cannot produce an invalid value), text fields, path fields,
and the Git-config identity fields. Free-text validation is a different problem with a different
answer, and P69 has no requirement for it.

### 13.9 Harness verification

`pnpm dev` with `VITE_MOCK_IPC=1`. **No new fixtures are required** — the affordance is produced by
typing, so the default fixture reaches it. The harness is headless: `requestAnimationFrame` is
paused and screenshots **fail outright**, but `setTimeout` runs normally, so the 600 ms timer is
genuinely exercisable.

| Check | How |
|---|---|
| `above` is immediate | Settings → Commit graph, click `#settings-graph-row`, type `128`, read `#settings-graph-row-draft` at once |
| `below` waits | Clear the field, type `6`, read immediately (expect empty), `computer{wait 1}`, read again (expect `Too low — will be set to 24 px.`) |
| Normal typing never warns | Select-all, type `30` at speed, read immediately — hint must be empty and `aria-invalid` absent |
| `blank` | Select-all, Delete, wait 1s → `No value — stays at 32 px.`, and the setting is unchanged |
| Blur clears everything | Tab away → hint empty, `aria-invalid` gone, field shows the clamped value |
| `aria-describedby` composes | Read the attribute on a slider that already has help text: expect `"{help-id} {id}-draft"`, both ids resolving |
| AI limit sliders | Seed `localStorage bonsai.mockUiSettings` with `aiConsented: true` and reload, otherwise the ten run knobs are disabled and cannot hold a draft |
| Light theme | `resize_window` `colorScheme: 'light'`, batched `javascript_tool` read of the computed `border-color` on the invalid input |

**USER CHECKPOINT.** Whether 600 ms feels right; the ring weight and whether `--warning` reads as
"check this" rather than "error", in both themes; the 120 ms fade (rAF paused); and native
IME/numeric-keypad entry on macOS and Windows (confirm composition does not fire a spurious blank
draft).
