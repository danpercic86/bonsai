# P89 — PR files & local diff view — UI contract

Owner: ui-designer. Backs `docs/contracts/P89-pr-local-diff.md` (data/state). Scope: the PR
detail panel gains a **changed-files section** (local `+X / −Y / N files` header + per-file rows)
and **click-to-view per-file diffs** reusing the existing diff viewer. No application code here.

## 0. Chosen pattern (one sentence + why)

**Inline accordion inside the PR detail's right-pane files section** — each file row expands in
place to render `DiffView`, reusing the `DiffBrowser`/`DiffCard` collapse + per-file lazy-fetch
pattern verbatim.

Why inline, not the centre-pane `DiffBrowser`/`DiffOverlay`:
- The architect's module table (§1) scopes the frontend delta to `PrDetailView.tsx (+ small child
  files)` and reuse of `DiffView`/`DiffViewSplit` — it does **not** list `RepoWorkspace.tsx`,
  `WorkspaceGraphPane.tsx`, or a new `DiffBrowserSource` `pr` mode. The centre-pane browser is
  App-owned state; `PrDiffStats` (with the `mergeBaseOid`/`headOid` a file fetch needs) is loaded
  inside `PrPanel`. Routing it to the centre pane forces a cross-pane state lift the contract did
  not budget. Inline keeps every byte of PR state in the PR-panel subtree.
- It is already an established house pattern: `DiffBrowser`'s `DiffCard` is a collapsible file
  header (chevron + badge + path + `+adds/−dels`) whose body mounts/unmounts `DiffView` with a
  bounded per-file fetch queue. We reuse that exact recipe; the PR files section reads identically
  to the commit all-changes view, just fed by `forgePrFileDiff`.
- Right pane is user-resizable and `DiffView` already renders unified at narrow widths, so an
  inline diff is not cramped.

Recommendation flagged to orchestrator (OQ-UI-1): if a future increment wants full-width PR
review, add a single "View all changes ↗" affordance that opens the centre-pane `DiffBrowser`
with a new `pr` source mode — but do **not** ship both a centre-pane browser *and* inline
accordion; pick one surface. This contract ships inline only.

## 1. Placement & geometry

The files section is a new block inside `PrDetailView`, placed **after the PR body/comments
(`children`) and before `PrActionsBar`** — review the description, then the changes, then act.
It scrolls with the rest of `.pr-detail` (one scroll container; the section is not independently
scrollable — inline diffs grow the page, matching `DiffBrowser`'s stacked cards).

```
.pr-detail  (right pane, scrolls)
├─ .pr-detail-header  (title / meta / stats row  ← +X/−Y/N now LOCAL)
├─ .pr-body | .pr-empty
├─ children (PrReviewComments)
├─ .pr-changes                        ← NEW section
│   ├─ .pr-changes-head   (label "Changed files" + count · Collapse all · Retry?)
│   └─ .pr-changes-list   (rows; each row expands to a DiffView body)
│        ├─ .pr-file-row  [chevron][badge][path.………][+n −n]
│        │    └─ .pr-file-body  (DiffView | skeleton | error) — only when expanded
│        └─ …
└─ PrActionsBar (pinned footer, unchanged)
```

Spacing (4/8/12/16/24 scale), reusing right-panel density tokens:
- `.pr-changes` top separator `1px solid var(--border)`, padding `12px 16px` cozy / `8px 12px`
  compact (matches `.pr-actions-bar`, §12.9).
- `.pr-changes-head`: `display:flex; align-items:center; gap:8px`, `margin-bottom:8px` cozy /
  `6px` compact. Section label `--text-2`, 12px/600; count `--text-3`.
- `.pr-file-row`: reuse the `.file-row` geometry exactly — row height `var(--rp-row-h)` (cozy
  default ~24px / compact 20px per §3 exemption), `gap:6px`, horizontal padding `4px`. Same as
  `StatusFileRow`, so PR files and working-dir files align pixel-for-pixel.
- `.pr-file-body`: reuse `.diff-card-body` (no extra padding; `DiffView` owns its gutters). A
  4px top gap from the header row.

## 2. Header stats row — local counts override

Replace the current forge-count spans (`PrDetailView.tsx` lines 132–136) with the locally
computed values once `PrDiffStats` is `ready`; fall back to `detail.additions/deletions/
changedFiles` while `loading`/`error`/`idle` (architect §4 "Counts strategy").

- Reuse existing classes verbatim: `.pr-stat-add` (`--success`), `.pr-stat-del` (`--danger`),
  `.pr-stat-files` (`--text-3`). No visual change to the row; only the source of the numbers.
- Glyphs stay `+` / `−` (U+2212 minus, as today) — colour is **not** the sole carrier: the `+`
  and `−` signs + the "files" word carry meaning without hue.
- While `loading` and no prior stats: show the fallback forge numbers (never a flash of `+0/−0`).
  If forge numbers are also 0/unknown, the fallback is honest; the section body shows the spinner.
- Pluralize: `1 file` / `N files` (existing logic, keep).

## 3. Changed-files row spec

Reuse the `DiffBrowser` `DiffCard` header structure and the shared `BADGES`/`file-status-*`
classes — do not invent a new row.

Each `.pr-file-row` (a `<button>` header, `aria-expanded`, inside an `<li>`):
- **Chevron**: `.file-chevron` (`.file-chevron-open` when expanded) — `›` rotating, `--text-3`.
- **Status glyph**: `.file-badge.mono` letter — `A` added / `M` modified / `D` deleted /
  `R` renamed / `T` typechange (map from `FileDiffHeader.status`; reuse `DiffBrowser`'s `BADGES`).
  The row also carries `file-status-<status>` so the badge tints via the existing status palette
  (§ added `--success`, modified `--warning`, deleted `--danger`, renamed `--accent`). Letter +
  colour = never colour-only.
- **Path**: `.pr-file-path.mono`, single line, `text-overflow:ellipsis`, `title` = full path.
  Reuse `StatusFileRow`'s dir/name split for de-emphasis: `.file-dir` (`--text-3`) + `.file-name`
  (`--text-1`) so the basename reads first; the directory prefix is muted and truncates from the
  left of the whole cell when space is tight (ellipsis at the start via existing `.file-path`
  rules). Renames render `orig → path` in `.file-rename` (reuse), full pair in `title`.
- **Counts**: `.file-counts.mono` at row end — `+{additions}` in `.file-count-add` (`--success`),
  `−{deletions}` in `.file-count-del` (`--danger`); binary files show `.file-count-bin` "bin"
  and are **not expandable** (no diff to fetch; `DiffView` shows the binary placeholder if
  clicked, so simply render the placeholder body on expand — mirror `DiffCardBody`).

Sort order: `PrDiffStats.files` is already path-ascending (architect §4); render in that order
(flat list — no tree toggle for PR files in v1; the working-dir tree/flat toggle is out of scope
and would be a new control).

## 4. States (contract §5/§6 state machine)

The section is a small state machine keyed on the `forgePrDiff` result for the current PR.

| State | Trigger | Body | Microcopy |
|---|---|---|---|
| `idle`/`loading` | `forgePrDiff` pending (auto-fetch + local diff) | `.pr-changes-head` label + `SkeletonRows` (reuse `CommitPanel` skeleton) in the list area | Head label: **Changed files**. Below, muted `--text-3`: **Computing diff…** |
| `ready` | stats resolved, `files.length > 0` | file rows (§3); header counts now local | — |
| `empty` | `files.length === 0` | one `.pane-empty` line | **No changes between base and head.** |
| `error` | fetch-failed / offline / base·head unresolved | inline `error-banner` (reuse `.error-banner`, `role="alert"`, `--danger` @12% bg) + **Retry** button (`.section-action`) that re-invokes `forgePrDiff` | see per-cause copy below; header keeps forge-count fallback |
| per-file `loading` | row expanded, `forgePrFileDiff` pending | `.pr-file-body` shows `skeleton-group` (reuse `DiffCardBody` loading) | — |
| per-file `error` | file fetch failed | in-body `error-banner` + **Retry** (reuse `DiffCardBody` error exactly) | **Couldn't load this file's diff.** + Retry |

Error copy (no raw libgit2/forge text; say what + what next), mapped from the `errorMessage`
of the `forgePrDiff` reject. Show ONE line in the banner:
- offline / `networkError` (no cached objects): **Couldn't reach the remote to fetch this pull
  request. Check your connection and retry.**
- `authFailed`: **Sign-in required to fetch this pull request.** (Retry re-runs; the panel's
  existing `onAuthFailed` reauth flow handles the credential prompt — reuse, don't add UI.)
- base/head unresolved (`git`, unrelated/missing oids): **Couldn't resolve this pull request's
  base or head commit.**
- `forgeRateLimited`: **Rate limited by the forge. Try again in a moment.**
- generic fallback: **Couldn't compute this pull request's diff.**

In every `error` state the header stats fall back to the forge-supplied counts (never blank),
and the Retry button is the single primary affordance in the section.

"Collapse all / Expand all": reuse `DiffBrowser`'s `.section-action` toggle, shown in
`.pr-changes-head` only when `ready` with ≥1 expandable file. Optional; recommend including it
since PR file counts can be large.

## 5. Caching / re-open (visual consequence only)

Per architect §5 the frontend keeps the last `PrDiffStats` keyed by PR number and reuses it when
`summary.headSha` is unchanged. UI consequence: reopening the same PR shows the files section
**instantly in `ready`** (no skeleton flash). When head advanced, show the `loading` skeleton
while it recomputes but keep the previous rows dimmed underneath via the existing
`.diff-stale` class on the list (reuse) so the panel doesn't jump.

## 6. Keyboard & accessibility

Match the working-dir file list behaviour (`StatusFileRow`) and `DiffCard`:
- `.pr-changes-list` is a `<ul>`; each row header is a real `<button>` (native focus, Enter/Space
  toggles expand), `aria-expanded` reflecting state, `aria-controls` → its body id.
- Tab order: header stats → Collapse-all (if shown) → each file row in path order → each
  expanded body's interactive content → `PrActionsBar`. Natural DOM order gives this; no
  `tabindex` juggling.
- Section landmark: `.pr-changes` is `role="region"` `aria-label="Changed files"` (mirrors
  `DiffBrowser`'s `aria-label="All changes"`).
- Expanded body: `DiffView` keeps its own semantics unchanged. No focus trap (inline, not modal);
  collapsing a row returns focus to that row's header button.
- Icon-only chevron is `aria-hidden` (the button's accessible name is the path + expand state).
- Retry buttons have explicit labels ("Retry"); the error banner is `role="alert"` so the cause
  is announced.
- Hit targets ≥24px: the row header meets `--rp-row-h` (≥20px content but ≥24px including padding
  per §3 rule); Retry / Collapse-all are `.section-action` (already ≥24px).
- Focus ring: 2px `--accent`, 1px offset, `:focus-visible` only (global rule; nothing new).
- Contrast: all reused pairs are already documented AA-passing (§2/§11: `--success` glyph 5.7/4.7,
  `--danger` glyph 4.4/4.6, badge letters on `--bg-*` ≥4.5). No new pair introduced.

## 7. Motion

Chevron rotation and body reveal reuse the existing `.file-chevron`/`.diff-card` transitions
(≤150ms, transform/opacity, `prefers-reduced-motion` honoured). The body **mounts/unmounts** (not
`display:none`) exactly as `DiffCard` does, so large `DiffView`s leave the DOM when collapsed —
no animated height on large content (avoids layout thrash and any graph-render contention). No new
motion.

## 8. Both themes & density

- Dark (default) and light: every colour is a reused semantic token (`--success`, `--danger`,
  `--warning`, `--accent`, `--text-1/2/3`, `--border`, `--bg-*`) already specced for both themes —
  no theme-specific overrides needed.
- Density: rows use `var(--rp-row-h)` and the right-panel density paddings (`.pr-detail` already
  swaps under `data-density`); section padding `12px 16px` cozy / `8px 12px` compact matches
  `.pr-actions-bar`. `DiffView` bodies carry their own density. No density-only geometry added.

## 9. New tokens

**None.** Every colour, radius, font, and spacing value reuses an existing token or class
(`--success`/`--danger`/`--warning`/`--accent`/`--text-*`/`--border`/`--bg-*`, `--rp-row-h`,
`.file-badge`, `.file-status-*`, `.file-count-add/-del/-bin`, `.file-chevron`, `.file-path`/
`.file-dir`/`.file-name`/`.file-rename`, `.error-banner`, `.pane-empty`, `.section-action`,
`.diff-stale`, `SkeletonRows`). `ui-reference.md` needs no token additions for P89; a one-line
note under §12 (PR panel) referencing this contract is the only doc touch.

## 10. Component decomposition & file paths

Keep `PrDetailView.tsx` a thin composer; the files section is its own small files (≤500 lines):

- `src/components/prPanel/PrChangesSection.tsx` — the section: `.pr-changes-head` (label + count +
  Collapse-all + Retry), the `idle/loading/ready/empty/error` switch, and the `<ul>` of rows.
  Receives `PrDiffStats | null`, a `state` discriminator, and callbacks (`onRetry`,
  `onFetchFileDiff`). No IPC of its own — presentational.
- `src/components/prPanel/PrFileRow.tsx` — one row header + its expandable `.pr-file-body` with
  per-file fetch state (idle/loading/ready/error), rendering `DiffView`. This may instead **reuse
  `DiffBrowser`'s `DiffCard`/`DiffCardBody`** if senior-dev can extract them to a shared file
  without growing `DiffBrowser.tsx`; state in the increment which path was taken and why. Prefer
  extraction over duplication.
- Fetch orchestration (the bounded per-file queue + `forgePrFileDiff` calls, keyed by
  `${mergeBaseOid}:${headOid}:${path}`) lives in the `PrPanel`/`PrDetailContainer` layer or a
  small `usePrFileDiffs` hook under `src/components/prPanel/` — mirroring `DiffBrowser`'s local
  cache. Do **not** append it to `PrDetailView.tsx`.
- `PrDetailView.tsx` gains one child slot (the section) between `children` and `PrActionsBar`, plus
  the local-count override on the stats row — no other growth.

## 11. Harness / fixtures to add (browser-verifiable)

The mock layer must serve `forgePrDiff` + `forgePrFileDiff` (architect P89b). Fixtures needed so
every state renders under `VITE_MOCK_IPC=1`:
- **ready (normal)**: a `PrDiffStats` with a mix of statuses (A/M/D/R), realistic `+/−`, ≥1 binary,
  and matching per-file `FileDiff`s.
- **empty**: `files: []` → "No changes between base and head".
- **loading**: a delayed/pending `forgePrDiff` (harness param) → skeleton + "Computing diff…".
- **error variants** (via an `?forge=`/query switch, per P89b): `networkError`, `authFailed`,
  unresolved-`git`, `forgeRateLimited` → each banner copy + Retry.
- **pathological**: a very long deep path, a long rename `orig → path`, and a large (near
  `MAX_FILE_DIFF_LINES`) file diff to verify truncation, ellipsis, tooltips, and mount/unmount.

All states are visible in the plain browser harness (right pane) — **no USER CHECKPOINT** for the
static rendering. Scroll-feel of a large expanded diff is a USER CHECKPOINT (headless harness has
no rAF), not an AI-gate item.
</content>
</invoke>
