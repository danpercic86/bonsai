# P82 — Submodule deinit/remove force (discard-dirty) — UI contract

Safety gap **F-A7-7**: today `deinitSubmodule` / `removeSubmodule` always shell out with `-f`, so a
single generic confirm silently destroys uncommitted work inside a dirty submodule. Backend fix
(implemented separately): drop the unconditional `-f`; plain ops **refuse** when the submodule
worktree is dirty; `force: boolean` re-runs with `-f`. This contract designs how the user opts into
force — a deliberate, secondary escalation, never the default.

Owner reads before implementing: `ConfirmDialog.tsx:28-87`,
`dialogs/SubmoduleDialogs.tsx:49-93`, `repoWorkspace/useSubmoduleActions.ts:47-138`,
`components/workspaceMenus.ts:429-474`, `sidebar/submoduleBadges.ts` (the `modifiedWorkdir` badge).

---

## 1. Decision — Flow A (attempt-then-offer-force). RECOMMENDED.

**Flow A wins over Flow B (upfront checkbox) decisively:**

- **The authoritative dirty signal lives in the backend, not the UI.** `SubmoduleStatus` has a
  `modifiedWorkdir` value, but that is a display classification and can be stale, and it does not
  cover every dirtiness the backend refuses on (staged-only changes, untracked files, a dirty
  nested submodule). Flow B's checkbox must decide up-front whether to even *show* the danger
  option: gate it on `modifiedWorkdir` and a differently-dirty submodule reaches a dead end (plain
  op fails, no way to force); don't gate it and the danger checkbox is **always visible on clean
  submodules** — exactly the "force is the default surface" footgun the rule forbids. Flow A keys
  the escalation off the **real refusal**, so it is correct by construction and never a dead end.
- **The safe path stays byte-identical to today.** `dialogs/SubmoduleDialogs.tsx:49-93` is unchanged
  for the clean case: deinit is still one `confirmVariant='primary'` confirm, remove is still one
  danger confirm. Force is reached only when the op genuinely cannot proceed — pure progressive
  disclosure, the GitButler-clean choice.
- **No new menu chrome.** The row menu (`workspaceMenus.ts:452-464`) keeps exactly its two items
  (`Deinitialize…`, `Remove…`). Force is never a peer menu entry — that would make the dangerous
  path a sibling of the safe one. A separate "Force remove…" menu item is explicitly rejected.
- **Reuses `ConfirmDialog` three ways** (two existing + one new danger instance in the same file);
  no new component, no new token.

Flow A's one cost is a backend-coordination dependency (§7) — the refusal must be a **typed** error
the UI branches on, not a string match. This is a net positive: it forces the correct, structural
signal instead of parsing a libgit2 sentence.

---

## 2. Flow (both ops share the shape; copy differs — never conflated)

```
Row menu ─▶ "Deinitialize…" / "Remove…"
   │
   ▼
[ Safe confirm ]  (existing dialogs, unchanged)
   │  Confirm
   ▼
handleDeinitSubmodule(name)          ← force:false (default)
handleRemoveSubmodule(name)
   │
   ├─ success ─▶ toast "Deinitialized <name>" / "Removed <name>"   (unchanged)
   │
   ├─ refused BECAUSE DIRTY (typed) ─▶ open [ Force escalation dialog ]  ← NOT a toast
   │                                        │  Confirm (danger, Cancel-focused)
   │                                        ▼
   │                                   handle…Submodule(name, { force:true })
   │                                        ├─ success ─▶ same success toast
   │                                        └─ failure ─▶ generic error toast (§5)
   │
   └─ failed for ANY OTHER reason ─▶ generic error toast (existing runRowOp catch)
```

Deinit and remove are **different ops** and their force copy must reflect it:
- **Deinit** keeps `.gitmodules` + the gitlink; only the local worktree/config are cleared →
  re-initializable. Force discards only the *uncommitted work inside* the worktree.
- **Remove** is a full teardown (drops `.gitmodules`, stages the gitlink removal, deletes the
  worktree) → not undoable from Bonsai. Force additionally discards the uncommitted work.

---

## 3. Copy (exact strings)

### 3.1 Safe confirms — UNCHANGED (verbatim from `SubmoduleDialogs.tsx:51-92`)

Deinit: title `Deinitialize submodule`, body `Deinitialize "<name>"?` + note
`Clears its local config and empties the working tree. The .gitmodules entry is kept, so you can
re-initialize it later.`, confirm `Deinitialize` (`confirmVariant='primary'`).

