# P83 — Merge & Close/Decline PRs — UI contract

Owner: ui-designer. Backend contract: `docs/contracts/P83-pr-actions.md`. This spec covers only
what the user sees; DTO/IPC shapes are the architect's. No new design tokens are introduced —
every color/radius/space below is an existing `--bg/-text/-border/-accent/-success/-warning/-danger`
token or an existing class (`btn-primary`/`btn-secondary`/`btn-danger`, `dialog-card`, `pr-field`,
`settings-segmented`).

---

## 1. Where the actions live

The two actions belong to the **PR detail view only, and only while `summary.state === 'open'`**.
A merged/closed PR shows its state pill and no actions (nothing to act on).

They render as a footer **action bar** pinned under the PR body/comments region of
`PrDetailView`, not floating in the header (the header is navigation + metadata; actions read as
a distinct commit-point). One primary action (**Merge**) + one quieter, danger-tinted secondary
(**Close/Decline**). No third button.

```
┌──────────────────────────── pr-detail ────────────────────────────┐
│  ← Pull requests                              Open in browser ↗     │
│  [Open]  Add dark-mode toggle              #128                     │
│  alice   feature/dark → main    opened 2026-08-19                   │
│  No conflicts   +214  −33   6 files                                 │
│  … body … comments …                                               │
├────────────────────────────────────────────────────────────────────┤
│  pr-actions-bar                                                      │
│  [ Close ]                              [  Merge…  ]  ← primary      │
└────────────────────────────────────────────────────────────────────┘
```

- **Bar geometry:** full panel width, top border `1px solid var(--border)`, padding
  `12px 16px` (cozy) / `8px 12px` (compact), `display:flex; justify-content:space-between`,
  `gap:8px`. Buttons are the standard button height (cozy 32px / compact 28px) — hit target ≥24px
  satisfied on both densities.
- **Order & hierarchy:** Close on the far left (`btn-secondary` with a danger-tinted class,
  §3), Merge on the far right (`btn-primary`). This mirrors dialog button order (destructive-left,
  affirmative-right) so muscle memory transfers.
- **Merge label:** `Merge…` — the ellipsis signals a dialog follows (house convention:
  `Manage identities…`, `Add another account…`).
- **Close label is per-forge** (`closeActionLabel(kind)` — new tiny pure map, colocated with the
  bar component):

  | ForgeKind    | button label | verb used in copy |
  |--------------|--------------|-------------------|
  | GitHub       | `Close`      | close             |
  | GitLab       | `Close`      | close             |
  | Bitbucket    | `Decline`    | decline           |
  | AzureDevOps  | `Abandon`    | abandon           |
  | (unknown)    | — (bar hidden; unsupported never reaches detail) | — |

---

## 2. Merge dialog (`PrMergeDialog`)

Merge needs a form (method picker + optional title/message + a checkbox), which the shared
`ConfirmDialog` cannot host. So Merge uses a **dedicated dialog built on the existing
`dialog-card` chrome** — and that dialog *is itself the confirmation* (no second modal). This is
the only justified new dialog; Close reuses `ConfirmDialog` unchanged (§3).

**Card:** `dialog-card pr-merge-card`, width 420px (matches the `ai-consent-card` precedent for a
form-bearing dialog; default 360px is too tight for the picker + fields). Same overlay, Esc, and
overlay-click-cancels behaviour as `ConfirmDialog`.

**Layout (top→bottom):**
1. `h2.dialog-title` — `Merge pull request #128?`
2. Summary line (`.dialog-body` lead paragraph): names method + base←compare + host + irreversibility.
   Copy is dynamic — see §4.
