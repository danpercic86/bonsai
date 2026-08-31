# Checkout — commit & branch menu structure (UI contract)

Scope: add "check out an arbitrary commit → detached HEAD" and unify it with the existing
branch checkout, across the graph commit rows, ref pills, and sidebar rows. Design-only; the
implementer (senior-dev) builds this against the referenced files. No new component is created —
this reuses the existing `ContextMenu` grouped-parent + `children` flyout idiom already used by
Reset (`workspaceMenus.ts:669`) and Rebase (`:356`). No new token, no new icon: `CheckoutIcon`
covers every entry; the word **(detached)** carries the meaning so colour/icon is never the sole
signal (ui-reference §7).

## 0. Concept correction (locked)

There is no "checkout HEAD". The new action is **checkout this commit → detached HEAD**: HEAD
points directly at the commit's oid, no branch attached. It is non-destructive and fully
reversible (check out any branch to re-attach). It is distinct from branch checkout, which moves
a *branch* into HEAD.

**Detached-option label (locked, single string everywhere): `Checkout commit (detached)`.**
The user referred to this as "Checkout HEAD", but a bare "Checkout HEAD" is ambiguous inside a
flyout where *every* child lands on the same commit — it would not distinguish the detach option
from the branch options, all of which also point HEAD at this commit. `Checkout commit (detached)`
names both the target (this commit) and the resulting state (detached), reads correctly as a
standalone top-level item AND as a flyout child, and is self-describing as its own accessible
name. This exact string is used on every surface below.

## 1. The one rule that drives every surface

For a given commit oid, look at the local branches whose tip == oid, **excluding the current
HEAD branch** (checking that out is a no-op):

- **0 branches** → a single, top-level item `Checkout commit (detached)`.
- **exactly 1 branch** → a **grouped** item whose parent click = that branch's checkout (the
  default); the flyout lists `Checkout <name>` + `Checkout commit (detached)`.
- **≥2 branches** → a **grouped, INERT-parent** item (parent label `Checkout`, no default action —
  clicking it only opens/closes the flyout). The flyout lists one `Checkout <name>` per branch (in
  snapshot order), then `Checkout commit (detached)` **last**. There is no implicit "first branch
  wins" default: with several equally-valid branches, silently picking one on a parent click is a
  footgun, so the parent forces an explicit choice.

Remote-tracking branches are handled only on the remote pill (they need tracking-branch
creation), never inflated into the commit-row branch list — see §2b.

## 2. Menu builders — exact structure

