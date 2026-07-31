// P24d §8.2: the context-profile manager. Lists profiles from the store, marks
// the active one, and hosts a create/edit form with a targets editor (single-
// file assets only). Save/delete round-trip through ipc; Activate opens the
// safety-gated ProfileActivateDialog. This component only renders + calls ipc.

import { useRef, useState } from 'react';
import { ipc } from '../ipc';
import type {
  AiAssetInventory,
  ContextProfile,
  ProfileActivation,
  ProfileStore,
  ProfileTarget,
} from '../ipc';
import { usePushToast } from '../ToastContext';
import { errorMessage, isAppError } from '../utils/errors';
import { ConfirmDialog } from './ConfirmDialog';
import { ProfileActivateDialog } from './ProfileActivateDialog';

export interface ProfileManagerProps {
  repoId: string;
  store: ProfileStore;
  inventory: AiAssetInventory;
  /** After a save/delete: the updated store from the backend. */
  onStoreChange(store: ProfileStore): void;
  /** After a successful activation: refresh profiles + inventory. */
  onActivated(activation: ProfileActivation): void;
}

/** A target in the editor carries a stable `uid` (never sent to the backend) so
 *  React keys survive add/remove/reorder without focus/DOM-reuse quirks. */
interface DraftTarget extends ProfileTarget {
  uid: number;
}

interface Draft {
  name: string;
  description: string;
  model: string;
  targets: DraftTarget[];
}