3. **Method picker** — reuse `SettingsSegmented` (`role="radiogroup"`), options filtered to
   `SUPPORTED_MERGE_METHODS[kind]`. All four forges expose ≤3 methods, within the segmented cap.
   - Label above the group (`.pr-field-label`, id `pr-merge-method-label`, wired via `labelledBy`):
     `Merge method`.
   - Segment labels (short): Merge→`Merge`, Squash→`Squash`, Rebase→`Rebase`,
     FastForward→`Fast-forward`.
   - A one-line `--text-3` description under the group, updated per selection:
     `Merge` → "Creates a merge commit." · `Squash` → "Combines all commits into one." ·
     `Rebase` → "Replays commits onto the base, no merge commit." ·
     `Fast-forward` → "Moves the base to the source tip, no merge commit."
   - Default selection = the forge default (Merge for all four). If
     `SUPPORTED_MERGE_METHODS[kind]` is empty the whole Merge affordance is hidden (bar shows only
     Close) — defensive; unsupported forges don't reach detail.
4. **Optional commit fields** (only meaningful for `Merge`/`Squash`; hide the pair for `Rebase`
   and `Fast-forward`, which ignore them):
   - `Commit title` — `input.pr-input`, placeholder = forge default title (empty ⇒ forge chooses).
   - `Commit message` — `textarea.pr-input.pr-textarea`, `rows={4}`, placeholder
     "Leave blank to use the forge default".
   Both use the `pr-field` label idiom from `PrCreateForm`.
5. **Delete-source-branch checkbox** — reuse the `.pr-draft-toggle` checkbox row idiom.
   Default **OFF**. Label: `Delete <sourceBranch> after merging` (branch name in `.mono`,
   truncated with title tooltip if long). GitHub ignores this on merge (architect §2) — still show
   it (consistent cross-forge form; harmless no-op there) so the form is identical everywhere.
6. `.dialog-buttons`: `Cancel` (`btn-secondary`, initial focus) + confirm
   `Merge pull request` (`btn-primary`; busy → `Merging…`, `disabled` while busy).

Merge's confirm is `btn-primary`, not `btn-danger`: merging is the constructive happy path, not
work-loss. Irreversibility is carried by the copy, per house destructive-copy rules.

---

## 3. Close/Decline confirmation (reuse `ConfirmDialog`)

Close/Decline is a plain destructive confirm — no form — so it reuses `ConfirmDialog` verbatim
(`confirmVariant='danger'`, default). Title/label/body are per-forge (§4).

The Close **button in the action bar** reads as secondary-but-dangerous: `btn-secondary` plus a
new modifier class `.btn-secondary-danger` that tints text/border to `--danger` on hover/focus
(base state stays `--text-2` so it does not scream for attention next to the primary). Color is
never the sole signal — the label verb (Close/Decline/Abandon) carries the meaning, and the
`ConfirmDialog` restates it. Contrast: `--danger` on `--bg-1` must be ≥4.5:1 in both themes
(verify against existing `btn-danger` text token, which already passes).

---

## 4. Microcopy (exact strings)

`{compare}` = `summary.sourceBranch`, `{base}` = `summary.targetBranch`, `{n}` = number,
`{host}` = `ctx.host`, `{method}` = lowercased segment label.

**Merge dialog**
- Title: `Merge pull request #{n}?`
- Summary: `This merges {compare} into {base} on {host} using a {method}. This can’t be undone from Bonsai.`
- Confirm button: `Merge pull request` / busy `Merging…`

**Close (GitHub/GitLab)**
- Title: `Close pull request #{n}?`
- Body: `This closes {compare} → {base} without merging its changes. Nothing in your local repository changes.`
- Confirm: `Close pull request` / busy `Closing…`

**Decline (Bitbucket)**
- Title: `Decline pull request #{n}?`
- Body: `This declines {compare} → {base} without merging its changes. Nothing in your local repository changes.`
- Confirm: `Decline pull request` / busy `Declining…`

**Abandon (Azure DevOps)**
- Title: `Abandon pull request #{n}?`
- Body: `This abandons {compare} → {base} without merging its changes. Nothing in your local repository changes.`
- Confirm: `Abandon pull request` / busy `Abandoning…`

Branch names in every string render as `.mono` inline spans, truncated with a `title` tooltip.