Remove: title `Remove submodule`, body `Remove "<name>" entirely?` + note `Drops the .gitmodules
entry and the gitlink (staged for the next commit) and deletes the submodule's working tree from
disk. This cannot be undone from Bonsai.`, confirm `Remove submodule` (danger, default variant).

### 3.2 Force escalation — NEW (reuses `ConfirmDialog`, `confirmVariant='danger'`)

`<name>` renders in `<span className="mono">` in the body (title stays generic, matching every
existing dialog). Both use two body blocks: a `<div>` lead + a `.dialog-body-note`.

**Deinit — force**
- Title: `Deinitialize and discard changes?`
- Body: `"<name>" has uncommitted changes, so it wasn't deinitialized.`
- Note: `Deinitializing now permanently discards the uncommitted work inside the submodule. The
  .gitmodules entry is still kept, so you can re-initialize it later — but that work cannot be
  recovered.`
- Confirm button (danger): **`Discard changes and deinitialize`**

**Remove — force**
- Title: `Remove and discard changes?`
- Body: `"<name>" has uncommitted changes, so it wasn't removed.`
- Note: `Removing now permanently discards the uncommitted work inside the submodule and deletes its
  working tree from disk. This cannot be undone from Bonsai.`
- Confirm button (danger): **`Discard changes and remove`**

Both lead with the irreversible consequence ("Discard changes"), name the op, and are danger-styled.
Cancel is the safe, focused default (§6). No "Are you sure?"; the target `<name>` and the
consequence are named explicitly, satisfying the destructive-action rule.

### 3.3 Refusal sentence the user sees

The escalation **dialog itself** is the refusal message (`… so it wasn't deinitialized/removed.`),
so no separate refusal toast fires on the dirty path. The backend must still return a human remedy
sentence for the non-typed fallback (§5) — reuse the house shape, e.g.
`This submodule has uncommitted changes. Force to discard them.` — but the UI must branch on the
typed flag, never on this text.

---

## 4. Placement, geometry, decomposition

- **No layout change.** Everything is overlay (`.dialog-overlay` / `.dialog-card`, default 360px).
  The escalation dialog reuses the same card width and the 4/8/12/16 dialog spacing already in
  `SubmoduleDialogs.tsx`.
- **`dialogs/SubmoduleDialogs.tsx`** (currently 220 lines) — **extend**, do not add a file. Add a
  third `ConfirmDialog` block after the remove block (`:93`), driven by one new prop group
  `pendingForce: { name: string; op: 'deinit' | 'remove' } | null` + `setPendingForce` +
  `handleForce(name, op)`. Copy is selected by `op` (a tiny local `FORCE_COPY` record — keep it in
  this file, ~15 lines; the file stays well under 500). This mirrors how the two existing confirms
  already live side by side.
- **`repoWorkspace/useSubmoduleActions.ts`** (163 lines) — `handleDeinitSubmodule` /
  `handleRemoveSubmodule` gain an optional `force = false`, passed to `ipc.deinitSubmodule(repoId,
  name, force)` / `ipc.removeSubmodule(repoId, name, force)`. In `runRowOp`'s catch (`:62-66`), when
  the error is the **typed dirty refusal** AND `force` was false, call a new
  `deps.onSubmoduleDirtyRefused(name, op)` instead of pushing the generic error toast; every other
  error keeps the existing generic toast. Add `onSubmoduleDirtyRefused` to the hook's deps. File
  stays under 500.
- **`RepoWorkspace.tsx`** (oversized container) — add only state + wiring, no render body:
  a `useState` for `pendingForceSubmodule` beside `:342-343`; `onSubmoduleDirtyRefused` sets it;
  pass the trio through `WorkspaceOverlays` → `SubmoduleDialogs` (extend the prop interfaces at
  `WorkspaceOverlays.tsx:83-91` / `:247-256` and `SubmoduleDialogs.tsx:4-18`). Render stays in
  `SubmoduleDialogs`.
- **`workspaceMenus.ts:452-464`** — no change.

---

## 5. States

