# P69 — Settings redesign (structural contract)

Owner: `architect`. Implementers: `senior-dev` (+ `refactorer` for P69f, `tester` for P69e/P69k).
Status: **shipped** — P69a…P69k are code-complete, reviewed and committed (P69k = `a13b729`); P69l
is this docs pass. Amended in place at P69l so the contract describes what shipped, not what was
planned.

**Amendment A is folded in.** `docs/contracts/P69-settings-shell-amendment-A.md` (rulings AM-1…AM-8)
is now a superseded-pointer stub; its binding content lives in §4.1 (AM-1/AM-2/AM-3 types), §4.3
(AM-4 guard algorithm, AM-4b blindness list, AM-6 naming, AM-7 file naming), §4.4 (AM-5, closed) and
§8 (AM-8 consequences). Cite section numbers here in preference to AM-numbers from now on; the
AM-numbers are retained in the headings so old `TODO.md` lines and commit messages still resolve.

**Input contracts (read first, do not duplicate):**
- `docs/contracts/P69-settings-ui.md` — visual/IA contract. Authoritative for geometry, copy,
  states, control vocabulary, taxonomy, and the coverage table (§1.3, 59 rows).
- `docs/contracts/ui-reference.md` §12 — the durable design-system extract.

This file owns everything downstream of the pixels: module boundaries, state ownership, data
shapes, file decomposition, test-impact, and increment sequencing. Where it contradicts the UI
contract, the deviation is called out explicitly with a reason (§8).

**IPC surface delta: NONE — P69 contributed +0 Tauri commands, +0 events, +0 channels.** That is
what kept `src-tauri/src/settings.rs` out of the file-size ratchet fight: it sits at its exact
baseline (663 lines) and was never opened. P69 consumes only existing commands — `getConfig` /
`setConfig` / `unsetConfig` / `applyIdentityProfile` / `getUiSettings` / `setUiSettings`. The mock
IPC layer needed **fixture variants only** (§7.3); every surface below is mock-implementable under
`VITE_MOCK_IPC=1` today. (The absolute command count is owned by
`src-tauri/src/commands/registration_tests.rs`, not by this file — the earlier "160 commands"
figure in this contract was already stale when it was written and is deleted rather than refreshed.
What P69 pins is the **delta**, and the delta is zero.)

---

## 0. Rulings (the six decisions, one line each)

