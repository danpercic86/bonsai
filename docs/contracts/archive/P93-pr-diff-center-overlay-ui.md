# P93 — PR changed-file diffs open in the center DiffOverlay (UI contract)

**Scope:** frontend only. No Rust, no IPC surface change. Reuses
`ipc.forgePrFileDiff(repoId, baseOid, headOid, path, origPath, fullContext, intraline)`.

**Problem.** The Pull requests tab is the only surface in Bonsai that renders a diff *inline*
inside the 320–420px right panel. Working-dir, commit, compare and conflict diffs all open in the
wide center `DiffOverlay` over the graph. P93 makes the PR tab match.

**Approved approach (locked by the user — do not re-litigate).** Per-file center overlay,
mirroring `handleToggleWorkdirDiff` exactly. Slot key `pr:<baseOid>:<headOid>:<path>`, new
`DiffOverlayMeta.kind` value `'pr'`. The inline body and `usePrFileDiffs.ts` are removed; the
right panel becomes a plain clickable file list.

**Baseline:** clean tree at `f5948d2`. `docs/contracts/ui-reference.md` is READ-ONLY this pass
(another session holds an in-flight §6.2 correction); proposed deltas are in §12 below.

> **Revision 2 (2026-08-31, post-implementation design review).** §1 (density numbers), §4.2/§4.3
> (row background token + binary hover), §6.1 (focus restore), §10 (`diff-card-collapsed`) and
> AC14 were corrected after measuring the built UI.
>
> **Revision 3 (2026-08-31, round-2 verification).** §6.1's description of the Esc path was
> factually wrong and is corrected; the `.diff-slot-error` Dismiss button is now an explicitly
> **sanctioned** third arm site; §11.6 is closed. Changes are logged in **§14**.

---

## 1. Placement & geometry

Nothing moves. The changed-files list stays in the right panel inside
`PrChangesSection` (`.pr-changes`, `padding: 12px 16px`; compact `8px 12px`). The diff moves to
the existing center overlay mounted at `WorkspaceGraphPane.tsx:329` — no new surface, no new
geometry, no new tokens.

```
┌──────────┬────────────────────────────────────────┬──────────────────────────┐
│ sidebar  │  commit graph                          │  right panel · Pull reqs │
│          │  ┌──────────────────────────────────┐  │  PR #142 header          │
│          │  │ DiffOverlay                      │  │  ── Changed files  3 ──  │
│          │  │ [M] src/app/main.rs  RUST  PR#142│  │  [M] src/app/main.rs  ◄ active
│          │  │  File Diff Split  Highlight   ×  │  │  [A] src/lib/util.rs     │
│          │  ├──────────────────────────────────┤  │  [D] old/gone.ts         │
│          │  │ hunks …                          │  │                          │
│          │  └──────────────────────────────────┘  │                          │
└──────────┴────────────────────────────────────────┴──────────────────────────┘
```

Row geometry after the change: the row is a **flat single-line row**, not a card. It keeps the
existing `.diff-card`/`.diff-card-header` grid it already uses (badge · path · ±counts) minus the
chevron and minus the body. Row content order and gaps unchanged: `8px` gap between badge, path
and counts; the counts group is right-aligned.

**Measured row box (corrected in rev 2; re-measured rev 3 at 29px after the AC18 border removal):**
`.diff-card-header` computes to **29–30px tall with `padding: 6px 10px` in BOTH densities** — no
`[data-density]` rule targets it (density only changes `.pr-changes` section padding: cozy
`12px 16px`, compact `8px 12px`). This satisfies the ≥24px hit target in both densities. Do **not**
add a density override in P93; the fact that these rows ignore `panelDensity` while the working-dir
file rows honour it is a **pre-existing P89 inconsistency**, filed as a follow-up NIT (§12.5), not
a P93 defect.

---

## 2. Component decomposition & file paths

No new component files. Several edited, one deleted.

