/** P38 follow-up: the repo-went-unusable teardown.
 *
 *  A repo can stop being usable *while it is open* — `.git` deleted, or the repo
 *  converted to bare. The other two `isUsableRepo` checks (useRepoTabs / App)
 *  only refuse to OPEN such a repo, so `RepoWorkspace` stays mounted and every
 *  overlay rendered purely off state would linger over the emptied pane. This
 *  module is the one place that performs that teardown, called from
 *  `runRefreshRound`'s `full`-scope usability check.
 *
 *  Why a deps bag with every participant REQUIRED — and what that does and does
 *  NOT buy. It buys CALL-SITE completeness: once a slice or overlay is a field
 *  here, the container cannot compile without passing it, and it cannot be
 *  dropped from the call later without a type error. It does NOT buy DISCOVERY:
 *  an overlay nobody thought to add to this interface is still silently missed —
 *  exactly how the reflog overlay, the commit-search bar and the replay overlay
 *  each stayed missing here for milestones. Adding a new state-driven overlay to
 *  this bag remains a manual step, so treat this file as the checklist: same
 *  reasoning behind `clearReadOverlays`'s six required fields.
 *
 *  Scope, stated precisely: this covers every repo-scoped surface that gates on
 *  WORKSPACE state. Two confirms deliberately sit outside it, both owned by
 *  components below this container and neither writing to local `.git`:
 *  `CommitBox`'s "replace message?" prompt (CommitBox.tsx `replaceConfirmOpen`,
 *  a draft-only decision) and the PR pane's merge / close confirms
 *  (prPanel/PrDetailContainer.tsx `showMergeDialog` / `showCloseConfirm`, which
 *  act on the remote host). Both are Esc-dismissable and are documented here so
 *  the claim is "every local-write surface", not "everything".
 *
 *  Also deliberately outside: two read-only badge caches that retain dead-repo
 *  data — `useCommitVerification`'s `cache` and `useForgeSignals`'
 *  `prByBranch`/`ciBySha`. Nothing renders them once `graph` is null and neither
 *  can drive a write, but a restore-then-refresh in the SAME mount would briefly
 *  show badges computed against the old repo (the same argument this file makes
 *  for `setCommitBrowserOpen`). */
import { clearReadOverlays } from './useReadOverlays';
import type { BlameState, HistoryState, ReflogState, Setter } from './types';

/** A close helper mirrored into a ref because its owning hook is instantiated
 *  AFTER `runRefreshRound` in the container (the `*OpenRef` pattern: the
 *  container assigns `.current` during render). `null` = never wired, which is
 *  a wiring BUG, not a no-op — see `tearDownUnusableRepo`. */
export interface CloseMirror {
  current: (() => void) | null;
}

