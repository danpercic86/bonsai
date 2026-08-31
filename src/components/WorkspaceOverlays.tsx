import type { Dispatch, SetStateAction } from 'react';
import { CherrypickMessageDialog } from './CherrypickMessageDialog';
import { NonFfPullDialog } from './dialogs/NonFfPullDialog';
import { UndoDialog } from './dialogs/UndoDialog';
import { CommandPalette } from './CommandPalette';
import { PromptDialog } from './PromptDialog';
import { ProposedOpDialog } from './ProposedOpDialog';
import { ChangelogDialog } from './ChangelogDialog';
import { ComposerDialog } from './ComposerDialog';
import { SubmoduleDialogs } from './dialogs/SubmoduleDialogs';
import type { PendingForceSubmodule } from './dialogs/SubmoduleDialogs';
import type { PaletteAction } from './paletteActions';
import type { NonFfPullInfo } from './repoWorkspace/useRemoteOps';
import type { UseCommitComposer } from './repoWorkspace/useCommitComposer';
import type {
  BranchesSnapshot,
  ChangelogRange,
  FileStatus,
  ProposedOperation,
  ResetMode,
  UndoPlan,
} from '../ipc';

/**
 * Presentational grouping of RepoWorkspace's overlay dialogs — the modals opened
 * from the command palette, the toolbar "Ask/Undo" affordances, and the pill
 * menus (cherry-pick message, non-FF pull, one-click undo, command palette,
 * Ask Bonsai, proposed-op preview, release-notes range, commit composer,
 * submodule add/deinit/remove). All state and behavior are owned by the
 * RepoWorkspace container and threaded in as props; this component only renders.
 */
export interface WorkspaceOverlaysProps {
  mutating: boolean;

  // P47d: cherry-pick message dialog.
  pendingCherrypick: { oid: string; initialMessage: string; loading: boolean } | null;
  setPendingCherrypick: Dispatch<
    SetStateAction<{ oid: string; initialMessage: string; loading: boolean } | null>
  >;
  confirmCherrypick: (oid: string, message: string) => void;

  // P60b: non-fast-forward pull dialog.
  pendingNonFfPull: NonFfPullInfo | null;
  setPendingNonFfPull: Dispatch<SetStateAction<NonFfPullInfo | null>>;
  handleMergeBranch: (name: string) => void;
  handleRebaseBranch: (name: string) => void;

  // P60c: one-click undo dialog.
  pendingUndo: UndoPlan | null;
  setPendingUndo: Dispatch<SetStateAction<UndoPlan | null>>;
  handleResetBranch: (oid: string, mode: ResetMode) => void;

  // P50c: command palette.
  paletteOpen: boolean;
  paletteActions: PaletteAction[];
  onClosePalette: () => void;
  paletteRunSearch: (text: string) => void;
  paletteJumpToCommit: (prefix: string) => void;

  // P55c: Ask Bonsai NL input.
  askOpen: boolean;
  askBusy: boolean;
  runPlanOperation: (request: string) => void;
  cancelAskBonsai: () => void;

  // P55c: proposed-op preview + confirm.
  pendingProposedOp: ProposedOperation | null;
  opDispatching: boolean;
  confirmProposedOp: () => void;
  cancelProposedOp: () => void;

  // P56b: release-notes range picker.
  changelogOpen: boolean;
  branches: BranchesSnapshot | null;
  headBranch: BranchesSnapshot['local'][number] | null;
  setChangelogOpen: Dispatch<SetStateAction<boolean>>;
  runChangelog: (range: ChangelogRange, title: string) => void;

  // P54c: commit composer overlay.
  composer: UseCommitComposer;
  composerStatusByPath: Map<string, FileStatus>;

  // P60d: submodule add / deinit / remove dialogs.
  pendingAddSubmodule: boolean;
  setPendingAddSubmodule: Dispatch<SetStateAction<boolean>>;
  handleAddSubmodule: (url: string, path: string) => void;
  pendingDeinitSubmodule: string | null;
  setPendingDeinitSubmodule: Dispatch<SetStateAction<string | null>>;
  handleDeinitSubmodule: (name: string, force?: boolean) => void;
  pendingRemoveSubmodule: string | null;
  setPendingRemoveSubmodule: Dispatch<SetStateAction<string | null>>;
  handleRemoveSubmodule: (name: string, force?: boolean) => void;
  // P82: dirty force-escalation dialog state.
  pendingForceSubmodule: PendingForceSubmodule | null;
  setPendingForceSubmodule: Dispatch<SetStateAction<PendingForceSubmodule | null>>;
}