export function ProfileManager({
  repoId,
  store,
  inventory,
  onStoreChange,
  onActivated,
}: ProfileManagerProps) {
  const pushToast = usePushToast();

  // Monotonic source for target `uid`s (see DraftTarget).
  const nextUidRef = useRef(0);
  const uid = (): number => (nextUidRef.current += 1);

  // Draft form state (null => no form shown). `editingName` is the profile the
  // form was seeded from, used only for the heading. NOTE: saving is an
  // upsert-by-name, so changing the name field creates a NEW profile in v1 and
  // leaves the original untouched (rename is not a first-class op here).
  const [draft, setDraft] = useState<Draft | null>(null);
  const [editingName, setEditingName] = useState<string | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const [deleteName, setDeleteName] = useState<string | null>(null);
  const [deleteBusy, setDeleteBusy] = useState(false);

  const [activateName, setActivateName] = useState<string | null>(null);

  // Single-file, managed descriptors are the only valid targets (OPEN #4).
  const singleFileAssets = inventory.assets.filter(
    (a) => a.kind === 'singleFile' && a.managed,
  );

  const startCreate = (): void => {
    setDraft({ name: '', description: '', model: '', targets: [] });
    setEditingName(null);
    setFormError(null);
  };

  const startEdit = (profile: ContextProfile): void => {
    setDraft({
      name: profile.name,
      description: profile.description ?? '',
      model: profile.model ?? '',
      targets: profile.targets.map((t) => ({ ...t, uid: uid() })),
    });
    setEditingName(profile.name);
    setFormError(null);
  };

  const cancelForm = (): void => {
    setDraft(null);
    setEditingName(null);
    setFormError(null);
  };

  const patchDraft = (patch: Partial<Draft>): void => {
    setDraft((cur) => (cur === null ? cur : { ...cur, ...patch }));
  };

  const patchTarget = (target: DraftTarget, patch: Partial<ProfileTarget>): void => {
    setDraft((cur) => {
      if (cur === null) return cur;
      const targets = cur.targets.map((t) => (t.uid === target.uid ? { ...t, ...patch } : t));
      return { ...cur, targets };
    });
  };

  const addTarget = (): void => {
    const firstId = singleFileAssets[0]?.id ?? '';
    setDraft((cur) =>
      cur === null
        ? cur
        : { ...cur, targets: [...cur.targets, { uid: uid(), assetId: firstId, content: '' }] },
    );
  };

  const removeTarget = (target: DraftTarget): void => {
    setDraft((cur) =>
      cur === null ? cur : { ...cur, targets: cur.targets.filter((t) => t.uid !== target.uid) },
    );
  };

  const loadFromCurrent = async (target: DraftTarget): Promise<void> => {
    const asset = inventory.assets.find((a) => a.id === target.assetId);
    if (asset === undefined) return;
    try {
      const content = await ipc.readAiAsset(repoId, asset.path);
      patchTarget(target, { content: content.content ?? '' });
      if (!content.exists) {
        pushToast('info', `${asset.path} does not exist yet — starting from empty`);
      }
    } catch (e) {
      pushToast('error', errorMessage(e));
    }
  };

  const save = async (): Promise<void> => {
    if (draft === null) return;
    setSaving(true);
    setFormError(null);
    // Trim optional fields to null so the store stays tidy (§5.1 skips None).
    const profile: ContextProfile = {
      name: draft.name.trim(),
      description: draft.description.trim() ? draft.description.trim() : null,
      model: draft.model.trim() ? draft.model.trim() : null,
      // Strip the editor-only `uid` before sending to the backend.
      targets: draft.targets.map((t) => ({ assetId: t.assetId, content: t.content })),
    };
    try {
      const updated = await ipc.saveProfile(repoId, profile);
      onStoreChange(updated);
      pushToast('success', `Saved profile '${profile.name}'`);
      cancelForm();
    } catch (e) {
      // invalidName surfaces inline in the form (§8.4); other errors toast.
      if (isAppError(e) && e.kind === 'invalidName') {
        setFormError(e.message);
      } else {
        pushToast('error', errorMessage(e));
      }
    } finally {
      setSaving(false);
    }
  };

  const confirmDelete = async (): Promise<void> => {
    if (deleteName === null) return;
    setDeleteBusy(true);
    try {
      const updated = await ipc.deleteProfile(repoId, deleteName);
      onStoreChange(updated);
      pushToast('success', `Deleted profile '${deleteName}'`);
      if (editingName === deleteName) cancelForm();
      setDeleteName(null);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setDeleteBusy(false);
    }
  };

  return (
    <section className="settings-section">
      <div className="asset-section-head">
        <h3 className="settings-section-title">Context profiles</h3>
        {draft === null && (
          <button type="button" className="btn-secondary settings-toggle-btn" onClick={startCreate}>
            New profile
          </button>
        )}
      </div>
      <p className="settings-section-desc">
        Profiles live in <span className="mono">.bonsai/profiles.json</span> — commit it to share.
      </p>

      {store.profiles.length === 0 && draft === null ? (
        <p className="settings-ai-status">No profiles yet. Create one to switch instruction sets.</p>
      ) : (
        <ul className="asset-list">
          {store.profiles.map((profile) => (
            <li className="asset-row" key={profile.name}>
              <div className="asset-row-main">
                <div className="asset-row-head">
                  <span className="asset-row-label">{profile.name}</span>
                  {store.activeProfile === profile.name && (
                    <span className="asset-chip asset-chip-active">active</span>
                  )}
                  {profile.model != null && profile.model !== '' && (
                    <span className="asset-chip asset-chip-muted">{profile.model}</span>
                  )}
                </div>
                {profile.description != null && profile.description !== '' && (
                  <span className="asset-row-path">{profile.description}</span>
                )}
                <span className="asset-row-path">
                  {profile.targets.length} target{profile.targets.length === 1 ? '' : 's'}
                </span>
              </div>
              <div className="asset-row-actions">
                <button
                  type="button"
                  className="btn-secondary settings-toggle-btn"
                  onClick={() => setActivateName(profile.name)}
                >
                  Activate
                </button>
                <button
                  type="button"
                  className="btn-secondary settings-toggle-btn"
                  onClick={() => startEdit(profile)}
                >
                  Edit
                </button>
                <button
                  type="button"
                  className="btn-secondary settings-toggle-btn"
                  onClick={() => setDeleteName(profile.name)}
                >
                  Delete
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {draft !== null && (
        <div className="asset-form">
          <h4 className="asset-form-title">
            {editingName === null ? 'New profile' : `Edit “${editingName}”`}
          </h4>

          <label className="dialog-label">
            Name
            <input
              type="text"
              className="dialog-input"
              value={draft.name}
              placeholder="opus-rich"
              onChange={(e) => patchDraft({ name: e.target.value })}
            />
          </label>

          <label className="dialog-label">
            Description (optional)
            <input
              type="text"
              className="dialog-input"
              value={draft.description ?? ''}
              onChange={(e) => patchDraft({ description: e.target.value })}
            />
          </label>

          <label className="dialog-label">
            Model label (optional)
            <input
              type="text"
              className="dialog-input"
              value={draft.model ?? ''}
              placeholder="opus"
              onChange={(e) => patchDraft({ model: e.target.value })}
            />
          </label>

          <div className="asset-targets-head">
            <span className="settings-control-label">Targets</span>
            <button
              type="button"
              className="btn-secondary settings-toggle-btn"
              disabled={singleFileAssets.length === 0}
              onClick={addTarget}
            >
              Add target
            </button>
          </div>

          {draft.targets.length === 0 ? (
            <p className="settings-section-desc">No targets. Add one to write a file on activation.</p>
          ) : (
            draft.targets.map((target) => (
              <div className="asset-target" key={target.uid}>
                <div className="asset-target-head">
                  <select
                    className="dialog-input asset-target-select"
                    value={target.assetId}
                    onChange={(e) => patchTarget(target, { assetId: e.target.value })}
                  >
                    {singleFileAssets.map((a) => (
                      <option key={a.id} value={a.id}>
                        {a.label} ({a.path})
                      </option>
                    ))}
                  </select>
                  <button
                    type="button"
                    className="btn-secondary settings-toggle-btn"
                    onClick={() => void loadFromCurrent(target)}
                  >
                    Load from current file
                  </button>
                  <button
                    type="button"
                    className="btn-secondary settings-toggle-btn"
                    onClick={() => removeTarget(target)}
                  >
                    Remove
                  </button>
                </div>
                <textarea
                  className="dialog-input dialog-textarea asset-target-content"
                  rows={6}
                  value={target.content}
                  placeholder="Instruction file content…"
                  onChange={(e) => patchTarget(target, { content: e.target.value })}
                />
              </div>
            ))
          )}

          {formError !== null && <p className="dialog-error">{formError}</p>}

          <div className="dialog-buttons">
            <button type="button" className="btn-secondary" disabled={saving} onClick={cancelForm}>
              Cancel
            </button>
            <button
              type="button"
              className="btn-primary"
              disabled={saving}
              onClick={() => void save()}
            >
              {saving ? 'Saving…' : 'Save profile'}
            </button>
          </div>
        </div>
      )}

      <ConfirmDialog
        open={deleteName !== null}
        title="Delete profile?"
        confirmLabel="Delete"
        busy={deleteBusy}
        onConfirm={() => void confirmDelete()}
        onCancel={() => setDeleteName(null)}
      >
        <div>
          Delete profile <span className="mono">{deleteName}</span>? This removes it from the store
          but does not change any instruction files already written.
        </div>
      </ConfirmDialog>

      <ProfileActivateDialog
        open={activateName !== null}
        repoId={repoId}
        name={activateName}
        onClose={() => setActivateName(null)}
        onActivated={onActivated}
      />
    </section>
  );
}