**Not-mergeable reason** (shown in the bar, replacing/annotating the disabled Merge — §5): reuse
the existing `pr-mergeable` pill text (`Has conflicts` / `Mergeability pending`); the disabled
Merge button's `title` = the sentence form (§5).

**Error banner / toast** after a failed action: verbatim forge message from the AppError, prefixed
by the action, e.g. `Could not merge pull request #{n}: {forge message}` (toast) and the raw
`{forge message}` in the in-panel `error-banner` (matches the existing detail-error pattern). No
libgit2/HTTP status leakage — the backend already maps to a clear sentence (architect §3).

---

## 5. States (every one)

| State | Merge button | Close button | Notes |
|---|---|---|---|
| PR open, `mergeable===true` | enabled `Merge…` | enabled | happy path |
| PR open, `mergeable===null` | **disabled**, title `Bonsai is still checking whether this can be merged.` | enabled | picker not reachable |
| PR open, `mergeable===false` | **disabled**, title `This pull request has conflicts and can’t be merged.` | enabled | the `Has conflicts` pill already shows in the header stats |
| method set empty (unknown/unsupported) | **hidden** | enabled | defensive only |
| action in flight | both **disabled**; the invoked dialog's confirm shows busy label + `disabled`; panel is locked (no row selection, no back nav) | see left | `aria-busy` on `.pr-detail` |
| success | — | — | container replaces `detail` with the returned `PrDetail`; state pill flips to `Merged`/`Closed`; action bar disappears; success toast; list refetch (`listTick++`) |
| error | re-enabled | re-enabled | dialog closes, `error-banner` shows forge message, toast fires, `loadDetail(n)` refetch — nothing changed locally |
| long content | branch names/title truncate with ellipsis + `title`; dialog message wraps, card scrolls if body exceeds viewport | | |

Disabled Merge uses `:disabled` styling only (never a color-only cue); the `title` sentence is the
explanation, and the header's `Has conflicts` pill is the always-visible non-hover signal.

---

## 6. Interaction, keyboard, a11y (hard requirements)

- **Merge flow:** Merge… button → `PrMergeDialog` opens, initial focus on **Cancel** (house rule:
  a stray Enter never fires a destructive/irreversible action). Tab order: Cancel → Merge →
  method radios → title → message → checkbox → (wrap). Arrow keys move within the radiogroup
  (native, free from `SettingsSegmented`). Esc and overlay-click cancel. On close, focus returns
  to the Merge… button (focus restore).
- **Close flow:** Close/Decline/Abandon button → `ConfirmDialog` (already focus-traps to Cancel,
  Esc/overlay cancel, Enter activates only the focused button). Focus returns to the Close button.
- **Roles/names:** merge dialog `role="dialog" aria-modal="true"` with
  `aria-labelledby` → the `dialog-title` id (name = "Merge pull request #128?"); method group
  `role="radiogroup" aria-labelledby="pr-merge-method-label"`; checkbox is a real
  `<input type="checkbox">` with a `<label>`. `ConfirmDialog` a11y is unchanged.
- **Focus ring:** 2px `--accent`, 1px offset, `:focus-visible` only (existing button/dialog rule).
- **Hit targets:** all controls ≥24px; bar buttons meet the standard 28/32px heights.
- **`aria-busy="true"`** on `.pr-detail` while an action is in flight; the confirm button's busy
  label ("Merging…") is the visible cue (no spinner-only state).
- **Reduced motion:** the only motion is the dialog's existing ≤150ms fade/scale-in — already
  honours `prefers-reduced-motion`; no new animation added.
- **Color-independence:** action meaning carried by verbs (Merge/Close/Decline/Abandon), the
  state pill text, and the conflicts pill — never hue alone.

---

## 7. Component decomposition + file paths

- **New** `src/components/PrActionsBar.tsx` — presentational footer bar: takes
  `state`, `kind`, `mergeable`, `supportedMethods`, `busy`, `onMerge`, `onClose`; renders the two
  buttons with per-forge close label and the disabled/hidden logic of §5. Contains the tiny
  `closeActionLabel(kind)` map. (~90 lines.)