export function WorkspaceOverlays({
  mutating,
  pendingCherrypick,
  setPendingCherrypick,
  confirmCherrypick,
  pendingNonFfPull,
  setPendingNonFfPull,
  handleMergeBranch,
  handleRebaseBranch,
  pendingUndo,
  setPendingUndo,
  handleResetBranch,
  paletteOpen,
  paletteActions,
  onClosePalette,
  paletteRunSearch,
  paletteJumpToCommit,
  askOpen,
  askBusy,
  runPlanOperation,
  cancelAskBonsai,
  pendingProposedOp,
  opDispatching,
  confirmProposedOp,
  cancelProposedOp,
  changelogOpen,
  branches,
  headBranch,
  setChangelogOpen,
  runChangelog,
  composer,
  composerStatusByPath,
  pendingAddSubmodule,
  setPendingAddSubmodule,
  handleAddSubmodule,
  pendingDeinitSubmodule,
  setPendingDeinitSubmodule,
  handleDeinitSubmodule,
  pendingRemoveSubmodule,
  setPendingRemoveSubmodule,
  handleRemoveSubmodule,
  pendingForceSubmodule,
  setPendingForceSubmodule,
}: WorkspaceOverlaysProps) {
  return (
    <>
      <CherrypickMessageDialog
        open={pendingCherrypick !== null}
        oid={pendingCherrypick?.oid ?? ''}
        initialMessage={pendingCherrypick?.initialMessage ?? ''}
        loading={pendingCherrypick?.loading ?? false}
        busy={mutating}
        onConfirm={(message) => {
          const p = pendingCherrypick;
          if (p !== null) void confirmCherrypick(p.oid, message);
        }}
        onCancel={() => setPendingCherrypick(null)}
      />
      {/* P60b: non-FF pull → Merge / Rebase, each routed through the EXISTING
          merge_branch / rebase_branch handlers (conflict overlay, op-state,
          toasts). Cancel is a no-op. This dialog is the confirm gate. */}
      <NonFfPullDialog
        open={pendingNonFfPull !== null}
        branch={pendingNonFfPull?.branch ?? ''}
        upstream={pendingNonFfPull?.upstream ?? ''}
        ahead={pendingNonFfPull?.ahead ?? 0}
        behind={pendingNonFfPull?.behind ?? 0}
        busy={mutating}
        onMerge={() => {
          const p = pendingNonFfPull;
          setPendingNonFfPull(null);
          if (p !== null) void handleMergeBranch(p.upstream);
        }}
        onRebase={() => {
          const p = pendingNonFfPull;
          setPendingNonFfPull(null);
          if (p !== null) void handleRebaseBranch(p.upstream);
        }}
        onCancel={() => setPendingNonFfPull(null)}
      />
      {/* P60c: one-click undo. The plan is computed READ-ONLY by describeLastUndo;
          confirming reuses the shipped resetBranch (mixed/hard per the plan). The
          dialog blocks itself when !undoable or a hard undo hits a dirty tree. */}
      <UndoDialog
        plan={pendingUndo}
        busy={mutating}
        onConfirm={() => {
          const p = pendingUndo;
          setPendingUndo(null);
          if (
            p !== null &&
            p.undoable &&
            p.resetMode !== null &&
            !(p.requiresCleanWorktree && p.worktreeDirty)
          ) {
            void handleResetBranch(p.targetOid, p.resetMode);
          }
        }}
        onCancel={() => setPendingUndo(null)}
      />
      <CommandPalette
        open={paletteOpen}
        actions={paletteActions}
        onClose={onClosePalette}
        onRunSearch={paletteRunSearch}
        onJumpToCommit={paletteJumpToCommit}
      />
      {/* P55c: the shared "Ask Bonsai to…" NL input — opened from the palette
          action or the toolbar ✨ Ask button. Submitting runs the READ-ONLY
          planner; the proposal (if any) then opens ProposedOpDialog below. */}
      <PromptDialog
        open={askOpen}
        title="Ask Bonsai to…"
        label="Describe what you want to do in plain language"
        placeholder="e.g. undo my last merge · switch to main · stash my changes"
        confirmLabel="Ask"
        busy={askBusy}
        validate={(v) => (v.trim() === '' ? 'Type a request' : null)}
        onSubmit={(v) => runPlanOperation(v.trim())}
        onCancel={cancelAskBonsai}
      />
      {/* P55c: preview + confirm gate. NOTHING mutates until its Confirm, which
          dispatches the resolved op via safeOpDispatch (existing typed command). */}
      <ProposedOpDialog
        open={pendingProposedOp !== null}
        operation={pendingProposedOp}
        busy={opDispatching}
        onConfirm={() => void confirmProposedOp()}
        onCancel={cancelProposedOp}
      />
      {/* P56b §6: the "Release notes…" range picker — opened from the palette
          "Release notes…" action (the tag-pill menu calls runChangelog directly).
          Submitting kicks off the READ-ONLY changelog; output renders in the
          AiOutputPanel over the graph. */}
      <ChangelogDialog
        open={changelogOpen}
        refNames={[
          ...(branches?.tags ?? []),
          ...(branches?.local.map((b) => b.name) ?? []),
          ...(branches?.remote.map((b) => b.name) ?? []),
        ]}
        currentBranch={headBranch?.name ?? null}
        onSubmit={(range, title) => {
          setChangelogOpen(false);
          runChangelog(range, title);
        }}
        onCancel={() => setChangelogOpen(false)}
      />
      {composer.open && (
        <ComposerDialog composer={composer} statusByPath={composerStatusByPath} />
      )}
      {/* P60d: submodule add (url + path) / deinit / remove. add + deinit + remove
          refetch submodules + status + graph on success (see useSubmoduleActions). */}
      <SubmoduleDialogs
        mutating={mutating}
        addOpen={pendingAddSubmodule}
        setAddOpen={setPendingAddSubmodule}
        handleAddSubmodule={(url, path) => void handleAddSubmodule(url, path)}
        pendingDeinit={pendingDeinitSubmodule}
        setPendingDeinit={setPendingDeinitSubmodule}
        handleDeinitSubmodule={(name, force) => void handleDeinitSubmodule(name, force)}
        pendingRemove={pendingRemoveSubmodule}
        setPendingRemove={setPendingRemoveSubmodule}
        handleRemoveSubmodule={(name, force) => void handleRemoveSubmodule(name, force)}
        pendingForce={pendingForceSubmodule}
        setPendingForce={setPendingForceSubmodule}
      />
    </>
  );
}