| # | Decision | Ruling |
|---|---|---|
| 1 | The ~41-prop problem | **React context, consumed by category pages only.** `SettingsPanel` keeps its full prop interface (it is App's state-ownership boundary and cannot shrink without moving `useUiSettings`); it becomes the adapter that builds one memoised context value. Pages read context; existing leaf sections keep their current props untouched. |
| 2 | Production defaults | **Yes to `src/settings/defaults.ts`, but not a re-export** — the mock's `DEFAULT_UI_SETTINGS` is a *harness seed* (2 seeded profiles) and is **not** Rust's defaults (`profiles: Vec::new()`). Production defaults live in `defaults.ts`, are pinned to a checked-in JSON oracle, and the mock *composes* its seed from them. |
| 3 | Catalog + search index | **One module `src/components/settings/settingsCatalog.ts`** (supersedes the UI contract's `settingsIndex.ts` name) carrying id/category/group/label/help/keywords/control/requires/repeats/reset. Anti-drift = a DOM↔catalog set-equality test keyed on a mandatory `data-setting-id` stamp, run over a maximal and a minimal fixture. |
| 4 | Identity boundary | **`src/hooks/useEffectiveIdentity.ts` over a tiny module-level store**, ONE `getConfig(repoId,'local')` call (curated entries already carry `effectiveValue` + `effectiveLevel`), explicit `invalidateEffectiveIdentity(repoId)`. `HeaderToolbar.tsx` absorbs the whole `.header-toolbar`, so `App.tsx` shrinks. |
| 5 | Sequencing | **Ten increments, P69c → P69l** (§6), on top of the pre-contract P69a/P69b. P69c–P69f are CSS-free and could run while a concurrent session held `src/styles.css`; **P69g was the first CSS increment**. Behaviour + splits landed before layout. |
| 6 | Test impact | 5 existing suites touched; 3 assertions changed because behaviour genuinely changed (self-labelling buttons → segmented radios; the external-tools reset button → row `↺`; rows now behind a rail click). New test files + fixtures per §7.2. |

---

## 1. Module map

Directory `src/components/settings/`. Nothing in `src/ipc/types.ts` — P69 used **zero** of its
headroom, as required.

### 1.1 New files — as shipped

Line counts are the P69k measurements, not estimates. Nothing here is within 40% of the 500-line
limit except `useSettingsPanelAdapter.ts` and the three test files, all noted.

| File | LOC | Responsibility |
|---|---|---|
| `settings/types.ts` | 119 | `SettingsCategoryId`, `SettingsCategory`, `SettingsRowId`, `SettingsControlKind` (incl. `'group'`), `SettingsRowRequirement` (incl. `'profile'`), `SettingsRowRepeat`, `PersistedSettingsValues`, `SettingsRowReset`, `SettingsIndexEntry`. No React. |
| `settings/SettingsContext.ts` | 130 | The two contexts + `useSettingsValues()` / `useSettingsActions()` + `SettingsPersistedValues` / `SettingsRuntimeValues` / `SettingsValues` / `SettingsActions`. **`.ts`, not `.tsx`** — the `ToastContext.ts` idiom (§8, D-7). |
| `settings/SettingsProvider.tsx` | 31 | The provider component alone, split out so `SettingsContext.ts` exports no component (`react-refresh/only-export-components`). |
| `settings/useSettingsPanelAdapter.ts` | 402 | **Not in the original plan.** `SettingsPanelProps` + the whole props→context adapter: the two `useMemo`s, the consent-aware wrappers, `mcpRegistering`, `resetRow`, and the `snapshot` build. Extracting it is why `SettingsPanel.tsx` is 46 lines (§8, D-7). Watch its size. |
| `settings/settingsCatalog.ts` | 140 | `SETTINGS_CATEGORIES` (7), `SETTINGS_INDEX` (composed from `catalog/*`), `findSettingsRow`, `searchSettings`, `settingsTabId`, `settingsRowLabelId`, `settingsRowHelpId`, `formatDefaultLabel`. No React, no IPC. |
| `settings/catalog/{general,appearance,graph,ai,repo,about}.ts` | 66/37/126/209/159/51 | The row entries, one module per category group (`repo.ts` carries both `IDENTITY_ENTRIES` and `GIT_CONFIG_ENTRIES`). Split so no file grows past the limit and a category is editable without reading the other six. |
| `settings/catalog/reset.ts` | 50 | Shared reset-descriptor builders used across the catalog modules. |
| `settings/settingsAvailability.ts` | 53 | **Not in the original plan — now load-bearing.** `SettingsAvailability`, `REQUIREMENT_PREDICATES`, `isRowAvailable`. The single definition of the `requires` preconditions (§4.5). |
| `settings/SettingsShell.tsx` | 221 | Card grid, category state, rail↔pane wiring, `requestSeq` re-seed, scroll reset, focus restore, the search-mode switch, the availability bag, per-category match counts. |
| `settings/SettingsRail.tsx` | 144 | `role="tablist"`, roving tabindex, **manual** activation (D-5), dividers, `repo` pill, match counts. |
| `settings/SettingsPaneHeader.tsx` | 28 | Title + subtitle + optional trailing slot. |
| `settings/SettingsGroup.tsx` | 40 | Group title + hairline-separated rows; self-filters via `useSettingsGroupVisible`. |
| `settings/SettingsRow.tsx` | 157 | §5.1 anatomy, stacked variant, conditional `↺`, the `data-setting-id` / `data-profile-id` stamps, label highlighting, the search self-filter, the DEV catalog check. |
| `settings/SettingsSwitch.tsx` | 46 | CSS skin over a native checkbox. |
| `settings/SettingsSwitchRow.tsx` | 52 | **Not in the original plan.** The `SettingsRow` + `SettingsSwitch` pairing, which is the majority row shape; extracted after it was written out ~20 times. |
| `settings/SettingsSegmented.tsx` | 64 | CSS skin over native radios (max 3). |
| `settings/SettingsEmpty.tsx` | 38 | In-pane empty variants; also the zero-match search state. |
| `settings/SettingsSearchBar.tsx` | 46 | Wraps `ListFilterInput`; owns placeholder/label + the visually-hidden `role="status"` count. **P69k.** |
| `settings/SettingsResults.tsx` | 89 | Cross-category result groups, `Go to {Category}`, `HeaderTrailing`, zero-match. **P69k.** |
| `settings/SettingsSearchContext.ts` | 66 | **Not in the original plan** (§4.2 called it `SettingsFilterContext`). `SettingsSearchState`, `SettingsSearchContext`, `useSettingsSearch`, `useSettingsRowVisible`, `useSettingsGroupVisible`. **P69k.** |
| `settings/settingsHighlight.tsx` | 63 | `highlightTerms(text, terms)` — merged non-overlapping `<mark class="settings-match">` ranges. **P69k.** |
| `settings/GitConfigAdvanced.tsx` | 261 | The `<details>` Advanced block lifted out of `SettingsGitConfigSection`; the two `role="group"` aggregate blocks; forced open while searching. |
| `settings/GitConfigScope.tsx` | 89 | `GitConfigScopeProvider` + `GitConfigScopeSwitch` (the one catalogued row that renders in the pane header). |
| `settings/GitConfigScopeContext.ts` | 33 | The scope context + hook, split out for the same `only-export-components` reason. |
| `settings/CuratedConfigControl.tsx` | 137 | One curated `user.*` / behaviour key control, lifted out of the Git-config section. |
| `settings/useGitConfigEditor.ts` | 282 | The Git-config load/edit/commit state machine, lifted out of the section. |
| `settings/IdentityProfileCard.tsx` | 254 | One profile card (`role="group"` + `data-profile-id`), its six repeated rows, `ProfileActionCell`, inline two-step delete. |
| `settings/coverageFixtures.ts` | 191 | `MAXIMAL` / `MINIMAL` / `FIXTURE_CONFIG_VIEW` / `FIXTURE_PROFILES` — data only, per AM-7. |
| `settings/categories/GeneralCategory.tsx` | 99 | Background activity rows + external tools. |
| `settings/categories/AppearanceCategory.tsx` | 75 | Replaces `SettingsAppearanceSection.tsx` (deleted). |
| `settings/categories/GraphCategory.tsx` | 17 | Wraps `SettingsGraphSection`. |
| `settings/categories/AiCategory.tsx` | 60 | `SettingsAiSection` + `SettingsAiRunSection` + `SettingsMcpSection`. |
| `settings/categories/IdentitiesCategory.tsx` | 20 | `SettingsProfilesSection` + empty state. |
| `settings/categories/GitConfigCategory.tsx` | 48 | No-repo empty state + `SettingsGitConfigSection`; owns the **`initialFocus` suppression while searching** (§4.6). |
| `settings/categories/AboutCategory.tsx` | 37 | Version + `SettingsUpdatesSection` + welcome-tour row. |
| `settings/categories/index.ts` | 39 | `SettingsCategoryPage { Page, HeaderTrailing? }` + `CATEGORY_PAGES`. The `HeaderTrailing` slot is additive to the original plan and is what keeps `SettingsShell` free of a `git-config` special case. |
| `src/components/HeaderToolbar.tsx` | 115 | The whole `.header-toolbar` + the identity trigger. |
| `src/components/IdentityMenu.tsx` | 302 | Trigger, anchor/open state, `ContextMenu`, apply, confirm, toasts. |
| `src/components/IdentityAvatar.tsx` | 36 | 22px initials/`?` circle, 4 states. |
| `src/hooks/useEffectiveIdentity.ts` | 254 | The effective-identity store + hook + invalidation + the P69h priming API (§5.1). |
| `src/settings/defaults.ts` | 120 | `DEFAULT_UI_SETTINGS`, `cloneDefaultUiSettings`, `ENV_DERIVED_DEFAULT_KEYS`. |
| `src/settings/uiSettingsDefaults.json` | 42 | The parity **oracle**. Not scanned by the size ratchet. |

### 1.2 Existing files — action and shipped result

| File | Before | Action | After (measured) |
|---|---|---|---|
| `SettingsPanel.tsx` | 365 | Props façade + provider only; the prop interface and the adapter moved to `useSettingsPanelAdapter.ts` and `SettingsPanelProps` is re-exported so no importer moved. | **46** |
| `SettingsGitConfigSection.tsx` | **436** | `level` state + Level row → `GitConfigScope`; `<details>` Advanced → `GitConfigAdvanced.tsx`; the editor state machine → `useGitConfigEditor.ts`; the curated control → `CuratedConfigControl.tsx`. | **134** |
| `SettingsProfilesSection.tsx` | 281 | Card → `IdentityProfileCard.tsx`; local-only match logic deleted in favour of `useEffectiveIdentity`. | **169** |
| `SettingsAppearanceSection.tsx` | 71 | **Deleted** (→ `AppearanceCategory.tsx`). | — |
| `NumberSlider.tsx` | 76 | Draft-display fix (P69c). | 155 |
| `SettingsGraphSection.tsx` | 174 | Re-skin. **Labels + `#settings-graph-row` frozen.** | 187 |
| `SettingsAiLimits.tsx` | — | Re-skin, **plus** three `useSettingsRowVisible` calls: its gate notes are `aria-describedby` targets, so they must disappear with their rows or the idref dangles (§4.2, consumer class 5). | 284 |
| `SettingsAiSection.tsx` / `SettingsAiRunSection.tsx` / `SettingsMcpSection.tsx` / `SettingsUpdatesSection.tsx` / `SettingsHooksToggle.tsx` / `SettingsExternalToolsSection.tsx` | — | Re-skin onto the primitives. Strings frozen except §8 A3/A4. | ≤ +30 each |
| `ContextMenu.tsx` | 321 | +3 additive fields (UI §4.4). | 388 |
| `App.tsx` | **1161 / baseline 1168** | Toolbar extraction, `settingsReq`, `openSettingsAt`, `Ctrl/Cmd+,`, 2 palette entries, 3 SettingsPanel props. | **1065** — well under baseline ✓ |
| `src/ipc/mock/persistence.ts` | ~360 | Composes its seed from `defaults.ts` (`persistence.ts:3`, `:94`). | ~300 |
| `src-tauri/src/settings_ui_tests.rs` | 396 | **Untouched** — the parity test went into its own module instead (the ratchet forbids growing `settings.rs`, and the subject is separate). | 396 |
| `src-tauri/src/settings_defaults_parity_tests.rs` | **141 (new, P69l)** | The Rust link of the §3.2 chain: `ui_settings_of(&Settings::default())` deep-compared against the `include_str!`-embedded oracle. Declared from `lib.rs`. **OQ-2 resolved.** | 141 |
| `src-tauri/src/settings.rs` | **663 = its exact baseline** | **Untouched**, as required. | 663 |

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

> Search (P69k) is the one case where that last sentence stops holding: `SettingsResults` mounts
> **every hit category's page at once**. The consequences are §4.2 and §4.6 — read both before
> adding an effect to a category page.

### 2.2 The context — as shipped

Two naming notes against the original draft, both deliberate:
`PersistedSettingsValues` was already taken in `settings/types.ts` (P69e) for the reset descriptors,
where it aliases the whole of `UiSettings`; the context type is therefore
**`SettingsPersistedValues`**. And the eight AI-run keys stay folded into one `aiRun: AiRunPrefs`
struct rather than being flattened, because that is the prop `SettingsPanel` receives and the struct
`SettingsAiRunSection` consumes whole — flattening and reassembling would mint a new object identity
per render for nothing. `AiRunPrefs` is itself a `Pick<UiSettings, …>`, so those eight keys are
drift-checked the same way.

```ts
// src/components/settings/SettingsContext.ts

export type SettingsPersistedValues = Pick<
  UiSettings,
  | 'theme' | 'listView' | 'panelDensity'
  | 'autoFetch' | 'healthRefresh' | 'graph'
  | 'aiEnabled' | 'aiConflictAutonomy' | 'aiConsented'
  | 'mcpConsented' | 'mcpWriteConsented'
  | 'autoCheckUpdates' | 'profiles'
  | 'terminalCommand' | 'editorCommand'
> & {
  /** The eight AI-run knobs, threaded whole (the `graph` / `autoFetch` idiom). */
  aiRun: AiRunPrefs;
};

/** Runtime facts that are NOT persisted settings. */
export interface SettingsRuntimeValues {
  repoPath: string | null;
  aiAvailability: AiAvailability | null;
  /** `aiEnabled && aiConsented` — computed once in the adapter, never re-derived per page. */
  aiActive: boolean;
  mcpStatus: McpStatus | null;
  mcpEnabled: boolean;
  mcpAllowWrite: boolean;
  mcpRegistering: McpScope | null;
  updateCurrentVersion: string | null;
  updateState: UpdateUiState;
  /** Passed through verbatim (`undefined` included) so the Git-config section's
   *  scroll+focus effect sees exactly the value it saw pre-P69. */
  configInitialFocus: 'identity' | null | undefined;
  /** P69i: the Identities card to focus on open (`null` ⇒ none). */
  focusProfileId: string | null;
  /** P69g: the whole-`UiSettings` view the catalog's reset descriptors compare
   *  against (`SettingsRowReset.isDefault`), so `SettingsRow` can decide whether
   *  to render `↺` without every page threading its own values. Built in the adapter. */
  snapshot: UiSettings;
}

export type SettingsValues = SettingsPersistedValues & SettingsRuntimeValues;

export interface SettingsActions {
  change(patch: UiSettingsPatch): void;
  toggleTheme(): void;
  toggleListView(): void;
  setAiEnabled(next: boolean): void;      // consent-aware wrapper (today's handleEnableToggle)
  setMcpEnabled(next: boolean): void;     // consent-aware wrapper
  setMcpAllowWrite(next: boolean): void;  // consent-aware wrapper
  registerMcp(scope: McpScope): void;     // holds mcpRegistering in the adapter
  showOnboarding(): void;
  /** UI §1.2 — App's folder picker, offered by the no-repo Git-config empty block. */
  openRepository(): void;
  checkUpdate(): void;
  openUpdateDialog(): void;
  /** UI §5.7 — per-row reset. Resolves the patch from the catalog's `reset`
   *  descriptor + `DEFAULT_UI_SETTINGS`; a row with no descriptor is a no-op, never a throw. */
  resetRow(id: SettingsRowId): void;
}

/** `null` ⇒ no provider above. The hooks throw on that. */
export const SettingsValuesContext: React.Context<SettingsValues | null>;
export const SettingsActionsContext: React.Context<SettingsActions | null>;

/** Throws (not `undefined`) outside a provider — a page rendered bare is a bug. */
export function useSettingsValues(): SettingsValues;
export function useSettingsActions(): SettingsActions;
```

```ts
// src/components/settings/SettingsProvider.tsx
export function SettingsProvider(props: {
  values: SettingsValues;
  actions: SettingsActions;
  children: React.ReactNode;
}): JSX.Element;

// src/components/settings/useSettingsPanelAdapter.ts
export interface SettingsPanelProps { /* App's ~44 props; re-exported from SettingsPanel.tsx */ }
export function useSettingsPanelAdapter(props: SettingsPanelProps): {
  values: SettingsValues;
  actions: SettingsActions;
};
```

`requestEnableAi` from the draft does **not** exist as an action: consent is handled inside
`setAiEnabled` / `setMcpEnabled` / `setMcpAllowWrite`, which is what "consent-aware wrapper" always
meant. `App` still owns the consent dialogs and passes the `onRequestEnable*` props the adapter
wraps.

**Memoisation rules (binding):**
1. Two contexts internally (`SettingsValuesContext`, `SettingsActionsContext`) behind the two hooks
   above. A control that only dispatches never re-renders when an unrelated value changes.
2. `useSettingsPanelAdapter` builds `values` with one `useMemo` and `actions` with one `useMemo`.
   Every callback in `actions` must be a `useCallback` (or an App-provided stable prop) — an inline
   arrow inside the `useMemo` is fine only if the memo deps cover it.
3. **No text-field draft state in context.** Text inputs (`terminalCommand`, `editorCommand`,
   `user.*`, custom keys) keep their existing local draft state and patch on their existing cadence.
   Nothing in P69 makes a keystroke re-render the settings tree. **The search query is the one
   deliberate exception and it lives in `SettingsShell`, not in context** (§4.2).
4. Pages must not put derived objects in `useMemo` deps by identity — `values.graph` is a stable
   reference between patches because `useUiSettings` only replaces it on a graph patch.

### 2.3 Leaf-section boundary (limits the blast radius)

**Existing leaf sections keep their current props.** Pages are the adapters: `AiCategory` reads
context and hands `SettingsAiRunSection` its existing `{ aiRun, aiActive, onChange }`. Consequences:
`SettingsAiRunSection.test.tsx` and `SettingsSections.test.tsx` keep rendering sections directly
with props and stay valid; only re-skin assertions change (§7).

Corollary that P69k made concrete: a leaf section rendered bare, with no provider above it, must
still work — `SettingsRow` therefore reads the two contexts with `useContext` and tolerates `null`
(falling back to the `reset` prop override), and `useSettingsRowVisible` returns `true` when no
search provider is present.

### 2.4 What "behaviour-preserving" meant for P69f (the refactorer pass)

Equivalence was proven by all four, not just the first:
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
   the hooks toggle, NOT `useEffectiveIdentity` — so P69h had THREE reads to collapse, not two.
   Superseded original text: (`getConfig` still fires once from the
   Git-config section, once from the profiles section until P69d merges them).

The one permitted visible change in P69f was **section order** (pages render in the new rail order
in one scrolling column). Nothing else moved: same classes, same roles, same accessible names, same
ids.

---

## 3. Decision 2 — defaults, and the parity mechanism

### 3.1 Files

```
src/settings/uiSettingsDefaults.json   ← the ORACLE. Exactly what serde emits for
                                          UiSettings::default(): camelCase, all keys, profiles: [].
src/settings/defaults.ts               ← DEFAULT_UI_SETTINGS (a literal, `satisfies UiSettings`),
                                          cloneDefaultUiSettings(), ENV_DERIVED_DEFAULT_KEYS.
src/settings/defaults.test.ts          ← deep-equals the oracle; spot-pins theme/rowHeight/profiles;
                                          asserts ENV_DERIVED_DEFAULT_KEYS is still [].
src-tauri/src/settings_defaults_parity_tests.rs  ← the serde <-> oracle test. LANDED in P69l (OQ-2 resolved).
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

> **⚠️ Shipped state: only two of those three legs exist.** The TS half is green; the Rust half was
> deferred at P69e under the P73 concurrency protocol and **has not landed** —
> `settings_ui_tests.rs` is unchanged at 396 lines. Today a serde-side default change fails
> nothing. The one known wrinkle for whoever lands it is noted in `defaults.test.ts:19`:
> `aiMaxBudgetUsd` is an `f64`, so the oracle writes `0.0` and `serde_json` must be compared as a
> `Value`, not as text — which is exactly how it shipped. **Landed in P69l** (OQ-2 resolved):
> `settings_defaults_parity_tests.rs` reports every drifting dotted path, distinguishing MISSING /
> UNEXPECTED / VALUE-mismatch, and appends an int-vs-float hint when two mismatched numbers are
> numerically equal, so the `0` vs `0.0` trap never surfaces as a baffling `0 != 0`. **The two sides
> agreed on the first run** — all 30 keys including the four nested objects — so the chain closed
> without moving either side.

### 3.3 The mock is a seed, not the defaults

`src/ipc/mock/persistence.ts`'s `DEFAULT_UI_SETTINGS` seeds **two identity profiles** so the harness
has a populated list; Rust's default is `profiles: Vec::new()`. A plain re-export would either leak
fixture profiles into production reset behaviour or break ~50 assertions across
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

**Escape hatch (shipped as `ENV_DERIVED_DEFAULT_KEYS`, currently `[]`):** if Rust ever computes an
env-dependent default, that field moves to that list in `defaults.ts`, and the Rust test compares
the oracle against `UiSettings::default()` with those keys removed from both sides.
`defaults.test.ts` asserts the list is empty, so adding a key to it is a reviewed change, never a
silent one.

---

## 4. Decision 3 — the catalog, search, and the anti-drift guard

### 4.1 Types (Amendment A AM-1 / AM-2 / AM-3 folded in)

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
  // NO `Page` field — see the note below.
}

/** `${category}.${slug}`, kebab-case slug. Enforced by the catalog test. */
export type SettingsRowId = string;

export type SettingsControlKind =
  | 'switch' | 'segmented' | 'radiogroup' | 'numberSlider' | 'text'
  /** A row whose value is displayed but not editable (`about.version`, the MCP URL). */
  | 'readonly'
  | 'button'
  /**
   * AM-2: an aggregate row standing for a dynamically-populated block. Stamped on a
   * `role="group"` element named by its heading via `aria-labelledby`. It has NO
   * `[data-setting-control]`; its children are runtime-generated and are not
   * individually catalogued (§4.3's blindness #2).
   */
  | 'group';

/**
 * A row that is not always rendered. Exactly these ids may be missing from a
 * fixture's render, and only when the matching predicate in `settingsAvailability.ts`
 * says so. `'profile'` is AM-3, folded in permanently.
 */
export type SettingsRowRequirement =
  'repo' | 'aiActive' | 'mcpRunning' | 'mcpStopped' | 'profile';

/**
 * AM-1: this row is rendered once per item of a runtime collection; the guard dedupes
 * it and checks the instance set against that collection. Instance identity is
 * `(data-setting-id, data-profile-id)` — `SettingsRowId` still identifies the ROW.
 */
export type SettingsRowRepeat = 'perProfile';

/** An alias of `UiSettings`: every resettable row is backed by a persisted UI setting.
 *  Distinct name so that if a future row needs a wider bag, THIS widens, not `UiSettings`. */
export type PersistedSettingsValues = UiSettings;

export interface SettingsRowReset {
  /** Shown in the ↺ title: 'Reset to default (28)'. Never empty. */
  defaultLabel: string;
  isDefault(current: PersistedSettingsValues, defaults: UiSettings): boolean;
  /** Whole-struct fields MERGE, e.g. `{ graph: { ...current.graph, rowHeight: d.graph.rowHeight } }`. */
  patch(current: PersistedSettingsValues, defaults: UiSettings): UiSettingsPatch;
}

export interface SettingsIndexEntry {
  id: SettingsRowId;
  category: SettingsCategoryId;
  /** Group title as rendered (uppercase styling is CSS; store it in sentence case). */
  group: string;
  /** MUST equal the rendered control's accessible name. Search matches on this. */
  label: string;
  /** The row's help line. Search matches on this. */
  help?: string;
  /** Never displayed. Lowercase, space-separated. UI §3.4 supplies the minimum set. */
  keywords?: string;
  control: SettingsControlKind;
  requires?: SettingsRowRequirement;
  /** AM-1. Absent ⇒ exactly one instance in the DOM. */
  repeats?: SettingsRowRepeat;
  /** Absent ⇒ no ↺ for this row. */
  reset?: SettingsRowReset;
}
```

Per-row invariants that go with `repeats` (asserted in `settingsCatalogRows.test.ts`):
`repeats === 'perProfile'` ⟺ `requires === 'profile'` (biconditional over the whole index); only
`identities` may carry `repeats`; a `repeats` entry never carries `reset`.

```ts
// src/components/settings/settingsCatalog.ts
export const SETTINGS_CATEGORIES: readonly SettingsCategory[];   // rail order, 7 entries
export const SETTINGS_INDEX: readonly SettingsIndexEntry[];      // 57 entries, UI §1.3 order
export function findSettingsRow(id: SettingsRowId): SettingsIndexEntry | undefined;

/** AND over whitespace-split terms, case-insensitive substring over label+help+keywords,
 *  RESTRICTED to the rows that are renderable right now (§4.5). */
export function searchSettings(
  query: string,
  availability: SettingsAvailability,
): readonly SettingsIndexEntry[];

/** DOM id of a rail tab — the `aria-labelledby` target the pane points at. */
export function settingsTabId(id: SettingsCategoryId): string;
/** Id of the row's visible label element. `instance` ⇒ per-profile ids (AM-1). */
export function settingsRowLabelId(id: SettingsRowId, instance?: string): string;
/** Id of the row's help paragraph — the `aria-describedby` target (UI §5.1). */
export function settingsRowHelpId(id: SettingsRowId, instance?: string): string;
/** '28' | 'On' | 'Off' | 'auto-detect' | 'Author' — for the ↺ title. */
export function formatDefaultLabel(entry: SettingsIndexEntry): string;
```

`SETTINGS_INDEX` is composed from `catalog/{general,appearance,graph,ai,repo,about}.ts` so a category
is editable without reading the other six, and `settingsCatalog.ts` stays at 140 lines. It holds
**57 entries covering UI §1.3's 59 coverage rows**: eight §1.3 rows are "dissolved" (a section
description, a pair label) and claim no entry of their own, while the per-profile card rows expand;
`settingsCatalogRows.test.ts` pins the 1..59 mapping literally and asserts
`owner.size === SETTINGS_INDEX.length`, so the arithmetic cannot drift silently either way.

`SETTINGS_CATEGORIES` deliberately has **no `Page` field**: the page components are zipped in from
`CATEGORY_PAGES` in `settings/categories/index.ts`, which is what keeps the catalog React-free and
its test DOM-free. That map's entry type also carries an optional `HeaderTrailing`, so a category can
place one control in the pane header without `SettingsShell` special-casing `git-config`.

### 4.2 The search mechanism (P69k) — binding

This is the load-bearing design decision of P69k and was only written down in code comments before
this pass.

**The pane's content is REPLACED while the query is non-empty.** Not the rail filtered, not the
current page filtered, not jump-and-highlight: `SettingsShell` renders `SettingsResults` instead of
`SettingsPaneHeader` + `Page`.

**There is no second renderer for the ~57 catalogued rows.** For each category that has a hit,
`SettingsResults` mounts that category's **real `Page`** (and its `HeaderTrailing`) inside a
`SettingsSearchContext.Provider`, and every `data-setting-id`-stamped row **self-filters** — it
`return null`s when a search is running and its id is not in the match set.

```ts
// src/components/settings/SettingsSearchContext.ts
export interface SettingsSearchState {
  /** Lowercased whitespace-split query terms. Never empty while a search runs. */
  terms: readonly string[];
  /** Ids of the rows that matched, across every category. */
  visible: ReadonlySet<SettingsRowId>;
  /** The category whose page this provider wraps — groups need it because group
   *  titles repeat across categories ("Identity" exists in two). */
  category: SettingsCategoryId;
}

export const SettingsSearchContext: React.Context<SettingsSearchState | null>;

/** The active search, or `null` when the pane is showing a plain category. */
export function useSettingsSearch(): SettingsSearchState | null;
/** False only when a search is running and this row is not one of its hits. */
export function useSettingsRowVisible(id: SettingsRowId): boolean;
/** False only when a search is running and none of the group's rows survived.
 *  Matched on (category, group title) via `SETTINGS_INDEX`. */
export function useSettingsGroupVisible(title: string): boolean;
```

Consequences, all binding:

1. **A result is live and editable in place** — it *is* the real control, with the real handler.
2. **A result cannot drift from the pane**, because it is the pane minus the non-matching rows.
   There is nothing to keep in sync when a page changes, and no `ROW_RENDERERS` registry (explicitly
   rejected: a second registry drifts).
3. **Absent provider ⇒ no search ⇒ everything renders.** This is what keeps every leaf-section suite
   valid.
4. **Any new hand-stamped row MUST consume the context**, or it renders unfiltered inside a result
   block. See §4.4 for the enumeration and the one that was missed.
5. Highlighting is **labels only**, via `highlightTerms(text, terms)` from `settingsHighlight.tsx`
   (merged non-overlapping ranges, so `set spend` over "Set a spend limit per run" yields two marks
   and never a nested pair). Help text is not highlighted — 12px `<mark>`s are noise. A term that
   matched only `keywords`, which are never displayed, simply produces no mark: the row still
   appears, it just has nothing visible to point at.
6. **No debounce.** Matching is a synchronous pass over ~57 static entries; a delay on a list this
   size only makes the box feel broken.
7. `SettingsResults` also renders `HeaderTrailing`, because exactly one catalogued row lives in the
   pane header rather than in a group (`git-config.scope`). It self-filters through the same context,
   so a category whose only hit is elsewhere shows no scope switch.
8. `Go to {Category}` clears the query and selects the category; clicking **any** rail item clears
   the query too, including a zero-count one — the rail is the way out of a search, not a second
   filtered view. `SettingsShell` resets `pane.scrollTop = 0` on every query change, category
   selection and deep link.
9. **Fifth consumer class, not a stamper:** a section that owns a note element which some *other*
   element points at with `aria-describedby` must consult the context as well, or the idref dangles
   when the row disappears (`ui-reference` §12.3.3). `SettingsAiLimits` does exactly this for its
   three gate notes. Dangling idrefs are worse than absent ones.

### 4.3 The DOM↔catalog anti-drift guard (Amendment A AM-4 / AM-6 / AM-7 folded in)

Three test files, one responsibility each, none over 500 lines. Per AM-7: if the coverage file
approaches the limit, split the **fixtures** into a data-only module — which is why
`coverageFixtures.ts` exists — never the assertions.

- `settingsCatalog.test.ts` (462) — catalog invariants, pure data, no DOM.
- `settingsCatalogRows.test.ts` (165) — UI §1.3 row bookkeeping and the `repeats` invariants.
- `settingsCatalog.coverage.test.tsx` (360) — the DOM↔catalog guard.
- `coverageFixtures.ts` (191) — `MAXIMAL` / `MINIMAL` / `FIXTURE_CONFIG_VIEW` / `FIXTURE_PROFILES`.

**Stamping is contract, not decoration.** `SettingsRow` stamps `data-setting-id={id}` on its root and
`data-setting-control` on the control wrapper; a `repeats` row also stamps `data-profile-id`. Both
the guard and e2e key on them.

#### 4.3.1 Fixtures and the requirement predicate table

```
MAXIMAL = { repoPath: '/repo', aiEnabled: true, aiConsented: true,
            mcpStatus: { enabled: true, allowWrite: true, port, toolCount },
            profiles: [P1, P2],            // stable ids 'p-1', 'p-2'
            terminalCommand: 'x', editorCommand: 'y',
            every numeric knob set OFF its default }
MINIMAL = { repoPath: null, aiEnabled: false, aiConsented: false,
            mcpStatus: { enabled: false }, profiles: [], all defaults }

REQUIREMENT_HOLDS = REQUIREMENT_PREDICATES        // IMPORTED from settingsAvailability.ts (§4.5)

REPEAT_INSTANCES: Record<SettingsRowRepeat, (fx) => readonly string[]> = {
  perProfile: fx => fx.profiles.map(p => p.id),
}
```

**AM-4 replaced §4.3's original rule "missing equals the ids of entries WITH a `requires` field",
which was unsound:** `ai.mcp-start` carries `requires: 'mcpStopped'` and renders *precisely* in the
minimal fixture, so the printed rule would have failed the first time an MCP row was migrated. With
the predicate table both fixtures get the same, stronger, symmetric check — the minimal fixture
becomes a **positive** check too, so a `mcpStopped` row that is missing while MCP is stopped now
fails.

The maximal fixture must give `git-config` at least one curated key and at least one custom key
(FAIL-K depends on it) and exactly two profiles with stable ids. Fixture data only — no IPC delta.

#### 4.3.2 Per-category algorithm (run for each fixture F ∈ {MAXIMAL, MINIMAL})

```
for each category c in SETTINGS_CATEGORIES:          // unconditionally — see §4.4
    render <SettingsPanel {...F} open initialCategory={c.id} />
    settle()                                          // one macrotask: Git config reads a ConfigView
    pane = getByRole('tabpanel')                       // scopes out rail, header, search bar

    entries  = SETTINGS_INDEX.filter(e => e.category === c.id)
    expected = entries.filter(e => e.requires === undefined || REQUIREMENT_HOLDS[e.requires](F))
    stamped  = [...pane.querySelectorAll('[data-setting-id]')]

    // (1) no nesting — a stamped row inside a stamped row breaks set-equality and search
    assert pane.querySelectorAll('[data-setting-id] [data-setting-id]').length === 0

    // (2) instance bookkeeping
    byId = groupBy(stamped, el => el.dataset.settingId)
    for (id, els) of byId:
        entry = findSettingsRow(id)
        assert entry !== undefined                         // FAIL-A
        assert entry.category === c.id                     // FAIL-B  (wrong-pane drift)
        if entry.repeats === undefined:
            assert els.length === 1                        // FAIL-C
            assert els[0].dataset.profileId === undefined  // FAIL-D
        else:
            got  = els.map(el => el.dataset.profileId)
            want = REPEAT_INSTANCES[entry.repeats](F)
            assert got has no undefined and no duplicates  // FAIL-E
            assert sorted(got) === sorted(want)            // FAIL-F (subsumes a bare count check)
            for el of els: assert el.closest('[data-profile-id]') === el
                                                           //   stamp sits on the row, not inherited

    // (3) set-equality, BOTH directions, deduped only where the catalog says so
    assert sorted(unique(keys(byId))) === sorted(expected.map(e => e.id))   // FAIL-G / FAIL-H

    // (4) per-instance shape and naming
    for entry of expected, for el of byId[entry.id]:
        if entry.control === 'group':
            assert el.getAttribute('role') === 'group'                     // FAIL-I
            assert accName(el) === entry.label                             // FAIL-J
            assert el.querySelectorAll('input,select,button,textarea').length > 0   // FAIL-K
        else:
            assert el.querySelector('[data-setting-control]') !== null     // FAIL-L
            assert within(el).getByRole(ROLE_FOR[entry.control],
                                        { name: entry.label }) resolves    // FAIL-M
        if entry.reset !== undefined:
            assert (↺ present in el) === !entry.reset.isDefault(valuesOf(F), DEFAULT_UI_SETTINGS)

    // (5) the wholly-gated pane
    if expected.length === 0:
        assert stamped.length === 0 and the SettingsEmpty element is present  // FAIL-N
```

`ROLE_FOR` stays in the test file: `switch`→`checkbox`, `segmented`/`radiogroup`→`radiogroup` named
by the row label (whose radios are named by their own option text — the row label is never a
radio's name), `numberSlider`→`spinbutton`, `text`→`textbox`, `button`→`button`;
`readonly` skips the role lookup and asserts the row's visible label text instead; `group` is
branch (4).

**`accName(el)` for a `'group'` row** — resolve by hand, no third-party dep:
`aria-label` if non-empty; else every id in `aria-labelledby` mapped through
`document.getElementById(id)?.textContent`, trimmed and space-joined; else fail with FAIL-J's "has no
accessible name" variant. Do **not** use `within(el).getByRole('group', …)` — Testing Library's
`within` searches descendants only and would never see the row element itself.

#### 4.3.3 `'group'` rows: the rendering contract they impose (AM-2)

`'readonly'` means only "a row whose value is displayed but not editable". A row standing for a
*block* of dynamically-generated controls is `'group'`, so the accessible-name check applies to the
block instead of being skipped — overloading `'readonly'` made the guard skip an entire pane section
while looking green. `git-config.behaviour` and `git-config.custom-keys` are the only two today.

```
<section role="group" aria-labelledby={hId} data-setting-id="git-config.behaviour">
  <h4 id={hId} className="settings-config-subtitle">Behaviour</h4>
  … curated controls …
</section>
```

- The heading text is the single source of the accessible name and **must** equal the catalog label
  byte-for-byte (`Behaviour`, `Custom keys` — the British spelling is deliberate and enforced).
- The outer `<details>` is **not** stamped; it is the "Advanced" group and carries the group classes.
  Its `<summary>` is the title. It may map to role `group` named `Advanced` in some ARIA versions;
  the guard's name-scoped lookup keeps that unambiguous because no catalog label is `Advanced`. If a
  future block is ever named `Advanced`, rename the block, not the guard.
- `settingsCatalog.test.ts`'s "buttons and read-only rows carry no `reset`" assertion includes
  `'group'`, and its control-kind union list includes `'group'`.
- **Compensating control (SHOULD):** each rendered curated key is stamped `data-config-key={key}` and
  `GitConfigAdvanced`'s own suite asserts the rendered key set equals the curated key list Rust
  returns. That restores per-key coverage inside the blind spot `'group'` opens, without putting
  repo-derived data in a static catalog.
- `GitConfigAdvanced` makes the same DEV `findSettingsRow` call `SettingsRow` makes, since its two
  ids are stamped outside `SettingsRow`.

#### 4.3.4 Repeated rows: naming (AM-6)

`IdentityProfileCard` is **not** a catalog row. Its root is
`<div role="group" aria-labelledby={cardTitleId} data-profile-id={p.id}>` and carries **no**
`data-setting-id`; the card title element carries `id={cardTitleId}`. Each of the six repeated rows
inside stamps **both** `data-setting-id` and `data-profile-id` on the `SettingsRow` root.
`identities.add` is unconditional, single-instance, and carries no `data-profile-id`.

The accessible name of every repeated control is a **constant** string equal to the catalog `label`.
It must not interpolate the profile label, and the guard observes the row in its idle state (no
`Applying…`, no delete-confirm swap). Per-profile disambiguation for screen readers comes from the
enclosing `role="group"` name, which is exactly what that role is for. **Any control whose name must
vary is a contract violation, not a guard exception.**

Because a repeated row exists once per card, its label/help element ids are per-instance
(`settingsRowLabelId(id, profileId)`), or the DOM would carry N duplicates of each and every card's
`aria-describedby` would resolve to the first one.

#### 4.3.5 Failure messages (mandatory wording — drift must be diagnosable)

Every assertion passes a message of the form `settings drift [<category>]: …`; the fixture name
(`maximal`/`minimal`) lives in the test title, not in every message. Minimum set:

- **FAIL-A** `rendered but not in the catalog — data-setting-id="general.foo". Add an entry to catalog/ or remove the stamp.`
- **FAIL-B** `"general.foo" is rendered on the graph pane but the catalog files it under general. Move the row or fix its category.`
- **FAIL-C** `"ai.model" rendered 3× but is not repeats:'perProfile'. A row rendered twice is a bug; if it is genuinely repeated, declare repeats.`
- **FAIL-D** `"ai.model" carries data-profile-id but declares no repeats.`
- **FAIL-E** `"identities.profile-name" has instances with missing/duplicate data-profile-id: [undefined, "p-1", "p-1"].`
- **FAIL-F** `"identities.profile-name" rendered for profiles ["p-1"] but the fixture has ["p-1","p-2"]. One card is dropping the row.`
- **FAIL-G** `in the catalog but not rendered — "appearance.foo" (label "Foo"). Either the row was deleted/renamed, or it is now conditional and needs a requires: flag.`
- **FAIL-H** is FAIL-A restated at set level; prefer FAIL-A's per-id message and keep the set assertion as the backstop.
- **FAIL-I/J** `group row "git-config.behaviour" has accessible name "Behavior", expected "Behaviour". The <h4> and the catalog label must match byte-for-byte.`
- **FAIL-K** `group row "git-config.custom-keys" contains no controls in the maximal fixture — the mock fixture no longer exercises this block, so the guard is checking nothing.`
- **FAIL-L** `"general.auto-fetch" has no [data-setting-control] descendant.`
- **FAIL-M** `"graph.row-height" — no spinbutton named "Row height" inside the row. Catalog label and rendered accessible name disagree, so search would match text the user cannot see.`
- **FAIL-N** `no repo open, so the pane must render SettingsEmpty and zero rows; found 2 stamped rows.`

Pure-data invariants that complement the DOM half, in `settingsCatalog.test.ts`: ids unique and
shaped `${category}.${kebab-slug}`; every `SettingsCategoryId` in `SETTINGS_CATEGORIES` exactly once
and every entry's category present there; labels non-empty and unique **within a category** (the
guard that killed the two `Interval` rows); `keywords` lowercase, single-spaced, no duplicates, no
term under 2 chars; `searchSettings('')` returns `[]` and multi-term AND holds; every `reset` is a
no-op at defaults with a non-empty `defaultLabel`.

**DEV guard (complements, does not replace, the test):** `SettingsRow` calls `findSettingsRow(id)`
under `import.meta.env.DEV` and `console.error`s (never throws) when the id is unknown, so a
mid-development row shows up in the harness console immediately.

#### 4.3.6 What the guard is blind to (write it down, don't rediscover it)

1. **Any control with no `data-setting-id`.** The guard's coverage floor is the stamp. UI §1.3 and
   `settingsCatalogRows.test.ts`'s 59-row literal map are what stop a row from being born unstamped.
2. **Everything inside a `'group'` row.** Curated Behaviour keys and every custom key are invisible:
   a key silently disappearing, a control there losing its label, or a fifth key appearing all pass.
   Mitigation is §4.3.3's `data-config-key` check plus `GitConfigAdvanced.test.tsx`. Search is
   correspondingly coarse — it can find "Behaviour", not "pull.rebase". **Accepted**: those rows are
   repo-derived, so no static catalog can own them.
3. **Per-profile *content*.** Each declared row exists once per profile; nothing checks it is bound
   to the right profile's data (a card wired to `profiles[0]` for every index passes).
   `SettingsProfilesSection`'s own tests own that.
4. **Transient control names.** Idle state only; `Applying…` and delete-confirm labels are unchecked
   by construction, which §4.3.4 makes safe by forbidding varying names.
5. **States neither fixture reaches** — AI enabled but not consented, an unborn HEAD. Two fixtures is
   the contract.
6. **Ordering.** Set-equality ignores DOM order, so a row moving between groups *within* a category
   is invisible. Group membership is data-only. Not worth coupling the guard to visual layout.

### 4.4 The stampers — there are FOUR (AM-5's successor bookkeeping)

**AM-5 is closed.** The `MIGRATED` / `PENDING` partition existed only to gate search on every
category being catalog-shaped. `PENDING` reached `[]` in P69j, search shipped in P69k, and the
partition plus its two tripwire tests were deleted in the same increment. The guard now loops over
`SETTINGS_CATEGORIES` unconditionally and **there is no longer a way to opt a category out of it.**
Do not reintroduce one.

What replaces that bookkeeping as the thing to keep honest is the **stamper count**. Four components
stamp `data-setting-id`, and each one must consume `SettingsSearchContext`:

| # | Component | What it stamps |
|---|---|---|
| 1 | `SettingsRow` | Every ordinary row (+ `data-profile-id` for a `repeats` row). |
| 2 | `IdentityProfileCard`'s `ProfileActionCell` | The Apply / Delete action cells, which are not `SettingsRow`s. |
| 3 | `GitConfigScope`'s `GitConfigScopeSwitch` | `git-config.scope` — the one catalogued row that renders in the pane **header**. |
| 4 | `GitConfigAdvanced` | The two `role="group"` aggregate blocks, `git-config.behaviour` and `git-config.custom-keys`. |

**#4 was missed in P69k's first cut** — and it is exactly the failure mode §4.2 consequence 4 warns
about: a search that hit git-config anywhere rendered the *whole* Advanced form, both blocks and the
add row, as a "result". Recorded here as the cautionary case, because it will recur the next time
someone hand-stamps a row.

`GitConfigAdvanced`'s `<details>` is additionally forced **open** while searching
(`open={searching ? true : undefined}`, uncontrolled otherwise so the user's own disclosure state
stays theirs): a hit inside a collapsed disclosure is an invisible result. The `<details>` itself
disappears via `useSettingsGroupVisible('Advanced')` when neither block survived.

### 4.5 `settingsAvailability.ts` — one definition of the `requires` preconditions

```ts
// src/components/settings/settingsAvailability.ts   (pure data: no React, no IPC, no DOM)

/** The runtime facts the `requires` predicates read. STRUCTURAL on purpose. */
export interface SettingsAvailability {
  repoPath: string | null;
  aiEnabled: boolean;
  aiConsented: boolean;
  mcpStatus: { enabled: boolean } | null;
  profiles: readonly unknown[];
}

/** One predicate per member of `SettingsRowRequirement` — the union IS the contract,
 *  so all five are implemented even though the catalog uses three today. */
export const REQUIREMENT_PREDICATES: Readonly<
  Record<SettingsRowRequirement, (available: SettingsAvailability) => boolean>
>;

/** True when this row renders under these conditions. No `requires` ⇒ always. */
export function isRowAvailable(entry: SettingsIndexEntry, availability: SettingsAvailability): boolean;
```

**Why it exists (binding rationale).** A `requires` row that fails its precondition is not in the
DOM, so matching it reports a count nobody can see. Without the filter, `bearer` with the MCP server
stopped reported "1 settings match" over a result block containing zero rows, and a query whose only
hits were unavailable never reached the zero-match state. `searchSettings` therefore filters with
`isRowAvailable`, and every consumer — the `role="status"` count, the rail counts, the result list —
reads that one list, so they cannot disagree.

**The anti-drift mechanism is the import direction, and it is binding.** The DOM↔catalog coverage
guard **imports** `REQUIREMENT_PREDICATES` as its §4.3.1 `REQUIREMENT_HOLDS` table rather than
restating it. A hand-copied table in the test was free to drift from the one search uses, and the two
would then disagree about whether a row exists at all. **Do not inline a second copy of these
predicates anywhere.**

`SettingsAvailability` is deliberately **structural**, not a `Pick<SettingsValues, …>`: both the live
values bag (`SettingsShell` builds one memoised object from `repoPath`, `aiEnabled`, `aiConsented`,
`mcpStatus`, `profiles`) and the coverage fixtures satisfy it without either side importing the
other's type — and widening `SettingsRowRequirement` in `types.ts` becomes a **compile error in this
module**, which is where the missing predicate belongs.

### 4.6 A search result is never a focus target (hazard, not a nicety)

`SettingsResults` mounts a category's real page. Therefore **a page whose section has a one-shot
focus effect guarded by a *per-mount* ref will re-arm on every keystroke that changes the hit set.**

This shipped as a real defect: after a `configMissing` deep link, typing a query that hit git-config
pulled focus into the `user.name` field mid-typing — and that field commits on blur, so the tail of
the query could be written into the repository's **real git config**.

The fix, and the general rule:

```tsx
// src/components/settings/categories/GitConfigCategory.tsx
const searching = useSettingsSearch() !== null;
…
initialFocus={searching ? undefined : configInitialFocus}
```

**Rule: no page may move focus, scroll, or fire a side effect on behalf of a deep link while a
search is running.** Deep-link focus targets are passed only when `useSettingsSearch() === null`.
The non-search deep link (the commit-error "Set identity…" linkage) is unaffected and is covered by
e2e. Any future `initialFocus`-style prop inherits this rule.

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

**The shipped module is larger than that two-function surface**, because P69h collapsed the three
mount reads of §2.4 into one. The extra exports are contract too:

```ts
/** Derive an identity from a ConfigView the CALLER already fetched. Pure. */
export function identityFromConfigView(view: ConfigView): EffectiveIdentity;
/** Announce "I am about to read this repo's config"; returns a claim seq. A consumer
 *  that mounts while the claimed read is pending sees the PENDING snapshot and issues
 *  no second call — this is what actually removes the duplicate. */
export function claimEffectiveIdentity(repoId: string): number;
/** Publish a claimed read's result. Stale claims (repo switched, invalidated) are dropped. */
export function primeEffectiveIdentity(repoId: string, localView: ConfigView, claimSeq?: number): void;
/** Publish a claimed read's failure. Same staleness rule. */
export function failEffectiveIdentity(repoId: string, message: string, claimSeq?: number): void;
/** Test-only: clear the cache, in-flight set and generation counters. */
export function resetEffectiveIdentityForTests(): void;
```

The Git-config section, which must fetch a `ConfigView` for its own form anyway, therefore
`claim`s and then `prime`s the store instead of letting the hook issue a second `getConfig`.

**Implementation constraints (no bodies):**
- **ONE** `ipc.getConfig(repoId, 'local')` call per repo. `CuratedConfigEntry` already carries
  `effectiveValue` + `effectiveLevel`, so the global fallback needs no second call — cheaper and
  atomic versus the UI contract's two-call design (§8, D-2).
- A module-level `Map<string, EffectiveIdentity>` + a listener set + a per-repo generation counter,
  read through `useSyncExternalStore`. Three surfaces (`IdentityMenu`, `SettingsProfilesSection`,
  `GitConfigCategory`) must never disagree, which rules out per-component `useState`.
- Cache invalidation triggers, exhaustively: repo switch (different key),
  `invalidateEffectiveIdentity` after a successful `applyIdentityProfile` / `setConfig` /
  `unsetConfig` of a `user.*` key, and repo close (delete the entry). **No `repo-changed`
  subscription** — `setConfig` deliberately does not emit it.
- Out-of-band edits (`git config` in a terminal) leave the cache stale until the next invalidation.
  Accepted: the Git-config pane's existing refetch/`Try again` is the manual refresh, matching the
  house watcher rule. Do **not** add a focus-rescan for this — it would fire a `getConfig` on every
  window focus for a value that changes monthly.
- In-flight dedupe: a second `useEffectiveIdentity(sameRepo)` mount during the first fetch, or during
  a claimed read, must not issue a second call.
- The generation counter is what makes a late `prime`/`fail` from a superseded read a no-op rather
  than a resurrection of stale data.

### 5.2 Header extraction

```ts
// src/components/HeaderToolbar.tsx
export interface HeaderToolbarProps {
  theme: Theme;
  onToggleTheme(): void;
  listView: ListView;
  onToggleListView(): void;
  /** Gates 🤖 / 📊 / the identity trigger, exactly as App did. */
  activeRepo: string | null;
  onOpenAiAssets(): void;
  onOpenHealth(): void;
  onOpenSettings(): void;
  /** Deep link for the identity menu's items 2/3/5 (UI §4.3). */
  onOpenSettingsAt(category: SettingsCategoryId, focus?: 'identity' | null): void;
  /** Lifted so App's Esc handler and global shortcuts keep early-returning while a
   *  menu is open — the TabStrip precedent. */
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

### 5.3 App.tsx delta (net negative — shipped at 1065 vs a 1168 baseline)

```ts
interface SettingsRequest { category: SettingsCategoryId | null; focus: 'identity' | null; seq: number }
const [settingsReq, setSettingsReq] = useState<SettingsRequest>({ category: null, focus: null, seq: 0 });
const openSettingsAt = useCallback((category: SettingsCategoryId | null, focus: 'identity' | null = null) => {
  setSettingsReq((r) => ({ category, focus, seq: r.seq + 1 }));
  setSettingsOpen(true);
}, []);
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
   open**, and its `useState<SettingsCategoryId>(requested ?? 'general')` is correct by
   construction — the category is selected on the very first render, before any child effect runs.
   `requested = initialCategory ?? (configInitialFocus === 'identity' ? 'git-config' : null)`, so the
   focus-only deep link selects the right pane without App having to name it twice.
2. `GitConfigCategory` (and thus `SettingsGitConfigSection`) only mounts when `git-config` is
   selected, so the existing scroll+focus effect fires with its `focusedOnce` guard intact.
3. Already-open case (commit fails while Settings is open): `SettingsShell` runs an effect keyed on
   the monotonic `requestSeq` — not on an `open` transition — that re-seeds the category and clears
   the query, so a second deep link in the same session still lands. `requestSeq` must change on
   *every* open path, including the plain ⚙ click (which passes `category: null` and therefore only
   clears state; it must never yank the user off the category they are reading).
4. **Page remount for a repeat focus request.** A deep link that asks for a focus target must re-run
   the target page's focus effect even when that page is already mounted (Settings open, already on
   Git config, second failed commit). `SettingsShell` therefore keys the page
   `` `${category.id}:${requestSeq}` `` **only** for `git-config` + `configInitialFocus === 'identity'`;
   ordinary category switches keep a stable key. Remounting is the honest way to say "this is a new
   request".
