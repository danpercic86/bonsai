import { RebasePlanEditor } from '../RebasePlanEditor';
import { WhatChangedDialog } from '../WhatChangedDialog';
import { StaleBranchesDialog } from '../StaleBranchesDialog';
import { ContextMenu } from '../ContextMenu';
import type { ContextMenuState } from '../ContextMenu';
import type {
  AiDigestRange,
  BranchInfo,
  BranchesSnapshot,
  RebaseTodoOp,
} from '../../ipc';

export interface CleanupDialogsProps {
  repoId: string;
  mutating: boolean;
  headBranch: BranchInfo | null;
  branches: BranchesSnapshot | null;

  staleCleanupOpen: boolean;
  setStaleCleanupOpen: (v: boolean) => void;
  refetchBranches(): Promise<void>;
  refetchGraph(): Promise<void>;

  whatChangedOpen: boolean;
  setWhatChangedOpen: (v: boolean) => void;
  runDigest(range: AiDigestRange, title: string): void;

  rebasePlan: {
    ontoOid: string;
    ontoLabel: string;
    initialTodos: RebaseTodoOp[];
    summaries: Record<string, string>;
  } | null;
  setRebasePlan: (
    v: {
      ontoOid: string;
      ontoLabel: string;
      initialTodos: RebaseTodoOp[];
      summaries: Record<string, string>;
    } | null,
  ) => void;
  rebasePlanError: string | null;
  setRebasePlanError: (v: string | null) => void;
  handleStartInteractiveRebase(ontoOid: string, ontoLabel: string, todos: RebaseTodoOp[]): void;

  menu: ContextMenuState | null;
  closeMenu(): void;
}

/** Remaining workspace dialogs: stale-branch cleanup, the "What changed"
 *  digest range picker, the interactive-rebase plan editor, and the graph
 *  context menu. */
export function CleanupDialogs({
  repoId,
  mutating,
  headBranch,
  branches,
  staleCleanupOpen,
  setStaleCleanupOpen,
  refetchBranches,
  refetchGraph,
  whatChangedOpen,
  setWhatChangedOpen,
  runDigest,
  rebasePlan,
  setRebasePlan,
  rebasePlanError,
  setRebasePlanError,
  handleStartInteractiveRebase,
  menu,
  closeMenu,
}: CleanupDialogsProps) {
  return (
    <>
      {/* P25d: B4 stale-branch cleanup. The nested ConfirmDialog inside lists the
          exact names before any delete; onDeleted refetches branches + graph. */}
      <StaleBranchesDialog
        open={staleCleanupOpen}
        onClose={() => setStaleCleanupOpen(false)}
        repoId={repoId}
        onDeleted={() => void Promise.all([refetchBranches(), refetchGraph()])}
      />

      {/* P28 §7: "What changed" range picker → runDigest → AiOutputPanel. */}
      <WhatChangedDialog
        open={whatChangedOpen}
        branchNames={[
          ...(branches?.local.map((b) => b.name) ?? []),
          ...(branches?.remote.map((b) => b.name) ?? []),
        ]}
        currentBranch={headBranch?.name ?? null}
        onSubmit={(range, title) => {
          setWhatChangedOpen(false);
          runDigest(range, title);
        }}
        onCancel={() => setWhatChangedOpen(false)}
      />

      {/* P23b: interactive-rebase plan editor. */}
      <RebasePlanEditor
        open={rebasePlan !== null}
        ontoLabel={rebasePlan?.ontoLabel ?? ''}
        ontoOid={rebasePlan?.ontoOid ?? ''}
        initialTodos={rebasePlan?.initialTodos ?? []}
        summaries={rebasePlan?.summaries ?? {}}
        mutating={mutating}
        error={rebasePlanError}
        onCancel={() => {
          setRebasePlan(null);
          setRebasePlanError(null);
        }}
        onStart={(todos) => {
          if (rebasePlan !== null) {
            void handleStartInteractiveRebase(rebasePlan.ontoOid, rebasePlan.ontoLabel, todos);
          }
        }}
      />

      {menu !== null && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={menu.items}
          header={menu.header}
          ariaLabel={menu.ariaLabel}
          onClose={closeMenu}
        />
      )}
    </>
  );
}
