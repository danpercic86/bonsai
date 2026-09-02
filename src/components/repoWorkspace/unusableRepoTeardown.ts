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
 *  reasoning behind `clearReadOverlays`'s six required fields. */
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
}

/** The mirrored close helpers, in teardown order. Listed once so the loop, the
 *  error message and the test that asserts every mirror fires stay in sync. */
export const CLOSE_MIRRORS = [
  'historySearchCloseRef',
  'commitSearchCloseRef',
  'replayExitRef',
  'composerCloseRef',
  'paletteCloseRef',
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

/** Empty every repo-scoped slice and close every overlay. Order is
 *  slices → center diff overlay → read overlays → AI panel → mirrored overlays
 *  (history search, commit search, replay, composer, palette); each close helper
 *  invalidates its own in-flight request, so nothing pops back open. (Replay and
 *  the palette have nothing in flight — replay animates the entry snapshot
 *  locally and the palette issues no IPC — so dropping their state is enough.) */
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
  closeMirroredOverlays(deps);
}