5. Initial focus on open is the **search input** — a text field, so no keystroke can activate
   anything, and it is the fastest route for a user who knows the setting's name but not its
   category. **Deep-linked opens are the exception and must stay one**: a search box that grabbed
   focus would silently defeat the commit-error linkage, so `SettingsShell` skips the focus when the
   open was deep-linked.
6. Acceptance test (e2e + vitest): with `?fixture=noconfig`, a failed commit's "Set identity…" opens
   Settings with the rail's `Git config` tab selected (`aria-selected="true"`) and focus on the
   `user.name` input.

---

## 6. Decision 5 — the increment sequence (all shipped)

Ten increments on top of the pre-contract P69a/P69b. Ordering constraints honoured: (a) each left
the app compiling and green; (b) behaviour before layout; (c) P69c–P69f touched no CSS and could run
while a concurrent session held `src/styles.css` — **P69g was the CSS gate**; (d) the `NumberSlider`
semantics question was settled in P69c and isolated to one file.

This supersedes both `TODO.md`'s "P69c primitives → P69h" list and UI §9.5's A→B→C→D. Mapping: UI's
A ≈ P69f+P69g, B ≈ P69h, C ≈ P69i, D ≈ P69j+P69k.

### P69c — `NumberSlider` typing fix (no CSS) — **shipped**
- **Goal.** Make two-digit entry possible without changing patch semantics.
- **Files:** `src/components/NumberSlider.tsx` only.
- **Contract.** Hold a `draft: string | null` for the number input's *display*. On `change`: set
  `draft`, and commit the clamped value exactly as before (per keystroke). On `blur` and on `Enter`:
  `setDraft(null)` so the field snaps back to the canonical clamped value. `draft` is cleared
  whenever the incoming `value` prop changes from a source other than this field. The range input is
  untouched — it must keep committing per `change` (`e2e/10-settings-persistence.spec.ts` drives it
  with arrow keys).
