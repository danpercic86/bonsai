# P90 — Per-branch CI Checks view — UI contract

Owner: ui-designer. Consumes the architect's P90 data contract (per-branch `CommitStatus` +
`StatusContext[]`, refreshed on fetch/pull/push). Reuses tokens/§ from
`docs/contracts/ui-reference.md`. No app code here — `senior-dev` implements.

---

## 1. Placement decision

**Recommendation: a third right-panel tab, `Checks`, added after `Working` and `Pull requests`**
in the existing `role="tablist"` (`WorkspaceRightPanel.tsx:251-270`).

Rationale: the user explicitly wants checks **by branch**, and the right-panel tab strip is
already the app's per-context detail surface. A tab (vs a section inside Working) keeps the checks
view *branch-scoped and independent of the graph's commit selection* — the Working tab is
commit/working-dir-scoped and would fight the "by branch" requirement. It mirrors the PR tab it
sits beside (both are forge-signal, branch-oriented, and share the account/connect story), so it
introduces no new idiom. It is the detailed counterpart to the graph's compact rollup badge
(`forgeBadges.ts`), reached deliberately rather than always on screen.

Rejected alternatives:
- *Section inside Working* — Working is dominated by staging/diff and is commit-selection-driven;
  a per-branch block there is a scope mismatch and displaces the primary staging action.
- *Overlay/dialog* — checks are reference data the user re-checks while working; a modal is wrong
  weight and breaks the "glance while doing" flow.
- *Sidebar-only (extend `SectionRollupBadge`)* — good for the collapsed at-a-glance signal (and
  we already have the graph badge), but there is no room for per-check name/description/link-out.

**Tab type change (architect coordination):** `rightPaneTab` union `'work' | 'prs'` →
`'work' | 'prs' | 'checks'`. `onSelectRightPaneTab` unchanged.

### 1.1 What drives "the branch"

The Checks tab is scoped to a **checks target branch**, resolved in this priority:
1. the last branch/remote row the user clicked in the sidebar (`BranchRow`/`RemoteRow` already emit
   `onReveal({kind:'ref', name})`, `sidebar/rows.tsx:80,127`) — thread that ref name into a new
   `checksTarget` state alongside the existing reveal;
2. else the current branch (HEAD), when attached;
3. else the tab shows the "pick a branch" empty state (§4.7).

