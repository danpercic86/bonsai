# P78 — Forge PR UI: token copy + Base/Compare branch dropdowns

UI contract. Two small, in-place changes to the existing forge (pull-request) surface.
No new tokens, no new components, no new states in the design system. Everything below
reuses existing tokens, classes, and the existing `Combobox`.

Owner input: plan `the-github-tokens-dont-jolly-summit.md`. Implementer: `senior-dev`.
Do NOT edit `Combobox.tsx` behavior — only consume it and add scoped CSS.

---

## Part 1 — GitHub token connect copy

**File:** `src/components/ForgeConnect.tsx` — the `gitHub` entry of `CONNECT_HINTS`
(lines 48–53). Rendered unchanged by the `.forge-connect-hint` `<p>` (lines 106–126)
and the masked `<input>` (lines 129–139). No structural change: `ConnectHint.scopes`
stays a **single string in one `<p>`**. The `<p>` wraps naturally, so a two-line render
is acceptable and needs no markup change — do NOT convert to a list.

Only the `gitHub` entry changes. GitLab / Bitbucket / Azure / unknown are untouched.

### Exact final values

```ts
gitHub: {
  scopes:
    'Use a fine-grained token with Pull requests (read/write) and Contents (read) ' +
    'permissions — Metadata is added automatically — or a classic token with the "repo" scope.',
  url: 'https://github.com/settings/personal-access-tokens/new',
  placeholder: 'github_pat_…',
},
```

- **Wording rationale:** fine-grained first (the modern default), permissions named as
  they appear in GitHub's grant UI (`Pull requests`, `Contents`, `Metadata`), classic
  `repo` kept as the explicit fallback. Sentence case, no jargon beyond GitHub's own
  labels. `"repo"` keeps its quotes to match GitHub's scope name and the other hints'
  style. `—` em dashes are already used in project copy; plain and readable.
- **Link label:** unchanged — `Create a token` (line 122). Confirmed.
- **Link target:** fine-grained creation page (above). This deep-links straight to the
  fine-grained token form, matching the copy's primary recommendation.
- **Placeholder:** `github_pat_…` (fine-grained token prefix), replacing `ghp_…`.

### States (all already handled by ForgeConnect — no change)

- Default / focus / disabled-while-submitting: the masked `<input>` (`type="password"`)
  and its `:focus` (`--accent` border) are unchanged.
- Link hover/focus and the platform-click passthrough (`isPlatformClick`) are unchanged.
- Long copy: the `<p>` wraps within the panel; no truncation, no overflow.
- The em dash and `/` are plain text; no encoding concerns.

### a11y

No change. Contrast is inherited from existing `.forge-connect-hint` (`--text-2/3` on
panel bg) and `.forge-connect-link` (`--accent`), both already AA in P64c. The link
remains a real `<a>` with an accessible name ("Create a token").

### Tests / harness note

- `ForgeConnect.test.tsx`: if it asserts the link `href`, `tester` updates it to the new
  fine-grained URL. If it only asserts the link exists, no change.
- `e2e/11-forge.spec.ts` asserts the "Personal access token" label, not the hint copy —
  unaffected.
- Harness: forge connect panel renders under `VITE_MOCK_IPC=1` when the mock host has no
  stored token. Verify the new copy + fine-grained link visually (dark + light).

---

## Part 2 — Base / Compare branch dropdowns

**File:** `src/components/PrCreateForm.tsx` — replace the two `<input class="pr-input mono">`
(lines 104–130) with `Combobox` instances (`allowFreeInput`). Prop-threading
(`RepoWorkspace → WorkspaceRightPanel → PrPanel → PrCreateForm`) and the derived option
lists / `defaultBase` wiring are per the plan — that is architect/senior-dev's boundary,
not this contract. This contract governs how the two fields must **look, read, and behave**.

### Placement & geometry (unchanged container)

The `.pr-create-branches` row is kept exactly: `display:flex; align-items:flex-end;
gap:8px`, with the `←` arrow (`.pr-create-arrow`, `aria-hidden`) between the two
`.pr-field` columns. Each field keeps its `.pr-field-label` ("Base" / "Compare") above
the control. The control changes from a bare `<input>` to `<Combobox>` (a
`div.combobox` wrapping an `<input>`), still inside the same `<label class="pr-field">`.

```
┌ Base ─────────┐        ┌ Compare ──────┐
│ [main      ▾] │   ←    │ [feature/x ▾] │
└───────────────┘        └───────────────┘
   (popover opens below the focused field, z-index 120)
```

Note: wrapping the combobox in `<label class="pr-field">` is fine — the combobox input
is not `htmlFor`-linked, and clicking the label region simply focuses the input. Keep the
existing `<span class="pr-field-label">` as the visible label; `ariaLabel` (below) carries
the accessible name.

### Visual match — CSS the implementer MUST add

`Combobox` hardcodes `className="dialog-input"` on its input and accepts no `className`
prop. `.dialog-input` differs from `.pr-input mono`: it uses `font-family: inherit`
(not mono), `background: var(--bg-2)` (not `--bg-1`), and `height: 32px` / `padding: 0
10px`. To make the two branch fields match the surrounding `pr-input mono` fields
(Title, Description), add a **scoped override** in `src/styles/forge-pr.css` (do not edit
`dialogs-forms.css`, do not edit `Combobox.tsx`):

```css
/* P78: branch comboboxes in the Open-PR form adopt the pr-input mono look. */
.pr-create-branches .combobox {
  flex: 1;
  min-width: 0;
}
.pr-create-branches .combobox .dialog-input {
  width: 100%;
  font-family: var(--font-mono);
  background: var(--bg-1);
}
```

