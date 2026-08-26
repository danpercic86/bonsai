# spec-003 — Graph Declutter Modes — UI Contract

Companion to `docs/specs/003-graph-declutter/spec.md` + `plan.md`. Backend contract (locked by the
plan): wire type `GraphFilter { firstParent: boolean; seedRefs: string[] | null }`, streamed
`meta.filtered?: boolean`, persisted globally like `graphStyle`. This contract covers everything the
user sees: the inline filter control + "filtered" indicator, the sidebar solo/hide interaction, the
Settings rows, and all states in both themes.

**No new theme tokens.** Every colour below is an existing `src/styles.css` custom property, and
every recipe is an existing canon: §11 pills, §12.3.1 switch, §12.2 rows, the `.graph-search-fab`
placement precedent, the ContextMenu idiom. No `ui-reference.md` change is required (nothing new
enters the canon; this file is the full spec). The graph pane is **outside both density scopes**
(ui-reference §3) — one geometry in cozy and compact.

---

## 1. Frontend filter model (intent, not wire shape)

The wire `seedRefs` whitelist cannot honestly represent "hide these 3 branches" (a persisted
whitelist silently hides branches created later). The UI therefore keeps an **intent** object and
derives the wire whitelist at request time:

```ts
// persisted UI setting (replaces the plan's bare `graphSeedRefs`; graphFirstParent unchanged)
type GraphRefFilter = { mode: 'solo' | 'hide'; refs: string[] } | null; // full ref names
```