| File | Change |
| --- | --- |
| `src/components/prPanel/PrFileRow.tsx` | Remove `PrFileBody`, the `useId`/`aria-controls`/chevron and the `entry`/`onRetry` props. Row becomes: `<li class="diff-card diff-card-collapsed pr-file-row">` (see §10) + a single `<button class="diff-card-header file-status-…">`. New props: `active: boolean`, `onOpen(header: FileDiffHeader): void`. Drops the `DiffView`, `SkeletonRows` and `PrFileState` imports. Target ≈60 lines. |
| `src/components/prPanel/PrChangesSection.tsx` | Delete the `expanded` Set, `toggle`, `toggleAll`, `expandable`, the Expand-all button and the `fileDiffs` prop. New props: `activePath: string \| null`, `restoreFocusTo` (§6.1), `onOpenFile(header: FileDiffHeader): void`. Keeps status machine (loading / empty / error+Retry / ready) and the `.diff-stale` dim. |
| `src/components/prPanel/usePrFileDiffs.ts` | **DELETE the file.** The bounded 4-way queue is no longer needed — the overlay fetches exactly one file at a time via `fetchDiffSlot`, which already carries req-id cancellation and keep-stale-during-swap. |
| `src/components/prPanel/PrDetailContainer.tsx` | Drop the `usePrFileDiffs` import + call. Thread the new callback down and the active path in (§3). |
| `src/components/PrPanel.tsx` | Pass through (§3). |
| `src/components/prPanel/PrPanelAccountHeader.tsx` | *(added during implementation)* the account header extracted out of `PrPanel`'s render body for the 500-line limit, owning the single `ctx?.viewer == null ⇒ render nothing` gate. Behaviour-preserving; verified unchanged in the harness. |
| `src/components/WorkspaceRightPanel.tsx` (~428) | Pass through (§3). |
| `src/components/RepoWorkspace.tsx` | New `pr:` branch in `overlayMeta`, new `prOverlayCtx` state, collapse rules (§6), the focus-restore token (§6.1), and the new `prNumber` prop passed to `WorkspaceGraphPane`. |
| `src/components/repoWorkspace/usePrFileOverlay.ts` | *(added during implementation)* `handleOpenPrFileDiff`, `handleClosePrFileDiff` and `handleDismissDiffOverlay`, all stable (refs only). Home of the §6.1 focus-restore token. |
| `src/components/repoWorkspace/prSlotKey.ts` | *(added during implementation)* `prSlotKey()` / `isPrSlotKey()`. |
| `src/components/WorkspaceGraphPane.tsx` (~329) | **Pass-through only:** accept `prNumber?: number \| null` and forward it to the mounted `DiffOverlay`. Needed because the overlay is mounted here, not in `RepoWorkspace`. |
| `src/components/repoWorkspace/usePartialStaging.ts` | `pr` branch in `handleSetViewMode` + `handleToggleIntraline` (§5). |
| `src/components/DiffOverlay.tsx` | `'pr'` added to the `kind` union; computed kind label; new `prNumber` prop; `'pr'` excluded from `isImage`. |
| `src/styles/forge-pr.css` | Delete dead rules, add the new ones (§10). |
| `src/ipc/mock/handlers/forge.ts` + `src/ipc/fixtures/prDiff.ts` | Fixture additions (§11). |

`RepoWorkspace.tsx` is already oversized — the additions here are ~25 lines of state + one
`overlayMeta` branch, all state code that belongs to the container. Do not add render markup to it,
and keep the handlers in `usePrFileOverlay.ts`.

---

## 3. The click chain (exact prop names)

The overlay key alone cannot carry the header status, `origPath`, or the PR number, and
`prDiff.stats.files` is local to `PrDetailContainer`. So the click callback carries a small
context payload up, and `RepoWorkspace` stores it for the lifetime of the slot.

```
PrFileRow          onOpen(header)
  → PrChangesSection   onOpenFile(header: FileDiffHeader)
  → PrDetailContainer  onOpenFileDiff(ctx: PrFileDiffOpen)
  → PrPanel            onOpenFileDiff?: (ctx: PrFileDiffOpen) => void
  → WorkspaceRightPanel onOpenPrFileDiff: (ctx: PrFileDiffOpen) => void
  → RepoWorkspace      handleOpenPrFileDiff(ctx)
```

`PrFileDiffOpen` (declare once, exported from `src/components/prPanel/PrChangesSection.tsx`):

```ts
export interface PrFileDiffOpen {
  prNumber: number;
  baseOid: string;   // prDiff.stats.mergeBaseOid
  headOid: string;   // prDiff.stats.headOid
  header: FileDiffHeader;
}
```

`PrDetailContainer` fills `prNumber`/`baseOid`/`headOid` from `summary.number` and
`prDiff.stats`; it must **not** invoke when `prDiff.stats === null`.

The active-row marker flows the other way, as a single string so no new type is needed:
`RepoWorkspace` derives `prOverlayPath: string | null` (the path of the open `pr:` slot, else
`null`) and passes it down the same chain as `activePath`.

`prOverlayCtx` in `RepoWorkspace` (plain `useState<PrOverlayCtx | null>`, set in
`handleOpenPrFileDiff`, cleared in every collapse path of §6):

```ts
interface PrOverlayCtx { prNumber: number; baseOid: string; headOid: string;
                         path: string; origPath: string | null; status: FileStatus }
```

`handleOpenPrFileDiff(ctx)` — mirrors `handleToggleWorkdirDiff` line for line:

1. `key = \`pr:${ctx.baseOid}:${ctx.headOid}:${ctx.header.path}\``
2. if `diffSlotRef.current?.key === key` → `collapseDiffSlot()`, clear `prOverlayCtx`, return
   (click-the-same-row-again collapses).
3. else set `prOverlayCtx`, then `void fetchDiffSlot(key, () => ipc.forgePrFileDiff(repoId,
   ctx.baseOid, ctx.headOid, ctx.header.path, ctx.header.origPath, diffViewMode === 'file',
   intraline))`.

Binary headers never reach this handler (§4.3).

---

## 4. The row: affordance, active state, and where the old states went

### 4.1 Affordance

The row is a **single-action open control**, not a disclosure. The chevron
(`<span class="file-chevron">›</span>`) is **removed with no replacement glyph** — no arrow, no
"open" icon. Rationale: the working-dir Changes list, which does the identical thing (click a row,
diff appears in the center overlay), has no glyph either; adding one here would make the PR list
the odd surface again. The badge already anchors the row's left edge, so no realignment is needed.

`title` on the button is unchanged: `path`, or `origPath → path` for a rename.

### 4.2 States (all densities, both themes — all inherited, plus one new rule set)