- **Acceptance.** Typing `3` then `0` in Row height (min 24) ends at 30, and the field never shows
  `24` mid-typing; blurring a blank field restores the current value; a blank/NaN field patches
  nothing.
- **Tests.** All three existing per-keystroke suites stay green and unmodified. New:
  `src/components/NumberSlider.test.tsx` pinning the draft display, the blur snap-back, the Enter
  commit.
- **Resolved:** OQ-1 — the recommended draft-display semantics were taken.

### P69d — standalone behaviour + a11y fixes and the two required splits (no CSS) — **shipped**
- **Files:** `SettingsPanel.tsx` (two labels), `SettingsAiRunSection.tsx` / `SettingsAiLimits.tsx`,
  `SettingsGitConfigSection.tsx` → `settings/GitConfigAdvanced.tsx`,
  `SettingsProfilesSection.tsx` → `settings/IdentityProfileCard.tsx`, new
  `src/hooks/useEffectiveIdentity.ts`.
- **Contract.** (1) The two `Interval` rows became `Fetch every` / `Refresh every` (UI §5.3.7); ids
  unchanged. (2) The AI-runs gate note moved to the top of the group with `id="ai-run-gate-note"` and
  `aria-describedby` on the `<fieldset>`. (3) `useEffectiveIdentity` landed and the profiles
  section's local-only match logic was deleted; the "Active on this repo" pill became
  effective-based (closes UI D6). (4) The two splits.