- **New** `src/components/prPanel/PrMergeDialog.tsx` — the merge form dialog (§2): local field
  state (method/title/message/deleteSourceBranch), builds the `MergePrInput`, calls
  `onConfirm(input)`. Presentational + local form state only; no IPC. (~150 lines.)
- **Edit** `src/components/PrDetailView.tsx` — render `<PrActionsBar>` at the foot when
  `summary.state==='open'`; add props `kind`, `mergeable` (already has detail), `busy`,
  `supportedMethods`, `onMerge`, `onClose`. Stays presentational and under the size limit
  (currently 137 lines).
- **Edit** `src/components/PrPanel.tsx` (container) — add `merging`/`closing` busy state,
  `showMergeDialog`/`showCloseConfirm` flags, `handleMerge(input)`/`handleClose()` calling
  `ipc.forgeMergePr`/`ipc.forgeClosePr`, success → replace `detail` + `listTick++` + toast, error →
  `setDetailError` + toast + `loadDetail(n)`; mount `PrMergeDialog` and a `ConfirmDialog` for close.
  Reuse `handleAuthFailed` for the `authFailed` branch (reauth, no toast — OD-3).
- **Reuse unchanged:** `ConfirmDialog.tsx`, `SettingsSegmented.tsx`, `dialog-card`/`pr-field`
  styles.
- **`SUPPORTED_MERGE_METHODS` + method/description label maps:** small pure module
  `src/components/prPanel/mergeMethods.ts` (labels here so the dialog and bar share them).

Do **not** append any of this to the already-large `PrPanel.tsx` render body beyond the container
state/handlers + the two dialog mounts.

---

## 8. CSS additions (existing tokens only)

- `.pr-actions-bar` — flex footer, top `1px var(--border)`, padding per density (§1).
- `.pr-merge-card` — width 420px on top of `.dialog-card` (mirrors `.ai-consent-card`).
- `.pr-merge-method-desc` — `--text-3`, `font-size:12px`, `margin:4px 0 0`.
- `.btn-secondary-danger` — `--danger` text/border on `:hover`/`:focus-visible`; base `--text-2`.
- Reuse `.pr-field`, `.pr-input`, `.pr-textarea`, `.pr-draft-toggle`, `.settings-segmented`.

All must be verified in dark and light and at cozy/compact.

---

## 9. Harness / mock states to see it (VITE_MOCK_IPC=1)

Per architect §4 the mock already flips state and adds a `mergeable===false` fixture row. To verify
every branch in the browser, the fixtures must cover:
- `?forge=auth` (GitHub): an **open, mergeable** PR (Merge enabled, 3-method picker),
- an open PR with `mergeable:false` (Merge disabled + conflicts reason),
- an open PR with `mergeable:null` (Merge disabled + pending reason),
- `?forge=gitlab` (2-method picker), `?forge=bitbucket` (Decline + Fast-forward method),
  `?forge=azure` (Abandon),
- a merged and a closed PR (action bar absent),
- a long-branch-name PR (truncation + tooltip in bar, dialog, and checkbox label),
- unauthenticated (`forgeAuthRequired` on action → reauth view, no toast).

Actual merge/close network round-trips against real forges are **USER CHECKPOINT** (architect §6);
the harness verifies the UI states, disabled logic, dialog copy, method filtering, and the
mock's merged/closed transition, but not real success/failure.

---

## 10. Flagged for orchestrator

- Merge confirm is styled `btn-primary` (constructive), Close/Decline `btn-danger`. If the team
  prefers Merge also read as danger (irreversible outward write), switch the merge confirm to
  `btn-danger` — I recommend primary, keeping danger for the work-abandoning action.
- `delete_source_branch` checkbox is shown even on GitHub (where merge ignores it) for a uniform
  cross-forge form. Alternative: hide it for GitHub. I recommend keeping it visible + a
  `--text-3` note "GitHub deletes the branch from its own settings" — but that adds copy; flagging
  the choice rather than deciding.