| State | Spec |
| --- | --- |
| Default | `.diff-card-header` as today. **Corrected rev 2:** the base rule paints `background: var(--bg-1)` (`diff-browser.css:176–184`) — it is **not** transparent. `--text-2` path, `--text-3` counts. |
| Hover | existing `.diff-card-header:hover` → `background: var(--bg-2)` (diff-browser.css:191). Cursor `pointer`. |
| Active (its diff is open in the overlay) | **new** `.pr-file-row-active .diff-card-header` — `background: var(--selection); box-shadow: inset 2px 0 0 var(--accent);` and `.pr-file-row-active .diff-card-path { color: var(--text-1); font-weight: 600; }`. This is verbatim the `.file-row-expanded` recipe (`src/styles/diff.css:46–55`), which is the house precedent for "the overlay is showing this row". Hover must not override it — write the selector as `.pr-file-row-active .diff-card-header, .pr-file-row-active .diff-card-header:hover`. |
| `:focus-visible` | inherited global ring (`:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px }`). Do not add a per-row rule. |
| Disabled | none. Rows are never disabled; a binary row is rendered as a non-button `<span>` (§4.3). |
| Loading | **not on the row.** See §4.3. |
| Error | **not on the row.** See §4.3. |
| Long content | unchanged: `.diff-card-path` keeps `overflow:hidden; text-overflow:ellipsis; white-space:nowrap` with the full path in `title`. Renames truncate the `orig → new` string. Never wrap. |
| Stale (head advanced mid-view) | the list keeps its existing `.diff-stale` opacity dim on `<ul class="pr-changes-list">`; rows stay clickable. |