All checkout logic lives in a new shared helper in
`src/components/workspaceMenus.ts`, mirroring the `resetMenuItems` / `commitActionItems`
extraction pattern. Keep it OUT of `commitActionItems` (which is spread into `branchMenuItems` at
`:384` and would otherwise duplicate the pill's own Checkout).

```
checkoutMenuItems(oid): ContextMenuItem[]
  if head === null || head.unborn: return []          // nothing to attach onto
  gate = mutating || opActive
  localTips = branches?.local.filter(b => b.tip === oid && !b.isHead) ?? []

  detached = {                                          // reused as child or top-level
    label: 'Checkout commit (detached)',
    icon: CheckoutIcon, disabled: gate,
    onSelect: () => void handleCheckoutCommit(oid),
  }

  if localTips.length === 0:
    if head.detached && head.oid === oid: return []     // pure no-op → omit
    return [ detached ]                                 // single top-level item

  children = localTips.map(b => ({
    label: `Checkout ${b.name}`, icon: CheckoutIcon, disabled: gate,
    onSelect: () => void handleCheckoutBranch(b.name),
  }))
  children.push(detached)                               // detached option LAST

  if localTips.length === 1:
    return [{
      label: `Checkout ${localTips[0].name}`,           // single branch → parent = default
      icon: CheckoutIcon, disabled: gate,
      onSelect: () => void handleCheckoutBranch(localTips[0].name),
      children,
    }]

  // ≥2 branches → INERT parent: no onSelect. Per ContextMenu.tsx (item.onSelect
  // omitted + children present) a parent click only toggles the flyout — it runs
  // no default action. See §2c.
  return [{
    label: 'Checkout',
    icon: CheckoutIcon, disabled: gate,
    // no onSelect — inert parent, opens flyout only
    children,
  }]
```

Wire-up (checkout is the most primary navigation action → it goes **first**):
- `commitMenuItems(oid)` (`:774`): prepend `...checkoutMenuItems(oid)` **before**
  `commitActionItems(oid)`.
- `tagMenuItems(name, oid)`: prepend `...checkoutMenuItems(oid)` when an oid is present
  (graph tag pills pass one; sidebar tag rows pass null → no checkout, matching today).
- `branchMenuItems` (`:252`): **replace** the existing flat `Checkout` item (`:269`) with the
  grouped form below. Do NOT call `checkoutMenuItems` here — the pill already knows its branch.

Branch/remote pill Checkout (replaces `:269`):
```
// local branch pill
{ label: 'Checkout', icon: CheckoutIcon, disabled: gate,
  onSelect: () => void handleCheckoutBranch(name),
  children: [
    { label: `Checkout ${name}`,            onSelect: () => void handleCheckoutBranch(name) },
    { label: 'Checkout commit (detached)',  onSelect: () => void handleCheckoutCommit(tip) },
  ] }   // every child: icon CheckoutIcon, disabled: gate

// remote branch pill (kind === 'remoteBranch')
{ label: 'Checkout', icon: CheckoutIcon, disabled: gate,
  onSelect: () => void handleCheckoutRemote(name),
  children: [
    { label: `Checkout ${name}`,            onSelect: () => void handleCheckoutRemote(name) },
    { label: 'Checkout commit (detached)',  onSelect: () => void handleCheckoutCommit(tip) },
  ] }
```
A pill always concerns exactly one branch, so its parent keeps the default (= its branch
checkout); the inert-parent rule is specific to the multi-branch *commit* row where no single
branch is privileged.

## 2c. Inert parent — how it renders in the ContextMenu idiom (multi-branch commit row)

`ContextMenuItem.onSelect` is already optional and its doc comment reads: a pure-submenu parent
may omit a default action. The `activate` path in `ContextMenu.tsx` (`:170`) handles this exactly:
`if (item.onSelect !== undefined) { run; close } else if (item.children) { toggle flyout }`. So an
inert parent is realized by **omitting `onSelect` entirely** on the grouped item (NOT by setting
`disabled: true`, and NOT by a no-op `onSelect: () => {}`):

- **Do not disable it.** A disabled parent (`disabled: gate` still applies for the in-flight gate,
  see §3) would not open its flyout when gated — but a non-gated inert parent must open its flyout
  on click / `Enter` / `ArrowRight`, which the `onSelect`-omitted form gives for free.
- Clicking the parent, or pressing `Enter`/`Space`/`ArrowRight` on it, toggles/opens the flyout
  (`activate` → `setOpenIndex`). Hover-open (`HOVER_OPEN_MS`) still works.
- `aria-haspopup="menu"` + `aria-expanded` are emitted automatically because `children` is present
  (`:273`), so the inert parent is announced as a submenu, not a dead button.
- Result: the parent reads as a grouping label that reveals choices, never as a button that "does
  nothing" surprisingly — there is no default action to mis-fire.

## 2b. Per-surface result table

| Surface | Menu shown |
|---|---|
| (a) graph commit row, exactly one local branch B tips it | grouped: parent `Checkout B` (default = checkout B); flyout `Checkout B` + `Checkout commit (detached)` |
| (a2) graph commit row, ≥2 local branches tip it | grouped, **inert** parent `Checkout` (no default — opens flyout only); flyout = one `Checkout <name>` per branch in snapshot order, then `Checkout commit (detached)` **last** |
| (b) graph commit row, only a remote-tracking branch | single `Checkout commit (detached)` in the row body. The tracking-branch checkout lives on the **remote pill** (`Checkout` → `checkoutRemoteBranch`), not duplicated into the row |
| (c) graph commit row, no branch | single `Checkout commit (detached)` |
| (d) branch pill (non-current local, or remote) | grouped as §2 — parent `Checkout` (default = attached checkout), flyout branch + detached-at-tip |
| (e) current-HEAD branch pill | `branchMenuItems` returns `[]` for `isHead` → the P60a commit fallback (`:852`) runs `commitMenuItems(tip)`, which now yields a single `Checkout commit (detached)` (the branch child is filtered out as the current HEAD). Detaching from the current branch is a real, valid state change, so this is kept — not a no-op |

Sidebar branch/tag rows use the same `branchMenuItems` / `tagMenuItems` builders, so they inherit
this structure automatically (parity is guaranteed by the shared builder, per `:247`).

## 3. States

- **Default / hover / focus-visible / pressed:** inherited from `ContextMenu` — no override.
  Focus ring is the house 2px `--accent` / 1px offset on `:focus-visible` only.
- **Disabled (gated):** every checkout entry with an action (single-branch parent, all children)
  sets `disabled: gate` (`mutating || opActive`), matching Reset/Rebase. The **inert multi-branch
  parent has no action to gate** — leave its `disabled` unset so its flyout stays openable for read
  while a write is in flight; the child actions inside carry `disabled: gate` and are the ones that
  block. A gated single-branch parent still opens its flyout for read. No separate loading spinner
  on the item — the global `mutating` gate + the AI/op activity dock (ui-reference §9) already
  signal in-flight work.
- **Omitted (not "disabled"):** unborn/absent HEAD → no checkout items at all. A commit already
  the exact detached-HEAD target with no other branch → omitted (pure no-op), consistent with
  `resetMenuItems` omitting `target === head`.
- **Long content / overflow:** branch names in `Checkout <name>` truncate with the menu's
  existing ellipsis + `title`; no wrapping. Many branches at one commit → the flyout scrolls
  (existing `ContextMenu` behaviour).

## 4. Interaction & keyboard

- Right-click a commit row / pill / sidebar row opens the menu at the house anchor
  (`rect.right`, `rect.bottom + 2`). Arrow keys move between rows; `→` / `Enter` on a grouped
  parent opens the flyout; `←` / `Esc` closes it; `Esc` again closes the menu and restores focus
  to the trigger (existing `ContextMenu` focus handling — no change).
- Single-branch (and pill) parent: activating it runs the default branch checkout; no
  double-action ambiguity because the parent and its first child are the same command (Reset
  precedent).
- Multi-branch inert parent: activating it (`Enter`/`Space`/click/`→`) only opens the flyout and
  focuses its first child — it performs no checkout. The user then picks a branch or the detached
  option explicitly.
- **Command palette:** no new global command. Checkout is inherently target-scoped (which
  commit?), and Bonsai already exposes ~150 commands — adding a palette entry would need a commit
  picker with no home. The graph selection already drives context; keep checkout in the context
  menu only. (If a palette entry is later wanted, it should act on the *selected* commit and read
  `Checkout selected commit (detached)` — noted, not specced.)

## 5. Accessibility

- No new colour pair → no new contrast obligation. The detached state's persistent surface is
  the existing HEAD-detached pill (ui-reference §6: white on `#b3261e`, **6.54:1** both themes),
  whose `HEAD` word — not its hue — carries the meaning.
- Grouped parents (single-branch, pill, and inert multi-branch) all get `aria-haspopup="menu"` +
  `aria-expanded` from `ContextMenu` (unchanged) — the inert parent is announced as a submenu, not
  a dead control.
- Every item is a real menu row ≥24px hit target (ui-reference §3.1); icon is decorative, the
  visible label is the accessible name — `Checkout commit (detached)` is self-describing, so no
  extra `aria-label` needed.
- Meaning never rides on colour: branch vs detached is distinguished by the label text.

## 6. Motion

None added. Flyout open/close uses the existing `ContextMenu` transition (already ≤150ms,
transform/opacity, `prefers-reduced-motion`-aware). Nothing here touches the canvas render budget.

## 7. Microcopy (actual strings)

- Parent (single branch, commit row): `Checkout <name>`.
- Parent (pill): `Checkout`.
- Parent (multiple branches, commit row): `Checkout` (inert).
- Branch child: `Checkout <name>`.
- Detached item (child or top-level): `Checkout commit (detached)`.
- **Success toast, clean tree** (info/`success` tone): `Detached HEAD at <shortOid>. Commit or
  create a branch to keep new work.` — `<shortOid>` = first 7 chars.
- **Success toast, dirty tree auto-stashed & re-applied** (reuse the `handleCheckoutBranch`
  wording pattern, `useBranchActions.ts:63`): `Detached HEAD at <shortOid> (stashed &
  re-applied).`
- **Success toast, conflicted re-apply** (`warning`): `Detached HEAD at <shortOid>; your changes
  were carried over with conflicts and kept safe at stash@{0} — resolve them in the status
  panel.`
- **Error toast** (backend `checkoutConflict`, if the command refuses instead of auto-stashing):
  `Can't checkout <shortOid>: your local changes would be overwritten. Commit or stash them
  first.` Never surface raw libgit2 text — route through `errorMessage(e)` like the siblings.

## 8. Destructive-action UX / dirty working dir

Detached checkout is **not** destructive — no confirm dialog. Rationale: the action loses nothing
(committed history is untouched; re-attaching is one click). A blocking `ConfirmDialog` here would
break the house rule that confirms are reserved for lossy actions (ui-reference §12.7 precedent).

- **Dirty tree:** mirror `checkoutBranch` (P33 dirty-safe): the new backend command should
  auto-stash → checkout → re-apply, returning `CheckoutResult` (`{stashed, apply?}`); the UI
  reports it via the §7 dirty/conflict toasts. This keeps commit checkout consistent with branch
  checkout. **Do not** invent a separate "you have uncommitted changes" modal.
- **Detached-HEAD footgun (commits made while detached can later become unreachable):** this is a
  *later-state* risk, not a checkout-time one. It is surfaced persistently by the red `HEAD`
  detached pill (§6). A proactive "you have commits on a detached HEAD that no branch points to"
  notice-bar warning (ui-reference §10) is the correct home for that and is **out of scope here** —
  flagged to the orchestrator as a follow-up.

## 9. Handler + IPC (input to architect / senior-dev)

- **New handler** `handleCheckoutCommit(oid: string)` in
  `src/components/repoWorkspace/useBranchActions.ts`, modelled on `handleCheckoutBranch`
  (`:51`): `setMutating(true)` → `await ipc.checkoutCommit(repoId, oid)` → `refreshAll()` (HEAD
  moves) → the §7 toasts → `errorMessage(e)` on reject. Thread it through the `workspaceMenus`
  deps bundle alongside `handleCheckoutBranch` / `handleCheckoutRemote`, and add
  `handleCheckoutCommit: vi.fn()` to `src/test/workspaceMenusFixtures.ts`.
- **New IPC command (architect to confirm/spec):**
  `checkoutCommit(repoId: string, oid: string): Promise<CheckoutResult>` — detach HEAD onto
  `oid`, dirty-safe (auto-stash/re-apply), returning the existing `CheckoutResult` shape.
  Rejects `operationInProgress | checkoutConflict | git | noRepo` (mirror `checkoutBranch`).
  **This command does not exist yet — it is the blocking backend prerequisite.**

## 10. Harness fixtures (VITE_MOCK_IPC=1)

The whole flow must be exercisable in the plain-browser harness. Needed mock states/seams:

- Commit-row menu cases: a commit with (i) one local branch tip, (ii) multiple local branch tips
  at one oid (→ inert parent + flyout, detached last), (iii) only a remote-tracking branch,
  (iv) no branch, (v) the current-HEAD tip.
- `checkoutCommit` mock outcomes, keyed by URL seam: clean success (→ detached HEAD, red pill
  renders), dirty auto-stash success, conflicted re-apply (stash retained), `checkoutConflict`
  error, and the already-detached-at-same-oid case (item omitted).
- After a successful mock detach, the graph must show the `HEAD` detached pill on the target row
  (verifies §6 wiring end-to-end).

Menu structure, labels, gating, and the resulting pill are all harness-verifiable. Real
detached-HEAD scroll feel / native window behaviour is a **USER CHECKPOINT**.

## 11. Open questions for orchestrator / architect

1. **Backend command missing** — `checkoutCommit` must be added (§9). Blocking.
2. **Multiple branches at one commit — RESOLVED (user decision).** Parent is inert (label
   `Checkout`, no default action, opens flyout only); flyout lists one `Checkout <name>` per local
   branch (snapshot order) then `Checkout commit (detached)` last. This supersedes the earlier
   "first-branch-in-snapshot-order default" recommendation for the multi-branch case only. The
   single-branch and 0-branch cases are unchanged.
3. **Remote-only commit row (case b).** Row body offers detached-only; tracking checkout stays on
   the remote pill. Recommend keeping this split (no tracking-branch creation from the row menu).
   Flag if you'd rather duplicate the remote checkout into the row.
4. **Detached-HEAD unreachable-commit warning** (§8) is out of scope — recommend a follow-up
   notice-bar item.
