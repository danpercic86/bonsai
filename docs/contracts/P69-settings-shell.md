# P69 — Settings redesign (structural contract)

Owner: `architect`. Implementers: `senior-dev` (+ `refactorer` for P69f, `tester` for P69e/P69k).
Status: contract.

**Input contracts (read first, do not duplicate):**
- `docs/contracts/P69-settings-ui.md` — visual/IA contract. Authoritative for geometry, copy,
  states, control vocabulary, taxonomy, and the coverage table (§1.3, 59 rows).
- `docs/contracts/ui-reference.md` §12 — the durable design-system extract.

This file owns everything downstream of the pixels: module boundaries, state ownership, data
shapes, file decomposition, test-impact, and increment sequencing. Where it contradicts the UI
contract, the deviation is called out explicitly with a reason (§8).

**IPC surface delta: NONE.** 160 Tauri commands unchanged; no new event, no new channel. P69 is
frontend-only and consumes existing `getConfig` / `setConfig` / `unsetConfig` /
`applyIdentityProfile` / `getUiSettings` / `setUiSettings`. The mock IPC layer needs **fixture
variants only** (§7) — every surface below is already mock-implementable today.

---

## 0. Rulings (the six decisions, one line each)

| # | Decision | Ruling |
|---|---|---|
| 1 | The ~41-prop problem | **React context, consumed by category pages only.** `SettingsPanel` keeps its full prop interface (it is App's state-ownership boundary and cannot shrink without moving `useUiSettings`); it becomes the adapter that builds one memoised context value. Pages read context; existing leaf sections keep their current props untouched. |
| 2 | Production defaults | **Yes to `src/settings/defaults.ts`, but not a re-export** — the mock's `DEFAULT_UI_SETTINGS` is a *harness seed* (2 seeded profiles) and is **not** Rust's defaults (`profiles: Vec::new()`). Production defaults live in `defaults.ts`, are pinned to a checked-in JSON oracle, and the mock *composes* its seed from them. |
| 3 | Catalog + search index | **One module `src/components/settings/settingsCatalog.ts`** (supersedes the UI contract's `settingsIndex.ts` name) carrying id/category/group/label/help/keywords/control/requires/reset. Anti-drift = a DOM↔catalog set-equality test keyed on a mandatory `data-setting-id` stamp, run over a maximal and a minimal fixture. |
| 4 | Identity boundary | **`src/hooks/useEffectiveIdentity.ts` over a tiny module-level store**, ONE `getConfig(repoId,'local')` call (curated entries already carry `effectiveValue` + `effectiveLevel`), explicit `invalidateEffectiveIdentity(repoId)`. `HeaderToolbar.tsx` absorbs the whole `.header-toolbar`, so `App.tsx` shrinks. |
| 5 | Sequencing | **10 increments, P69c → P69l** (§6). P69c–P69f are CSS-free and can run while a concurrent session holds `src/styles.css`; **P69g is the first CSS increment**. Behaviour + splits land before layout. |
| 6 | Test impact | 5 existing suites touched; 3 assertions change because behaviour genuinely changed (self-labelling buttons → segmented radios; the external-tools reset button → row `↺`; rows now behind a rail click). 10 new test files + 1 Rust test (§5). |

---

## 1. Module map

New directory `src/components/settings/`. Nothing in `src/ipc/types.ts` (baseline **2701**, current
~2660 — P69 uses **zero** of that headroom).

### 1.1 New files

| File | ~LOC | Responsibility |
|---|---|---|
| `settings/types.ts` | 70 | `SettingsCategoryId`, `SettingsCategory`, `SettingsRowId`, `SettingsControlKind`, `SettingsIndexEntry`, `SettingsRowReset`, `PersistedSettingsValues`, `SettingsRuntimeValues`, `SettingsActions`. No React. |
| `settings/SettingsContext.tsx` | 90 | `SettingsProvider`, `useSettingsValues()`, `useSettingsActions()`. Memoisation lives here. |
| `settings/settingsCatalog.ts` | 220 | The 7 categories (order, labels, subtitles, pill, dividers) + ~59 row entries. Pure data + per-entry pure reset fns. No React, no IPC. |
| `settings/SettingsShell.tsx` | 190 | Card grid, category state, rail↔pane wiring, `requestSeq` re-seed, scroll reset, search mode switch (from P69k). |
| `settings/SettingsRail.tsx` | 120 | `role="tablist"`, roving tabindex, **manual** activation, dividers, `repo` pill, match counts. |
| `settings/SettingsPaneHeader.tsx` | 50 | Title + subtitle + optional trailing slot. |
| `settings/SettingsGroup.tsx` | 45 | Group title + hairline-separated rows. |
| `settings/SettingsRow.tsx` | 120 | §5.1 anatomy, stacked variant, conditional `↺`, the `data-setting-id` stamp + DEV catalog check. |
| `settings/SettingsSwitch.tsx` | 55 | CSS skin over a native checkbox. |
| `settings/SettingsSegmented.tsx` | 75 | CSS skin over native radios (max 3). |
| `settings/SettingsEmpty.tsx` | 55 | 3 in-pane empty variants. |
| `settings/SettingsSearchBar.tsx` | 60 | Wraps `ListFilterInput`; owns placeholder/label + live-region text. **P69k.** |
| `settings/SettingsResults.tsx` | 140 | Cross-category groups, `Go to {Category}`, `<mark>`, zero-match. **P69k.** |
| `settings/GitConfigAdvanced.tsx` | 160 | The `<details>` Advanced block lifted out of `SettingsGitConfigSection`. |
| `settings/IdentityProfileCard.tsx` | 160 | One profile card + inline two-step delete, lifted out of `SettingsProfilesSection`. |
| `settings/categories/GeneralCategory.tsx` | 150 | Background activity rows + external tools. |
| `settings/categories/AppearanceCategory.tsx` | 85 | Replaces `SettingsAppearanceSection.tsx` (deleted). |
| `settings/categories/GraphCategory.tsx` | 60 | Wraps `SettingsGraphSection` in Geometry / Row details / Badges. |
| `settings/categories/AiCategory.tsx` | 90 | `SettingsAiSection` + `SettingsAiRunSection` + `SettingsMcpSection`. |
| `settings/categories/IdentitiesCategory.tsx` | 80 | `SettingsProfilesSection` + empty state + draft prefill. |
| `settings/categories/GitConfigCategory.tsx` | 95 | Owns `level` state + scope switch + `SettingsEmpty` + `SettingsGitConfigSection`. |
| `settings/categories/AboutCategory.tsx` | 100 | Version + `SettingsUpdatesSection` + welcome-tour row. |
| `src/components/HeaderToolbar.tsx` | 105 | The whole `.header-toolbar` + the identity trigger. |
| `src/components/IdentityMenu.tsx` | 175 | Trigger, anchor/open state, `ContextMenu`, apply, confirm, toasts. |
| `src/components/IdentityAvatar.tsx` | 45 | 22px initials/`?` circle, 4 states. |
| `src/hooks/useEffectiveIdentity.ts` | 110 | The effective-identity store + hook + invalidation. |
| `src/settings/defaults.ts` | 70 | `DEFAULT_UI_SETTINGS` — production defaults, Rust-parity. |
| `src/settings/uiSettingsDefaults.json` | 60 | The parity **oracle**. Not scanned by the size ratchet. |

### 1.2 Existing files — action

| File | Now | Action | After |
|---|---|---|---|
| `SettingsPanel.tsx` | 365 | Props façade + handlers + `SettingsProvider` + the category array; renders `SettingsShell`. | ~250 |
| `SettingsGitConfigSection.tsx` | **436** | `level` state + Level row → `GitConfigCategory`; `<details>` Advanced → `settings/GitConfigAdvanced.tsx`. **Required split — must never approach 500.** | ~230 |
| `SettingsProfilesSection.tsx` | 281 | Card → `settings/IdentityProfileCard.tsx`; local-only match logic deleted in favour of `useEffectiveIdentity`. | ~140 |
| `SettingsAppearanceSection.tsx` | 71 | **Deleted** (→ `AppearanceCategory.tsx`). | — |
| `NumberSlider.tsx` | 76 | Draft-display fix (P69c). | ~95 |
| `SettingsGraphSection.tsx` | 174 | Re-skin. **Labels + `#settings-graph-row` frozen.** | ~200 |
| `SettingsAiSection.tsx` / `SettingsAiRunSection.tsx` / `SettingsAiLimits.tsx` / `SettingsMcpSection.tsx` / `SettingsUpdatesSection.tsx` / `SettingsHooksToggle.tsx` / `SettingsExternalToolsSection.tsx` | — | Re-skin onto the primitives. Strings frozen except §8 A3/A4. | ≤ +30 each |
| `ContextMenu.tsx` | 321 | +3 additive fields (UI §4.4). | ~356 |
| `App.tsx` | **1161 / baseline 1168** | −~50 (toolbar extraction) +1 (`<HeaderToolbar>`) +~12 (`settingsReq`, `openSettingsAt`, `Ctrl/Cmd+,`, 2 palette entries, 3 SettingsPanel props). **Net ≈ −37.** | ~1124 |
| `src/ipc/mock/persistence.ts` | ~360 | Table body moves out; composes its seed from `defaults.ts`. | ~300 |
| `src-tauri/src/settings_ui_tests.rs` | 396 | +~20 (parity test). Must stay ≤500. | ~416 |
| `src-tauri/src/settings.rs` | **663 = its exact baseline** | **Untouched.** It already ends with its four `#[path] mod` declarations; even +1 line fails the ratchet. | 663 |

**Do not edit `scripts/file-size-baseline.json`** — a concurrent session owns it.

---

## 2. Decision 1 — state ownership and the prop collapse

### 2.1 What actually shrinks

`SettingsPanel`'s ~41 props are the **App → Settings state-ownership boundary**. They exist because
`useUiSettings` lives in `App`; removing them means moving state ownership, which is out of scope
and would grow `App.tsx`. What P69 removes is the *second* hop: Settings → page → section
threading, which with 7 pages would otherwise multiply.

**Trade-off (committed):** a context costs an implicit dependency edge and forces test renders to go
through a provider; it buys exactly one declaration site per value, no page-level prop churn when a
row moves category, and a single memo boundary. Grouped-props objects would need one bespoke
`Pick<>` per page (7 more types to keep in sync with the coverage table); per-page threading
reproduces the problem it is meant to solve. Context wins because *only one page is mounted at a
time*, so the re-render blast radius of a context update is a single pane.

### 2.2 The context

```ts
// src/components/settings/types.ts

/** Persisted values the pages read. `Pick<UiSettings,…>` (the `AiRunPrefs` precedent) so a
 *  rename in ipc/types.ts is a compile error here, not silent drift. */
export type PersistedSettingsValues = Pick<
  UiSettings,
  | 'theme' | 'listView' | 'panelDensity'
  | 'autoFetch' | 'healthRefresh' | 'graph'
  | 'aiEnabled' | 'aiConflictAutonomy' | 'aiConsented'
  | 'mcpConsented' | 'mcpWriteConsented'
  | 'autoCheckUpdates' | 'profiles'
  | 'terminalCommand' | 'editorCommand'
  | 'aiConflictTools' | 'aiStreamLog' | 'aiIncludePartialMessages'
  | 'aiIdleTimeoutSecs' | 'aiHardCapSecs' | 'aiMaxTurns' | 'aiMaxBudgetUsd' | 'aiBulkMaxBytes'
>;

/** Runtime facts that are NOT persisted settings. */
export interface SettingsRuntimeValues {
  repoPath: string | null;
  aiAvailability: AiAvailability | null;
  /** `aiEnabled && aiConsented` — computed once in the façade, never re-derived per page. */
  aiActive: boolean;
  mcpStatus: McpStatus | null;
  mcpEnabled: boolean;
  mcpAllowWrite: boolean;
  mcpRegistering: McpScope | null;
  updateCurrentVersion: string | null;
  updateState: UpdateUiState;
  configInitialFocus: 'identity' | null;
}

export interface SettingsActions {
  change(patch: UiSettingsPatch): void;
  toggleTheme(): void;
  toggleListView(): void;
  requestEnableAi(): void;
  setAiEnabled(next: boolean): void;      // consent-aware wrapper (today's handleEnableToggle)
  setMcpEnabled(next: boolean): void;     // consent-aware wrapper
  setMcpAllowWrite(next: boolean): void;  // consent-aware wrapper
  registerMcp(scope: McpScope): void;     // holds mcpRegistering internally
  showOnboarding(): void;
  checkUpdate(): void;
  openUpdateDialog(): void;
  /** §1.2 of the UI contract — the no-repo Git-config empty block. New App prop. */
  openRepository(): void;
  /** Per-row reset (§4). Resolves the patch from the catalog + defaults table. */
  resetRow(id: SettingsRowId): void;
}

export interface SettingsContextValue {
  values: PersistedSettingsValues & SettingsRuntimeValues;
  actions: SettingsActions;
}
```

```ts
// src/components/settings/SettingsContext.tsx
export function SettingsProvider(props: {
  values: PersistedSettingsValues & SettingsRuntimeValues;
  actions: SettingsActions;
  children: React.ReactNode;
}): JSX.Element;

/** Throws (not returns undefined) outside a provider — a page rendered bare is a bug. */
export function useSettingsValues(): PersistedSettingsValues & SettingsRuntimeValues;
export function useSettingsActions(): SettingsActions;
```

**Memoisation rules (binding):**
1. Two contexts internally (`ValuesContext`, `ActionsContext`) behind the two hooks above. A control
   that only dispatches never re-renders when an unrelated value changes.
2. `SettingsPanel` builds `values` with one `useMemo` over its ~23 value props, and `actions` with
   one `useMemo` over its callback props. Every callback in `actions` must be a `useCallback` (or an
   App-provided stable prop) — an inline arrow inside the `useMemo` is fine only if the memo deps
   cover it.
3. **No text-field draft state in context.** Text inputs (`terminalCommand`, `editorCommand`,
   `user.*`, custom keys) keep their existing local draft state and patch on their existing cadence.
   Nothing in P69 makes a keystroke re-render the settings tree.
4. Pages must not put derived objects in `useMemo` deps by identity — `values.graph` is a stable
   reference between patches because `useUiSettings` only replaces it on a graph patch.

### 2.3 Leaf-section boundary (limits the blast radius)

**Existing leaf sections keep their current props.** Pages are the adapters: `AiCategory` reads
context and hands `SettingsAiRunSection` its existing `{ aiRun, aiActive, onChange }`. Consequences:
`SettingsAiRunSection.test.tsx` (281) and `SettingsSections.test.tsx` (154) keep rendering sections
directly with props and stay valid; only re-skin assertions change (§5).

### 2.4 What "behaviour-preserving" means for P69f (the refactorer pass)

Equivalence is proven by all four, not just the first:
1. **Identical before/after test counts** — vitest and Playwright, same numbers.
2. **P69f modifies no existing test file.** This is the crisp proof: if a test needed editing, the
   observable surface moved and it is not this pass.
3. **Identical patch payload sequences.** For each interaction the `onChange` mock receives the same
   `UiSettingsPatch` objects, in the same order, with the same whole-struct-vs-single-key shape.
4. **No new IPC call and no changed call COUNT** at mount. ORDER may change: the section reorder
   this increment performs necessarily reorders these calls, and that is permitted — the original
   wording ("no changed call order") contradicted the reorder carve-out and is amended here.
   ⚠️ CORRECTED 2026-08-19 by measurement: the three mount reads are `SettingsGitConfigSection`
   (its own view load), **`SettingsHooksToggle`** (nested INSIDE the Git-config section), and
   `useEffectiveIdentity` (reached only from the profiles section). The second Git-config read is
   the hooks toggle, NOT `useEffectiveIdentity` — so P69h has THREE reads to collapse, not two.
   Superseded original text: (`getConfig` still fires once from the
   Git-config section, once from the profiles section until P69d merges them).

The one permitted visible change in P69f is **section order** (pages render in the new rail order in
one scrolling column). Nothing else may move: same classes, same roles, same accessible names, same
ids.

---

## 3. Decision 2 — defaults, and the parity mechanism

### 3.1 Files

```
src/settings/uiSettingsDefaults.json   ← the ORACLE. Exactly what serde emits for
                                          UiSettings::default(): camelCase, all keys, profiles: [].
src/settings/defaults.ts               ← export const DEFAULT_UI_SETTINGS: UiSettings
                                          (a literal, `satisfies UiSettings`, doc-commented).
src/settings/defaults.test.ts          ← deep-equals the oracle; spot-pins theme/rowHeight/profiles.
src-tauri/src/settings_ui_tests.rs     ← +1 test: serde ⟷ oracle.
src/ipc/mock/persistence.ts            ← composes its harness seed from DEFAULT_UI_SETTINGS.
```

### 3.2 Is a frontend defaults table the right call?

Yes, with one correction to the UI contract's A2. Rust remains **authoritative** — `defaults.ts` is a
declared *mirror*, exactly like `src/settings/ranges.ts` (the established idiom, pinned by
`ranges.test.ts` + Rust-side consts). What makes it safe rather than a second source of truth is
that both sides are pinned to one checked-in artefact:

```rust
// src-tauri/src/settings_ui_tests.rs  (+~20 lines; settings.rs itself is NOT touched)
#[test]
fn ui_settings_default_matches_the_typescript_oracle() {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("../../src/settings/uiSettingsDefaults.json"))
            .expect("oracle is valid JSON");
    // Exact both ways: an added, removed, or changed default fails here.
    assert_eq!(serde_json::to_value(UiSettings::default()).unwrap(), oracle);
}
```

```ts
// src/settings/defaults.test.ts
import oracle from './uiSettingsDefaults.json'; // tsconfig.json has resolveJsonModule: true
it('DEFAULT_UI_SETTINGS equals the shared Rust⟷TS oracle', () => {
  expect(DEFAULT_UI_SETTINGS).toEqual(oracle);
});
```

Change a serde default and the Rust test fails. Change `defaults.ts` and the vitest fails. Change
the oracle and both fail until both sides follow.

### 3.3 The mock is a seed, not the defaults

`src/ipc/mock/persistence.ts:77`'s `DEFAULT_UI_SETTINGS` seeds **two identity profiles** so the
harness has a populated list; Rust's default is `profiles: Vec::new()`. A plain re-export would
either leak fixture profiles into production reset behaviour or break ~50 assertions across
`persistence.test.tsx`, `scheduler.test.tsx`, `App.test.tsx`. **Compose instead** — zero call-site
churn, and the mock's exported name keeps meaning what those tests assert:

```ts
// src/ipc/mock/persistence.ts
import { DEFAULT_UI_SETTINGS as PRODUCTION_DEFAULT_UI_SETTINGS } from '../../settings/defaults';

/** The MOCK's default seed = production defaults + harness-only fixture data.
 *  Divergence is deliberate and must stay listed here. */
export const DEFAULT_UI_SETTINGS: UiSettings = {
  ...structuredClone(PRODUCTION_DEFAULT_UI_SETTINGS),
  profiles: [ /* mock-work, mock-personal — unchanged */ ],
};
```

`persistence.test.tsx` gains one assertion: `DEFAULT_UI_SETTINGS` differs from production defaults
**only** in `profiles` (iterate keys, assert equality for every other key). That pins the divergence
list.

### 3.4 Environment- or repo-derived defaults

| Field | Default | `defaultLabel` on the `↺` | Rule |
|---|---|---|---|
| `terminalCommand`, `editorCommand` | `''` (backend auto-detects per OS) | `auto-detect` | `''` is a real, stable default — parity holds. `↺` appears only when the value is non-empty and patches `''`. |
| `profiles` | `[]` | — | No row, no `↺`. |
| `paneWidths`, `onboardingSeen`, `theme`, `listView` | in the oracle | — | No `↺` row (theme/listView are segmented; a reset is meaningless). |
| `aiIdleTimeoutSecs`, `aiHardCapSecs`, `aiMaxBudgetUsd` | `300` / `0` / `0` — `0` is a documented **mode sentinel** | — | `reset` omitted from the catalog. A `↺` on the number row would silently disable the feature, which is not what "reset" means to the user. The switch above is already the off-switch. |
| `user.name`, `user.email`, custom config keys, MCP read-only rows, every button row | n/a | — | `reset` omitted. |

**Escape hatch:** if Rust ever computes an env-dependent default, that field moves to an explicit
`ENV_DERIVED_DEFAULT_KEYS` list in `defaults.ts`, and the Rust test compares the oracle against
`UiSettings::default()` with those keys removed from both sides. Adding a key to that list must be a
reviewed change, never a silent one.

---

## 4. Decision 3 — the catalog and the anti-drift test

### 4.1 Types

```ts
// src/components/settings/types.ts
export type SettingsCategoryId =
  | 'general' | 'appearance' | 'graph' | 'ai' | 'identities' | 'git-config' | 'about';

export interface SettingsCategory {
  id: SettingsCategoryId;
  /** Rail label, pane title, and search result-group header. One string, three uses. */
  label: string;
  /** Pane subtitle — states the scope once (UI §1.1). */
  subtitle: string;
  /** Hueless rail pill. Only 'git-config' sets it. */
  pill?: 'repo';
  /** Hairline divider rendered ABOVE this rail item. */
  dividerBefore?: boolean;
  /** Pane content. Takes NO props — reads SettingsContext. */
  Page: React.ComponentType;
}

/** `${category}.${slug}`, kebab-case slug. Enforced by the catalog test. */
export type SettingsRowId = string;

export type SettingsControlKind =
  | 'switch' | 'segmented' | 'radiogroup' | 'numberSlider' | 'text' | 'button' | 'readonly';

/** A row that is not always rendered. Exactly these ids may be missing from the
 *  minimal-fixture render — nothing else. */
export type SettingsRowRequirement = 'repo' | 'aiActive' | 'mcpRunning' | 'mcpStopped';

export interface SettingsRowReset {
  /** Shown in the ↺ title: 'Reset to default (28)'. Never empty. */
  defaultLabel: string;
  isDefault(current: PersistedSettingsValues, defaults: UiSettings): boolean;
  /** Whole-struct fields must MERGE, e.g. { graph: { ...current.graph, rowHeight: d.graph.rowHeight } }. */
  patch(current: PersistedSettingsValues, defaults: UiSettings): UiSettingsPatch;
}

export interface SettingsIndexEntry {
  id: SettingsRowId;
  category: SettingsCategoryId;
  /** Group title as rendered (uppercase styling is CSS, store it in sentence case). */
  group: string;
  /** MUST equal the rendered control's accessible name. Search matches on this. */
  label: string;
  /** The row's help line. Search matches on this. */
  help?: string;
  /** Never displayed. Lowercase, space-separated. UI §3.4 supplies the minimum set. */
  keywords?: string;
  control: SettingsControlKind;
  requires?: SettingsRowRequirement;
  /** Absent ⇒ no ↺ for this row. */
  reset?: SettingsRowReset;
}
```

```ts
// src/components/settings/settingsCatalog.ts
export const SETTINGS_CATEGORIES: readonly SettingsCategory[];   // rail order, 7 entries
export const SETTINGS_INDEX: readonly SettingsIndexEntry[];      // ~59 entries, UI §1.3 order
export function findSettingsRow(id: SettingsRowId): SettingsIndexEntry | undefined;
/** AND over whitespace-split terms, case-insensitive substring over label+help+keywords. */
export function searchSettings(query: string): readonly SettingsIndexEntry[];
/** '28' | 'On' | 'Off' | 'auto-detect' | 'Author' — for the ↺ title. */
export function formatDefaultLabel(entry: SettingsIndexEntry): string;
```

The `Page` field means `settingsCatalog.ts` imports the 7 page components. To keep the data module
React-free and unit-testable in isolation, `SETTINGS_CATEGORIES` lives in
`settingsCatalog.ts` **without** `Page`, and `SettingsPanel` zips it with a
`CATEGORY_PAGES: Record<SettingsCategoryId, React.ComponentType>` map declared in
`settings/categories/index.ts`. The catalog test then needs no DOM.

### 4.2 How a result maps to a row, and drives focus

- `SettingsRow` **must** stamp `data-setting-id={id}` on its root element and
  `data-setting-control` on the control wrapper. Both are contract, not decoration — the anti-drift
  test and e2e both key on them.
- In search mode, `SettingsResults` renders the *real* row (live, editable in place) by looking up a
  `ROW_RENDERERS: Partial<Record<SettingsRowId, React.ComponentType>>`? **No** — rejected: a second
  registry drifts from the pages. Instead `SettingsResults` renders each matching category's `Page`
  inside a `SettingsFilterContext` that carries the matching id set; `SettingsRow` returns `null`
  when a filter is active and its id is not in the set, and `SettingsGroup` returns `null` when it
  renders no visible row. One mechanism, no duplicate JSX, and a row can only appear in results if
  it really exists on a page.
- Highlight: `SettingsRow` reads the filter's `terms` from the same context and wraps label matches
  in `<mark class="settings-match">`. Help text is not highlighted.
- `Go to {Category}` clears the query and selects the category; `SettingsShell` then sets
  `pane.scrollTop = 0` and, if the activation was keyboard-driven, focuses the pane.

### 4.3 The anti-drift test (the most valuable artefact in P69)

Two files:

**`src/components/settings/settingsCatalog.test.ts`** — pure data, no DOM:
1. Ids unique; every id is `${category}.${kebab-slug}`; category is a known `SettingsCategoryId`.
2. Every `SettingsCategoryId` appears in `SETTINGS_CATEGORIES` exactly once, and every catalog entry's
   category exists there.
3. Labels non-empty; no two entries in the same category share a label (this is the guard that killed
   the two `Interval` rows).
4. `keywords`, when present: lowercase, single-spaced, no duplicate terms, no term shorter than 2 chars.
5. `searchSettings('')` returns `[]`; multi-term AND holds (`searchSettings('graph row')` includes
   `graph.row-height` and excludes `general.fetch-interval`); matching is case-insensitive.
6. For every entry with `reset`: `isDefault(defaultsAsValues, DEFAULT_UI_SETTINGS) === true`;
   `patch(defaultsAsValues, DEFAULT_UI_SETTINGS)` applied to defaults is a no-op; `defaultLabel` non-empty.
7. Every row listed in UI §1.3's coverage table has an entry — pinned as a literal expected-id list
   in the test (59 ids), so deleting a row from the catalog fails here even if it is also deleted
   from the UI.

**`src/components/settings/settingsCatalog.coverage.test.tsx`** — the DOM↔catalog guard:

```
maximalFixture = { repoPath: '/repo', aiEnabled: true, aiConsented: true,
                   mcpStatus: { enabled: true, allowWrite: true, port, toolCount },
                   profiles: 2, terminalCommand: 'x', editorCommand: 'y',
                   every numeric knob set OFF its default }
minimalFixture = { repoPath: null, aiEnabled: false, aiConsented: false,
                   mcpStatus: { enabled: false }, profiles: [], all defaults }

for each category c in SETTINGS_CATEGORIES:
    render <SettingsPanel {...maximalFixture} open initialCategory={c.id} />
    domIds  = [...pane.querySelectorAll('[data-setting-id]')].map(el => el.dataset.settingId)
    catIds  = SETTINGS_INDEX.filter(e => e.category === c.id).map(e => e.id)
    assert domIds has no duplicates                       // a row rendered twice is a bug
    assert new Set(domIds) equals new Set(catIds)          // BOTH directions: no orphan row,
                                                           // no phantom index entry
    for each rendered row r:
        assert r.querySelector('[data-setting-control]') !== null
        assert accessibleName(control in r) === catalog.label(r.id)   // keeps SEARCH honest
        assert (↺ present in r) === !entry.reset?.isDefault(maximalValues, DEFAULT_UI_SETTINGS)

render minimalFixture the same way, per category:
    missing = catIds \ domIds
    assert missing equals ids of entries WITH a `requires` field
    // i.e. a row may only be conditionally absent if the catalog SAYS it is conditional
```

Failure modes it catches: a control added to a page and not to the catalog (search cannot find it);
a catalog entry whose row was deleted or renamed (search offers a dead result); a label changed on
one side only (search matches text the user cannot see); a `↺` shown for a value that equals its
default; a newly-gated row that forgot its `requires` flag.

Third-party dependency: `accessibleName` — use `getByRole(role, { name })` scoped to the row via
`within(row)`, with `role` derived from `entry.control` (`switch`→`checkbox`, `segmented`/`radiogroup`
→`radio`s inside a `radiogroup` named `label`, `numberSlider`→`spinbutton`, `text`→`textbox`,
`button`→`button`, `readonly`→skip). Keep that mapping in the test file, not in the catalog.

**DEV guard (complements, does not replace, the test):** `SettingsRow` calls `findSettingsRow(id)`
under `import.meta.env.DEV` and `console.error`s (never throws) when the id is unknown, so a
mid-development row shows up in the harness console immediately.

---

## 5. Decision 4 — identity module boundary

### 5.1 The effective-identity store

```ts
// src/hooks/useEffectiveIdentity.ts
export interface EffectiveIdentity {
  name: string | null;
  email: string | null;
  signingKey: string | null;
  /** Where the effective user.name came from ('local' | 'global' | 'system' | 'other');
   *  null ⇒ unset everywhere. Falls back to user.email's level when name is unset. */
  source: ConfigLevelName | null;
  /** True only while the FIRST read for this repo is in flight (UI §4.2 loading state). */
  loading: boolean;
  /** getConfig rejected. name/email/source are null; the UI renders state 3 with the
   *  "couldn't read" copy. */
  error: string | null;
}

/** `repoId === null` ⇒ { …nulls, loading: false, error: null } and NO IPC call. */
export function useEffectiveIdentity(repoId: string | null): EffectiveIdentity;

/** Drop the cache entry, refetch, notify every subscriber. Call after a successful
 *  applyIdentityProfile, or a setConfig/unsetConfig of user.name|user.email|user.signingkey. */
export function invalidateEffectiveIdentity(repoId: string): void;
```

**Implementation constraints (no bodies):**
- **ONE** `ipc.getConfig(repoId, 'local')` call. `CuratedConfigEntry` already carries
  `effectiveValue` + `effectiveLevel` (`src/ipc/types.ts:972-979`), so the global fallback needs no
  second call — this is cheaper and atomic versus the UI contract's two-call design (§8, D-4).
- A module-level `Map<string, EffectiveIdentity>` + a `Set<() => void>` listener set, read through
  `useSyncExternalStore`. Two surfaces (`IdentityMenu`, `SettingsProfilesSection`, and from P69h also
  `GitConfigCategory`) must never disagree, which rules out per-component `useState`.
- Cache invalidation triggers, exhaustively: repo switch (different key), `invalidateEffectiveIdentity`
  after a successful `applyIdentityProfile` / `setConfig` / `unsetConfig` of a `user.*` key, and repo
  close (delete the entry). **No `repo-changed` subscription** — `setConfig` deliberately does not
  emit it (`types.ts:2263`).
- Out-of-band edits (`git config` in a terminal) leave the cache stale until the next invalidation.
  Accepted: the Git-config pane's existing refetch/`Try again` is the manual refresh, matching the
  house watcher rule (watcher + manual refresh). Do **not** add a focus-rescan for this — it would
  fire a `getConfig` on every window focus for a value that changes monthly.
- In-flight dedupe: a second `useEffectiveIdentity(sameRepo)` mount during the first fetch must not
  issue a second call.

### 5.2 Header extraction

```ts
// src/components/HeaderToolbar.tsx
export interface HeaderToolbarProps {
  theme: Theme;
  onToggleTheme(): void;
  listView: ListView;
  onToggleListView(): void;
  /** Gates 🤖 / 📊 / the identity trigger, exactly as App does today. */
  activeRepo: string | null;
  onOpenAiAssets(): void;
  onOpenHealth(): void;
  onOpenSettings(): void;
  /** Deep link for the identity menu's items 2/3/5 (UI §4.3). */
  onOpenSettingsAt(category: SettingsCategoryId, focus?: 'identity' | null): void;
  /** Lifted so App.tsx:806 (Esc) and :844 (global shortcuts) keep early-returning
   *  while a menu is open — the TabStrip.tsx:35-37 precedent. */
  onMenuOpenChange(open: boolean): void;
  profiles: IdentityProfile[];
}
```

```ts
// src/components/IdentityMenu.tsx
export interface IdentityMenuProps {
  repoId: string;                      // rendered only when a repo is open
  profiles: IdentityProfile[];
  onOpenSettingsAt(category: SettingsCategoryId, focus?: 'identity' | null): void;
  onMenuOpenChange(open: boolean): void;
}
```
- Toasts come from `ToastContext` inside `IdentityMenu` — **not** a prop. That is what keeps
  `HeaderToolbar` self-contained and `App.tsx`'s diff at +1 line for the whole feature.
- `IdentityMenu` owns: open/anchor state, the `ConfirmDialog` for the differing-local case (UI §4.5),
  the `Applying…` in-flight label, the `applyIdentityProfile` call, and
  `invalidateEffectiveIdentity(repoId)` on success.

```ts
// src/components/IdentityAvatar.tsx
export function IdentityAvatar(props: {
  identity: EffectiveIdentity;
  /** The saved profile whose trimmed name AND email equal the effective identity, if any. */
  matchedProfile: IdentityProfile | null;
}): JSX.Element;
export function identityInitials(name: string | null): string; // exported for its unit test
```

### 5.3 App.tsx delta (net negative)

```ts
interface SettingsRequest { category: SettingsCategoryId | null; focus: 'identity' | null; seq: number }
const [settingsReq, setSettingsReq] = useState<SettingsRequest>({ category: null, focus: null, seq: 0 });
const openSettingsAt = useCallback((category: SettingsCategoryId | null, focus: 'identity' | null = null) => {
  setSettingsReq((r) => ({ category, focus, seq: r.seq + 1 }));
  setSettingsOpen(true);
}, []);
// Replaces App.tsx:106-109 verbatim in behaviour.
const openIdentitySettings = useCallback(() => openSettingsAt('git-config', 'identity'), [openSettingsAt]);
```
- `configFocus` state is **replaced** by `settingsReq.focus`; the two `setConfigFocus(null)` calls
  become `setSettingsReq((r) => ({ category: null, focus: null, seq: r.seq }))`.
- `Ctrl/Cmd+,` joins the existing global-shortcut effect (~4 lines), allowed while typing.
- Two palette entries: `app.gitConfig` → `openSettingsAt('git-config')` (disabled when no repo),
  `app.identities` → `openSettingsAt('identities')`, keywords per UI §3.4.

### 5.4 The `configMissing` deep link — mechanism (hard constraint)

```ts
// SettingsPanel props (additive)
initialCategory?: SettingsCategoryId | null;
requestSeq: number;
onOpenRepository(): void;
```
1. `SettingsPanel` keeps `if (!open) return null`. Therefore `SettingsShell` **mounts fresh on every
   open**, and its `useState<SettingsCategoryId>(initialCategory ?? 'general')` is correct by
   construction — the category is selected on the very first render, before any child effect runs.
2. `GitConfigCategory` (and thus `SettingsGitConfigSection`) only mounts when `git-config` is
   selected, so the existing scroll+focus effect at `SettingsGitConfigSection.tsx:139-144` fires
   with its `focusedOnce` guard intact and **unchanged**.
3. Already-open case (commit fails while Settings is open): `SettingsShell` runs
   `useEffect(() => { if (initialCategory !== null) { setCategory(initialCategory); setQuery(''); } }, [requestSeq])`.
   Keyed on the monotonic seq, not on an `open` transition, so a second deep link in the same
   session still lands. `requestSeq` must change on *every* open path, including the plain ⚙ click
   (which passes `category: null` and therefore only clears the query).
4. Acceptance test (e2e + vitest): with `?fixture=noconfig`, a failed commit's "Set identity…" opens
   Settings with the rail's `Git config` tab selected (`aria-selected="true"`) and focus on the
   `user.name` input.

---

## 6. Decision 5 — the reconciled increment sequence

Ten increments. Ordering constraints honoured: (a) each leaves the app compiling and green;
(b) behaviour before layout; (c) **P69c–P69f touch no CSS** and can run while a concurrent session
holds `src/styles.css` — **P69g is the CSS gate**; (d) the `NumberSlider` semantics question is
settled in P69c and isolated to one file.

This supersedes both `TODO.md`'s "P69c primitives → P69h" list and UI §9.5's A→B→C→D. Mapping: UI's
A ≈ P69f+P69g, B ≈ P69h, C ≈ P69i, D ≈ P69j+P69k.

### P69c — `NumberSlider` typing fix (no CSS)
- **Goal.** Make two-digit entry possible without changing patch semantics.
- **Files:** `src/components/NumberSlider.tsx` only.
- **Contract.** Hold a `draft: string | null` for the number input's *display*. On `change`: set
  `draft`, and commit the clamped value exactly as today (per keystroke). On `blur` and on `Enter`:
  `setDraft(null)` so the field snaps back to the canonical clamped value. `draft` is cleared
  whenever the incoming `value` prop changes from a source other than this field (compare against
  the last committed value). The range input is untouched — it must keep committing per `change`
  (`e2e/10-settings-persistence.spec.ts:49-51` drives it with arrow keys).
- **Acceptance.** Typing `3` then `0` in Row height (min 24) ends at 30, and the field never shows
  `24` mid-typing; blurring a blank field restores the current value; a blank/NaN field still patches
  nothing.
- **Tests.** All three existing per-keystroke suites stay **green and unmodified**
  (`SettingsSections.test.tsx:56-64`, `SettingsPanel.test.tsx:129-140`,
  `SettingsAiRunSection.test.tsx:195-226`). New: `src/components/NumberSlider.test.tsx` (~6 cases)
  pinning the draft display, the blur snap-back, and the Enter commit.
- **Flagged:** OQ-1 (§8) — the alternative commit-on-blur/Enter semantics and what it would cost.

### P69d — standalone behaviour + a11y fixes and the two required splits (no CSS)
- **Goal.** Land every fix that is independent of the new layout, while the old layout still makes
  the diff readable.
- **Files:** `SettingsPanel.tsx` (two labels), `SettingsAiRunSection.tsx` (+ `SettingsAiLimits.tsx`
  if the note lives there), `SettingsGitConfigSection.tsx` → new `settings/GitConfigAdvanced.tsx`,
  `SettingsProfilesSection.tsx` → new `settings/IdentityProfileCard.tsx`, new
  `src/hooks/useEffectiveIdentity.ts`.
- **Contract.**
  1. The two `Interval` rows become `Fetch every` / `Refresh every` (UI §5.3.7 — MUST-FIX
     independent of the redesign). Ids unchanged.
  2. The AI-runs gate note moves to the top of the group with `id="ai-run-gate-note"` and
     `aria-describedby` on the `<fieldset>`. **Copy unchanged** pending A3.
  3. `useEffectiveIdentity` lands and `SettingsProfilesSection`'s local-only match logic is deleted;
     the "Active on this repo" pill becomes effective-based (closes UI D6 immediately, in the old
     layout).
  4. `SettingsGitConfigSection` → ≤240 lines; `SettingsProfilesSection` → ≤150.
- **Acceptance.** Two distinct accessible names in Background jobs; the profiles pill lights up in
  the default harness state (global identity, empty local); `pnpm lint:size` clean; identical
  category structure otherwise.
- **Tests.** `SettingsPanel.test.tsx` interval assertions updated to the new names (rename, not a
  weakening). `SettingsAiRunSection.test.tsx:230,245` still pass (same string, new position; the
  describedby-targets-exist test now also covers the fieldset). New:
  `src/hooks/useEffectiveIdentity.test.tsx` (local wins over global; global-only; unset; reject →
  `error`; one `getConfig` call per repo; invalidation refetches; two consumers see one value).
  New `settings/IdentityProfileCard.test.tsx` optional — covered via the section's suite.

### P69e — the data layer (no CSS, no UI change)
- **Goal.** Defaults + catalog + their guards, before anything renders from them.
- **Files:** new `src/settings/uiSettingsDefaults.json`, `src/settings/defaults.ts`,
  `src/settings/defaults.test.ts`, `src/components/settings/types.ts`,
  `src/components/settings/settingsCatalog.ts`, `settingsCatalog.test.ts`; modified
  `src/ipc/mock/persistence.ts`, `src-tauri/src/settings_ui_tests.rs`.
- **Contract.** §3 and §4.1/4.3 (pure-data half). Catalog entries for **all 59** coverage rows land
  here, even for categories not yet re-skinned — the DOM half of the guard attaches per category as
  each is migrated (§4.3 loops over `SETTINGS_CATEGORIES`, so it is written once in P69g with an
  explicit `MIGRATED` id list that must be empty by P69k).
- **Acceptance.** `pnpm vitest src/settings src/components/settings` green; `cargo test -p bonsai
  settings_ui` green; no visual change (`SettingsPanel` untouched).
- **Flagged:** OQ-6 — the Rust half needs one `cargo test` run, which the P73 concurrency protocol
  forbids. If the window is still open, land the JSON + TS + mock composition and defer only the
  Rust test to a follow-up step.

### P69f — props → context, behind the current layout (refactorer; no CSS)
- **Goal.** Introduce `SettingsContext` and the 7 page components with **zero** observable change.
- **Files:** new `settings/SettingsContext.tsx`, `settings/categories/*.tsx` (7 files) +
  `settings/categories/index.ts`; modified `SettingsPanel.tsx` (becomes façade + provider);
  `SettingsAppearanceSection.tsx` deleted (its markup moves verbatim into `AppearanceCategory`).
- **Contract.** §2.2–2.4. Pages render as fragments, one after another, in the new rail order inside
  the existing `.settings-card`. No class, role, name, or id changes. Leaf sections keep their props.
- **Acceptance.** The four equivalence conditions in §2.4, including **no existing test file edited**.
  `pnpm lint:size` clean; `SettingsPanel.tsx` ≤ 260 lines.

### P69g — the two-pane shell + primitives + row reset (**first CSS increment**)
- **Goal.** The 880px shell, the rail, the control vocabulary, per-row reset, and the re-skin of the
  three simplest categories.
- **Files:** new `settings/SettingsShell.tsx`, `SettingsRail.tsx`, `SettingsPaneHeader.tsx`,
  `SettingsGroup.tsx`, `SettingsRow.tsx`, `SettingsSwitch.tsx`, `SettingsSegmented.tsx`,
  `SettingsEmpty.tsx`; re-skin `GeneralCategory`, `AppearanceCategory`, `AboutCategory` (+ the leaf
  sections they own: `SettingsExternalToolsSection`, `SettingsUpdatesSection`); modified
  `SettingsPanel.tsx` (`initialCategory`, `requestSeq`, `onOpenRepository`), `src/styles.css`,
  `src/App.tsx` (3 props). **Graph / AI / Identities / Git config pages keep their legacy interiors
  and stay fully reachable** — nothing disappears mid-milestone.
- **Contract.** UI §2 geometry, §5.1/5.5/5.6/5.7, §6, §7. Rail = `role="tablist"` with
  `role="tab"` items, `aria-controls` → the pane's `role="tabpanel"` + `aria-labelledby`;
  **manual activation** (arrows move focus, Enter/Space selects) — automatic activation would fire a
  `getConfig` every time focus passes `Git config`.
- **Acceptance.** `getByRole('dialog', { name: 'Settings' })` still resolves; 7 tabs; the selected
  tab has `aria-selected="true"`; every migrated row carries `data-setting-id`; `↺` appears exactly
  when a value differs from its default and patches it back; the coverage test passes for
  `general` / `appearance` / `about`.
- **Tests.** See §7 for the `SettingsPanel.test.tsx` rail-click and segmented rewrites.

### P69h — Git config category + scope + shortcuts
- **Files:** new `settings/categories/GitConfigCategory.tsx` (already created in P69f, now filled);
  modified `SettingsGitConfigSection.tsx` (level state + Level row removed), `SettingsShell.tsx`
  (deep-link seq), `App.tsx` (`settingsReq`, `openSettingsAt`, `Ctrl/Cmd+,`, `app.gitConfig` palette
  entry), `styles.css`, `SettingsHooksToggle.tsx` (switch).
- **Contract.** UI §1.1 pill + §1.2 empty block + §4.7 deep link + §5.4 mechanism above.
- **Acceptance.** Scope switch in the pane header naming the real file; `SettingsEmpty` with a
  working `Open repository…` when no repo; the `configMissing` deep link lands on Git config →
  Identity with focus in `user.name`, both when Settings is closed and when it is already open;
  `Ctrl/Cmd+,` opens Settings.

### P69i — identity extraction
- **Files:** new `HeaderToolbar.tsx`, `IdentityMenu.tsx`, `IdentityAvatar.tsx`; modified `App.tsx`
  (−50/+1), `ContextMenu.tsx` (+3 additive fields), `settings/categories/IdentitiesCategory.tsx`,
  `SettingsProfilesSection.tsx` (re-skin + `Use in this repository`), `styles.css`; new mock fixtures
  (§7.2).
- **Contract.** UI §4 in full; §5.2/5.3 above; `ContextMenu` `checked`/`detail`/`header` remain
  byte-identical for existing call sites when absent.
- **Acceptance.** All four trigger states in the harness; a checked `menuitemradio` under
  `?fixture=identitymatch`; the confirm fires only for a *differing local* identity; `App.tsx` line
  count strictly below its pre-P69i value; global shortcuts suppressed while the menu is open.

### P69j — graph + AI/MCP re-skin
- **Files:** `SettingsGraphSection.tsx`, `SettingsAiSection.tsx`, `SettingsAiRunSection.tsx`,
  `SettingsAiLimits.tsx`, `SettingsMcpSection.tsx`, `GraphCategory.tsx`, `AiCategory.tsx`,
  `styles.css`.
- **Contract.** Every remaining row onto `SettingsRow` + `SettingsSwitch` + `SettingsSegmented` +
  `NumberSlider`; `Repository access` → segmented; `Date basis` → segmented over the same radios;
  conflict-resolution stays a radio group. **Frozen:** `#settings-graph-row`, `Row height`, and the
  eight graph toggle names (UI §11).
- **Acceptance.** Coverage test green for `graph` and `ai`; every `getByRole('checkbox'|'radio')`
  name in the existing suites and in `e2e/10-settings-persistence.spec.ts:44-46,71-73` still
  resolves.

### P69k — search
- **Files:** new `settings/SettingsSearchBar.tsx`, `SettingsResults.tsx`, plus the filter context
  hook inside `SettingsRow`/`SettingsGroup`; modified `SettingsShell.tsx`, `styles.css`.
- **Contract.** UI §3; §4.2 above. The coverage test's `MIGRATED` list must be empty here — search is
  only truthful once every row is catalog-backed, which is why it ships last (§8, D-3).
- **Acceptance.** `AND` matching over 59 entries; results are live-editable in place; rail match
  counts; zero-match block; `role="status"` announcement; first Esc clears, second closes.

### P69l — docs
`TODO.md`, `CHANGELOG.md`, `docs/contracts/INDEX.md`, and a `ui-reference.md` §12 cross-check by
`ui-designer`. No code.

---

## 7. Decision 6 — test-impact map

### 7.1 Existing suites

| File | Breaks when | What breaks | Minimal **honest** update |
|---|---|---|---|
| `src/components/SettingsPanel.test.tsx` (287) | P69g | Everything that queries a control now behind a rail tab. `renderPanel` renders one pane, not eleven sections. | Add one local helper `selectCategory(id)` (clicks `getByRole('tab', { name })`) and call it at the top of the tests for non-default categories: appearance (3 tests), AI + MCP (7), about/tour+version (1). General is the default pane → auto-fetch/health tests unchanged. **Not a weakening** — the control genuinely lives behind one click. |
| ″ | P69g | `:107-113` `getByRole('button', { name: 'Dark' })` / `'Flat'` and `:117-127` `'Cozy'` / `'Compact'`. **Behaviour genuinely changed** — a self-labelling toggle button became a 2-option segmented radio group. | Rewrite: `fireEvent.click(getByRole('radio', { name: 'Light' }))` fires `onToggleTheme` once; `getByRole('radio', { name: 'Flat' })` fires `onToggleListView`; `getByRole('radio', { name: 'Compact' })` patches `{ panelDensity: 'compact' }` alone. **Add** the assertion the old button could not have: clicking the *already selected* segment fires nothing (the old button always toggled — this is a real new guard). |
| ″ | P69d | The two `Interval` names. | Query `Fetch every` / `Refresh every`. Ids unchanged, so `:135`/`:144` `getElementById` assertions stay. |
| `src/components/SettingsSections.test.tsx` (154) | P69g | `:92` "reset is disabled when empty (auto-detect) and clears a set template" — the dedicated `Reset to auto-detect` button is replaced by the row `↺`, which is **absent** (not disabled) at the default. **Behaviour genuinely changed.** | Rewrite as three assertions: with `terminalCommand: ''`, `queryByRole('button', { name: 'Reset Terminal command to default' })` is `null`; with a set template it exists with `title="Reset to default (auto-detect)"`; clicking it patches `{ terminalCommand: '' }`. |
| ″ | P69j | Graph section re-skin. Roles/names/ids are preserved by design, so `:35-64` should pass untouched. | No change expected. If a query breaks, the re-skin is wrong — fix the component, not the test. |
| `src/components/SettingsAiRunSection.test.tsx` (281) | P69j | `:262` "the access button has a name that says what it controls" and `:271` "the read grant is disclosed in words" — the self-labelling button became segmented. **Behaviour genuinely changed.** | Rewrite to `getByRole('radiogroup', { name: 'Repository access' })` containing exactly two radios, `Read-only` and `No file access`; keep `:101`'s intent verbatim (no radio's name may mention write/modify); keep the words-not-icons disclosure assertion against the row help text. |
| ″ | P69d | `:230-243` gate-note position (text unchanged) and `:245` describedby-targets-exist. | `:230/:240` pass unchanged; extend `:245` to assert the `<fieldset>`'s `aria-describedby` resolves to `#ai-run-gate-note`. If A3's reword is approved, `:230`'s expected string changes in the same edit. |
| ″ | P69c | Nothing, under the recommended draft-display fix. | None. (Under the rejected commit-on-blur alternative, `:195-226` would all need `fireEvent.blur` added — see OQ-1.) |
| `src/settings/ranges.test.ts` (87) | never | Nothing. | None. New sibling `defaults.test.ts` carries the defaults mirror. |
| `e2e/10-settings-persistence.spec.ts` (119) | P69g/P69j | `:44-52` and `:71-74` query graph controls that are now behind the `Commit graph` tab. | Insert `await dialog.getByRole('tab', { name: 'Commit graph' }).click();` after each `openSettings` in that test (two places). Slider keyboard steps, `#settings-graph-row`, and the `localStorage` poll are unaffected. |
| ″ | P69i | `:23`/`:38` `Switch to light/dark theme` — page-level header buttons moved into `HeaderToolbar`. | No change: the extraction is verbatim, same `aria-label`s. This pair is the regression oracle for the extraction. |
| ″ | never | `:77-90` list-view via the command palette; `:107-117` corrupt-settings boot. | None. |

### 7.2 New test files

| File | Increment | Covers |
|---|---|---|
| `src/components/NumberSlider.test.tsx` | P69c | Draft display, blur snap-back, Enter, blank/NaN, external value change. |
| `src/hooks/useEffectiveIdentity.test.tsx` | P69d | local/global/unset/error, one call per repo, in-flight dedupe, invalidation, two consumers agree, `repoId === null` makes no call. |
| `src/settings/defaults.test.ts` | P69e | Deep-equal against the JSON oracle + spot pins. |
| `src/components/settings/settingsCatalog.test.ts` | P69e | §4.3 pure-data invariants incl. the 59-id literal list. |
| `src/components/settings/settingsCatalog.coverage.test.tsx` | P69g (grows each increment) | §4.3 DOM↔catalog set equality, control accessible names, `↺` visibility, `requires` gating. |
| `src/components/settings/SettingsPrimitives.test.tsx` | P69g | Switch = native checkbox with the right name/checked/disabled/describedby; segmented = radiogroup + radios, max-3 dev warning; row anatomy incl. stacked + help id wiring. |
| `src/components/settings/SettingsShell.test.tsx` | P69g | Tab roles + roving tabindex + Home/End, manual activation, `initialCategory` seeding, `requestSeq` re-seed while open, scroll reset, backdrop/✕ close. |
| `src/components/settings/SettingsResults.test.tsx` | P69k | AND matching, `<mark>`, zero-match, `Go to {Category}`, live region, Esc layering. |
| `src/components/IdentityMenu.test.tsx` | P69i | 4 trigger states, initials, `menuitemradio` + `aria-checked`, confirm only on differing local, `Applying…`, success/error toasts, `onMenuOpenChange`. |
| `src/components/HeaderToolbar.test.tsx` | P69i | Every button's name/gating; menu-open state lifts. |
| `e2e/24-settings-shell.spec.ts` | P69h/P69i | Rail navigation, a graph knob surviving a category switch + reload, the `configMissing` deep link, `Ctrl/Cmd+,`, the identity menu against `?fixture=identitymatch`. |
| `src-tauri/src/settings_ui_tests.rs` (+1 test) | P69e | Serde defaults ⟷ JSON oracle. |

### 7.3 Mock/harness deltas (no new commands)

| Fixture | Where | Change |
|---|---|---|
| `?fixture=identitymatch` | `src/ipc/mock/repoState.ts:238-240`, `src/ipc/fixtures/config.ts` | `makeMockConfigStore` gains a variant arg; seeds **local** `user.name = Ada Lovelace`, `user.email = work@bonsai.dev`, and a matching profile in the settings seed. |
| `?fixture=configerror` | `src/ipc/mock/handlers/config.ts` | `getConfig` rejects → identity state 3 with the "couldn't read" title + the pane's error banner. |
| `?fixture=slowconfig` | ″ | Delay `getConfig` → skeleton rows + the `·` loading circle. Reuse the existing latency knob if present. |
| `?fixture=longsettings` | `repoState.ts` + settings seed | 120-char profile label, 90-char email, 300-char `terminalCommand`, long custom key. Ellipsis + `title` proof. |
| `profiles: []` | `localStorage bonsai.mockUiSettings` | Already possible; drives the Identities empty state + `Add an identity…`. |
| `aiConsented: true` | ″ | Required to see the AI run knobs enabled (known harness step). |

Every fixture is a data seed behind the existing `?fixture=` seam — the mock layer keeps serving the
whole Settings surface in a plain browser with `VITE_MOCK_IPC=1`.

---

## 8. Deviations from the UI contract, and open questions

Deviations (decided here; the UI contract's intent is preserved in each):

- **D-1 · `settingsIndex.ts` → `settingsCatalog.ts`.** The module now carries control kind,
  conditional-render requirements, and reset descriptors, not just search text. One module, one test
  pair. Do not create both files.
- **D-2 · `useEffectiveIdentity` makes ONE `getConfig` call**, not a local-then-global pair.
  `CuratedConfigEntry.effectiveValue`/`effectiveLevel` already answer both questions atomically; two
  calls would also open a window where local and global disagree mid-read.
- **D-3 · Search ships last (P69k), and the search bar is not rendered before it.** Rendering a
  search box that can only find 3 of 7 categories' rows is a control that lies; rendering it disabled
  is a dead control. The content column is `grid-template-rows: 1fr` until P69k adds the 56px row.
  *Alternative considered:* a `migrated` flag in the catalog gating results — rejected, it puts
  increment bookkeeping into a data file that outlives the increment.
- **D-4 · No focus trap in P69.** UI §9.1 assigns `SettingsShell` "focus trap/restore". There is **no
  shared focus-trap hook in this codebase** and no dialog has one today; adding one to Settings only
  creates an inconsistency and risks the ~30 role-based queries. **Ship focus *restore*** (return
  focus to the ⚙ trigger on close, ~5 lines) and defer the trap to a dialog-wide a11y milestone that
  covers `ShortcutOverlay`, `AiAssetsPanel`, `RepoHealthPanel`, `ConfirmDialog`, and Settings together.
- **D-5 · Rail activation is manual**, not automatic — arrowing across `Git config` must not fire a
  `getConfig`. Consistent with UI §2.2's "focus moves to the pane only on keyboard activation".
- **D-6 · The mock's `DEFAULT_UI_SETTINGS` composes production defaults** rather than re-exporting
  them (§3.3), because the two genuinely differ in `profiles`.

Open questions for the orchestrator:

- **OQ-1 · `NumberSlider` commit semantics.** Recommended: **draft display + clamped commit per
  keystroke** (P69c) — fixes the real defect (with `min: 24`, typing `3` clamps to `24`, so `30` is
  unreachable), changes no patch semantics, and leaves all three pinning suites green and unedited.
  *Alternative:* draft + commit on blur/Enter — fewer writes and no intermediate live preview, but it
  rewrites `SettingsSections.test.tsx:56-64`, `SettingsPanel.test.tsx:129-140` and
  `SettingsAiRunSection.test.tsx:195-226` (all three currently assert a patch per `fireEvent.change`),
  and it needs a rule for the range input, which must stay immediate for the e2e keyboard-step test.
  **Ask before choosing the alternative.**
- **OQ-2 · The Rust parity test needs one `cargo test` run**, which `TODO.md`'s P73 concurrency
  protocol forbids. Recommendation: land P69e's TS half immediately and run the Rust half as a
  2-minute follow-up step once the tree is free. The guard is worthless if it is never executed.
- **OQ-3 · `SettingsPanel` prop count rises 41 → 44** (`initialCategory`, `requestSeq`,
  `onOpenRepository`) even though page-level threading is eliminated. This is inherent to
  `useUiSettings` living in App. Collapsing it further means moving settings state into a provider
  App also consumes — a real option, but a separate milestone with its own risk to `App.tsx`'s four
  App-owned writers (`theme`, `listView`, `paneWidths`, `onboardingSeen`, hardened in P69b).
  **Recommendation: do not attempt it inside P69.**
- **OQ-4 · `src/ipc/types.ts` has ~41 lines of slack** below its 2701 baseline. P69 must use none of
  it; if a future pass needs `SettingsCategoryId` in the IPC layer (it should not — no command takes
  it), that slack is the only room available.
- Already decided by the user, not relitigated here: **A1** (CRUD in Settings → Identities),
  **A3/A4** (copy sign-off), **A5** ("Reset all settings" out of scope), **A6** (rail dividers).

---

## 9. Acceptance criteria (milestone level)

1. 7-category two-pane shell at 880px; all 59 controls from UI §1.3 reachable, each in exactly one
   place; nothing from the old panel lost.
2. `getByRole('dialog', { name: 'Settings' })`, `#settings-graph-row`, `Row height`,
   `Switch to light/dark theme`, and all eight graph toggle names still resolve.
3. Every toggle is a native `<input type="checkbox">`; every exclusive choice is native radios.
4. Cross-category search finds any row by label, help, or keyword, and edits it in place.
5. The header identity trigger shows the **effective** identity and its source in all four states;
   applying confirms only when it would overwrite a differing local identity.
6. Per-row `↺` appears exactly when a value differs from its default and restores it.
7. `pnpm lint:size` clean: `App.tsx` strictly below 1168, `types.ts` unchanged, `settings.rs`
   unchanged, no new file over 500.
8. `pnpm vitest` and `pnpm e2e` green with counts ≥ pre-P69, and `cargo test` green.
9. The harness serves every state listed in UI §10 with `VITE_MOCK_IPC=1`; 160 Tauri commands
   unchanged.
10. **USER CHECKPOINT** (per UI §10): all visual proof, `Ctrl/Cmd+,` on macOS, a real `.git/config`
    write from the identity menu, scroll/focus-ring feel, and `:has()` support in the shipped
    WebView2 / WebKitGTK.