export interface UnusableRepoTeardownDeps {
  /** The ten per-slice reset callbacks (each owned by its data hook). */
  clearStatus: () => void;
  clearGraph: () => void;
  clearBranches: () => void;
  clearStashes: () => void;
  clearSubmodules: () => void;
  clearWorktrees: () => void;
  clearRemotes: () => void;
  clearTagSync: () => void;
  clearOpState: () => void;
  clearCompare: () => void;
  /** P23d + P38 read overlays: state setters + reqId stale-guards, handed to
   *  `clearReadOverlays` (the overlay state lives in the container). */
  setBlame: Setter<BlameState | null>;
  setHistory: Setter<HistoryState | null>;
  setReflog: Setter<ReflogState | null>;
  blameReqId: { current: number };
  historyReqId: { current: number };
  reflogReqId: { current: number };
  /** P53/P57b AI output panel — bumps aiPanelReqId and nulls the panel. Not a
   *  mirror: `useAiPanel` sits above `runRefreshRound`. */
  closeAiPanel: () => void;
  /** P57c semantic-history panel (`useHistorySearch.close`). */
  historySearchCloseRef: CloseMirror;
  /** P50b commit-search bar (`useCommitSearch.close`) — rendered off
   *  `search.open`, so it survives `clearGraph()`. */
  commitSearchCloseRef: CloseMirror;
  /** Spec-007 replay overlay (`useReplayController.onExit`) — rendered off the
   *  entry SNAPSHOT state, not the live graph, so it too survives `clearGraph()`. */
  replayExitRef: CloseMirror;
  /** P3a center diff overlay (`collapseDiffSlot`): bumps `fileDiffReqId`, nulls
   *  `diffSlot` and drops the P93 `prOverlayCtx`. Not a mirror — it is declared
   *  above `runRefreshRound`. Listed EXPLICITLY even though the container's
   *  `clearStatus` happens to call it too: that call is a status-panel concern
   *  (a workdir slot must not outlive its snapshot), and the overlay's other
   *  kinds — `conflict:` / `ai-proposal:` / `pr:` — do not depend on `status` at
   *  all, so narrowing that transitive call would silently re-open this leak.
   *  Idempotent, so the double call is harmless. */
  collapseDiffSlot: () => void;
  /** P54c commit composer (`useCommitComposer.close` — NOT `escClose`, which
   *  only pops the preview layer). Refuses while an apply is in flight, which is
   *  correct — a teardown must not force-close mid-write. Residual: only the
   *  apply SUCCESS path closes the dialog and runs its own refresh round; on
   *  failure it sets the error and leaves the dialog open with no refresh, so a
   *  teardown racing a failing apply does leave it up (Esc works by then).
   *  Rendered off `composer.open` in WorkspaceOverlays, so it survives every
   *  slice clear. */
  composerCloseRef: CloseMirror;
  /** P50c command palette (`usePalette.close`). Closed even though a few of its
   *  entries are repo-independent (open a repo, switch tab): the registry is
   *  merged with the REPO-scoped actions, so an open palette over a dead repo
   *  offers ops that would run against it. `close` is a bare `setOpen(false)`
   *  with no side effects, and the hook already force-closes on tab deactivation
   *  — an involuntary close is established behaviour here, and Ctrl/Cmd-K
   *  reopens with the global entries intact. */
  paletteCloseRef: CloseMirror;
  /** P38 final batch — `useWorkspaceDialogState.resetArmedDialogs`: returns all
   *  39 armed dialog flags for this repo to their initial value and settles the
   *  parked Commit & Push promise. ONE field rather than 39: the container cannot
   *  enumerate members it does not own, so completeness is enforced inside that
   *  hook by an enumeration test (`useWorkspaceDialogState.reset.test.tsx`).
   *  Several of those flags gate DESTRUCTIVE ops (`pendingReset`,
   *  `pendingDiscardForce`, `pendingForcePush`, `pendingDeleteBranch`,
   *  `pendingDeleteRemote`, `pendingDropStash`) — leaving one armed over a dead
   *  repo offers a write against a repo that is gone. Declared above
   *  `runRefreshRound`, so a plain field, not a mirror. */
  resetArmedDialogs: () => void;
  /** P39b two-click bisect: the oid marked BAD, parked while the user picks a
   *  known-GOOD commit. Plain container state, no in-flight request. */
  setPendingBisectBad: (v: string | null) => void;
  /** P11g commit-mode DiffBrowser explicit-open flag. Not reachable today —
   *  `diffBrowserViewOf` also requires `commitDiff !== null`, which `clearGraph`
   *  nulls — but a left-armed flag means that if the user restores `.git` and
   *  refreshes in the SAME mount, the next commit selection auto-opens the
   *  browser as though they had opened it. Nothing in flight. */
  setCommitBrowserOpen: (v: boolean) => void;
  /** The open context menu (`setMenu(null)`). Same argument as the command
   *  palette: it is rendered off `menu !== null` and its entries are the
   *  REPO-scoped commit/branch/tag/stash actions, so an open menu over a dead
   *  repo offers writes against it. Passed as a closure because `closeMenu` is
   *  declared below `runRefreshRound`; `setMenu` itself is above it. */
  closeContextMenu: () => void;
  /** P20 amend mode: `amend` puts CommitBox into amend mode and `amendMessage`
   *  holds HEAD's message, fetched from the repo that just disappeared. Left
   *  armed, a restore-then-refresh in the SAME mount would amend with the stale
   *  prefill — a WRITE carrying dead-repo content. The typed draft is CommitBox's
   *  own state and survives leaving amend mode. */
  setAmend: (v: boolean) => void;
  setAmendMessage: (v: string | null) => void;
  /** P55c Ask Bonsai (`useAskBonsai.tearDownAsk`): the NL input AND the
   *  ProposedOpDialog — a resolved git op one Confirm away from dispatch. A
   *  mirror: `useAskBonsai` needs `refreshAll`, so it is instantiated BELOW
   *  `runRefreshRound`. Refuses while a confirmed op is dispatching. */
  askTeardownRef: CloseMirror;
  /** P68f bulk-AI confirm (`useBulkAiResolve.confirm.onCancel`): "Resolve all N
   *  conflicts with AI". Same shape as the ProposedOpDialog — rendered
   *  UNCONDITIONALLY by `WorkspaceDialogs` and gated purely on hook-local
   *  `pending !== null`, so clearing `conflicts` (via `clearOpState`) does NOT
   *  close it, and its Confirm starts an AI run that READS and STAGES files on a
   *  stale path snapshot in a repo that is gone. It also pins `dialogOpen` true,
   *  suppressing the workspace shortcuts until Esc. A mirror: `useBulkAiResolve`
   *  needs `aiRuns`, so it is instantiated BELOW `runRefreshRound`. `onCancel` is
   *  a bare `setPending(null)` — no parked promise, no reqId, and no run to stop
   *  (nothing starts until Confirm). A LIVE bulk run is separate state owned by
   *  `useAiRuns` and is deliberately NOT cancelled here: it is a write in
   *  flight. */
  bulkAiCancelRef: CloseMirror;
  /** P59a hook gate (`useHookGate.onHookCancel`): a commit/amend/merge BLOCKED by
   *  a git hook, parked behind the HookOutputDialog. Nothing is in flight in git
   *  terms (the hook already refused the write), so cancelling is safe, and it
   *  settles the parked promise with COMMIT_HOOK_CANCELED — the same sentinel Esc
   *  uses, so CommitBox keeps the typed message and shows no error banner.
   *  Dropping the flag without that settle would hang the awaiting submit. */
  closeHookGate: () => void;
  /** First-run hook-execution disclosure (`useHookDisclosure`): its parked
   *  promise gates the op until acknowledged; the cancel resolves it `false`, so
   *  the op cancels silently through the existing sentinel path. */
  closeHookDisclosure: () => void;
}