| State | Surface | Treatment |
|---|---|---|
| idle | row menu → safe confirm | existing |
| busy (plain) | row badge `deinitializing…` / `removing…` (`submoduleBusy`, unchanged); confirm button `disabled` via `busy` | existing |
| refused-dirty | force escalation dialog opens; **no toast** | new, §3.2 |
| busy (force) | same row badge participle; force confirm `disabled` via `busy` | reuse |
| force success | success toast `Deinitialized <name>` / `Removed <name>` | existing copy |
| force failure | generic toast `Couldn't deinitialize <name>. <sentence>` / `Couldn't remove <name>. <sentence>`, keyed `submodule:<name>` | existing runRowOp path |
| non-dirty failure | same generic toast | existing |
| long content | `<name>` in title/body: dialog card is fixed-width; name in `.mono` wraps/ellipsizes per existing `.dialog-body` rules — verify with `LONG_SUBMODULE_PATH` | reuse |

Only one dialog is ever open at a time: the safe confirm closes (its `setPending…(null)` fires) on
Confirm *before* the op runs, exactly as today, so the escalation opens onto a clear overlay.

---

## 6. Keyboard, focus, a11y

- Reuses `ConfirmDialog` (`ConfirmDialog.tsx:28-87`): `role="dialog"`, `aria-modal="true"`,
  `aria-label={title}`. Initial focus → **Cancel** (`cancelRef`, `:42-43`); **Enter activates only
  the focused button**, so a stray Enter can never trigger `Discard changes and deinitialize/remove`.
  Esc + overlay-click cancel (capture-phase `stopPropagation`, `:45-56`), and focus restores to the
  triggering context as it does for the existing confirms.
- Confirm button accessible name = its visible danger label (§3.2) — self-describing to SR users;
  no icon-only control introduced.
- Danger is carried by the **button label text + the body copy**, never by color alone (WCAG
  1.4.1). `btn-danger` already meets AA in both themes; no token pair is introduced, so no new
  contrast check is owed — the existing `--danger` treatment stands.
- Hit targets: dialog buttons already ≥24px (existing `.dialog-buttons`).
- Motion: none added; dialogs fade with the existing overlay transition (≤150ms, honours
  `prefers-reduced-motion` via existing CSS).
- **Command palette:** not added. Deinit/remove are per-row, target-scoped ops with no palette
  entry today; force is a sub-step of those and must not become independently invocable.

---

## 7. Backend / IPC coordination (architect — flagged)

Flow A needs the refusal to be **structurally distinguishable**, or the UI is forced to string-match
a libgit2 sentence (fragile, unlocalized, and the app never leaks raw libgit2 text). Requests:

1. `deinitSubmodule(repoId, name, force: boolean)` and `removeSubmodule(repoId, name, force: boolean)`
   — add the `force` param (default `false`).
2. The dirty refusal must be a **typed** `AppError` the UI can branch on — recommend
   `kind: 'submoduleDirty'` (or an additive `retriableWithForce: true` field). The UI branches on
   this, not on `message`. The `message` should still be a house remedy sentence for the §5 fallback.

If the architect cannot supply (2), fall back to gating the escalation on
`sub.status === 'modifiedWorkdir'` — but flag that this reintroduces the dead-end for
non-`modifiedWorkdir` dirtiness and is strictly worse. **Recommendation: ship (2).**

---

## 8. Harness states (`src/ipc/mock/`, `VITE_MOCK_IPC=1`)

- **modified fixture row:** ensure `fixtures/submodules.ts` seeds at least one `modifiedWorkdir`
  submodule in the default repo (drives the badge + a realistic force target).
- **dirty-refusal seam:** extend `mock/handlers/submodules.ts` — `deinitSubmodule` / `removeSubmodule`
  currently call only `failSeam` (`:143` / `:154`). Add a `?submodule=dirty` (or reuse `notEmpty`)
  branch that throws the **typed** dirty refusal (mirror `msgDirtyWorkdir`, `:36-38`) so the
  escalation dialog + force retry are verifiable in a plain browser: refuse when `force` is false,
  succeed when `force` is true.
- **empty / loading / error:** existing seams cover no-submodules, `?submodule=slow` (busy badge),
  `#fail` / `?submodule=fail` (generic error toast — non-dirty failure).
- **long content:** `LONG_SUBMODULE_PATH` (91-char) as the `<name>` in the escalation title/body.

All states are browser-verifiable; **no USER CHECKPOINT** is required for this UI.

---

## 9. ui-reference.md

No new tokens, no new geometry, no new component — the escalation reuses `ConfirmDialog` + `btn-danger`
+ `.mono` + `.dialog-body-note`. No `ui-reference.md` edit is owed by this increment.
