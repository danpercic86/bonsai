import { ConfirmDialog } from '../ConfirmDialog';
import { PromptDialog } from '../PromptDialog';
import { TagCreateDialog } from '../TagCreateDialog';
import { BranchNameSuggest } from '../BranchNameSuggest';
import type { BranchesSnapshot, BranchNameProposal } from '../../ipc';

export interface BranchTagDialogsProps {
  mutating: boolean;
  branches: BranchesSnapshot | null;

  pendingDeleteBranch: string | null;
  setPendingDeleteBranch: (v: string | null) => void;
  handleDeleteBranch(name: string): void;

  pendingRebase: { name: string; cur: string } | null;
  setPendingRebase: (v: { name: string; cur: string } | null) => void;
  handleRebaseBranch(name: string): void;

  pendingCreateBranch: { oid: string } | null;
  setPendingCreateBranch: (v: { oid: string } | null) => void;
  handleCreateBranchHere(oid: string, name: string): void;
  /** P53c: gate + grounding for the "Suggest name ✨" affordance. */
  aiEligible: boolean;
  workingDirty: boolean;
  suggestBranchName(): Promise<BranchNameProposal>;

  pendingCreateTag: { oid: string } | null;
  setPendingCreateTag: (v: { oid: string } | null) => void;
  handleCreateTag(oid: string, name: string, message: string | null): void;

  pendingDeleteTag: string | null;
  setPendingDeleteTag: (v: string | null) => void;
  handleDeleteTag(name: string): void;
}

/** Branch/tag actions: delete branch, rebase-onto, create branch, create tag,
 *  delete tag. */
export function BranchTagDialogs({
  mutating,
  branches,
  pendingDeleteBranch,
  setPendingDeleteBranch,
  handleDeleteBranch,
  pendingRebase,
  setPendingRebase,
  handleRebaseBranch,
  pendingCreateBranch,
  setPendingCreateBranch,
  handleCreateBranchHere,
  aiEligible,
  workingDirty,
  suggestBranchName,
  pendingCreateTag,
  setPendingCreateTag,
  handleCreateTag,
  pendingDeleteTag,
  setPendingDeleteTag,
  handleDeleteTag,
}: BranchTagDialogsProps) {
  return (
    <>
      <ConfirmDialog
        open={pendingDeleteBranch !== null}
        title="Delete branch"
        confirmLabel="Delete branch"
        busy={mutating}
        onConfirm={() => {
          const name = pendingDeleteBranch;
          setPendingDeleteBranch(null);
          if (name !== null) void handleDeleteBranch(name);
        }}
        onCancel={() => setPendingDeleteBranch(null)}
      >
        <div>Delete branch "<span className="mono">{pendingDeleteBranch ?? ''}</span>"?</div>
        <div className="dialog-body-note">
          This cannot be undone from Bonsai.
        </div>
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingRebase !== null}
        title="Rebase branch"
        confirmLabel="Rebase"
        busy={mutating}
        onConfirm={() => {
          const p = pendingRebase;
          setPendingRebase(null);
          if (p !== null) void handleRebaseBranch(p.name);
        }}
        onCancel={() => setPendingRebase(null)}
      >
        <div>
          Rebase "<span className="mono">{pendingRebase?.cur ?? ''}</span>" onto "
          <span className="mono">{pendingRebase?.name ?? ''}</span>"?
        </div>
        <div className="dialog-body-note">
          This rewrites the current branch&apos;s commits. Recoverable via reflog.
        </div>
      </ConfirmDialog>

      <PromptDialog
        open={pendingCreateBranch !== null}
        title="Create branch here"
        label="Branch name"
        placeholder="feature/my-branch"
        confirmLabel="Create branch"
        busy={mutating}
        validate={(v) => {
          const t = v.trim();
          if (t === '' || t.startsWith('-')) return 'Enter a valid branch name';
          if (branches?.local.some((b) => b.name === t) === true)
            return 'A branch with that name already exists';
          return null;
        }}
        onSubmit={(v) => void handleCreateBranchHere(pendingCreateBranch!.oid, v.trim())}
        onCancel={() => setPendingCreateBranch(null)}
        extraContent={(setValue) => (
          <BranchNameSuggest
            aiEligible={aiEligible}
            workingDirty={workingDirty}
            suggest={suggestBranchName}
            onPick={setValue}
          />
        )}
      />

      {/* P22: create tag at the right-clicked commit. */}
      <TagCreateDialog
        open={pendingCreateTag !== null}
        targetOid={pendingCreateTag?.oid ?? ''}
        busy={mutating}
        existingTags={branches?.tags ?? []}
        onSubmit={(name, message) => {
          const oid = pendingCreateTag?.oid ?? null;
          setPendingCreateTag(null);
          if (oid !== null) void handleCreateTag(oid, name, message);
        }}
        onCancel={() => setPendingCreateTag(null)}
      />

      {/* P22: delete tag (local only). */}
      <ConfirmDialog
        open={pendingDeleteTag !== null}
        title="Delete tag"
        confirmLabel="Delete tag"
        busy={mutating}
        onConfirm={() => {
          const name = pendingDeleteTag;
          setPendingDeleteTag(null);
          if (name !== null) void handleDeleteTag(name);
        }}
        onCancel={() => setPendingDeleteTag(null)}
      >
        <div>Delete tag "<span className="mono">{pendingDeleteTag ?? ''}</span>"?</div>
        <div className="dialog-body-note">
          Deletes the local tag only; a tag already pushed to a remote is not removed there.
        </div>
      </ConfirmDialog>
    </>
  );
}
