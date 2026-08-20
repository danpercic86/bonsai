# P77 — Tag Sync Management — UI Contract

## LOCKED DECISIONS (orchestrator + user, 2026-08-20) — override any conflicting draft below
- **Status set that ships:** `in-sync` (no badge) · `local-only` (muted "unpushed" pill; ALSO
  covers the would-be deleted-on-remote case for v1) · `stale`/moved (⚠ out-of-sync pill) ·
  **`remote-only` IS IN SCOPE** — render ghost rows for upstream tags absent locally, with a
  "fetch this tag" action (the architect report supplies them cheaply from ls-remote). The
  open-question §0 recommendation to CUT remote-only is **overruled — build it.**
- **No distinct `deleted-on-remote` badge in v1** (folded into local-only/unpushed).
- **Single remote for v1:** compare against `origin`/first remote; name it in every label/tooltip.
- **Rollup header badge:** counts true divergences only (out-of-sync); ghost/remote-only and
  unpushed do NOT inflate the ⚠ count.
- Destructive remote ops (delete-remote, force-move-remote) → confirm dialogs (`TagSyncDialogs.tsx`).
  Local "update to remote target" (force-refresh) stays frictionless (reversible via reflog).

Owner: ui-designer. Implementer: senior-dev. This contract is downstream of the architect's P77
data contract (not yet on disk — see §0 "Data the UI needs"). All colours, radii and fonts are
existing tokens from `docs/contracts/ui-reference.md`; **no new `:root` / light token is
introduced.** One small reusable pattern (the section-header rollup badge) is added to
`ui-reference.md` §11 in the same pass.

Product lock honoured: **inline sidebar only** — the per-tag sync state extends the existing Tags
rows and their context menu. No new panel, tab, or list dialog. The only new modals are the two
destructive-op confirm dialogs the Bonsai invariant requires.

---

## 0. Data the UI needs (input to architect — FLAG)

The UI is built against this shape. If the architect's contract differs, reconcile before build.
Per **currently-local** tag, keyed by tag name:

- `sync`: `'inSync' | 'unpushed' | 'outOfSync' | 'deletedOnRemote'`
- `localShort`: string (7-char) — the local target, for the out-of-sync tooltip + force-move copy.
- `remoteShort`: string | null — the remote target where it differs (`outOfSync`).
- `remote`: string — the remote this verdict is against (see the single-remote decision, §2.4).

Plus a **repo-level** block:

- `remoteOnly`: `{ name: string; remoteShort: string }[]` — tags on the remote, absent locally.
- `state`: `'idle' | 'checking' | 'ready' | 'unavailable'` — the ls-remote lifecycle.
- `checkedAt`: number | null — unix secs of the last successful check (for the "last checked"
  tooltip).

The verdict is **per-open, live** — it is not persisted between app launches (a stale cached
verdict is exactly the bug this feature exists to kill). `sync` is absent/`inSync` for every tag
until `state === 'ready'`; while `checking`/`unavailable`, rows render from local data with **no
badge** (§2.2).

---

## 1. Placement & geometry

Everything lives inside the existing left sidebar Tags `section` (`Sidebar.tsx` ~L731) and
`tagMenuItems` (`workspaceMenus.ts` L503). No layout change to the 3-pane shell.

### 1.1 TagRow anatomy (extended)

Existing (`Sidebar.tsx` `TagRow` ~L214): `[glyph #] [name flex ellipsis]`. Add a trailing badge,
exactly as worktree/submodule rows already append pills:

```
┌ .branch-row (24px, 0 4px pad, 6px gap) ───────────────────────────┐
│ [# 14px] [ v1.1.0 ················· flex, ellipsis ] [⚠ out of sync]│
└───────────────────────────────────────────────────────────────────┘
```

- Badge is `flex: none`, sits after `.branch-name`, uses the existing `.submodule-badge-*` classes
  (§2.1). Row height, gap, padding, hover unchanged (`sidebar.css` `.branch-row`).
- The **name** ellipsizes; the badge never compresses (§11 rule). Long tag name + badge → name
  truncates with its existing `title`.
- Density-invariant: the sidebar has one geometry in both `cozy` and `compact` (ui-reference §3).