/** The mirrored close helpers, in teardown order. Listed once so the loop, the
 *  error message and the test that asserts every mirror fires stay in sync. */
export const CLOSE_MIRRORS = [
  'historySearchCloseRef',
  'commitSearchCloseRef',
  'replayExitRef',
  'composerCloseRef',
  'paletteCloseRef',
  'askTeardownRef',
  'bulkAiCancelRef',
] as const;

/** Close every mirrored overlay, then fail LOUDLY if any mirror was never
 *  wired. A `null` mirror means the container dropped its render-time
 *  assignment — the overlay would silently linger over the emptied pane, and no
 *  test of this module alone can see it. `runRefreshRound` catches this and
 *  surfaces it as a `Refresh failed: …` toast: visible, non-fatal. Every wired
 *  mirror runs before the throw, so an UNWIRED mirror cannot block the others.
 *  (A wired closer that itself throws does still abort the loop — the closers
 *  are all plain setState-and-bump helpers today, so that is not a live case.) */
function closeMirroredOverlays(deps: UnusableRepoTeardownDeps): void {
  const unwired: string[] = [];
  for (const name of CLOSE_MIRRORS) {
    const close = deps[name].current;
    if (close === null) {
      unwired.push(name);
      continue;
    }
    close();
  }
  if (unwired.length > 0) {
    throw new Error(
      `Unusable-repo teardown: unwired close mirror(s) ${unwired.join(', ')} — ` +
        'RepoWorkspace must assign .current during render, or the overlay lingers ' +
        'over the emptied pane.',
    );
  }
}

/** Empty every repo-scoped slice, disarm every dialog and close every overlay.
 *  Order is slices → center diff overlay → read overlays → AI panel → armed
 *  dialogs (the 39 in `useWorkspaceDialogState` + bisect, the DiffBrowser flag,
 *  the context menu, amend mode, the hook gate and the hook disclosure) →
 *  mirrored overlays (history search, commit search, replay, composer, palette,
 *  Ask Bonsai, bulk-AI confirm); each close helper invalidates its own
 *  in-flight request, so nothing pops back open. (Replay and the palette have
 *  nothing in flight — replay animates the entry snapshot locally and the palette
 *  issues no IPC — so dropping their state is enough; same for the bisect and
 *  DiffBrowser flags. The three parked promises in the workspace — Commit & Push,
 *  the hook gate, the hook disclosure — are each SETTLED by the helper that drops
 *  their flag, so nothing is left awaiting a promise that can never resolve.) */
export function tearDownUnusableRepo(deps: UnusableRepoTeardownDeps): void {
  deps.clearStatus();
  deps.clearGraph();
  deps.clearBranches();
  deps.clearStashes();
  deps.clearSubmodules();
  deps.clearWorktrees();
  deps.clearRemotes();
  deps.clearTagSync();
  deps.clearOpState();
  deps.clearCompare();
  // The center diff overlay: `clearStatus` above already collapses it today, but
  // only as a side effect of the workdir snapshot going away — see the field doc.
  deps.collapseDiffSlot();
  // All three read-overlay siblings go together — an open reflog is just as
  // stale as an open blame once the repo is unusable.
  clearReadOverlays({
    setBlame: deps.setBlame,
    setHistory: deps.setHistory,
    setReflog: deps.setReflog,
    blameReqId: deps.blameReqId,
    historyReqId: deps.historyReqId,
    reflogReqId: deps.reflogReqId,
  });
  deps.closeAiPanel();
  // Every armed confirm/prompt for this repo, including the destructive ones.
  deps.resetArmedDialogs();
  deps.setPendingBisectBad(null);
  deps.setCommitBrowserOpen(false);
  deps.closeContextMenu();
  deps.setAmend(false);
  deps.setAmendMessage(null);
  deps.closeHookGate();
  deps.closeHookDisclosure();
  closeMirroredOverlays(deps);
}