This reuses the existing reveal click — **no new click target on branch rows**. The tab header
names the resolved branch so the scope is never ambiguous. Selecting a *commit* in the graph does
**not** change the Checks target (that is the Working tab's job); this keeps the two tabs orthogonal.

---

## 2. Layout

Container width 380px (right panel). Two-zone: a sticky **header block** + a scrolling **check
list**. Reuses the PR panel's shell rhythm.

```
+----------------------------------------------------------+  right panel (380px)
| [ Working ] [ Pull requests ] [ Checks ]                 |  tablist (unchanged strip)
+----------------------------------------------------------+
| feat/pr-local-diff                              ⟳        |  header row 1: branch + refresh
|  ⌂ tip a0f0575 · "wip(P89c): PR changed-files…"          |  header row 2: tip sha + summary
|  [ ✓ 5 checks passed ]        Updated 2 min ago          |  header row 3: rollup pill + freshness
+----------------------------------------------------------+
|  ✓  build / linux            Compiled in 42s        ↗    |  check row
|  ✓  build / windows          Compiled in 51s        ↗    |
|  ⚠  test / integration       3 failing               ↗    |
|  ⊘  deploy / preview         Errored: timeout        ↗    |
|  ●  lint                     Queued…                     |  (no target_url → no ↗)
|  –  codecov/patch            Neutral                 ↗    |
+----------------------------------------------------------+
```

### 2.1 Component decomposition + file paths

Follow the PrPanel container/presentational split. New files:

- `src/components/ChecksPanel.tsx` — **container**. Mounted only while `rightPaneTab === 'checks'`.
  Owns: resolve `checksTarget` → tip sha, the `forgeCommitStatuses`/`forgeCommitStatus` IPC call
  (architect names the exact command), last-wins req-id guard (mirror `PrPanel`'s `detailReqRef`),
  view state, `lastUpdated` timestamp, and the fetch/pull/push-driven refetch subscription. Reuses
  `ForgeAccountHeader` for the connect/unsupported flows exactly as `PrPanel` does (§4.2/§4.3).
- `src/components/checksPanel/ChecksHeader.tsx` — presentational header block (§2.2).
- `src/components/checksPanel/CheckRow.tsx` — one `StatusContext` row (§2.3).
- `src/components/checksPanel/ChecksRollupPill.tsx` — the overall rollup verdict pill (§3), a thin
  wrapper over the §11 verdict-pill recipe; reusable so the sidebar could later adopt it.

CSS: new `src/styles/checks-panel.css`, added to the import list after `commit-panel.css`. No edits
to `styles.css`. All selectors prefixed `.checks-`.

Density: the right panel is a `--rp-*` density scope (§3). Reuse the existing `--rp-row-h` for check
rows and `--rp-*` paddings via `var(--x, <fallback>)`; **introduce no new density block**.

### 2.2 Header block (`ChecksHeader`)

Sticky, `--bg-1`, `padding: 12px`, `border-bottom: 1px solid var(--border)`, `gap: 4px` column.

- **Row 1 — branch + refresh.** Branch name 13px/600 `--text-1`, truncate with ellipsis + `title`
  (long-branch case). Trailing `⟳` icon button, `.btn-icon` 32×32 (24×24 hit floor met), right-
  aligned (`margin-left:auto`), `aria-label="Refresh checks"`. While refetching: label becomes
  disabled + `aria-busy="true"`; no spinner (§8 house rule) — the freshness line shows `Checking…`.
- **Row 2 — tip commit.** `⌂`-free: `tip <short-sha>` in mono 12px `--text-2` + `·` + commit
  summary 12px `--text-2`, single line, ellipsis + `title`. Clicking the sha reveals that commit in
  the graph (reuse `onRevealCommit(oid)` — same affordance as BlameView/FileHistoryView).
- **Row 3 — rollup + freshness.** Left: the rollup pill (§3). Right (`margin-left:auto`): freshness
  readout, 11px `--text-2` (never `--text-3` — it is read, §2): `Updated <relative> ago` /
  `Checking…` / `Never checked`.

### 2.3 Check row (`CheckRow`)

`grid-template-columns: 16px 1fr auto; column-gap: 8px; align-items: center;`
`min-height: var(--rp-row-h, 32px)` (cozy) — compact tracks `--rp-row-h` (22/20px). `padding: 0 12px`.
Sibling rows separated by 1px `--border`. Hover `background: var(--bg-2)`.

- **Col 1 — state glyph.** 14px, `aria-hidden`, colour + glyph per §3.1. It is the non-colour
  carrier; never colour alone.
- **Col 2 — name + description.** Name 13px `--text-1`, single line, ellipsis + `title`. Optional
  `description` on a second line, 12px `--text-2`, ellipsis + `title`. When `description` is null the
  row is single-line and vertically centred. When a `duration`/`timestamp` timing field exists
  (may or may not be added), it appends to the description line as ` · 42s` / ` · 2 min ago` in
  `--text-2` — layout is unchanged whether or not timing is present (requirement met).
- **Col 3 — link-out.** `↗` icon button `.btn-icon` when `targetUrl` is non-null; opens via
  `ipc.openUrl` (reuse `PrPanel.openPrPage`'s pattern + its failure toast). `aria-label="Open
  <name> in browser"` (§7.2: starts with the visible verb, names the object). When `targetUrl` is
  null the column collapses (no dead affordance). External-link safety: §5.

The row is **not itself a button** — its only actions are the sha (header) and the per-row `↗`.
Keeps the list a plain, screen-readable list rather than a grid of ambiguous click targets.

---

## 3. Rollup + per-check state → tokens/glyphs

Reuse the **rollup badge's colour language** (`forgeBadges.ts` `ciBadgeVisual`): `--badge-good`
(== `--success` values), `--badge-warn` (== `--danger` values), `--warning`, `--text-3`. This keeps
the graph badge and the detailed list visually identical per state.

### 3.1 Per-check `StatusContext.state` (`CheckRollup`)

| state | glyph | colour token | word (label/desc) | notes |
|---|---|---|---|---|
| `success` | `✓` | `--badge-good` | (glyph only / desc) | matches badge `check` |
| `failure` | `⚠` | `--badge-warn` | e.g. `3 failing` | §7 vocab: ⚠ = failed |
| `error`   | `⊘` | `--badge-warn` | e.g. `Errored: …` | distinct glyph from failure (both `--badge-warn`, so glyph carries the difference — colour is never the sole carrier) |
| `pending` | `●` | `--warning` | `Queued…` / `Running…` | matches badge `dot`; participle per §8 |
| `neutral` | `–` | `--text-3` (glyph) / `--text-2` (word) | `Neutral` / `Skipped` | matches badge `dash`; `–` glyph is graphics (3.38:1 on `--bg-1`, clears 3:1) |
| `none`    | — | — | — | not rendered as a row |

Glyphs are all distinct on this surface (✓ ⚠ ⊘ ● –), satisfying "one glyph = one meaning per
surface" (§7). All glyphs are `aria-hidden`; the state is in the row's accessible name (§5).

### 3.2 Overall rollup pill (`ChecksRollupPill`)

The §11 **verdict-pill** recipe (label `--text-1`, hue in 40% border + 100% `aria-hidden` glyph
over a 14% tint, hue via local `--h`). Label is the **count summary**, not a bare word:

| rollup | pill | `--h` |
|---|---|---|
| success | `✓ {n} checks passed` | `--badge-good` |
| failure | `⚠ {failed} of {total} failed` | `--badge-warn` |
| error | `⊘ {n} errored` | `--badge-warn` |
| pending | `● {pending} running` (hueless-warning style ok) | `--warning` |
| neutral | `– {n} neutral` | hueless (§11 informational: `--text-2` over 12% tint) |
| none | no pill — the panel is in the "no checks" empty state (§4.6) |

Singular/plural: `1 check passed` / `2 checks passed`. Accessible name folds the glyph meaning in
(§11: `aria-label` on the pill, e.g. `All checks passed`, `3 of 8 checks failed`).

---

## 4. All states (copy included)

Each is the §8 in-pane empty pattern: title 13px/600 `--text-1` + one-line reason 12px `--text-2`
(+ one action button where an action exists). `.pane-empty` / `EmptyState` conventions.

- **4.1 Loading** — `SkeletonRows` (reuse `CommitPanel`'s) in the list area; header shows the
  branch name + `Checking…` freshness. No spinner over anything.
- **4.2 No forge configured / unsupported** — reuse `PrPanel`'s `unsupported` copy shape:
  title `No CI checks here` · reason `<host> isn't a supported forge yet, so Bonsai can't read its
  checks.` (or `This repository's origin isn't a supported forge.`). No button.
- **4.3 Not connected** — render `ForgeConnect` exactly as `PrPanel` does (same account/connect
  flow; checks and PRs share auth). Do not duplicate the connect UI.
- **4.4 Branch has no upstream** — title `No checks for this branch` · reason `feat/foo hasn't been
  pushed, so there are no CI results yet.` · button `Push branch` (wire to the existing push
  action) when the branch exists locally; omit the button if push isn't applicable.
- **4.5 Branch resolves but tip has no status yet** (pushed, checks not started) — title
  `Waiting for checks` · reason `No checks have reported for <short-sha> yet.` · the `⟳` header
  refresh is the action.
- **4.6 No checks (forge returned an empty set)** — title `No checks configured` · reason `This
  repository doesn't run CI on <branch>.` No button.
- **4.7 No branch selected** (fresh, detached HEAD, none clicked) — title `Pick a branch` · reason
  `Select a branch in the sidebar to see its CI checks.` No button.
- **4.8 All pass** — normal list; rollup pill `✓ N checks passed`.
- **4.9 Mixed / failure** — normal list; failed/errored rows sort to the **top** (failure, error,
  pending, then success, then neutral) so the problem is first without scrolling; rollup pill shows
  the failed count. Within a group, preserve forge order (stable sort).
- **4.10 Error (fetch failed)** — reuse `PrPanel`'s inline `error-banner` (`role="alert"`,
  `--danger` 12% bg) at the top of the list area with a `Retry` button, **not** a toast (the panel
  is the natural home). Copy: `Couldn't load checks. <backend remedy sentence>` — backend message
  surfaced verbatim (§8), never raw libgit2/HTTP prose. `authFailed` routes to reauth like PrPanel
  (no toast). A refetch that also failed updates the freshness line: `Couldn't refresh — tried HH:MM.`

---

## 5. Refresh, feedback, freshness

- **Auto-update on fetch/pull/push (the user's core ask).** The container subscribes to the same
  completion signal the app already fires after remote ops (WorkspaceToolbar's fetch/pull/push
  completion — architect exposes the hook/event). On completion it refetches the checks target's
  tip status. Perceived feedback: the freshness line flips to `Checking…` (with header `⟳`
  disabled + `aria-busy`), then to `Updated just now`; changed rows simply re-render. **No motion,
  no toast** for the auto-refresh — it must not interrupt (a success toast on every fetch would be
  noise). This satisfies the watcher+manual-refresh invariant: auto (on remote op + window focus,
  reusing the app's existing focus rescan) **paired with** the manual `⟳`.
- **Manual refresh.** Header `⟳` (§2.2). Also add a command-palette entry `Refresh checks`
  (enabled only when the Checks tab's forge is connected).
- **Freshness readout.** `Updated <relative> ago` from `lastUpdated`; `Checking…` while in flight;
  `Never checked` before the first load; `Couldn't refresh — tried HH:MM.` on a failed refetch that
  still has stale data to show. 11px `--text-2`. A stale-while-error list keeps the last-good rows
  visible under the error banner rather than blanking.
- **Live region.** A single visually-hidden `role="status" aria-live="polite"` announces settle
  transitions only: `Checks updated: 3 of 8 failed.` / `Checks up to date, all passed.` Debounce
  ~150 ms so a burst of fetches doesn't flood the reader (same split the AI dock/notice bar use).

---

## 6. Accessibility

- **Tab semantics.** The new `Checks` button is a `role="tab"` in the existing tablist,
  `aria-selected`, panel `role="tabpanel"` with `aria-label="Checks"`. Arrow-key roving within the
  tablist follows the strip's existing behaviour — extend, don't fork.
- **List semantics.** The check list is a `<ul>`/`role="list"`; each `CheckRow` a `role="listitem"`.
  Each row has an accessible name folding state + name + description: e.g.
  `Failed: test / integration, 3 failing`. State word is in the name (glyphs are `aria-hidden`) —
  colour is never the sole carrier (§7).
- **Keyboard nav.** Rows are not focusable (not interactive). The interactive children — header
  `⟳`, tip-sha button, per-row `↗` — are in natural DOM/tab order, each ≥24px hit target, visible
  `:focus-visible` ring (2px `--accent`, 1px offset). PageUp/Down/scroll act on the scroll
  container.
- **Link-out safety (§5 privacy).** `↗` calls `ipc.openUrl(targetUrl)` — the backend opens the
  system browser; the frontend never embeds forge content in an `<a href>` that could be a
  `javascript:`/`data:` URL, and never puts repo data in a URL it constructs. `targetUrl` is
  forge-supplied untrusted data: it is used only as the `openUrl` argument, never rendered as text
  or a real anchor. Failure → the shared "Could not open …" toast.
- **Contrast, both themes.** All state colours are reused rollup-badge tokens already measured in
  §2/§7/§11: `--badge-good` glyph ≥4.6:1, `--badge-warn` glyph ≥4.4:1, `--warning` glyph 7.3:1
  dark / 4.5:1 light, `–` neutral glyph (`--text-3`) 3.38:1 on `--bg-1` (graphics ✓). Row text
  `--text-1`/`--text-2` per §2. No new token, so no new contrast pair to clear.
- **Reduced motion.** The `pending ●` must not pulse unless `prefers-reduced-motion: no-preference`;
  default to a static dot. No height/appearance animation on the panel or rows.

---

## 7. Microcopy (canonical strings)

- Tab label: **`Checks`** (not "CI" — plainer; matches "Working"/"Pull requests" register).
- Command palette: `Refresh checks`, `Show checks` (switches to the tab).
- Rollup pills: `✓ {n} checks passed` · `⚠ {f} of {t} failed` · `⊘ {n} errored` ·
  `● {p} running` · `– {n} neutral`.
- Freshness: `Updated just now` · `Updated {n} min ago` · `Checking…` · `Never checked` ·
  `Couldn't refresh — tried {HH:MM}.`
- Empty/error: see §4 (each has title + reason). Link-out `aria-label`: `Open {check name} in
  browser`. Load error: `Couldn't load checks. {backend remedy}`.

---

## 8. Harness fixture states (`src/ipc/mock/`, `VITE_MOCK_IPC=1`)

`fixtures/forge.ts` already has `commitStatusFor(sha)` covering every `CheckRollup` across the mock
branch tips. Extend the fixtures so the tab is fully verifiable in the browser:

- **all-pass** — a tip whose `CommitStatus` is all `success` (rollup pill green).
- **mixed/failure** — a tip with `failure` + `error` + `pending` + `success` + `neutral` contexts
  (exercises §4.9 sort + all five glyphs).
- **empty checks** — a tip with `total: 0` (§4.6).
- **no upstream** — a local-only branch fixture so the target resolves but has no pushed tip (§4.4).
- **loading** — a delayed mock response (§4.1 skeleton).
- **error** — a rejecting `forgeCommitStatus` (§4.10 banner + Retry).
- **pathological long-content** — a check with a 90-char name and a 200-char description + a very
  long branch name, to prove ellipsis + `title` + row height hold.
- **link-out** — mix of `targetUrl` present and null across rows (§2.3 column collapse).

Auto-refresh-on-fetch feedback and the `pending` non-pulse are DOM-observable (freshness text +
`aria-busy`) and thus AI-gate-verifiable; **frame timing / real scroll-feel remain USER
CHECKPOINT** (headless harness has no rAF).

---

## 9. New tokens

**None.** Every colour is an existing token (`--badge-good`, `--badge-warn`, `--warning`,
`--text-1/2/3`, `--bg-0/1/2`, `--border`, `--accent`, `--danger`). `ui-reference.md` needs **no new
token entry**; §3.1's state→glyph mapping is recorded here (contract-local) and mirrors the already-
documented `forgeBadges.ts` badge language, so no ui-reference edit is required this pass.
