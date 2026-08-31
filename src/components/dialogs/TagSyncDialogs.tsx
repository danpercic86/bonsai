// P77 §4: the two destructive remote-tag confirm dialogs (Bonsai invariant —
// every remote-destructive op is gated behind an explicit confirm). Kept in a
// dedicated small file rather than grown into BranchTagDialogs.tsx (which owns
// the LOCAL tag-delete). Copy is verbatim from the UI contract §4.1/§4.2.
import { ConfirmDialog } from '../ConfirmDialog';

/** A remote tag delete, armed from the sidebar tag menu. */
export interface PendingDeleteRemoteTag {
  name: string;
  remote: string;
}

/** A remote force-move (push_tag force=true): shows old → new committish. */
export interface PendingForceMoveTag {
  name: string;
  remote: string;
  oldShort: string;
  newShort: string;
}

export interface TagSyncDialogsProps {
  busy: boolean;

  pendingDeleteRemoteTag: PendingDeleteRemoteTag | null;
  setPendingDeleteRemoteTag: (v: PendingDeleteRemoteTag | null) => void;
  handleDeleteRemoteTag(remote: string, name: string): void;

  pendingForceMoveTag: PendingForceMoveTag | null;
  setPendingForceMoveTag: (v: PendingForceMoveTag | null) => void;
  handleForceMoveRemoteTag(remote: string, name: string, newShort: string): void;
}

/** P77 remote-tag destructive confirms: delete-on-remote (§4.1) and
 *  force-move-on-remote (§4.2). Both `confirmVariant='danger'` (the default). */
export function TagSyncDialogs({
  busy,
  pendingDeleteRemoteTag,
  setPendingDeleteRemoteTag,
  handleDeleteRemoteTag,
  pendingForceMoveTag,
  setPendingForceMoveTag,
  handleForceMoveRemoteTag,
}: TagSyncDialogsProps) {
  const del = pendingDeleteRemoteTag;
  const move = pendingForceMoveTag;
  return (
    <>
      {/* §4.1: delete a tag ON the remote. */}
      <ConfirmDialog
        open={del !== null}
        title={`Delete tag on ${del?.remote ?? ''}?`}
        confirmLabel={`Delete on ${del?.remote ?? ''}`}
        busy={busy}
        onConfirm={() => {
          setPendingDeleteRemoteTag(null);
          if (del !== null) void handleDeleteRemoteTag(del.remote, del.name);
        }}
        onCancel={() => setPendingDeleteRemoteTag(null)}
      >
        <div>
          Delete "<span className="mono">{del?.name ?? ''}</span>" from{' '}
          <span className="mono">{del?.remote ?? ''}</span>?
        </div>
        <div className="dialog-body-detail">
          This removes the tag for everyone who uses {del?.remote ?? 'the remote'}. Anyone who has
          already fetched it keeps their copy until they prune. This cannot be undone from Bonsai.
        </div>
      </ConfirmDialog>

      {/* §4.2: force-move a tag ON the remote (push_tag force=true). */}
      <ConfirmDialog
        open={move !== null}
        title={`Force-move tag on ${move?.remote ?? ''}?`}
        confirmLabel="Force-move"
        busy={busy}
        onConfirm={() => {
          setPendingForceMoveTag(null);
          if (move !== null) void handleForceMoveRemoteTag(move.remote, move.name, move.newShort);
        }}
        onCancel={() => setPendingForceMoveTag(null)}
      >
        <div>
          Move "<span className="mono">{move?.name ?? ''}</span>" on{' '}
          <span className="mono">{move?.remote ?? ''}</span> from{' '}
          <span className="mono">{move?.oldShort ?? ''}</span> to{' '}
          <span className="mono">{move?.newShort ?? ''}</span>?
        </div>
        <div className="dialog-body-detail">
          {move?.remote ?? "The remote"}&apos;s tag currently points to {move?.oldShort ?? ''}; this
          overwrites it with your local {move?.newShort ?? ''}. Anyone who already fetched{' '}
          {move?.name ?? 'the tag'} keeps the old target until they re-fetch it by force — moving a
          shared tag is a common source of confusion. This cannot be undone from Bonsai.
        </div>
      </ConfirmDialog>
    </>
  );
}
