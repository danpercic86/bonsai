import { ConfirmDialog } from '../ConfirmDialog';

export interface StashDialogsProps {
  mutating: boolean;

  pendingDropStash: { index: number; oid?: string } | null;
  setPendingDropStash: (v: { index: number; oid?: string } | null) => void;
  handleDropStash(index: number, oid?: string): void;

  pendingReservedStash: { index: number; op: 'apply' | 'pop'; paths: string[]; oid?: string } | null;
  setPendingReservedStash: (
    v: { index: number; op: 'apply' | 'pop'; paths: string[]; oid?: string } | null,
  ) => void;
  handleApplyStashSkipping(index: number, oid?: string): void;
  handlePopStashSkipping(index: number, oid?: string): void;
}

/** Stash confirmations: drop a stash, and the Windows "skip reserved files"
 *  apply/pop gate. */
export function StashDialogs({
  mutating,
  pendingDropStash,
  setPendingDropStash,
  handleDropStash,
  pendingReservedStash,
  setPendingReservedStash,
  handleApplyStashSkipping,
  handlePopStashSkipping,
}: StashDialogsProps) {
  return (
    <>
      <ConfirmDialog
        open={pendingDropStash !== null}
        title="Drop stash"
        confirmLabel="Drop stash"
        busy={mutating}
        onConfirm={() => {
          const p = pendingDropStash;
          setPendingDropStash(null);
          if (p !== null) void handleDropStash(p.index, p.oid);
        }}
        onCancel={() => setPendingDropStash(null)}
      >
        <div>Drop <span className="mono">stash@{`{${pendingDropStash?.index ?? 0}}`}</span>?</div>
        <div className="dialog-body-note">
          This permanently discards the stashed changes and cannot be undone.
        </div>
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingReservedStash !== null}
        title="Skip files Windows can't restore?"
        confirmLabel="Apply the rest"
        confirmVariant="primary"
        busy={mutating}
        onConfirm={() => {
          const p = pendingReservedStash;
          setPendingReservedStash(null);
          if (p === null) return;
          if (p.op === 'pop') handlePopStashSkipping(p.index, p.oid);
          else handleApplyStashSkipping(p.index, p.oid);
        }}
        onCancel={() => setPendingReservedStash(null)}
      >
        <div>
          <span className="mono">stash@{`{${pendingReservedStash?.index ?? 0}}`}</span> contains
          files Windows can&apos;t restore (
          <span className="mono">{pendingReservedStash?.paths.join(', ') ?? ''}</span>). Apply the
          rest, skipping those files?
        </div>
        <div className="dialog-body-note">
          {pendingReservedStash?.op === 'pop'
            ? 'The stash will be kept so those files are not lost.'
            : 'The skipped files remain in the stash.'}
        </div>
      </ConfirmDialog>
    </>
  );
}