- `mode: 'solo'` → `seedRefs = refs` (backend adds HEAD's ancestry itself).
- `mode: 'hide'` → `seedRefs = (all currently known refs) − refs`, computed per request from the
  same ref list the sidebar renders. New branches created while a hide set is active thus appear.
- `null` and `refs: []` → `seedRefs = null` (full graph).
- Modes are **exclusive**: starting a solo replaces a hide set and vice versa (see §3.1 menu rules).

> **FLAG (orchestrator/architect):** this changes the persisted setting from the plan's
> `graphSeedRefs: string[] | null` to `graphRefFilter: GraphRefFilter` (settings shape only — the
> IPC `GraphFilter` is untouched; derivation is frontend). **Recommended.** Fallback if rejected:
> persist the raw whitelist and the chip label degrades to "Showing N refs" — hide mode then
> mis-persists across ref churn; I consider that a defect worth the schema change.

> **FLAG 2 (architect, stale honesty):** with only `meta.filtered: boolean`, the stale-fallback
> case ("your saved refs no longer exist, showing full graph") is detectable **only** when
> `firstParent` is off (requested refs + `filtered === false`). With `firstParent` on, or with a
> partially stale set, the flag reads `true` and the UI cannot tell. **Recommend** an additive
> `meta.seedRefsApplied?: boolean` ("the seed-ref restriction took effect"). §2.2 specs both the
> with-field and without-field behaviour so implementation is unblocked either way.

**New files** (container/presentational split, all well under 500 lines):

- `src/hooks/useGraphFilter.ts` — intent state, hydrate/persist via `useUiSettings`, derived
  `GraphFilter` (memoized), actions: `toggleFirstParent()`, `soloRef(ref)`, `addToSolo(ref)`,
  `hideRef(ref)`, `unfilterRef(ref)`, `clearAll()`; exposes `activeSummary` (chip label string).
- `src/components/GraphFilterChip.tsx` — the fab/chip control + stale variant (presentational).
- `src/components/GraphFilterPopover.tsx` — the anchored popover (presentational).
- `src/components/settings/SettingsGraphDeclutterSection.tsx` — the two Settings rows, composed by
  `GraphCategory` next to `SettingsGraphSection` (never appended into an existing large file).

Wiring goes through `WorkspaceGraphPane.tsx` (two new rendered children + props) and the existing
sidebar context-menu builders; sidebar rows gain a trailing marker glyph (§3.3).

---

## 2. Inline control + indicator: one element, two states

One control answers both scope items (quick toggle + filtered indicator): a **filter chip** in the
graph pane's top-right fab cluster, beside the existing `.graph-search-fab`.

```
                          graph pane, top-right
      ┌ inactive ┐   ┌ active ─────────────────────┐  ┌ search fab ┐
      │  ⛛ (icon) │   │ ⛛ First-parent · Solo: main ✕│  │     ⌕      │
      └───────────┘   └──────────────────────────────┘  └────────────┘
```

- **Placement.** Absolutely positioned in `.graph-pane`, same top offset as `.graph-search-fab`,
  sitting **8px to its left** (the cluster grows leftward). Above the canvas, below dialogs.
- **Visibility gate — deliberately wider than the search fab's.** Hidden only when
  `graph === null`, `head.unborn`, or `anyOverlayOpen` (diff/blame/history/reflog/AI/diff-browser
  cover the whole graph, so no indicator is owed). While the CommitSearchBar or the Ask-history
  panel is open the graph stays visible below them, so **the chip stays rendered** (AC5) — its top
  offset shifts to `search-bar height + 8px` so it never collides with the bar; the icon-only
  inactive state may hide under open search (it is chrome, not an indicator), only the **active**
  chip and the stale chip must persist.
- **Inactive state (no filter requested).** Icon-only ghost button, mirroring `.graph-search-fab`
  geometry exactly (same box, radius, background, hover). Glyph: Lucide **`ListFilter`**, 16px,
  `strokeWidth={2}`, `aria-hidden`, `currentColor` (`--text-2`; hover `--text-1`).
  `aria-label="Graph filters"`, `title="Graph filters"`. This is *not* the AC5 indicator — it is
  quiet chrome, visually identical in weight to the search fab.
- **Active state (any filter requested) = the "filtered" indicator (AC5).** The button expands to
  a labeled chip: `height 24px; padding 0 4px 0 8px; border-radius: 999px; display: inline-flex;
  align-items: center; gap: 4px; background: var(--bg-2); border: 1px solid var(--border);
  font-size: 12px; font-weight: 600; color: var(--text-1)`. Leading `ListFilter` glyph 14px in
  `--accent` (3.68:1 dark / 3.38:1 light on `--bg-2` — clears the 3:1 graphics bar, ui-ref §2; the
  **words** are the meaning carrier, never the hue alone). Trailing **clear button**: 24×24
  (§3.1 floor; the glyph stays 12px `✕`), `aria-label="Clear graph filters"`,
  `title="Clear graph filters — show the full graph"`, hover
  `background: color-mix(in srgb, currentcolor 12%, transparent)` (toast-dismiss idiom).
- **Chip label** (`activeSummary`, max-width 260px, ellipsis + full text in `title`):
  - first-parent only → `First-parent`
  - solo → `Solo: {shortName}` / `Solo: {shortName} +{n−1}` (short name = ref without
    `refs/heads/` etc.)
  - hide → `1 branch hidden` / `{n} branches hidden` (tags/remotes counted in the same number;
    "branches" is the plain-language umbrella, the popover lists exact refs)
  - both → `First-parent · {ref summary}`
- **Why commits are missing:** the `title` on the chip body appends the explanation —
  `Graph is filtered — some commits are hidden. Click to review or clear.`
- **States.** Hover: `background: var(--bg-3)`. Pressed: `--bg-3` + no transform. Focus:
  `:focus-visible` 2px `--accent` outline, 1px offset. Disabled: never (hidden instead). Popover
  open: `aria-expanded="true"` + persistent `--bg-3` fill.
- **A11y.** Chip body: `<button aria-haspopup="dialog" aria-expanded aria-label="Graph filters:
  {activeSummary}">` when active, `aria-label="Graph filters"` when inactive. The ✕ is a separate
  sibling button (never a nested button).
- **Motion.** None on state change (inactive↔active swaps content; no width animation — locked:
  no filtered/unfiltered transitions). Popover: 120ms opacity ease-out only; snap under
  `prefers-reduced-motion`.

### 2.1 Popover — "Graph filters"

Anchored below the chip, right-aligned to it (`top: chipBottom + 4px`). Surface: `width: 280px;
background: var(--bg-1); border: 1px solid var(--border); border-radius: 6px; padding: 12px;
box-shadow: 0 8px 24px rgb(0 0 0 / .32)` (existing menu shadow); same z-layer as `ContextMenu`.
`role="dialog" aria-label="Graph filters"`. Contents, top → bottom:

1. **First-parent row.** §12.3.1 Switch (native checkbox, 36×24), label 13px `--text-1`
   `First-parent only`, help 12px `--text-2` below:
   `Follow each commit's first parent. Merged-in side histories collapse.` Switch wired with
   `aria-describedby` to the help line. Toggling applies immediately (graph reloads; no animation).
2. **1px `--border` divider**, 12px block margins.
3. **Ref filter section.** Header 11px uppercase `--text-3` (decorative — duplicates the list):
   `SOLO` or `HIDDEN` per mode; omitted when no ref filter. Then one row per ref, 24px tall,
   `display:flex; gap: 8px; align-items:center`:
   mode glyph (`ListFilter` 12px `--accent` for solo, Lucide `EyeOff` 12px `--text-2` for hidden,
   `aria-hidden`) → ref short name 12px `--text-1`, ellipsis + `title` = full ref name → trailing
   24×24 ✕, `aria-label` = `Stop soloing {name}` / `Show {name}`. Removing the last ref clears the
   ref filter. **Long-content case:** 60+-char branch names ellipsize; the list itself caps at
   `max-height: 192px; overflow-y: auto` (8 rows) for pathological sets.
   **Empty state** (first-parent only active, or everything just cleared): one 12px `--text-2`
   line — `No branch filters. Right-click a branch in the sidebar to solo or hide it.`
4. **Footer.** Full-width `.btn-secondary` (32px): label `Show full graph`,
   `aria-label="Clear all graph filters"`. Disabled (`opacity: .55`, the one dim per subtree) when
   nothing is active. It clears **both** knobs.

**Keyboard/focus.** Opens on chip Enter/Space/click; initial focus = the switch. No focus trap
(house rule, ui-ref §12.4 — no Bonsai dialog traps; a lone trap would be the inconsistency).
Tab order: switch → ref ✕s (DOM order) → Show full graph. Esc closes and restores focus to the
chip; if the chip unmounted (all filters cleared then closed), focus goes to `.graph-scroll`.
Click-away and focus leaving the popover close it. While open, App's global shortcuts are
suppressed via the existing `onMenuOpenChange` lift (TabStrip/identity-menu precedent).

### 2.2 Stale-fallback state (honest messaging)

Trigger: the request carried `seedRefs` but the backend reports the restriction did not apply
(all persisted refs stale → full graph shown). Detection: `meta.seedRefsApplied === false` if the
FLAG-2 field ships; otherwise only the detectable subset (`seedRefs` sent ∧ `firstParent` off ∧
`meta.filtered === false`) — and with `firstParent` on the chip must then show only
`First-parent` and **must not** claim the ref filter is active (drop the ref summary from the
label whenever staleness is detected or undecidable-and-refs-invalid; never lie).

Chip swaps to the §11 **verdict-pill** recipe with `--h: var(--warning)`: 14% tint over `--bg-2`
background, 40% `--h` border, leading `⚠` glyph in `--warning` (`aria-hidden`), label `--text-1`:
`Filter not applied`. `title`:
`Your saved branch filter doesn't match any branches in this repository — showing the full graph.`
Popover section 3 replaces the ref list with that sentence (12px `--text-2`, max-width 56ch) and
the footer button reads `Clear saved filter` (enabled). Contrast: label `--text-1` over warning
14% tint = 9.2:1+ dark / 11.7:1+ light; `⚠` glyph ≥3.8:1 both themes (measured recipe, ui-ref
§10.2/§11). No toast, no banner — the state is passive until the user looks.

---

## 3. Sidebar solo / hide

### 3.1 Context-menu items

Added to the existing branch / remote-branch / tag row context menus (the `{ label, onSelect }`
item shape), in their **own group after a separator, below all existing items and above any
destructive item** (Delete branch stays last). Wording per ref kind: "branch" / "branch" (remote
rows too — users read `origin/x` as a branch) / "tag". Rules (mode-exclusive, §1):

| Current state | Items shown on the row |
|---|---|
| No ref filter | `Solo branch` · `Hide branch` |
| Solo active, ref **not** in set | `Add to solo` |
| Solo active, ref in set | `Remove from solo` |
| Hide active, ref **not** hidden | `Solo branch` *(replaces the hide set)* · `Hide branch` |
| Hide active, ref hidden | `Solo branch` · `Show branch` |

- `Solo branch` always starts a fresh solo set `[ref]` (replacing any hide set — nondestructive,
  no confirm). While a solo set is active, a non-member deliberately gets **only** `Add to solo`
  (no second `Solo branch`): two solo verbs one line apart invite replacing a built-up set by
  mistake; restarting a solo is one clear-then-solo away. `Hide branch` while solo is active is
  not offered (everything outside the solo set is already absent).
- The checked-out branch gets no special-casing: the backend always keeps HEAD's ancestry, so
  hiding it only drops its pill. Detached HEAD: rows behave identically.
- Every change applies immediately (graph reload; selection rule per spec — kept + revealed if
  the commit survives, else cleared, view rests at top; the right panel then falls back to the
  existing status view — no toast, no extra copy).

### 3.2 Command palette

Two actions (group `action`): `Toggle first-parent view` (always enabled while a repo is open;
keywords `graph filter declutter simplify mainline`) and `Clear graph filters` (disabled when
none active; keywords `solo hide show full graph`). No keyboard shortcut assigned — palette-only
(the shortcut map is crowded; flag if the user wants one).

### 3.3 Row markers (state visibility in the sidebar)

Filtered rows carry a trailing 12px `aria-hidden` glyph after the name (before any status pill),
`flex: none`:

- Hidden ref: Lucide `EyeOff`, `--text-2`, `title="Hidden from the graph"`.
- Solo member: Lucide `ListFilter`, `--accent`, `title="Solo — the graph shows only these
  branches (plus HEAD)"`.

Never dim filtered rows (they stay fully interactive — ui-ref §3.1 dimming budget). The state is
also folded into the row's accessible name via a visually-hidden span: `, hidden from graph` /
`, solo'd in graph`. Rows **not** in an active solo set get no marker (marking 200 rows is noise;
the chip + the soloed rows' markers carry the state).

---

## 4. Settings — Commit graph category, new group "Declutter"

Rendered by `SettingsGraphDeclutterSection.tsx` after the existing groups; standard §12.2 row
anatomy; both rows registered in the settings catalog (searchable).

1. **Row `graph.first-parent`** — Switch. Label `First-parent only`. Help (12px `--text-2`):
   `Follow each commit's first parent so merged-in side histories collapse out of the graph.`
   Keywords: `first parent simplify mainline declutter filter merge`. Reset `↺` shown when on.
   This is the same value as the popover switch — one setting, two controls, always in sync.
2. **Row `graph.branch-filters`** — read-only summary + action (the **sidebar** is the editor;
   no ref picker is built in Settings). Label `Branch filters`. Control: a `.btn-secondary`
   `Clear` (32px), disabled when no ref filter, `aria-label="Clear branch filters"`. Stateful
   `.settings-row-note` (never a static help line beside it, §12.2):
   `Solo: {name} +{n}` / `{n} branches hidden` / `Saved filter matches no branches in this
   repository.` (stale) / `None. Right-click a branch in the sidebar to solo or hide it.`
   Keywords: `solo hide branch filter declutter show full graph`.

Because persistence is **global** (plan risk, accepted), the note's stale sentence is the honest
per-repo readout. Both themes: all row parts are existing settings primitives — no new pairs.

---

## 5. States matrix (implement all)

| State | Chip | Popover §3 | Sidebar | Settings note |
|---|---|---|---|---|
| No filter | icon-only ghost | empty-state line | no markers | `None. Right-click…` |
| First-parent only | `First-parent` | switch on, empty-state line | no markers | `None…` (row 1 on) |
| Solo set | `Solo: x +n` | SOLO list | `ListFilter` on members | `Solo: x +n` |
| Hide set | `n branches hidden` | HIDDEN list | `EyeOff` on hidden | `n branches hidden` |
| Both | `First-parent · Solo: x` | switch on + list | markers | both rows active |
| Stale fallback | ⚠ `Filter not applied` | stale sentence | no markers | stale sentence |
| Search bar open + filter active | chip stays, shifted below the bar (§2 gate) | — | — | — |
| Loading (graph reload) | chip stays; no spinner (reload uses existing stream path) | — | — | — |
| Error (graph load fails) | existing `.graph-error-banner` owns it; chip unchanged | — | — | — |
| Empty/unborn repo | hidden (fab gate) | — | — | rows enabled but inert |
| Long content | label ellipsis + title | ref rows ellipsize, list scrolls at 8 | name ellipsizes before marker | note ellipsis + title |

---

## 6. Mock-IPC harness states (`VITE_MOCK_IPC=1`)

The plan's `src/ipc/mock/handlers/graphFilter.ts` makes all of these browser-verifiable:

1. Default fixture, no filter → no indicator, byte-identical graph (AC4/AC5-absent).
2. Toggle first-parent via chip popover → row count shrinks, `meta.filtered` true, chip labeled.
3. Solo `refs/heads/feature/…` via sidebar menu → pills vanish, HEAD ancestry kept, marker shown.
4. Both filters → combined label.
5. Stale fallback: seed mock persistence with `graphRefFilter = { mode:'solo',
   refs:['refs/heads/deleted-branch'] }` → warning chip + honest copy. The mock emits
   `filtered:false` for unmatched refs; it additionally emits `seedRefsApplied:false` **only if**
   FLAG-2 lands (the UI must show the stale state on the detectable path either way).
6. Pathological: solo set of 10 refs incl. a 70-char name → popover scroll + ellipsis.
7. Selection: select a side-branch commit, enable first-parent → selection clears, top rest;
   select a mainline commit → stays selected and revealed (AC7).

Native scroll-feel on 20k rows under filters remains a **USER CHECKPOINT** (headless harness has
no rAF).

---

## 7. Out of scope / locked

No transition animation between filtered/unfiltered layouts. No changes to graph canvas metrics,
palettes, or ref-pill rendering (hidden pills simply never arrive from Rust). No per-repo
persistence UI (global, per plan). Linear-run folding is spec-004.
