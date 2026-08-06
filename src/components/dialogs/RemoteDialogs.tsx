import { ConfirmDialog } from '../ConfirmDialog';
import { PromptDialog } from '../PromptDialog';
import { RemoteEditDialog } from '../RemoteEditDialog';
import type { RemoteInfo } from '../../ipc';

export interface RemoteDialogsProps {
  mutating: boolean;
  remotes: RemoteInfo[];

  pendingDeleteRemote: string | null;
  setPendingDeleteRemote: (v: string | null) => void;
  handleDeleteRemoteTracking(name: string): void;

  pendingAddRemote: boolean;
  setPendingAddRemote: (v: boolean) => void;
  handleAddRemote(name: string, url: string): void;

  pendingEditUrl: { name: string; url: string } | null;
  setPendingEditUrl: (v: { name: string; url: string } | null) => void;
  handleSetRemoteUrl(name: string, url: string): void;

  pendingRenameRemote: { name: string } | null;
  setPendingRenameRemote: (v: { name: string } | null) => void;
  handleRenameRemote(name: string, newName: string): void;

  pendingRemoveRemote: string | null;
  setPendingRemoveRemote: (v: string | null) => void;
  handleRemoveRemote(name: string): void;
}

/** Remote management: delete remote-tracking ref, add / edit-url / rename /
 *  remove remote. */
export function RemoteDialogs({
  mutating,
  remotes,
  pendingDeleteRemote,
  setPendingDeleteRemote,
  handleDeleteRemoteTracking,
  pendingAddRemote,
  setPendingAddRemote,
  handleAddRemote,
  pendingEditUrl,
  setPendingEditUrl,
  handleSetRemoteUrl,
  pendingRenameRemote,
  setPendingRenameRemote,
  handleRenameRemote,
  pendingRemoveRemote,
  setPendingRemoveRemote,
  handleRemoveRemote,
}: RemoteDialogsProps) {
  return (
    <>
      <ConfirmDialog
        open={pendingDeleteRemote !== null}
        title="Delete remote-tracking reference"
        confirmLabel="Delete reference"
        busy={mutating}
        onConfirm={() => {
          const name = pendingDeleteRemote;
          setPendingDeleteRemote(null);
          if (name !== null) void handleDeleteRemoteTracking(name);
        }}
        onCancel={() => setPendingDeleteRemote(null)}
      >
        <div>Delete the remote-tracking reference "<span className="mono">{pendingDeleteRemote ?? ''}</span>"?</div>
        <div className="dialog-body-note">
          This removes only Bonsai's local copy of the remote branch. It does NOT delete the branch on
          the server — a future fetch may recreate it.
        </div>
      </ConfirmDialog>

      {/* P22: add a new remote (name + url both editable). */}
      <RemoteEditDialog
        open={pendingAddRemote}
        title="Add remote"
        confirmLabel="Add remote"
        busy={mutating}
        existingNames={remotes.map((r) => r.name)}
        onSubmit={(name, url) => {
          setPendingAddRemote(false);
          void handleAddRemote(name, url);
        }}
        onCancel={() => setPendingAddRemote(false)}
      />

      {/* P22: edit an existing remote's fetch URL (name read-only). */}
      <RemoteEditDialog
        open={pendingEditUrl !== null}
        title="Edit remote URL"
        confirmLabel="Save URL"
        busy={mutating}
        nameReadOnly
        initialName={pendingEditUrl?.name}
        initialUrl={pendingEditUrl?.url}
        existingNames={remotes.map((r) => r.name)}
        onSubmit={(_name, url) => {
          const target = pendingEditUrl;
          setPendingEditUrl(null);
          if (target !== null) void handleSetRemoteUrl(target.name, url);
        }}
        onCancel={() => setPendingEditUrl(null)}
      />

      {/* P22: rename a remote (single-field → reuse PromptDialog). */}
      <PromptDialog
        open={pendingRenameRemote !== null}
        title="Rename remote"
        label="New remote name"
        placeholder="origin"
        initialValue={pendingRenameRemote?.name}
        confirmLabel="Rename"
        busy={mutating}
        validate={(v) => {
          const t = v.trim();
          if (t === '') return 'Enter a remote name';
          if (/\s/.test(t)) return 'Remote name cannot contain whitespace';
          if (t !== pendingRenameRemote?.name && remotes.some((r) => r.name === t))
            return 'A remote with that name already exists';
          return null;
        }}
        onSubmit={(v) => {
          const target = pendingRenameRemote;
          setPendingRenameRemote(null);
          if (target !== null) void handleRenameRemote(target.name, v.trim());
        }}
        onCancel={() => setPendingRenameRemote(null)}
      />

      {/* P22: remove a remote (drops its tracking refs locally). */}
      <ConfirmDialog
        open={pendingRemoveRemote !== null}
        title="Remove remote"
        confirmLabel="Remove remote"
        busy={mutating}
        onConfirm={() => {
          const name = pendingRemoveRemote;
          setPendingRemoveRemote(null);
          if (name !== null) void handleRemoveRemote(name);
        }}
        onCancel={() => setPendingRemoveRemote(null)}
      >
        <div>Remove remote "<span className="mono">{pendingRemoveRemote ?? ''}</span>"?</div>
        <div className="dialog-body-note">
          Removes the remote and its remote-tracking branches from this repo. The server is not
          affected.
        </div>
      </ConfirmDialog>
    </>
  );
}
