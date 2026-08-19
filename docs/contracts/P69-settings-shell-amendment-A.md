# P69 — Amendment A to `P69-settings-shell.md` §4 (catalog + anti-drift guard)

Owner: `architect`. Status: **binding amendment**. Supersedes the marked clauses of §4.1 / §4.3 of
`docs/contracts/P69-settings-shell.md`; everything in §4 not named here stands unchanged.

> **Filing note (for the orchestrator):** this amendment was authored as a sibling file because the
> architect session had no `Edit` tool and a wholesale `Write` of the 866-line parent risked
> truncating it. Please add one pointer line under `P69-settings-shell.md` §4.3's heading:
> `> Amended by docs/contracts/P69-settings-shell-amendment-A.md (rulings AM-1…AM-6).`
> No other change to the parent file is needed.

**Trigger.** P69e implemented §4.3's pure-data half and found the DOM half cannot hold as written
for `identities` (one card per profile ⇒ duplicate ids) and `git-config` (dynamic Behaviour /
Custom-keys blocks ⇒ no single control carries the row's accessible name). The reviewer concurred.
The guard is written in **P69g**, so these rulings block that increment.

**Scope of authority.** P69g implements §4.3 **as amended here**, not as printed in the parent.
Where the two differ, this file wins.

---

## AM-0 — Summary of rulings

| # | Question | Ruling |
|---|---|---|
| AM-1 | `identities` renders N cards | **Adopt** the reviewer's shape: explicit `repeats?: 'perProfile'`, dual stamp `data-setting-id` + `data-profile-id`, dedupe **only** for flagged entries, and assert the instance set equals the fixture's profile-id set (stronger than a bare count). |
| AM-2 | `git-config` dynamic blocks | **Adopt** the reviewer's shape with one tightening: new control kind `'group'`, stamped on a `role="group"` element named by its heading via `aria-labelledby`, and the accessible-name check **applies** to it. `'readonly'` reverts to meaning only "a read-only value row". |
| AM-3 | `'profile'` requirement value | **Fold into §4.1's union permanently.** P69e's extension is correct and is now contract. |
| AM-4 | Conditional-row rule (§4.3's "missing equals ids of entries WITH a `requires` field") | **Rejected as written — it is unsound** (`mcpStopped` rows render precisely in the minimal fixture). Replaced by a per-requirement predicate table evaluated per fixture, giving set-equality on **both** fixtures. |
| AM-5 | `MIGRATED_CATEGORIES` | **Revised**: partition into `MIGRATED` + `PENDING`, both literal, union = all 7, intersection = ∅. Moves out of `settingsCatalog.test.ts` into the coverage file in P69g. |
| AM-6 | Repeated-row naming | Accessible names of repeated controls **must be profile-independent**; personalisation lives on the card's group name, never on the control. |

---

## AM-1 — `identities`: repeats, not an exemption

**Ruling: adopt the reviewer's recommendation.** Do not relax the no-duplicates rule globally and do
not exempt the category. Repetition is a *property of the row*, so the catalog declares it and the
guard checks it exactly.

Rationale: relaxing globally would silently swallow the real bug the rule exists to catch (a row
accidentally rendered twice, e.g. a section mounted in two pages after the P69f split). Exempting
the category would blind the guard to six of the eight identity rows — and identities is the one
category P69i actively rewrites.

### Rendering contract (P69i, and P69g's fixture must already satisfy it)

- `IdentityProfileCard` is **not** a catalog row. Its root is
  `<div role="group" aria-labelledby={cardTitleId} data-profile-id={p.id}>`; the card title element
  (`.settings-config-subtitle`, currently the profile label or `Untitled profile`) carries
  `id={cardTitleId}`. The card root carries **no** `data-setting-id`.
- Each of the six repeated rows inside the card stamps **both** attributes on the element that
  already carries `data-setting-id` (the `SettingsRow` root once re-skinned):
  `data-setting-id="identities.profile-label"` (etc.) **and** `data-profile-id={p.id}`.
- `identities.add` is unconditional, single-instance, and carries **no** `data-profile-id`.
- **AM-6:** the accessible name of every repeated control is a constant string equal to the catalog
  `label`. It must not interpolate the profile label, and the guard must observe the row in its idle
  state (no `Applying…`, no delete-confirm swap). If P69i wants per-profile disambiguation for
  screen readers, it comes from the enclosing `role="group"` name, which is exactly what that role
  is for. Any control whose name must vary is a contract violation, not a guard exception.

### Type change (`src/components/settings/types.ts`, stated in prose — do not edit from this file)

Add, next to `SettingsRowRequirement`:

- a new exported type `SettingsRowRepeat`, a string union whose only member today is `'perProfile'`,
  documented as "this row is rendered once per item of a runtime collection; the guard dedupes it
  and checks the instance set against that collection".
- a new **optional** field on `SettingsIndexEntry`, `repeats?: SettingsRowRepeat`, placed
  immediately after `requires`, documented as "absent ⇒ exactly one instance in the DOM".

No other field changes. `SettingsRowId` still identifies the *row*, not the instance — instance
identity is `(data-setting-id, data-profile-id)`.

### Catalog change (`catalog/repo.ts`)

The six `identities.*` entries that already carry `requires: 'profile'` gain `repeats: 'perProfile'`.
`identities.add` gains neither.

### Pure-data invariants to add (put them in `settingsCatalogRows.test.ts`, not the 430-line file)

1. `repeats === 'perProfile'` ⟺ `requires === 'profile'` (biconditional over the whole index).
2. Only `identities` may carry `repeats` (mirror of the existing `allowed` requirement table).
3. A `repeats` entry never carries `reset` (already implied by the identities-wide rule; assert it
   against `repeats` directly so a future repeated row in another category inherits the ban).

---

## AM-2 — `git-config`: a real `'group'` kind, not overloaded `'readonly'`

**Ruling: adopt the reviewer's recommendation.** `'readonly'` is reverted to its honest meaning
("a row whose value is displayed but not editable", e.g. `ai.mcp-server-url`, `about.version`) and a
distinct `'group'` kind is added for a row that stands for a *block* of dynamically-generated
controls. The accessible-name check then applies to the block instead of being skipped, which is the
whole point: overloading `'readonly'` made the guard skip an entire pane section while looking green.

### Type change (`src/components/settings/types.ts`)

- `SettingsControlKind` gains a `'group'` member (append after `'readonly'`).
- Doc it as: "an aggregate row standing for a dynamically-populated block. Stamped on a
  `role="group"` element named by its heading. It has no `[data-setting-control]`; its children are
  runtime-generated and are NOT individually catalogued (see AM-4b for what this costs)."

### Catalog change (`catalog/repo.ts`)

`git-config.behaviour` and `git-config.custom-keys` change `control: 'readonly'` → `control: 'group'`
and drop the "keeps the DOM guard from demanding one control" apology in their comments (replace with
a pointer to this amendment).

### Rendering contract (P69h)

`GitConfigAdvanced` keeps its `<details>` / `<summary>Advanced</summary>` wrapper. Inside it, each of
the two blocks becomes:

```
<section role="group" aria-labelledby={hId} data-setting-id="git-config.behaviour">
  <h4 id={hId} className="settings-config-subtitle">Behaviour</h4>
  … curated controls …
</section>
```

- The heading text is the single source of the accessible name and **must** equal the catalog label
  byte-for-byte (`Behaviour`, `Custom keys` — note the British spelling; that is deliberate and the
  guard now enforces it).
- The outer `<details>` is not stamped. It may map to role `group` named `Advanced` in some
  ARIA versions; the guard's name-scoped lookup keeps that unambiguous because no catalog label is
  `Advanced`. If a future block is ever named `Advanced`, rename the block, not the guard.
- **Test-only reset check:** `settingsCatalog.test.ts`'s "buttons and read-only rows carry no
  `reset`" assertion (currently `entry.control === 'button' || entry.control === 'readonly'`) must
  add `|| entry.control === 'group'`, and the control-kind union list in the same file gains
  `'group'`.

### Recommended compensating control (P69h, SHOULD not MUST)

Stamp each rendered curated key with `data-config-key={key}` and assert in
`GitConfigAdvanced`'s own suite that the rendered key set equals the curated key list the Rust side
returns. That restores per-key coverage inside the blind spot AM-2 opens, without putting
repo-derived data in the static catalog.

---

## AM-3 — `'profile'` joins §4.1's requirement union

§4.1's `SettingsRowRequirement` is amended to
`'repo' | 'aiActive' | 'mcpRunning' | 'mcpStopped' | 'profile'`. P69e's code is correct as written;
P69g should shorten the "EXTENSION … flagged for the architect" comment in `types.ts` to a normal
doc line citing this amendment.

---

## AM-4 — The guard algorithm, as P69g must implement it

File: `src/components/settings/settingsCatalog.coverage.test.tsx` (§4.3's reserved name — see AM-7).

### 4a. Fixtures and the requirement predicate table

```
MAXIMAL = { repoPath: '/repo', aiEnabled: true, aiConsented: true,
            mcpStatus: { enabled: true, allowWrite: true, port, toolCount },
            profiles: [P1, P2],            // stable ids 'p-1', 'p-2'
            terminalCommand: 'x', editorCommand: 'y',
            every numeric knob set OFF its default }
MINIMAL = { repoPath: null, aiEnabled: false, aiConsented: false,
            mcpStatus: { enabled: false }, profiles: [], all defaults }

REQUIREMENT_HOLDS: Record<SettingsRowRequirement, (fx) => boolean> = {
  repo:       fx => fx.repoPath !== null,
  aiActive:   fx => fx.aiEnabled && fx.aiConsented,
  mcpRunning: fx => fx.mcpStatus.enabled,
  mcpStopped: fx => !fx.mcpStatus.enabled,
  profile:    fx => fx.profiles.length > 0,
}

REPEAT_INSTANCES: Record<SettingsRowRepeat, (fx) => readonly string[]> = {
  perProfile: fx => fx.profiles.map(p => p.id),
}
```

**This replaces §4.3's "missing equals ids of entries WITH a `requires` field", which was unsound:**
`ai.mcp-start` (`requires: 'mcpStopped'`) renders *only* in the minimal fixture, so the printed rule
would have failed the first time an MCP row was migrated. With the predicate table, both fixtures get
the same, stronger, symmetric check.

### 4b. Per-category algorithm (run for each fixture F ∈ {MAXIMAL, MINIMAL})

```
for each category c in SETTINGS_CATEGORIES:
    if c.id not in MIGRATED: apply the AM-5 tripwire and continue

    render <SettingsPanel {...F} open initialCategory={c.id} />
    pane = getByRole('tabpanel')                       // scopes out rail, header, search bar

    entries  = SETTINGS_INDEX.filter(e => e.category === c.id)
    expected = entries.filter(e => e.requires === undefined || REQUIREMENT_HOLDS[e.requires](F))

    stamped  = [...pane.querySelectorAll('[data-setting-id]')]

    // (1) no nesting — a stamped row inside a stamped row breaks both set-equality and search
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
            assert sorted(got) === sorted(want)            // FAIL-F  (subsumes count === profiles.length)
            for el of els: assert el.closest('[data-profile-id]') === el
                                                           //   the stamp sits on the row, not inherited

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
            assert (↺ button present in el) === !entry.reset.isDefault(valuesOf(F), DEFAULT_UI_SETTINGS)

    // (5) the wholly-gated pane
    if expected.length === 0:
        assert stamped.length === 0 and the SettingsEmpty element is present  // FAIL-N
```

`ROLE_FOR` stays in the test file (§4.3's mapping): `switch`→`checkbox`, `segmented`/`radiogroup`→
`radio` inside a `radiogroup` named `label`, `numberSlider`→`spinbutton`, `text`→`textbox`,
`button`→`button`, `readonly`→ skip the role lookup and instead assert the row's visible label text
equals `entry.label`, `group`→ handled by branch (4)/`'group'`.

**`accName(el)` for a `'group'` row** — resolve in this order, no third-party dep:
1. `el.getAttribute('aria-label')` if non-empty; else
2. every id in `aria-labelledby`, mapped to `document.getElementById(id)?.textContent`, trimmed and
   joined with a single space; else
3. fail with FAIL-J's "has no accessible name" variant.
Do **not** use `within(el).getByRole('group', …)` — Testing Library's `within` searches descendants
only and would never see the row element itself.

### 4c. Failure messages (mandatory wording — drift must be diagnosable, not cryptic)

Every assertion passes a message of the form `settings drift [<category>]: …`. Minimum set:

- **FAIL-A** `settings drift [general]: rendered but not in the catalog — data-setting-id="general.foo". Add an entry to catalog/general.ts or remove the stamp.`
- **FAIL-B** `settings drift [graph]: "general.foo" is rendered on the graph pane but the catalog files it under general. Move the row or fix its category.`
- **FAIL-C** `settings drift [ai]: "ai.model" rendered 3× but is not repeats:'perProfile'. A row rendered twice is a bug; if it is genuinely repeated, declare repeats.`
- **FAIL-D** `settings drift [ai]: "ai.model" carries data-profile-id but declares no repeats.`
- **FAIL-E** `settings drift [identities]: "identities.profile-name" has instances with missing/duplicate data-profile-id: [undefined, "p-1", "p-1"].`
- **FAIL-F** `settings drift [identities]: "identities.profile-name" rendered for profiles ["p-1"] but the fixture has ["p-1","p-2"]. One card is dropping the row.`
- **FAIL-G** `settings drift [appearance]: in the catalog but not rendered — "appearance.foo" (label "Foo"). Either the row was deleted/renamed, or it is now conditional and needs a requires: flag.`
- **FAIL-H** is FAIL-A restated at set level; prefer FAIL-A's per-id message and keep the set assertion as the backstop.
- **FAIL-I/J** `settings drift [git-config]: group row "git-config.behaviour" has accessible name "Behavior", expected "Behaviour". The <h4> and the catalog label must match byte-for-byte.`
- **FAIL-K** `settings drift [git-config]: group row "git-config.custom-keys" contains no controls in the maximal fixture — the mock fixture no longer exercises this block, so the guard is checking nothing.`
- **FAIL-L** `settings drift [general]: "general.auto-fetch" has no [data-setting-control] descendant.`
- **FAIL-M** `settings drift [graph]: "graph.row-height" — no spinbutton named "Row height" inside the row. Catalog label and rendered accessible name disagree, so search would match text the user cannot see.`
- **FAIL-N** `settings drift [git-config]: no repo open, so the pane must render SettingsEmpty and zero rows; found 2 stamped rows.`

Include the fixture name (`maximal`/`minimal`) in the test title, not in every message.

---

## AM-4b — What the guard still catches, and what it is now blind to

**Still caught (the four drift cases named in the brief, plus three):**

1. *A control added to the UI with no catalog entry* — FAIL-A, provided it is stamped. (Unstamped
   controls: see blindness #1.)
2. *An entry whose control disappeared* — FAIL-G, in whichever fixture the row is expected.
3. *A label changed in one place only* — FAIL-M for normal rows, FAIL-J for `'group'` rows. This is
   the assertion the `'readonly'` overload was destroying for the whole Advanced block.
4. *A row rendered in the wrong category* — FAIL-B, and independently FAIL-A/G in the two categories
   involved.
5. *A row rendered twice by accident* — FAIL-C, preserved despite the identities carve-out.
6. *A card dropping one of its fields for one profile only* — FAIL-F. Bare `count === profiles.length`
   would have missed a card rendering the same row twice while another rendered none; set-equality
   over profile ids does not.
7. *A newly-gated row that forgot `requires`, or a `requires` that no longer matches reality* —
   both directions, because AM-4a's predicate table makes the minimal fixture a positive check too
   (a `mcpStopped` row missing when MCP is stopped now fails, which the printed rule could not do).

**Blind (the cost of the carve-outs — written down here so it is not discovered later):**

1. **Any control with no `data-setting-id`.** Unchanged from the original design and unchanged by
   these rulings, but restate it: the guard's coverage floor is the stamp. The pure-data
   59-id coverage list in `settingsCatalogRows.test.ts` and UI §1.3 are what stop a row from being
   born unstamped.
2. **Everything inside a `'group'` row.** The curated Behaviour keys and every custom key are
   invisible to the guard: a curated key silently disappearing, a control there losing its label, or
   a fifth key appearing all pass. Mitigation is AM-2's `data-config-key` recommendation plus
   `GitConfigAdvanced`'s own suite. Search is correspondingly coarse — it can find "Behaviour", not
   "pull.rebase". **Accepted**: those rows are repo-derived, so no static catalog can own them.
3. **Per-profile *content*.** The guard checks that each declared row exists once per profile; it
   does not check that the row is bound to the right profile's data (a card wired to `profiles[0]`
   for every index passes). `SettingsProfilesSection`'s own tests own that.
4. **Transient control names.** The guard observes idle state only; `Applying…` and delete-confirm
   labels are unchecked by construction (AM-6 makes that safe by forbidding varying names).
5. **Cross-fixture states that neither fixture reaches** — e.g. AI enabled but not consented, or a
   repo with an unborn HEAD. Two fixtures is the contract; a third may be added later but is not
   required by P69.
6. **Ordering.** Set-equality ignores DOM order, so a row moving between groups *within* the same
   category is invisible. Group membership (`entry.group`) is data-only. **Recommendation (not a
   MUST):** if P69k's search result grouping looks wrong in the harness, add a group-order assertion
   then — not now; it would couple the guard to visual layout.

---

## AM-5 — `MIGRATED_CATEGORIES`: confirmed mechanism, revised bookkeeping

**Confirmed:** a category joins the migrated list **in the same increment that re-skins it**, and the
existing two-way tripwire stays (listed-without-renderer fails; renderer-without-listing fails). That
tripwire is correct and must survive the move.

**Revised — a partition, not a single list.** In `settingsCatalog.coverage.test.tsx`:

```
const MIGRATED: readonly SettingsCategoryId[] = [...]
const PENDING:  readonly SettingsCategoryId[] = [...]   // each with a // P69x comment
// asserted once: union(MIGRATED, PENDING) === all 7 ids, intersection === ∅
```

Reason: an omitted category and a migrated one are indistinguishable in a single list, so a category
could quietly never join. The partition makes the remaining debt a literal, reviewable list.

**Expected schedule** (each increment moves its ids from `PENDING` to `MIGRATED` in the same commit
that re-skins the pane): P69g → `general`, `appearance`, `about`; P69h → `git-config`;
P69i → `identities`; P69j → `graph`, `ai`; P69k → `PENDING` is `[]`.

**Location.** P69g **creates** `settingsCatalog.coverage.test.tsx` with the partition and both
renderers/tripwire, and **deletes** the placeholder `MIGRATED_CATEGORIES` / `CATEGORY_RENDERERS`
block and its `describe('DOM↔catalog guard, per category')` DOM half from `settingsCatalog.test.ts`
(≈45 lines). The `${c.id}: has a coherent expected row set` test in that file is pure data and
**stays**. One source of truth, and it buys back headroom in a file already at 430/500.

**The emptiness condition for P69k.** `PENDING` must be `[]` **before** the search bar ships, and
P69k adds the literal assertion `expect(PENDING).toEqual([])` as its last edit, after which both
lists and the tripwire are deleted and the loop runs unconditionally over all 7 categories. The
reason is not tidiness: search claims "every setting is findable and editable in place", and a
category with an unverified DOM can contribute a result that scrolls to nothing or edits the wrong
control, plus a rail match count that lies. **Only permitted escape** (flag to the orchestrator if it
is ever taken): a still-pending category must also be excluded from `searchSettings` and from the
rail counts, and that exclusion must be visible in the UI — which is worse than finishing the
migration. **Recommendation: do not take it.**

---

## AM-7 — File naming

- `src/components/settings/settingsCatalog.coverage.test.tsx` — **reserved by §4.3 and now claimed**
  by P69g for the JSX half. P69e deliberately did not use it; its pure-data split is
  `settingsCatalogRows.test.ts`, which stays as-is (row bookkeeping) alongside
  `settingsCatalog.test.ts` (catalog invariants).
- Three test files total, one responsibility each. None may exceed 500 lines; if the coverage file
  approaches it, split the fixtures into `settings/fixtures/coverageFixtures.ts` (data only), not the
  assertions.

## AM-8 — Consequences for other increments (flag list)

1. **P69k / search over `'group'` rows.** §4.2's filter mechanism (`SettingsRow` returns `null` when
   filtered out) does not cover `'group'` rows, which are rendered by `GitConfigAdvanced`, not
   `SettingsRow`. P69k must make that component consult `SettingsFilterContext` and return `null`
   when a filter is active and `git-config.behaviour` / `git-config.custom-keys` is not in the set.
   Rendering the whole block as a single live-editable result is the intended behaviour.
2. **P69g fixture obligation.** The maximal fixture must give `git-config` at least one curated key
   and at least one custom key (FAIL-K enforces this) and exactly two profiles with stable ids. That
   is a mock-fixture change only — **no IPC surface delta**, consistent with the parent contract.
3. **`src/ipc/types.ts` gains nothing.** `SettingsRowRepeat` and `'group'` live in
   `src/components/settings/types.ts`, which is React-free and IPC-free by design.
4. **DEV guard unchanged.** `SettingsRow`'s `findSettingsRow(id)` DEV `console.error` still applies;
   `'group'` rows are stamped outside `SettingsRow`, so `GitConfigAdvanced` should make the same DEV
   call for its two ids.