- **Acceptance.** Two distinct accessible names in Background jobs; the profiles pill lights up in
  the default harness state; `pnpm lint:size` clean.
- **Tests.** New `src/hooks/useEffectiveIdentity.test.tsx` (local wins over global; global-only;
  unset; reject → `error`; one `getConfig` per repo; invalidation refetches; two consumers agree;
  `repoId === null` makes no call).

### P69e — the data layer (no CSS, no UI change) — **shipped**
- **Files:** new `src/settings/uiSettingsDefaults.json`, `defaults.ts`, `defaults.test.ts`,
  `src/components/settings/types.ts`, `settingsCatalog.ts` + `catalog/*.ts`,
  `settingsCatalog.test.ts`, `settingsCatalogRows.test.ts`; modified
  `src/ipc/mock/persistence.ts`.
- **Contract.** §3 and §4.1 + §4.3's pure-data half. Catalog entries for all of UI §1.3's rows landed
  here, even for categories not yet re-skinned.
- **Deviation found and ruled on during this increment:** the `'profile'` requirement and the
  unsoundness of the original minimal-fixture rule — now §4.1 and §4.3.1.
- **Closed in P69l:** the Rust parity test landed as `settings_defaults_parity_tests.rs` (OQ-2).

### P69f — props → context, behind the current layout (refactorer; no CSS) — **shipped**
- **Files:** new `settings/SettingsContext.ts` + `SettingsProvider.tsx` +
  `useSettingsPanelAdapter.ts`, `settings/categories/*.tsx` (7) + `categories/index.ts`; modified
  `SettingsPanel.tsx`; `SettingsAppearanceSection.tsx` deleted (markup moved verbatim into
  `AppearanceCategory`).