- `width:100%` makes the combobox input fill its `.pr-field` column (the bare `<input>`
  did so implicitly).
- `font-family: var(--font-mono)` restores the `mono` look branch names had.
- `background: var(--bg-1)` matches `.pr-input`.
- Border (`--border`), radius (`6px`), and `:focus` accent border already match between
  `.dialog-input` and `.pr-input`, so no override needed there. The 32px height (vs
  ~27px for `.pr-input`) is acceptable and keeps `.pr-create-arrow`'s `padding-bottom:8px`
  bottom-alignment correct; do not fight it.
- Both themes: `--font-mono`, `--bg-1`, `--border`, `--accent` are all themed tokens —
  the override is theme-correct for dark and light with no extra rules.

### Combobox props

- **Base:** `value={base}`, `onChange={setBase}`, `options={baseOptions}`,
  `allowFreeInput`, `placeholder="target branch (e.g. main)"`, `ariaLabel="Base branch"`,
  `disabled={submitting}`.
- **Compare:** `value={head}`, `onChange={setHead}`, `options={compareOptions}`,
  `allowFreeInput`, `placeholder="source branch"`, `ariaLabel="Compare branch"`,
  `disabled={submitting}`.
- Do NOT set `autoFocus` on either (the form has no single obvious first field to steal
  focus; keep current tab-in behavior).

### CRITICAL — placeholder strings are load-bearing

The placeholders MUST render **verbatim**:
- Base: `target branch (e.g. main)`
- Compare: `source branch`

`e2e/11-forge.spec.ts` locates these fields via `getByPlaceholder(...)`. `Combobox`
forwards `placeholder` to its `<input>`, and `allowFreeInput` keeps Playwright `.fill()`
working (each keystroke fires `onChange`). Any deviation breaks the e2e selectors.

### States (must all be covered)

- **Empty options list** (no branches loaded / `options=[]`): field behaves as free text —
  the user can still type a branch name. Opening the popover shows the existing
  `No matches` row (from `Combobox`); that is the correct empty affordance, no new copy.
- **Long branch lists:** the `.combobox-popover` already caps at `max-height:220px` and
  scrolls; filter-as-you-type narrows it. Long branch names truncate with ellipsis via
  the existing `.combobox-option-label` (`text-overflow:ellipsis; white-space:nowrap`).
- **Value not in options** (free-typed base, or a remote-only branch): allowed —
  `allowFreeInput` shows the typed `value` in the input even when it matches no option.
  This is why free input is required, not strict select. `canSubmit` still reads the
  typed `base`/`head`, so submission works.
- **Disabled while submitting:** `disabled={submitting}` greys the input; the popover
  cannot open. Matches the other `.pr-input` fields' disabled state.
- **Default / hover / active / focus:** inherited from `Combobox` + the scoped override
  (accent focus border, `--accent`/white active option row).
- **Both themes + both densities:** all tokenized; the row lives in the right panel which
  is not density-variant for these dialog forms — no per-density heights to spec.

### a11y

- Accessible names: `ariaLabel="Base branch"` / `"Compare branch"` on the combobox
  inputs (the visible `.pr-field-label` shows the short "Base"/"Compare"; the ariaLabel
  gives screen-reader users the fuller name). `role="combobox"`, `aria-expanded`,
  `aria-controls`, `aria-autocomplete="list"`, `aria-activedescendant` are all provided
  by `Combobox`.
- Keyboard: inherited from `Combobox` — ArrowUp/Down open + move highlight, Enter selects
  the highlighted option, Escape closes only the popover (capture-phase) and keeps focus,
  typing filters. No new bindings.
- Hit target: the 32px-tall input exceeds the 24px minimum.
- Focus ring: existing `.dialog-input:focus` accent border; unchanged.

### Microcopy

No new strings. Placeholders are fixed (above). The `No matches` empty-popover row is
existing `Combobox` copy.

### Motion

None added. `Combobox` popover appears without animation; honor existing behavior.

### Destructive-action UX

N/A — opening a PR is not destructive; no confirmation change.

### Harness / fixture states to verify

Under `pnpm dev` + `VITE_MOCK_IPC=1` (seed `bonsai.mockUiSettings` `aiConsented:true` if
AI eligibility is needed for the generate button):
1. **Populated:** mock `listBranches` returns several local + remote branches → both
   fields show a filtering dropdown; picking and typing both update the field.
2. **Empty options:** a fixture with no branches (or before load) → fields accept typed
   text; popover shows `No matches`.
3. **Long list / long names:** a fixture with many branches incl. a very long branch name
   → popover scrolls and the long name truncates with ellipsis.
4. **Free-typed value:** type a branch not in the list → it stays in the field and the
   form can submit.
5. **Disabled:** trigger submitting state → both fields greyed, popover won't open.
6. **Light theme:** `resize_window` colorScheme light → mono font, `--bg-1`, borders, and
   active-option contrast all correct.

All of the above are AI-gate-verifiable in the harness. No USER CHECKPOINT for the UI
itself; the only checkpoint is the real-GitHub token auth flow (Part 1 backend), which is
already a plan-level checkpoint and outside this contract.

---

## Design-system impact

None. No new tokens, no new component, no `ui-reference.md` edit. Part 2 is a
textbook application of the existing "more than 3 exclusive values → `Combobox`" rule
(ui-reference §, line 605) with `allowFreeInput` for remote-only branches. The scoped
CSS override lives in the feature stylesheet, not the shared combobox rules.