Contrast (measured rev 2): dark `--selection #2a3b57` + `--text-1 #e8eaed`, light `--selection
#dbe7ff` + `--text-1 #1c1f24` — both far above 4.5:1, and both already shipped by
`.file-row-expanded`. The `--accent` inset rail against `--selection` measures **≈3.7:1 (dark,
#4f8cff on #2a3b57)** and **≈4.0:1 (light, #2f6fe4 on #dbe7ff)** — both clear the 3:1 graphics
threshold. No new token pair.

### 4.3 Where loading / error / binary now live

- **Loading → the overlay.** `fetchDiffSlot` sets `state:'loading'` and `DiffSlotView` renders its
  skeleton; because the key is unchanged on a refetch, the previous content stays visible during a
  view-mode swap. The row shows the active mark immediately on click, so the click is acknowledged
  in the panel even before the overlay body paints. The row-level `SkeletonRows` skeleton is gone.
- **Error → the overlay.** `DiffSlotView`'s existing dismissible `.diff-slot-error` banner carries
  it. Message: the raw `errorMessage(e)` is already what every other slot shows; keep that path.
  **The per-file "Retry" button does not survive.** Recovery is: dismiss the banner (or press Esc)
  and click the row again — the same recovery every other overlay slot offers. There is **no**
  row-level error indicator; a failed file leaves no mark on the list. The section-level Retry in
  `.pr-changes-head` (for `forgePrDiff` itself failing) is unaffected and stays.
  **Rev 3:** because the banner's Dismiss is `onDismissError={onClose}`, it routes through the same
  `handleDismissDiffOverlay` and therefore **also arms focus restore**. That is intended and
  sanctioned — see the §6.1 bump table.
- **Binary → the row does not open.** A header with `binary: true` renders `.diff-card-header` as a
  **non-interactive `<span>`** (mirroring `StatusFileRow`'s non-expandable branch) with
  `class="diff-card-header file-status-… pr-file-row-binary"`, the `bin` count chip it already
  shows, and `title="Binary file — no text diff"`. It is not focusable and not in the arrow-key
  order. Rationale: opening a full-pane overlay whose entire body is the words "Binary file" wastes
  the surface, and the header already tells us it is binary — no fetch needed.

  **Binary row CSS — CORRECTED (rev 2).** Rev 1 specified
  `.pr-file-row-binary:hover { background: none; }`. That is wrong: the base
  `.diff-card-header` rule paints `var(--bg-1)`, so `background: none` does not "cancel the hover",
  it **removes the row's own fill** and lets the card's `var(--bg-0)` show through. Both hover
  selectors have specificity (0,2,0), and the forge-pr.css rule sits later in the cascade, so it
  wins — hovering an inert binary row visibly *darkens* it. Use the same doubled-selector pattern
  that already protects the active row, so the outcome is independent of rule order and the intent
  is explicit:

  ```css
  .pr-file-row-binary,
  .pr-file-row-binary:hover {
    cursor: default;
    background: var(--bg-1); /* the .diff-card-header base fill — no hover reaction */
  }
  ```

  Correct in both themes: `--bg-1` is the same token the base rule uses, so the binary row is
  pixel-identical to an unhovered interactive row, hovered or not. *(Rev 3: shipped and verified —
  computed `background-color` is `rgb(29,32,38)` = `--bg-1` in dark and `rgb(246,247,249)` =
  `--bg-1` in light, identical to a plain unhovered row.)*
- **Images: OUT OF SCOPE for P93.** Binary image files are binary rows (above). A non-binary
  image-extension path (rare; SVG is deliberately excluded from `isImagePath`) opens the overlay
  and renders whatever `DiffView` produces. **`'pr'` must be excluded from `DiffOverlay`'s
  `isImage` predicate** alongside `'conflict'`/`'aiProposal'` — otherwise the overlay switches to
  `DiffImageView`, while `RepoWorkspace`'s image-diff effect (which serves workdir kinds only)
  clears the image state, producing a permanently empty pane with a Side-by-side/Onion/Swipe
  switcher. Deferred as §12.3.

---

## 5. The overlay for `kind: 'pr'`

### 5.1 Key parsing (`overlayMeta`, `RepoWorkspace.tsx` ~661)

Add a `pr:` branch **before** the generic `key.indexOf(':')` fallback — that fallback casts the
prefix to `WorkdirSection` and would produce a garbage section for a `pr:` key. Because a path may
contain `:`, do **not** split naively: strip the `pr:` prefix, drop the first two
colon-delimited segments (the oids), and take the remainder verbatim as the path. Prefer taking
the meta from `prOverlayCtx` (it carries `status`/`origPath`, which the key cannot):

```
if (key.startsWith('pr:')) {
  const ctx = prOverlayCtx;                       // may be null after a remount
  return { path: ctx?.path ?? <remainder-after-2-oid-segments>,
           origPath: ctx?.origPath ?? null,
           status: ctx?.status ?? null,           // null = no badge (existing fallback)
           kind: 'pr' };
}
```

### 5.2 Header copy

`KIND_LABEL` is a static `Record`, so a PR label that names the PR cannot come from it. Make the
kind chip **computed**: keep the record for the seven existing kinds and, for `'pr'`, render

- **`PR #142`** when the PR number is known (`prOverlayCtx.prNumber`),
- **`Pull request`** as the fallback when it is not.

Add `prNumber?: number | null` to `DiffOverlayProps` (not to `DiffOverlayMeta` — meta is a
"derived, never stored" shape and the number is context, not file identity), threaded
`RepoWorkspace → WorkspaceGraphPane → DiffOverlay`. Render it in the existing
`<span class="diff-overlay-kind">`; no new element, no new class.

The chip must read unambiguously as "vs the merge base". `PR #142` alone does not say that, so add
`title="Diff against the merge base of pull request #142"` on the same span (and
`title="Diff against the pull request's merge base"` in the fallback case). This matches how the
other kind chips are terse text with the detail in the surrounding context.

### 5.3 Toolbar behaviour

- **File / Diff / Split** and **Highlight changes** render for `'pr'` (DiffOverlay only hides them
  for conflict/aiProposal) — so `handleSetViewMode` and `handleToggleIntraline` in
  `src/components/repoWorkspace/usePartialStaging.ts` **must gain a `pr` branch**, refetching the
  same key via `ipc.forgePrFileDiff(repoId, ctx.baseOid, ctx.headOid, ctx.path, ctx.origPath,
  m === 'file', intraline)`. Without it the toggles silently do nothing. Pass `prOverlayCtx` in via
  a ref, the same way `overlayMetaRef` is passed today.
- **Read-only:** `stageable` already returns `null` for any non-workdir kind, so no gutter, hunk or
  range controls appear. No change needed — do not add a `pr` case there.
- **Explain:** whatever gate already governs `onExplain` for commit/compare slots applies
  unchanged; P93 does not add or remove it.

---

## 6. Cross-surface interactions

A `pr:` slot is only meaningful while its PR detail is on screen. Collapse it (and clear
`prOverlayCtx`) in each of these cases, using the `clearCompare` prefix-check pattern
(`if (diffSlotRef.current?.key.startsWith('pr:') === true) collapseDiffSlot();`):

- **(C1) Right-panel tab leaves `prs`** (`PrPanel` unmounts) — collapse. An overlay whose
  originating list is gone is orphaned chrome.
- **(C2) A different PR is selected, or the detail returns to the PR list (Back)** — collapse.
  `PrDetailContainer` fires a single `onClosePrFileDiff()` on unmount / on `summary.number`
  change; `RepoWorkspace` handles it with the same prefix-checked collapse.
- **(C3) Head advance (`prDiff` refetch yields a new `headOid`)** — **collapse**, do not dim. The
  key's oids are now stale, so the visible diff no longer describes the PR. The list itself keeps
  its existing `.diff-stale` dim while the new stats load (P89 SF2 behaviour, unchanged), and the
  user re-clicks to see the new diff. Do **not** attempt to re-key the open slot: the file may not
  exist at the new head.
- **(C4) Repo change / repo close** — existing global collapse paths already cover this; no
  addition.

Nothing else closes it. Selecting a commit in the graph does not close it either — that path owns
its own slot key and *replaces* the `pr:` slot naturally via `fetchDiffSlot`. Call that **(C5)**.

### 6.1 Focus restore — CORRECTED (rev 2, prose + bump table corrected rev 3).

**The rev-1 rule was wrong and must not be implemented.** It restored focus when `activePath`
transitioned to `null` and the previously-active row was still in `files`. That is a real
focus-stealing bug: `activePath` is derived from the *slot key* (`prOverlayPath =
prOverlaySlot?.path ?? null`, `RepoWorkspace.tsx:2505`), so **(C5) selecting a commit in the
graph** replaces the slot, drives `activePath → null` while the row is still present in `files`,
and yanks focus out of the graph and into the right panel mid-interaction. The "still in `files`"
test cannot distinguish the two causes, and no downstream test can: by the time
`PrChangesSection` sees the transition, the *reason* is gone.

**Governing principle.** Focus restore is triggered by the **dismissal event**, never inferred
from an `activePath` transition. `activePath → null` is ambiguous (user closed it / another slot
replaced it / PR switch / head advance / tab leave) and must never be used as the trigger.

**New prop on `PrChangesSection`** (and the pass-through chain, same route as `activePath`):

```ts
/** P93 §6.1: set ONLY when the user dismissed this PR file's overlay. `token`
 *  changes on every dismissal so a repeat close of the same row still fires. */
restoreFocusTo: { path: string; token: number } | null;
```

**Producer — `usePrFileOverlay.ts`** (`RepoWorkspace` owns the state: one `useState` plus a
monotonic counter ref). Capture `prOverlayCtx.path` *before* clearing the ctx, then
`setRestoreFocusTo({ path, token: ++counter })`.

**"Bumped in exactly one place" means ONE SHARED FUNCTION, not one call site — CORRECTED (rev 3).**
Rev 2's prose claimed the `×` button and the Esc key "both funnel through the overlay's
`onClose`". **They do not.** The two dismissal affordances reach the collapse independently:

- `×` → `DiffOverlay`'s `onClose` prop;
- **Esc** → `src/components/repoWorkspace/useWorkspaceKeyboard.ts:194`, which calls its
  `collapseDiffSlot` prop **directly**, never touching `onClose`;
- the `.diff-slot-error` banner's Dismiss → `DiffOverlay.tsx:428/441/448`
  `onDismissError={onClose}`, i.e. the same route as `×`.

The correct implementation — and what round 2 shipped — is a single shared
`handleDismissDiffOverlay()` in `usePrFileOverlay.ts` wired at **all** of those sites
(`RepoWorkspace.tsx:2145` binds it to the keyboard hook's `collapseDiffSlot`; `:2456` binds it to
the overlay's `onClose`). One function, several wirings. It is prefix-checked on `isPrSlotKey`, so
for every non-`pr:` slot it is byte-for-byte `collapseDiffSlot`.

The token is bumped by these paths, and **NOT** by the rest:

| Cause of the slot going away | Bump? | Why |
| --- | --- | --- |
| Overlay `×` clicked (`onClose`) | **yes** | user dismissal; focus was inside the overlay |
| Esc pressed (`useWorkspaceKeyboard.ts:194`) | **yes** | same — guard 3 below decides whether focus actually moves |
| `.diff-slot-error` banner **Dismiss** (`onDismissError={onClose}`) | **yes** *(sanctioned rev 3)* | user dismissal from inside the overlay, and it makes the §4.3 recovery ("dismiss, then click the row again") a single keypress: the Dismiss button unmounts with the overlay, focus falls to `<body>`, and guard 3 lets the restore land on the originating row. Do **not** special-case it out. |
| Same row clicked again (§3 step 2) | **no** | native focus is already on that button (harness-verified) |
| **(C5)** a commit / compare / workdir slot replaces the `pr:` slot | **no** | the user is interacting elsewhere — this is the bug being fixed |
| **(C2)** a different PR selected, or Back to the list | **no** | the originating row is gone |
| **(C3)** head advance | **no** | same |
| **(C1)** right-panel tab leaves `prs` | **no** | the panel unmounts |
| **(C4)** repo change / close | **no** | the whole surface is replaced |

Arming the token is never on its own sufficient to move focus — guard 3 is the real safety net.
Any future dismissal affordance added to the overlay should route through
`handleDismissDiffOverlay` too rather than calling `collapseDiffSlot` directly.

`restoreFocusTo` must also be reset to `null` whenever a new `pr:` slot is opened, so a stale
token can never re-fire.

**Consumer — `PrChangesSection`.** One effect, `deps: [restoreFocusTo]`, keyed on `token`
(skip the initial render; keep a `useRef` of the last-handled token). It focuses the `<button>` of
the row whose `path === restoreFocusTo.path` only if **all three** hold:

1. that path is still present in `stats?.files` (else do nothing — never focus a different row);
2. the row's `<button>` exists in the list (a binary row has none);
3. `document.activeElement === document.body` (or `null`). If focus has already landed somewhere
   real — the graph canvas, the sidebar, another panel — the user moved it deliberately and we
   must not take it back.

Delete the rev-1 `prevActiveRef` / `activePath`-transition effect at
`PrChangesSection.tsx:62–72` entirely; do not keep both.

**Accepted consequence:** pressing Esc while focus sits in the graph pane closes the overlay
without moving focus (guard 3 fails). That is correct — Esc there is a global "get this off my
screen", not a request to enter the PR list. AC14 is amended accordingly.

## 7. Keyboard & accessibility

- **Row semantics.** Each row stays `<li>` inside `<ul class="pr-changes-list">`; the interactive
  element stays a `<button>`. Accessible name = the button's text content (badge letter + path +
  counts), with the full/rename path in `title`.
- **`aria-expanded`:** keep it, set to `active`, and drop `aria-controls`. This is not the purest
  ARIA — the controlled region is a sibling surface, not a child — but it is exactly what
  `StatusFileRow.tsx:103` does for the identical interaction, and diverging on one of the two lists
  would be worse than both being imperfect. Migrating **both** surfaces to `aria-current="true"` is
  filed as a deferred follow-up (§12.2); do not do it in P93 for the PR list alone.
- **Tab order.** One tab stop per non-binary row, in list order, after the section head's
  Retry button (when present). Binary rows are skipped (non-focusable `<span>`).
- **Arrow keys.** The list is *not* a roving-tabindex composite today and P93 does not make it one
  — Up/Down are browser-default scroll. Adding roving focus here would diverge from the working-dir
  list, which also uses plain tab stops. Enter and Space on a focused row open (native button
  behaviour); no custom key handling.
- **Hit targets** ≥ 24px tall in both densities (§1: measured 29px); the row spans the full panel
  width.
- **Colour is never the sole carrier:** the A/M/D/R/T status letter badge stays; the active row is
  marked by the 2px `--accent` inset rail *and* the bolded `--text-1` path, not by colour alone.
- **`prefers-reduced-motion`:** no new motion is introduced, so nothing to honour. The overlay's
  existing mount transition is unchanged.

## 8. Motion

None added. The row's background/inset change on activation uses no transition (instant, matching
`.file-row-expanded`) — the overlay painting is the feedback, and a 150ms row fade would lag behind
it. Nothing here touches the canvas render budget.

## 9. Microcopy (exact strings)

| Where | String |
| --- | --- |
| Section head label | `Changed files` (unchanged) |
| Section head count | `3 files` / `1 file` (unchanged) |
| Section head Retry | `Retry` (unchanged, error state only) |
| Overlay kind chip | `PR #142` / fallback `Pull request` |
| Overlay kind chip tooltip | `Diff against the merge base of pull request #142` / `Diff against the pull request's merge base` |
| Binary row tooltip | `Binary file — no text diff` |
| Removed | `Expand all` / `Collapse all`; `Couldn't load this file's diff.`; the row-level `Retry`; the `Binary file` placeholder body |

No destructive action exists on this surface, so no confirmation UX applies.

## 10. Dead CSS & dead code to remove in the same pass

- `src/styles/forge-pr.css:805` — `.pr-changes-collapse-all` selector in the shared
  `margin-left:auto` rule. Keep `.pr-changes-retry`; the rule becomes single-selector.
- `.pr-file-body` — grep `src/styles/` and remove if a rule exists (verified rev 2: **no rule ever
  existed**; nothing to remove).
- `.pr-file-row` — **keep** the class on the `<li>` (it now hosts the `.pr-file-row-active`
  variant); remove it only if no rule references it and no test selects it.
- `diff-card-collapsed` — **CORRECTED (rev 2).** Rev 1 said its usage "disappears from
  `PrFileRow`". That was wrong and produced a visible defect on every row. The `<li>` must carry
  `diff-card-collapsed` **statically**, because the row now *is* permanently collapsed — it has no
  body, ever. The existing rule (`diff-browser.css:197`) is exactly the needed fix:
  `border-bottom: none; border-radius: 5px;`. Without it every row renders (a) its header's
  `border-bottom: 1px solid var(--border)` stacked directly on the card's own bottom border — a
  doubled 2px divider — and (b) a `5px 5px 0 0` header inside a `6px`-radius card, so the active
  row's `--selection` fill has square bottom corners poking past the rounded card. Reuse the house
  class; do **not** author a new `.pr-file-row .diff-card-header` override.
  The rule is also still used by `DiffBrowser` — do not delete it.
  *(Rev 3: shipped and verified — every row's header computes `border-bottom: 0px none` and
  `border-radius: 5px` inside a `6px` `<li>`, in both themes, including the active row.)*
- `src/components/prPanel/usePrFileDiffs.ts` — delete the file and any test that targets it.
- `SkeletonRows` / `DiffView` / `PrFileState` imports in `PrFileRow.tsx`; `UsePrFileDiffs` import
  in `PrChangesSection.tsx`.

New CSS added (append to the P89 block in `src/styles/forge-pr.css`, no new tokens):
`.pr-file-row-active .diff-card-header{,:hover}`, `.pr-file-row-active .diff-card-path`, and the
single doubled-selector `.pr-file-row-binary{,:hover}` rule from §4.3.

## 11. Harness states (`VITE_MOCK_IPC=1`)

`src/ipc/mock/handlers/forge.ts:270` already serves `forgePrFileDiff` from
`src/ipc/fixtures/prDiff.ts` — no new handler needed. Reach the PR tab in the harness with
`http://localhost:1420/?forge=auth` (the default sentinel shows `<ForgeConnect>` instead).
Required fixture coverage:

1. **Ready** — existing `PR_DIFF_STATS`; clicking a row opens the overlay with hunks.
2. **Empty** — existing `?forge=empty` → `PR_DIFF_STATS_EMPTY`, "No changes between base and head."
3. **Loading** — the existing 120ms `delay()` is enough to see the overlay skeleton; widen to
   ~800ms behind a URL param if it proves unobservable.
4. **Error** — a path sentinel in `mockPrFileDiff` (any path containing `fail`) that rejects with
   an `AppError`. *(Shipped: `src/ci/fail-on-purpose.rs`.)*
5. **Binary** — `PR_DIFF_STATS.files` must contain at least one `binary: true` header.
   *(Shipped: `assets/preview.png`.)*
6. **`fullContext` / `intraline` must change the payload.** **CLOSED (rev 3).** `PR_FILE_SOURCES`
   in `src/ipc/fixtures/prDiff.ts` now holds whole-file old/new bodies and feeds `lineDiff(...,
   fullContext)`, so the toggle is unambiguous: `README.md` renders **18 lines / 11 context / 2
   hunk headers** in Diff mode vs **29 lines / 22 context / 0 hunk headers** in File mode;
   `src/server.rs` renders **30 / 21 / 3** vs **39 / 30 / 0**. AC9 is now provable in the harness.
7. **Pathological long content** — one ~180-char deep path and one long rename pair.
   *(Shipped; both ellipsize on one line, `.pr-panel` horizontal overflow measured 0px.)*
8. **Not covered by any fixture — hand to `tester`:** a path containing `:` (AC3) and a simulated
   head advance to a new `headOid` (AC13). Neither can be exercised in the harness today.

Not verifiable in the harness (**USER CHECKPOINT**):

- the native-window feel of the overlay open/close and the scroll of a real large PR diff (AC20);
- **AC17's runtime proof.** The Browser pane is headless-ish and `requestAnimationFrame` never
  fires, so canvas clicks are no-ops and a graph commit selection cannot be driven from the
  harness. Accepted substitute AI evidence: the unit tests in
  `src/components/prPanel/PrChangesSection.test.tsx` (an `activePath → null` transition with no
  token leaves `document.activeElement === document.body`) and
  `PrDetailContainer.test.tsx` (head advance does not move focus). The end-to-end "click a commit
  in the graph while a PR overlay is open" case must be confirmed by the user in
  `pnpm tauri dev`.

## 12. Deferred: `ui-reference.md` updates (do NOT apply this pass)

1. **Overlay kinds table** — add `pr` to the `DiffOverlayMeta.kind` list with the computed
   `PR #<n>` label, noting it is the one kind whose chip is not a static string.
2. **Active-row precedent** — document `.file-row-expanded` / `.pr-file-row-active` as the single
   canonical "the overlay is showing this row" recipe (`--selection` bg + 2px `--accent` inset
   rail + `--text-1` 600 label), and record the open question of migrating both surfaces from
   `aria-expanded` to `aria-current="true"`.
3. **Image diffs in PR files** — out of scope in P93; record that `'pr'` is excluded from the
   overlay's `isImage` predicate and that enabling it needs an `ImageDiffRequest` variant for
   base…head blobs (a Rust/IPC change, hence a separate milestone).
4. **Focus restore is event-driven** (§6.1) — record the house rule that a focus restore must be
   driven by an explicit dismissal token, never by inferring intent from a derived-state
   transition; that the restore is suppressed unless `document.activeElement` is `body`; and that
   every dismissal affordance on a surface should route through one shared dismissal handler
   rather than calling the collapse directly.
5. **Row density gap** — the PR changed-file rows ignore `panelDensity` (only the section padding
   responds) while working-dir file rows honour it. Pre-existing since P89; decide whether to
   align. *(Still deliberately deferred as of rev 3 — do not fix inside P93.)*
6. **Inert-row hover rule** (§4.3) — record that `background: none` never cancels a hover on
   `.diff-card-header`; an inert variant must restate the base `--bg-1` fill on both the base and
   `:hover` selector.

## 13. Acceptance criteria

1. **AI gate** — Clicking a non-binary row in the PR Changed-files list renders no inline body;
   the diff appears in `.diff-overlay` over the graph pane, with the file's hunks.
2. **AI gate** — The open slot's key is exactly `pr:<baseOid>:<headOid>:<path>`, and the overlay
   header shows the correct badge letter, the path (or `orig → new`), and the kind chip `PR #<n>`
   with the merge-base tooltip.
3. **AI gate** — A path containing `:` renders the correct full path in the overlay header (key
   parsing does not split the path). *(No fixture — unit test required.)*
4. **AI gate** — Clicking the already-active row collapses the overlay; the row's active mark
   clears; `aria-expanded` returns to `false`; focus stays on that button.
5. **AI gate** — Exactly one row carries `.pr-file-row-active`, and it is the file shown in the
   overlay; computed style shows `background: var(--selection)` and a 2px `--accent` inset rail in
   both `dark` and `light` themes, and in both `cozy` and `compact` densities.
6. **AI gate** — A `binary: true` row renders a non-button element, is not in the tab order, is not
   clickable, shows the `bin` chip and the `Binary file — no text diff` tooltip.
7. **AI gate** — No `Expand all` / `Collapse all` button exists in `.pr-changes-head`; the file
   count and the error-state `Retry` are unchanged.
8. **AI gate** — The error fixture (§11.4) opens the overlay and shows the dismissible
   `.diff-slot-error` banner; no error indicator appears on the row; dismiss + re-click refetches.
9. **AI gate** — With a PR slot open, toggling File / Diff / Split and Highlight changes refetches
   with the new `fullContext` / `intraline` flags and the overlay body visibly changes — the Diff→
   File toggle must add rendered context lines and drop the hunk headers (§11.6 numbers).
10. **AI gate** — With a PR slot open, no staging gutter, hunk button, or line-range control
    renders in the overlay (`stageable === null`).
11. **AI gate** — A PR file with an image extension does **not** render the
    Side-by-side/Onion/Swipe switcher (`'pr'` excluded from `isImage`).
12. **AI gate** — Switching the right-panel tab away from Pull requests, pressing Back to the PR
    list, or selecting a different PR each collapses the `pr:` overlay; no other slot kind is
    collapsed by these actions.
13. **AI gate** — A simulated head advance (new `headOid`) collapses the `pr:` overlay while the
    list keeps its `.diff-stale` dim and stays clickable. (Unit test is acceptable evidence.)
14. **AI gate (amended, rev 2)** — Esc, the overlay `×`, and the error banner's Dismiss move focus
    back to the originating row's button **unless the user has already focused something outside
    the overlay** (`document.activeElement !== document.body`), in which case focus is not moved.
    When the originating row no longer exists, focus is not moved into a different row.
15. **AI gate** — `src/components/prPanel/usePrFileDiffs.ts` no longer exists;
    `.pr-changes-collapse-all` and `.pr-file-body` no longer appear in `src/styles/`; `tsc` and the
    unit suite pass.
16. **AI gate** — A ~180-char path and a long rename each render on one line with an ellipsis and a
    full `title`, in both densities, with no horizontal scrollbar on the right panel.
17. **AI gate via unit test + USER CHECKPOINT for the runtime case (new rev 2, scoped rev 3)** —
    Selecting a commit in the graph while a `pr:` overlay is open replaces the overlay content
    **without moving focus into the PR changed-files list**. Not reachable in the harness (no
    `requestAnimationFrame`, so canvas clicks are no-ops); the AI evidence is the C5-shaped unit
    test in `PrChangesSection.test.tsx`, and the end-to-end case is a USER CHECKPOINT.
18. **AI gate (new, rev 2)** — Every `.pr-file-row` renders with a single 1px bottom border and a
    uniform corner radius: `.diff-card-header` computes `border-bottom-width: 0px` and
    `border-radius: 5px`, and the active row's `--selection` fill does not overhang the card's
    rounded bottom corners.
19. **AI gate (new, rev 2)** — A binary row's computed `background-color` is **identical hovered
    and unhovered** (`--bg-1`) in both themes; it never darkens to `--bg-0` or lightens to
    `--bg-2`.
20. **USER CHECKPOINT** — In `pnpm tauri dev`: opening a real PR's file diffs in the center overlay
    feels the same as clicking a working-dir file (no flash, no layout shift in the right panel),
    and scrolling a large real PR diff in the overlay is smooth.

## 14. Revision log

**Rev 2 — 2026-08-31, after reviewing the implementation in the mock harness.** All five changes
are **contract errata I own**, not implementer errors — rev 1 was built faithfully:

1. **§6.1 focus restore rewritten.** The rev-1 "restore when `activePath → null` and the row is
   still in `files`" rule is a focus-stealing bug (graph commit selection replaces the slot).
   Replaced with the dismissal-token rule + the `document.activeElement === body` guard.
   New AC17, amended AC14.
2. **§10 `diff-card-collapsed`.** Rev 1 removed it; it must be applied statically. Restores the
   single divider and the uniform corner radius. New AC18.
3. **§4.3 binary hover.** Rev 1's `background: none` removes the row's base `--bg-1` fill rather
   than cancelling hover, so the inert row darkens on hover. Replaced with a doubled-selector rule
   that restates `--bg-1`. New AC19. §4.2 "Default" corrected: the base fill is `--bg-1`, not
   transparent.
4. **§1 density numbers.** Rev 1 claimed cozy 26px / compact 22px. Measured: **30px, `6px 10px`,
   identical in both densities** — no density rule targets `.diff-card-header`. The ≥24px
   requirement still holds. The density gap is a pre-existing P89 NIT (§12.5).
5. **§11.6 fixture.** `intraline` variance shipped and is observable; `fullContext` variance is too
   weak to prove AC9 (same line count in Diff and File mode). SHOULD-FIX.

Also renumbered the §6 collapse cases as **C1–C5** so §6.1's bump table can reference them (rev 1
used "§6.1–§6.4", which collided with the new §6.1 heading).