- **Contract.** §2.2–2.4. Pages render as fragments, one after another, in the new rail order inside
  the existing `.settings-card`. No class, role, name, or id changes. Leaf sections keep their props.
- **Acceptance.** The four equivalence conditions in §2.4, including **no existing test file edited**.

### P69g — the two-pane shell + primitives + row reset (**first CSS increment**) — **shipped**
- **Files:** new `SettingsShell.tsx`, `SettingsRail.tsx`, `SettingsPaneHeader.tsx`,
  `SettingsGroup.tsx`, `SettingsRow.tsx`, `SettingsSwitch.tsx`, `SettingsSwitchRow.tsx`,
  `SettingsSegmented.tsx`, `SettingsEmpty.tsx`, `settingsCatalog.coverage.test.tsx`,
  `coverageFixtures.ts`; re-skin `GeneralCategory`, `AppearanceCategory`, `AboutCategory` (+ the leaf
  sections they own); modified `SettingsPanel.tsx`, `src/styles.css`, `src/App.tsx`.
- **Contract.** UI §2 geometry, §5.1/5.5/5.6/5.7, §6, §7; §4.3 in full. Rail = `role="tablist"` with
  `role="tab"` items, `aria-controls` → the pane's `role="tabpanel"` + `aria-labelledby`;
  **manual activation** (D-5).