### 1.2 Section-header rollup (collapsed-by-default problem)

Tags start collapsed (`Sidebar.tsx` L398). A user must see there is a problem **without expanding**.
`SectionHeader` already accepts an `extra` ReactNode slot (L106/L124) rendered right of the toggle —
Stashes uses it. Put a rollup badge there:

```
┌ .sidebar-section-header (24px) ────────────────────────────────┐
│ › TAGS                                              [⚠ 2]        │  ← extra slot
└─────────────────────────────────────────────────────────────────┘
```

- Reuses `.submodule-badge-warn` (§2.1) — a warning verdict pill showing the **count** of tags that
  have diverged (`outOfSync` + `deletedOnRemote`).
- `flex: none`, `margin-left: auto` so it hugs the right edge (Stashes' `extra` precedent).

---

## 2. Badge design per status

### 2.1 Row badge vocabulary (status → visual)

Reuse the two existing pill recipes from `sidebar.css` / ui-reference §11 — **do not invent a
parallel colour language.** Verdict pills carry the `⚠` glyph in a `.submodule-badge-glyph` span;
muted pills are word-only.

| Status | Row badge | Class | Glyph | Label |
|---|---|---|---|---|
| **in-sync** | *none* — clean row | — | — | — |
| **local-only (unpushed)** | muted pill | `.submodule-badge-muted` | none | `unpushed` |
| **stale / moved** | warning verdict pill | `.submodule-badge-warn` | `⚠` | `out of sync` |
| **deleted-on-remote** | warning verdict pill | `.submodule-badge-warn` | `⚠` | `deleted on remote` |
| **remote-only** | muted pill (on a dimmed row) | `.submodule-badge-muted` | none | `remote only` |
| **unknown / unavailable** | *none* (§2.2) | — | — | — |

Rationale for hues: `out of sync` and `deleted on remote` are the actionable divergences this
feature exists to surface → `--warning` (a verdict, not `--danger`: nothing is lost locally, the
fix is one click). `unpushed` and `remote only` are neutral facts, not faults → the hueless muted
pill (`--text-2` label on its own 12% tint, 5.79:1 dark / 6.22:1 light). **No new token.** In-sync
is deliberately badge-less — restraint; the section rollup carries the "all clear" signal by its
absence.

Labels are lowercase (§11 row-pill rule). The word is the non-colour meaning carrier for muted
pills; the `⚠` glyph is the non-colour carrier for verdict pills (measured 5.64:1 dark / 3.80:1
light on the 14% tint — clears the 3:1 graphics bar). Colour never travels alone anywhere.

### 2.2 Loading / checking

Rows render **immediately** from local `BranchesSnapshot` data — the network never blocks the list.
While `state === 'checking'` (ls-remote in flight): rows show **no badge** (a busy pill on every tag
would be noise). The signal lives on the header only:

- Section-header `extra` shows a hueless `.submodule-badge-muted` pill labelled `checking…`
  (trailing U+2026), on a container with `aria-busy="true"`. No spinner (ui-reference §8 — words,
  not spinners; reduced-motion-safe).

When the result arrives (`ready`), badges appear. Motion: `opacity` 0→1, 120ms ease-out, on the
badge only. `prefers-reduced-motion` ⇒ no transition (reduced-motion block, ui-reference §9).

### 2.3 Offline / auth-fail / unavailable

ls-remote is a network call and **must degrade, never explode** (no error banner, no toast for a
routine offline). When `state === 'unavailable'`:

- Rows render exactly as before the feature — **no badges** (in-sync and unavailable are
  indistinguishable at row level; that is intentional — we never assert a verdict we could not
  verify).
- Collapsed header shows **no rollup** — a false `⚠` on an unreachable remote is worse than silence.
- Expanded, a single `.branch-muted` line sits under the filter, above the list:
  `Couldn't reach {remote} — showing local tags only.` with `title` = `Last checked {relative}` when
  `checkedAt` is set. This is informational, not an error banner.

### 2.4 No remote configured

Feature gracefully absent: `remotes.length === 0` ⇒ no sync check runs, no badges, no rollup, no
"remote only" rows, and every remote-scoped menu item (§3) is omitted. The Tags section is
byte-identical to the pre-P77 UI.

**Single-remote decision (FLAG for user/architect).** Tags do not track a remote. v1 compares each
local tag against **one** remote — `origin` if present, else the first configured remote — and every
badge/menu label names that remote explicitly (`Delete tag on origin`). Comparing against *all*
remotes multiplies the state space and the menu length for a rare case. Recommend single-remote for
v1; revisit if users ask.

### 2.5 Section rollup badge

| Header state | `extra` content | Class | aria |
|---|---|---|---|
| ≥1 diverged tag (`outOfSync`+`deletedOnRemote`) | `⚠ {N}` | `.submodule-badge-warn` | `aria-label="{N} tags out of sync on {remote}"` |
| checking | `checking…` | `.submodule-badge-muted` | container `aria-busy="true"` |
| all in-sync / unpushed / remote-only only | *nothing* | — | — |
| unavailable | *nothing* | — | — |

The digit `{N}` is the non-colour carrier for the verdict rollup (a count is a sanctioned carrier,
ui-reference §7). `title` = `{N} tags differ from {remote}. Expand to resolve.`

**Rollup counts divergences only, not `unpushed`/`remote-only` (FLAG — design choice).** A user who
tags locally all day would otherwise see a permanent count on a collapsed section for tags they
simply haven't pushed yet, draining the signal's meaning. The collapsed `⚠` should mean "something
diverged that you did not intend." Unpushed and remote-only are surfaced only on expand. If the
user wants unpushed counted too, it is a one-line change.

### 2.6 Remote-only rows (scope note — FLAG)

Showing remote-only tags requires the list to include tags that are **not local** — a small
expansion of what the Tags section renders. Spec: after the local tags (both flat and tree modes),
append the `remoteOnly` entries as `.branch-row .branch-row-readonly` rows with the `#` glyph at
`opacity: .7` on the glyph+name (a ghost row: it isn't in your repo yet), a `remote only` muted
pill, and the tag name in `.branch-name-muted`. They participate in the list filter. If the
architect cannot supply `remoteOnly` cheaply, **cut this status for v1** and keep the other four —
say so and I will drop §2.6 and the "Create local tag" menu item; the feature still solves the
reported bug (stale/moved is the reported case).

---

## 3. Context menu spec (`tagMenuItems`)

`ContextMenu` supports `tone: 'danger'` (red icon+label) and per-item `disabled`; it has **no
separator primitive**, so grouping is by order only. Full ordered list (items appear only when their
precondition holds; `gate = mutating || opActive` disables during any in-flight op, matching the
existing menu). Existing items are marked *(existing)*.

| # | Label | Shown when | Enabled when | tone | → confirm? |
|---|---|---|---|---|---|
| 1 | `Update to remote target` | `outOfSync` | `!gate` | default | no |
| 2 | `Create local tag` | remote-only row | `!gate` | default | no |
| 3 | `Push tag to {remote}` *(existing)* | remote configured | `!gate` | default | no |
| 4 | `Copy tag name` *(existing)* | always | always | default | no |
| 5 | `Release notes since previous tag` *(existing)* | always | `aiEligible` | default | no |
| 6 | `Delete tag` *(existing, local)* | always | `!gate` | default | yes (existing local confirm) |
| 7 | `Delete tag on {remote}…` | tag exists on remote (`inSync`/`outOfSync`/`deletedOnRemote`→ n/a; i.e. not `unpushed`, not `deletedOnRemote`) | `!gate` | **danger** | **yes (§4.1)** |
| 8 | `Force-move tag on {remote}…` | `outOfSync` | `!gate` | **danger** | **yes (§4.2)** |

Ordering logic: resolve-in-place actions first (the point of the feature, cheapest fix on top),
then publish, then the frozen utility items, then destructive local delete, then the two
destructive **remote** ops last — the two `tone:'danger'` remote items are the visual floor of the
menu, so a mis-click can't land on them. The trailing `…` on items 7/8 signals "opens a dialog"
(house convention, matches `Remove…`, `Rename…`).

Notes:
- **Item 1 (`Update to remote target`)** resolves the reported bug: force-refresh the stale local
  tag to the remote's target. No confirm — it is a purely **local** pointer move, low-risk, and the
  whole point is "fixable at a glance". The success toast states the change so it is never silent
  (§5). Its inverse is item 8 (push local over remote), which *is* destructive to shared history and
  *is* gated.
- **Item 2 (`Create local tag`)** appears on a remote-only ghost row only; creates the local tag at
  the remote's target.
- **Item 7** is omitted for `unpushed` (nothing on the remote to delete) and for `deletedOnRemote`
  (already gone). It is the destructive counterpart to keeping only the local copy.
- Icons reuse the existing set: `DeleteIcon` (6,7), `TagIcon` (2,3,8), `CopyIcon` (4),
  `SummarizeIcon` (5). Item 1 reuses a refresh/sync glyph — use the existing refresh icon component
  if one is exported, else `TagIcon`; do not add a new SVG for it.

---

## 4. Confirm-dialog copy (destructive — Bonsai invariant)

Both use the shared `ConfirmDialog` (`confirmVariant='danger'`, default). Put them in a **new small
file** `src/components/dialogs/TagSyncDialogs.tsx` (do not grow `BranchTagDialogs.tsx`, which already
owns the local tag-delete). Tag names and SHAs render in `<span className="mono">`. The must-read
consequence line uses `.dialog-body-detail` (12px `--text-2`), matching the force-push confirm.

### 4.1 Delete tag on remote

- **Title:** `Delete tag on {remote}?`
- **Body:** `Delete "`{`v1.1.0`}`" from `{`origin`}`?`
- **Detail (`.dialog-body-detail`):** `This removes the tag for everyone who uses {remote}. Anyone
  who has already fetched it keeps their copy until they prune. This cannot be undone from Bonsai.`
- **Confirm button:** `Delete on {remote}`
- **Busy:** `remoteOp === 'push'` (or the P77 op flag the architect exposes).

### 4.2 Force-move tag on remote

- **Title:** `Force-move tag on {remote}?`
- **Body:** `Move "`{`v1.1.0`}`" on `{`origin`}` from `<span className="mono">`{`oldShort`}`</span>`
  to `<span className="mono">`{`newShort`}`</span>`?` — always show **old → new** target, like a
  good force-push confirm.
- **Detail (`.dialog-body-detail`):** `{remote}'s tag currently points to {oldShort}; this
  overwrites it with your local {newShort}. Anyone who already fetched {tag} keeps the old target
  until they re-fetch it by force — moving a shared tag is a common source of confusion. This cannot
  be undone from Bonsai.`
- **Confirm button:** `Force-move`
- **Busy:** as §4.1.

Both: initial focus on Cancel (ConfirmDialog default — a stray Enter never confirms), Esc and
overlay-click cancel, focus restore is the ConfirmDialog default. No focus trap (house-wide
decision, ui-reference §12.4 — do not add one here alone).

---

## 5. Microcopy (final strings)

**Row-badge tooltips (`title`):**
- unpushed: `Not on {remote} yet. Right-click to push it.`
- out of sync: `Your {tag} points to {localShort}; {remote} has {remoteShort}. Right-click to resolve.`
- deleted on remote: `{tag} was removed from {remote}. Your local copy remains.`
- remote only: `On {remote}, not in your repo. Right-click to create it locally.`

**Rollup tooltip:** `{N} tags differ from {remote}. Expand to resolve.`

**Expanded unavailable line:** `Couldn't reach {remote} — showing local tags only.`
(`title="Last checked {relative}"` when known.)

**Success toasts** (tone `success`, auto-dismiss; §5 house shape):
- update to remote: `Updated {tag} to match {remote}.`
- create local: `Created local tag {tag}.`
- delete remote tag: `Deleted {tag} on {remote}.`
- force-move remote tag: `Moved {tag} on {remote} to {newShort}.`

**Failure toasts** (tone `error`, sticky, dedupe `key: "tagsync:{tag}"`): the frontend supplies
`Couldn't {verb} {tag}.` and surfaces the backend `AppError.message` remedy verbatim
(ui-reference §8) — e.g. `Couldn't delete {tag} on {remote}.` + backend sentence. Never raw libgit2
prose. A routine offline check is **not** a toast — it is the quiet §2.3 unavailable state.

Empty state unchanged: `No tags` (`.branch-muted`, existing).

---

## 6. Interaction, keyboard & sync trigger

**When sync runs (UX — recommend, live per-open):**
1. On **Tags section expand** (first expand per app-open, then cached for the session).
2. On the existing **manual refresh** button (workspace toolbar) — re-checks.
3. On **window focus** (paired with the notify watcher per the CLAUDE.md invariant — remote state
   is only knowable by network, the watcher can't see it, so focus-rescan is the substitute).

No auto-polling. `checkedAt` drives the "last checked" tooltip. This is the only sanctioned trigger
set; do not fetch on every render.

**Keyboard / pointer:**
- Rows keep their existing right-click → `onTagContextMenu` behaviour; badges are display-only and
  add no new pointer target inside the row.
- The rollup badge in the header is **display-only** (not a button) — it sits in the header's `extra`
  slot beside the existing toggle button, which remains the sole tab stop and expand control.
  Clicking anywhere on the header still toggles via the existing `.sidebar-section-toggle`.
- Context-menu items are keyboard-reachable via the existing `ContextMenu` roving focus; the two new
  destructive items are `tone:'danger'` and last, reached by continued arrow-down.
- Confirm dialogs: Esc/overlay cancel, Enter activates the focused (Cancel) button only.

**Command palette:** no new palette entries — these actions are tag-instance-scoped (they need a
selected tag + remote), which the palette's global-action model doesn't carry. Consistent with the
existing tag actions, none of which are in the palette.

---

## 7. Accessibility

- **Non-colour affordance on every badge (WCAG 1.4.1).** Verdict pills carry the `⚠` glyph; muted
  pills carry the word. Colour is never the sole carrier — the house precedent (A/M/D/U/R, §7).
- **The badge meaning is in the accessible name of the row.** The pill's visible text already reads
  (`out of sync`, `unpushed`), and `.submodule-badge-glyph` is `aria-hidden` (the glyph duplicates
  the word). No extra ARIA needed on the row — the badge text is a normal child of the `<li>`.
  Verify the badge text is not `aria-hidden`.
- **Rollup badge** is `aria-label`'d (`{N} tags out of sync on {remote}`) since `⚠ 2` alone is
  ambiguous to a screen reader; the `⚠` glyph itself is `aria-hidden`.
- **Contrast (both themes, measured recipes reused from ui-reference §2/§11):** warn verdict pill
  label `--text-1` on `--warning` 14% tint = 9.24–10.30:1 dark / 11.68–12.00:1 light (AA text ✓);
  `⚠` glyph 5.64:1 dark / 3.80:1 light (1.4.11 ✓); muted pill `--text-2` on its own 12% tint =
  5.79:1 / 6.22:1 (AA text ✓). No new pair introduced → nothing new to measure.
- **Hit targets:** no new interactive control below 24px — badges are non-interactive; the header
  toggle stays the existing 24px stretched control; menu items and dialog buttons are the existing
  ≥24px controls.
- **Confirm dialogs:** `role="dialog" aria-modal="true"`, initial focus Cancel, Esc cancels, focus
  restore — all inherited from `ConfirmDialog`.
- **`checking` state** container carries `aria-busy="true"`; the settled verdict is announced only
  by the row content changing (no assertive live region — a routine background check must not
  interrupt).
- **Reduced motion:** the only motion is the badge opacity fade (§2.2), suppressed under
  `prefers-reduced-motion`.

---

## 8. Component decomposition & file paths

- `src/components/sidebar/TagSyncBadge.tsx` *(new, ~40 lines)* — presentational: takes the tag's
  `sync` status (+ short SHAs, remote) and renders the correct `.submodule-badge-*` pill with glyph
  + `title`. Returns `null` for `inSync`/absent. Reused by `TagRow` and the remote-only ghost row.
- `src/components/sidebar/SectionRollupBadge.tsx` *(new, ~30 lines, reusable)* — takes a count +
  state and renders the header `extra` pill (`⚠ {N}` / `checking…` / null). Generic enough that
  Branches/Remotes could adopt it later; keep it status-agnostic (props: `count`, `busy`, `label`,
  `ariaLabel`).
- `src/components/dialogs/TagSyncDialogs.tsx` *(new, ~90 lines)* — the two §4 confirm dialogs, wired
  by props from the container, mirroring `BranchTagDialogs.tsx`'s shape. **Do not** add these to
  `BranchTagDialogs.tsx`.
- `src/components/Sidebar.tsx` *(edit, small)* — `TagRow` gains a `sync` prop and renders
  `<TagSyncBadge>`; the Tags `<SectionHeader>` gains an `extra={<SectionRollupBadge …>}`; the
  remote-only rows are appended; the `unavailable` `.branch-muted` line is added. All additive and
  small — no new large block; if the Tags section render grows past the file's comfort, extract a
  `TagsSection.tsx`, but the diff should be small enough not to need it.
- `src/components/workspaceMenus.ts` *(edit)* — `tagMenuItems` gains the four status-gated items
  (§3); the container gains `pendingDeleteRemoteTag` / `pendingForceMoveTag` state + handlers,
  following the `pendingDeleteTag` precedent.
- **No CSS file changes** — every class already exists in `sidebar.css`. If the remote-only ghost
  dim needs a class, add `.branch-row-ghost { opacity: .7 }` to `sidebar.css` (single rule, within
  the §2 dimming budget: one `.7` layer, and the row is non-interactive-by-content so the budget
  applies cleanly). Prefer reusing `.branch-name-muted` on the name and skipping a row-level dim if
  it reads clearly.

---

## 9. Harness / mock-IPC states (`src/ipc/mock/`, `VITE_MOCK_IPC=1`)

The feature is fully browser-verifiable (no native-only surface). Fixtures needed:

- **ready + mixed:** a Tags list with one of each status — `inSync`, `unpushed`, `outOfSync`
  (e.g. `v1.1.0`, the reported case, local `a1b2c3d` vs remote `9f8e7d6`), `deletedOnRemote`, and a
  `remoteOnly` entry. Verifies all five badges + the `⚠ 2` rollup + both menus.
- **checking:** `state: 'checking'` — verifies rows render badge-less + header `checking…` pill.
- **unavailable:** `state: 'unavailable'` — verifies graceful degrade (no badges, no rollup, the
  expanded "couldn't reach" line).
- **no remote:** `remotes: []` — verifies the feature is absent (§2.4).
- **all in-sync:** verifies the clean/no-rollup baseline.
- **pathological:** a 60-char tag name + an `outOfSync` badge (name truncates, badge intact); a repo
  with ~40 diverged tags (rollup shows `⚠ 40`, doesn't overflow the 24px header).

All of the above are visible in a plain browser. **No USER CHECKPOINT items** beyond the native
frame-timing note (the badge opacity fade can't be judged headless — rAF is paused — but it is
non-load-bearing).

---

## 10. Acceptance checklist (design review)

- [ ] All five statuses + unknown render the correct badge (or correct absence) per §2.1.
- [ ] Collapsed Tags header surfaces divergence via the `⚠ {N}` rollup; no rollup when clean,
      checking, or unavailable.
- [ ] Offline/auth-fail degrades to the §2.3 quiet state — no error banner, no error toast.
- [ ] No-remote repo is byte-identical to pre-P77 (§2.4).
- [ ] Context menu matches §3 order + gating; the two remote destructive items are `tone:'danger'`,
      last, and route to confirm dialogs.
- [ ] Confirm-dialog copy matches §4 verbatim, shows old→new target for force-move, uses
      `.dialog-body-detail` for the consequence, `confirmVariant='danger'`.
- [ ] Every badge pairs colour with a glyph or word (1.4.1); rollup has an `aria-label`.
- [ ] All tokens are existing custom properties; **zero hardcoded hex**; no new `:root`/light token.
- [ ] Both themes and both densities verified (density-invariant, but confirm nothing broke).
- [ ] All states present: default, hover, checking, unavailable, empty, long-content overflow.
- [ ] Success/failure toasts match §5; failure toasts dedupe on `tagsync:{tag}`.
- [ ] New files are the small presentational units in §8; nothing appended to a large file.