**Rev 3 — 2026-08-31, round-2 verification of the rev-2 follow-up.** AC18, AC19, AC5 and AC9 all
verified by computed style in the harness in both themes; no implementation changes requested.
Two contract-text corrections, both errata I own:

1. **§6.1 Esc prose was factually wrong.** Rev 2 said `×` and Esc "both funnel through the
   overlay's `onClose`". Esc actually calls `useWorkspaceKeyboard.ts:194`'s `collapseDiffSlot` prop
   directly. Restated: "bumped in exactly one place" means **one shared function**
   (`handleDismissDiffOverlay`) wired at every dismissal site — which is what round 2 shipped
   (`RepoWorkspace.tsx:2145` and `:2456`).
2. **The third arm site is sanctioned, not accidental.** `DiffOverlay`'s
   `onDismissError={onClose}` routes the `.diff-slot-error` banner's Dismiss through the same
   handler, so dismissing the banner also arms focus restore. **Intended** — it makes the §4.3
   "dismiss the banner and click the row again" recovery one keypress, and guard 3
   (`activeElement === body`) keeps it safe. Added as a `yes` row in the §6.1 bump table, noted in
   §4.3, and folded into AC14. §12.4 gains the "route every dismissal affordance through one
   shared handler" house rule.

Also closed §11.6 with the measured Diff-vs-File line counts, corrected the measured row height to
29px after the AC18 border removal (§1, §7), and scoped AC17 to "unit test + USER CHECKPOINT"
since the harness cannot dispatch canvas clicks.

Everything else in rev 1 was implemented as specified and verified in the harness: the active-row
recipe verbatim in both themes, tokens only, the `aria-expanded`/no-`aria-controls` contract,
genuinely inert (non-focusable) binary rows, the computed `PR #<n>` chip and its merge-base
tooltip, the §5.3 refetch branches, `stageable === null`, `'pr'` excluded from `isImage`, and the
C1/C2 collapse rules.