- **Acceptance.** `getByRole('dialog', { name: 'Settings' })` still resolves; 7 tabs; the selected tab
  has `aria-selected="true"`; every migrated row carries `data-setting-id`; `↺` appears exactly when
  a value differs from its default and patches it back.

### P69h — Git config category + scope + shortcuts — **shipped**
- **Files:** `settings/categories/GitConfigCategory.tsx`, new `GitConfigScope.tsx` +
  `GitConfigScopeContext.ts` + `CuratedConfigControl.tsx` + `useGitConfigEditor.ts`; modified
  `SettingsGitConfigSection.tsx`, `SettingsShell.tsx`, `App.tsx`, `styles.css`,
  `SettingsHooksToggle.tsx`.
- **Contract.** UI §1.1 pill + §1.2 empty block + §4.7 deep link + §5.4 mechanism above. The three
  mount `getConfig` reads of §2.4 collapse to one via §5.1's claim/prime API.
- **Acceptance.** Scope switch in the pane header naming the real file; `SettingsEmpty` with a working
  `Open repository…` when no repo; the `configMissing` deep link lands on Git config → Identity with
  focus in `user.name`, both when Settings is closed and when it is already open; `Ctrl/Cmd+,` opens
  Settings.

### P69i — identity extraction — **shipped**
- **Files:** new `HeaderToolbar.tsx`, `IdentityMenu.tsx`, `IdentityAvatar.tsx`; modified `App.tsx`,
  `ContextMenu.tsx` (+3 additive fields), `IdentitiesCategory.tsx`, `SettingsProfilesSection.tsx`,
  `IdentityProfileCard.tsx`, `styles.css`; new mock fixtures (§7.3).
- **Contract.** UI §4 in full; §5.2/5.3 above; §4.3.4's repeat/naming rules; `ContextMenu`
  `checked`/`detail`/`header` remain byte-identical for existing call sites when absent.
- **Acceptance.** All four trigger states in the harness; a checked `menuitemradio` under
  `?fixture=identitymatch`; the confirm fires only for a *differing local* identity; `App.tsx` line
  count strictly below its pre-P69i value; global shortcuts suppressed while the menu is open.

### P69j — graph + AI/MCP re-skin — **shipped**
- **Files:** `SettingsGraphSection.tsx`, `SettingsAiSection.tsx`, `SettingsAiRunSection.tsx`,
  `SettingsAiLimits.tsx`, `SettingsMcpSection.tsx`, `GraphCategory.tsx`, `AiCategory.tsx`,
  `styles.css`.
- **Contract.** Every remaining row onto the primitives; `Repository access` → segmented;
  `Date basis` → segmented over the same radios; conflict-resolution stays a radio group.
  **Frozen:** `#settings-graph-row`, `Row height`, and the eight graph toggle names (UI §11).
- **Acceptance.** Coverage guard green for `graph` and `ai` — and `PENDING` reaches `[]` here, which
  is the precondition P69k depended on.

### P69k — search — **shipped**
- **Files:** new `SettingsSearchBar.tsx`, `SettingsResults.tsx`, `SettingsSearchContext.ts`,
  `settingsHighlight.tsx`, `settingsAvailability.ts`, `SettingsSearch.test.tsx`,
  `e2e/25-settings-search.spec.ts`; modified `SettingsShell.tsx`, `SettingsRail.tsx`,
  `SettingsRow.tsx`, `SettingsGroup.tsx`, `settingsCatalog.ts`, `IdentityProfileCard.tsx`,
  `GitConfigScope.tsx`, `GitConfigAdvanced.tsx`, `GitConfigCategory.tsx`, `SettingsAiLimits.tsx`,
  `settingsCatalog.coverage.test.tsx`, `styles.css`.
- **Contract.** UI §3; §4.2, §4.4, §4.5 and §4.6 above. AM-5's `PENDING === []` precondition was met
  in P69j; the partition and its two tripwire tests were **deleted** here (§4.4) — search is only
  truthful once every row is catalog-backed, which is why it shipped last (§8, D-3).
- **Acceptance.** AND matching over the whole index; unavailable rows are never matches; results are
  live-editable in place; rail match counts; zero-match block; `role="status"` announcement; first
  Esc clears, second closes; a deep link beats a live query; no dangling `aria-describedby` idref in
  any result block.

### P69l — docs — **this pass**
- `docs/contracts/P69-settings-shell.md` (this file) amended against the shipped tree;
  `P69-settings-shell-amendment-A.md` folded in and reduced to a stub. `TODO.md`, `CHANGELOG.md`,
  `docs/contracts/INDEX.md` are the orchestrator's / `docs-curator`'s; `P69-settings-ui.md`,
  `P69c-draft-feedback-ui.md` and `ui-reference.md` §12 are `ui-designer`'s (concurrent). No code.

---

## 7. Decision 6 — test-impact map

### 7.1 Existing suites

| File | Breaks when | What breaks | Minimal **honest** update |
|---|---|---|---|
| `src/components/SettingsPanel.test.tsx` | P69g | Everything that queries a control now behind a rail tab. `renderPanel` renders one pane, not eleven sections. | One local helper `selectCategory(id)` (clicks `getByRole('tab', { name })`), called at the top of the tests for non-default categories. General is the default pane → auto-fetch/health tests unchanged. **Not a weakening** — the control genuinely lives behind one click. |
| ″ | P69g | `getByRole('button', { name: 'Dark' })` / `'Flat'` / `'Cozy'` / `'Compact'`. **Behaviour genuinely changed** — a self-labelling toggle button became a 2-option segmented radio group. | Rewrite to `getByRole('radio', …)`. **Add** the assertion the old button could not have: clicking the *already selected* segment fires nothing (the old button always toggled). |
| ″ | P69d | The two `Interval` names. | Query `Fetch every` / `Refresh every`. Ids unchanged, so the `getElementById` assertions stay. |
| `src/components/SettingsSections.test.tsx` | P69g | "reset is disabled when empty (auto-detect) and clears a set template" — the dedicated `Reset to auto-detect` button is replaced by the row `↺`, which is **absent** (not disabled) at the default. **Behaviour genuinely changed.** | Rewrite as three assertions: at `terminalCommand: ''` the reset button is `null`; with a set template it exists with `title="Reset to default (auto-detect)"`; clicking it patches `{ terminalCommand: '' }`. |
| ″ | P69j | Graph section re-skin. Roles/names/ids are preserved by design, so it should pass untouched. | No change expected. If a query breaks, the re-skin is wrong — fix the component, not the test. |
| `src/components/SettingsAiRunSection.test.tsx` | P69j | "the access button has a name that says what it controls" and "the read grant is disclosed in words" — the self-labelling button became segmented. **Behaviour genuinely changed.** | Rewrite to `getByRole('radiogroup', { name: 'Repository access' })` containing exactly two radios, `Read-only` and `No file access`; keep the "no radio's name may mention write/modify" intent verbatim; keep the words-not-icons disclosure assertion against the row help text. |
| ″ | P69d | Gate-note position (text unchanged) and describedby-targets-exist. | Position assertions pass unchanged; extend the describedby test to assert the `<fieldset>`'s `aria-describedby` resolves to `#ai-run-gate-note`. |
| ″ | P69c | Nothing, under the shipped draft-display fix. | None. |
| `src/settings/ranges.test.ts` | never | Nothing. | None. New sibling `defaults.test.ts` carries the defaults mirror. |
| `e2e/10-settings-persistence.spec.ts` | P69g/P69j | The graph controls are now behind the `Commit graph` tab. | Insert `await dialog.getByRole('tab', { name: 'Commit graph' }).click();` after each `openSettings` (two places). Slider keyboard steps, `#settings-graph-row`, and the `localStorage` poll are unaffected. |
| ″ | P69i | `Switch to light/dark theme` — page-level header buttons moved into `HeaderToolbar`. | No change: the extraction is verbatim, same `aria-label`s. This pair is the regression oracle for the extraction. |
| ″ | never | List-view via the command palette; corrupt-settings boot. | None. |

### 7.2 New test files — as shipped

| File | LOC | Increment | Covers |
|---|---|---|---|
| `src/components/NumberSlider.test.tsx` | — | P69c | Draft display, blur snap-back, Enter, blank/NaN, external value change. |
| `src/hooks/useEffectiveIdentity.test.tsx` | — | P69d | local/global/unset/error, one call per repo, in-flight dedupe, invalidation, two consumers agree, `repoId === null` makes no call, claim/prime staleness. |
| `src/settings/defaults.test.ts` | 122 | P69e | Deep-equal against the JSON oracle + spot pins + `ENV_DERIVED_DEFAULT_KEYS === []`. |
| `src/components/settings/settingsCatalog.test.ts` | 462 | P69e | §4.3.5's pure-data invariants + the control-kind union. **Watch its size.** |
| `src/components/settings/settingsCatalogRows.test.ts` | 165 | P69e | UI §1.3's 59-row literal map, the dissolved-row accounting, the AM-1 `repeats` invariants. |
| `src/components/settings/settingsCatalog.coverage.test.tsx` | 360 | P69g, grew each increment | §4.3.2 in full, over both fixtures, over all 7 categories unconditionally. Imports `REQUIREMENT_PREDICATES` (§4.5). |
| `src/components/settings/coverageFixtures.ts` | 191 | P69g | `MAXIMAL` / `MINIMAL` / `FIXTURE_CONFIG_VIEW` / `FIXTURE_PROFILES`. Data only (AM-7). |
| `src/components/settings/SettingsPrimitives.test.tsx` | 206 | P69g | Switch = native checkbox with the right name/checked/disabled/describedby; segmented = radiogroup + radios, max-3 dev warning; row anatomy incl. stacked + help id wiring + the stamp. |
| `src/components/settings/SettingsShell.test.tsx` | 207 | P69g | Tab roles + roving tabindex + Home/End, manual activation, `initialCategory` seeding, `requestSeq` re-seed while open, scroll reset, focus restore, backdrop/✕ close. |
| `src/components/settings/SettingsSearch.test.tsx` | 421 | P69k | **Named for the mechanism, not the component** (the draft called it `SettingsResults.test.tsx`): cross-category results, every stamped row self-filtering, unavailable rows are not matches, no dangling idref, AND matching, `<mark>`, group headers, the rail, the live region, zero-match, focus + Escape layering, and a deep link beating a live query. **Watch its size.** |
| `src/components/settings/GitConfigScope.test.tsx` | 136 | P69h | Scope switch naming the real file, provider wiring, the header-slot stamp. |
| `src/components/settings/GitConfigAdvanced.test.tsx` | 119 | P69h | The two `'group'` blocks' stamps and names, the unstamped `<details>`, §4.3.3's `data-config-key` set check. |
| `src/components/IdentityMenu.test.tsx` | — | P69i | 4 trigger states, initials, `menuitemradio` + `aria-checked`, confirm only on differing local, `Applying…`, success/error toasts, `onMenuOpenChange`. |
| `src/components/HeaderToolbar.test.tsx` | — | P69i | Every button's name/gating; menu-open state lifts. |
| `e2e/24-settings-shell.spec.ts` | — | P69h/P69i | Rail navigation, a graph knob surviving a category switch + reload, the `configMissing` deep link, `Ctrl/Cmd+,`, the identity menu against `?fixture=identitymatch`. |
| `e2e/25-settings-search.spec.ts` | — | P69k | Search across categories in a real browser, live editing of a result, Escape layering. |
| `src-tauri/src/settings_defaults_parity_tests.rs` | 141 | P69l | Serde defaults <-> JSON oracle, + a test pinning `aiMaxBudgetUsd` as a JSON float. **LANDED.** |

### 7.3 Mock/harness deltas (no new commands)

| Fixture | Where | Change |
|---|---|---|
| `?fixture=identitymatch` | `src/ipc/mock/repoState.ts`, `src/ipc/fixtures/config.ts` | Seeds **local** `user.name = Ada Lovelace`, `user.email = work@bonsai.dev`, and a matching profile in the settings seed (`MockIdentityFixture = 'localMatch'`). |
| `?fixture=noconfig` | ″ | Drops the seeded identity, so the commit-error → "Set identity…" deep link is reachable. |
| `?fixture=configerror` | `src/ipc/mock/handlers/config.ts` | `getConfig` rejects → identity state 3 with the "couldn't read" title + the pane's error banner. |
| `?fixture=slowconfig` | ″ | Delay `getConfig` → skeleton rows + the `·` loading circle. |
| `?fixture=longsettings` | `repoState.ts` + settings seed | 120-char profile label, 90-char email, 300-char `terminalCommand`, long custom key. Ellipsis + `title` proof. |
| `profiles: []` | `localStorage bonsai.mockUiSettings` | Drives the Identities empty state + `Add an identity…`. |
| `aiConsented: true` | ″ | Required to see the AI run knobs enabled (known harness step) — and required for `requires: 'aiActive'` rows to be searchable at all (§4.5). |

Every fixture is a data seed behind the existing `?fixture=` seam — the mock layer keeps serving the
whole Settings surface in a plain browser with `VITE_MOCK_IPC=1`.

---

## 8. Deviations, and what is still open

Deviations from the UI contract (the UI contract's intent is preserved in each):

- **D-1 · `settingsIndex.ts` → `settingsCatalog.ts`.** The module carries control kind, conditional
  render requirements, repeats and reset descriptors, not just search text. Rows live in
  `catalog/*.ts`. Do not create a second index module.
- **D-2 · `useEffectiveIdentity` makes ONE `getConfig` call**, not a local-then-global pair.
  `CuratedConfigEntry.effectiveValue`/`effectiveLevel` already answer both questions atomically; two
  calls would also open a window where local and global disagree mid-read.
- **D-3 · Search shipped last (P69k), and the search bar was not rendered before it.** RESOLVED —
  the bar is live and the pane's `grid-template-rows` gained the search row in P69k. Rendering a box
  that could only find 3 of 7 categories' rows is a control that lies; rendering it disabled is a
  dead control. *Alternative considered:* a `migrated` flag in the catalog gating results — rejected,
  it puts increment bookkeeping into a data file that outlives the increment.
- **D-4 · No focus trap in P69.** UI §9.1 assigns `SettingsShell` "focus trap/restore". There is no
  shared focus-trap hook in this codebase and no dialog has one, so adding one to Settings only
  creates an inconsistency. **Focus *restore* shipped** (return focus to what was focused on mount,
  falling back to the ⚙ trigger, then `<body>`); the trap is deferred to a dialog-wide a11y milestone
  covering `ShortcutOverlay`, `AiAssetsPanel`, `RepoHealthPanel`, `ConfirmDialog` and Settings
  together.
- **D-5 · Rail activation is MANUAL, not automatic — and this supersedes `ui-reference.md` §12.4's
  "move and activate".** Arrows move focus only; **Enter/Space selects**; `→` hands focus to the
  pane. Automatic activation would fire a `getConfig` round-trip every time focus passed over
  `Git config` — arrowing from `AI` to `About` would hit the network. Consistent with UI §2.2's
  "focus moves to the pane only on keyboard activation". `SettingsRail.tsx:6-8` cites this ruling by
  name; `ui-designer` is reconciling the `ui-reference` side concurrently.
- **D-6 · The mock's `DEFAULT_UI_SETTINGS` composes production defaults** rather than re-exporting
  them (§3.3), because the two genuinely differ in `profiles`.
- **D-7 · `SettingsPanel` is 46 lines, not ~250, because the adapter moved out.** The plan had the
  panel holding the props interface, the two `useMemo`s and the consent wrappers; they live in
  `settings/useSettingsPanelAdapter.ts` (402 lines) with `SettingsPanelProps` declared beside them
  and re-exported from `SettingsPanel.tsx`, so no importer moved. Same reason the two context modules
  are `.ts` + a separate `.tsx` provider: `react-refresh/only-export-components`. Consequence to
  respect: **the adapter is now the file to watch**, and further growth belongs in a sibling hook,
  not in it.
- **D-8 · `searchSettings` takes a second argument.** §4.1's original one-argument signature could
  not tell whether a row was renderable, and shipped a status line, a rail count and a result block
  that disagreed with each other. `searchSettings(query, availability)` and §4.5's predicate module
  are the fix, and the coverage guard's **import** of those predicates is what keeps the production
  filter and the test table from drifting.
- **D-9 · Amendment A's rejection of §4.3's original minimal-fixture rule** is folded into §4.3.1.
  The rule "missing == ids with a `requires` field" was unsound and is gone; do not restore it.

Open:

- **OQ-2 · RESOLVED in P69l.** The parity chain's fourth link is live:
  `src-tauri/src/settings_defaults_parity_tests.rs` (141 lines) serialises
  `ui_settings_of(&Settings::default())` to `serde_json::Value` and deep-compares it against the
  oracle embedded with `include_str!`. **The two sides agreed on the first run** — all 30 keys
  including the four nested objects — so nothing on either side had to move, and
  `ENV_DERIVED_DEFAULT_KEYS` being empty means there was no legitimate divergence to exempt.
  Three implementation facts worth not re-deriving:
  - **It lives in its own module, declared from `lib.rs`, not under `settings`.** The ratchet
    forbids `settings.rs` (baselined at 663) from growing even by a 3-line `mod` declaration, and
    `settings_ui_tests.rs` is at 396. The test needs no `settings`-private items, so nothing is lost.
  - **The oracle is located at compile time** (`include_str!`, two `../` hops). A runtime relative
    read would depend on each `cargo nextest` process's cwd, and a moved oracle would silently skip
    the test instead of breaking the build. Cargo also tracks the file for rebuilds.
  - **`ui_settings_of` is now `pub(crate)`** — it was private, and there is no `UiSettings::default()`
    to serialise instead.
  Negative control verified twice, covering all three failure branches: a flipped `bool` default
  reports `VALUE mismatch at \`aiStreamLog\`: Rust = Bool(false), oracle = Bool(true)`, and a renamed
  serde field reports both `MISSING key` and `UNEXPECTED key` with remedial hints.
- **OQ-3 · `SettingsPanel`'s prop count is ~44**, even though page-level threading is eliminated.
  This is inherent to `useUiSettings` living in App. Collapsing it further means moving settings state
  into a provider App also consumes — a real option, but a separate milestone with its own risk to
  App's four state writers (`theme`, `listView`, `paneWidths`, `onboardingSeen`, hardened in P69b).
  **Recommendation: do not attempt it as a P69 follow-up.**
- **OQ-4 · `src/ipc/types.ts` slack.** P69 used none of it, as required. If a future pass needs
  `SettingsCategoryId` in the IPC layer (it should not — no command takes it), that slack is the only
  room available.
- Resolved and not relitigated: **OQ-1** (draft display + clamped commit per keystroke, P69c),
  **OQ-2** (the Rust parity link, P69l — see above),
  **A1** (CRUD in Settings → Identities), **A3/A4** (copy sign-off), **A5** ("Reset all settings" out
  of scope), **A6** (rail dividers), **AM-5** (`PENDING` reached `[]`, partition deleted).

---

## 9. Acceptance criteria (milestone level)

1. 7-category two-pane shell at 880px; every control from UI §1.3 reachable, each in exactly one
   place; nothing from the old panel lost. (57 catalog entries covering 59 §1.3 rows — §4.1.)
2. `getByRole('dialog', { name: 'Settings' })`, `#settings-graph-row`, `Row height`,
   `Switch to light/dark theme`, and all eight graph toggle names still resolve.
3. Every toggle is a native `<input type="checkbox">`; every exclusive choice is native radios.
4. Cross-category search finds any **renderable** row by label, help, or keyword, and edits it in
   place; an unavailable row is never counted and never offered (§4.5); no result is a focus
   target (§4.6).
5. The header identity trigger shows the **effective** identity and its source in all four states;
   applying confirms only when it would overwrite a differing local identity.
6. Per-row `↺` appears exactly when a value differs from its default and restores it.
7. `pnpm lint:size` clean: `App.tsx` strictly below 1168 (shipped 1065), `src/ipc/types.ts`
   unchanged, `src-tauri/src/settings.rs` unchanged at 663, no new file over 500.
8. `pnpm vitest` and `pnpm e2e` green with counts ≥ pre-P69, and `cargo test` green.
9. The harness serves every state listed in UI §10 with `VITE_MOCK_IPC=1`; **+0 Tauri commands,
   +0 events, +0 channels.**
10. **USER CHECKPOINT** (per UI §10): all visual proof, `Ctrl/Cmd+,` on macOS, a real `.git/config`
    write from the identity menu, scroll/focus-ring feel, and `:has()` support in the shipped
    WebView2 / WebKitGTK.
</content>
</invoke>
