// P44 §7.1: self-contained "Identity profiles" Settings section. Named identity
// profiles are global app settings; CRUD is lifted to the parent via
// `onProfilesChange` (whole-array replace, persisted by App's debounced
// onChange). This section owns its own Apply IPC (`applyIdentityProfile`) and
// the read-only "Active" match indicator fetch (`getConfig`, Local) — mirroring
// how SettingsGitConfigSection owns its own config IPC.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { ipc } from '../ipc';
import type { IdentityProfile } from '../ipc';
import { errorMessage } from '../utils/errors';

export interface SettingsProfilesSectionProps {
  /** Open repo id (== workdir path). Null → Apply disabled + "open a repo" note. */
  repoId: string | null;
  /** Current profiles (from UiSettings). */
  profiles: IdentityProfile[];
  /** Persist the WHOLE next list (replace semantics). Parent maps to
   *  `onChange({ profiles: next })`; debounced persistence lives upstream. */
  onProfilesChange(next: IdentityProfile[]): void;
}

/** The repo's Local `user.name`/`user.email` target values (for the match
 *  indicator). `null` while unfetched or on fetch failure. */
interface LocalIdentity {
  name: string | null;
  email: string | null;
}

export function SettingsProfilesSection({
  repoId,
  profiles,
  onProfilesChange,
}: SettingsProfilesSectionProps) {
  const [localIdentity, setLocalIdentity] = useState<LocalIdentity | null>(null);
  const [applyingId, setApplyingId] = useState<string | null>(null);
  const [appliedId, setAppliedId] = useState<string | null>(null);
  const [applyErrors, setApplyErrors] = useState<Record<string, string>>({});
  const appliedTimer = useRef<number | null>(null);

  // Best-effort read of the repo's Local identity for the "Active" badge. Runs
  // on mount / repo change and after each successful apply. Failures are
  // swallowed (no error surface, no badge).
  const fetchLocalIdentity = useCallback(async () => {
    if (repoId === null) {
      setLocalIdentity(null);
      return;
    }
    try {
      const view = await ipc.getConfig(repoId, 'local');
      const name = view.curated.find((c) => c.key === 'user.name')?.targetValue ?? null;
      const email = view.curated.find((c) => c.key === 'user.email')?.targetValue ?? null;
      setLocalIdentity({ name, email });
    } catch {
      setLocalIdentity(null);
    }
  }, [repoId]);

  useEffect(() => {
    void fetchLocalIdentity();
  }, [fetchLocalIdentity]);

  useEffect(
    () => () => {
      if (appliedTimer.current !== null) window.clearTimeout(appliedTimer.current);
    },
    [],
  );

  // At most one profile is "Active": the first whose userName + userEmail both
  // equal the repo's Local target identity (trimmed, exact compare). An empty
  // Local identity matches nothing.
  const activeId = useMemo<string | null>(() => {
    if (localIdentity === null) return null;
    const ln = (localIdentity.name ?? '').trim();
    const le = (localIdentity.email ?? '').trim();
    if (ln === '' || le === '') return null;
    const match = profiles.find((p) => p.userName.trim() === ln && p.userEmail.trim() === le);
    return match ? match.id : null;
  }, [localIdentity, profiles]);

  const updateProfile = useCallback(
    (id: string, patch: Partial<IdentityProfile>) => {
      onProfilesChange(profiles.map((p) => (p.id === id ? { ...p, ...patch } : p)));
    },
    [profiles, onProfilesChange],
  );

  const addProfile = useCallback(() => {
    const next: IdentityProfile = {
      id: crypto.randomUUID(),
      label: '',
      userName: '',
      userEmail: '',
      signingKey: null,
    };
    onProfilesChange([...profiles, next]);
  }, [profiles, onProfilesChange]);

  const deleteProfile = useCallback(
    (id: string) => {
      onProfilesChange(profiles.filter((p) => p.id !== id));
      setApplyErrors((m) => {
        const n = { ...m };
        delete n[id];
        return n;
      });
    },
    [profiles, onProfilesChange],
  );

  // Applies the profile's CURRENT in-memory fields (not an id): App's `profiles`
  // state updates synchronously on edit while only the persist is debounced, so
  // the live fields are always current — this sidesteps the edit-then-Apply
  // staleness race a settings-read-by-id would suffer.
  const applyProfile = useCallback(
    async (profile: IdentityProfile) => {
      if (repoId === null) return;
      const { id } = profile;
      setApplyingId(id);
      setApplyErrors((m) => {
        const n = { ...m };
        delete n[id];
        return n;
      });
      try {
        await ipc.applyIdentityProfile(
          repoId,
          profile.userName,
          profile.userEmail,
          profile.signingKey,
        );
        await fetchLocalIdentity();
        setAppliedId(id);
        if (appliedTimer.current !== null) window.clearTimeout(appliedTimer.current);
        appliedTimer.current = window.setTimeout(() => {
          setAppliedId((cur) => (cur === id ? null : cur));
        }, 2500);
      } catch (e) {
        setApplyErrors((m) => ({ ...m, [id]: errorMessage(e) }));
      } finally {
        setApplyingId(null);
      }
    },
    [repoId, fetchLocalIdentity],
  );

  return (
    <section className="settings-section">
      <h3 className="settings-section-title">Identity profiles</h3>
      <p className="settings-section-desc">
        Named identities (e.g. Work, Personal). Apply one to write its
        user.name, user.email, and signing key to the open repository&rsquo;s Local Git config.
      </p>

      {profiles.length === 0 && (
        <p className="settings-config-hint">No profiles yet. Add one to get started.</p>
      )}

      {profiles.map((p) => {
        const labelHint = p.label.trim() === '' ? 'Name this profile' : null;
        const emailWarn =
          p.userEmail.trim() !== '' && !p.userEmail.includes('@')
            ? 'This does not look like an email address (missing @).'
            : null;
        const isActive = p.id === activeId;
        const applying = applyingId === p.id;
        const applied = appliedId === p.id;
        const err = applyErrors[p.id];

        return (
          <div className="settings-config-group settings-profile" key={p.id}>
            <div className="settings-profile-head">
              <span className="settings-config-subtitle">{p.label.trim() || 'Untitled profile'}</span>
              {isActive && <span className="settings-profile-badge">Active on this repo</span>}
            </div>

            <div className="settings-control">
              <label className="settings-control-label" htmlFor={`profile-label-${p.id}`}>
                Label
              </label>
              <input
                id={`profile-label-${p.id}`}
                className="settings-number settings-config-field"
                type="text"
                value={p.label}
                placeholder="Work"
                onChange={(e) => updateProfile(p.id, { label: e.target.value })}
              />
              {labelHint !== null && <p className="settings-config-hint">{labelHint}</p>}
            </div>

            <div className="settings-control">
              <label className="settings-control-label" htmlFor={`profile-name-${p.id}`}>
                user.name
              </label>
              <input
                id={`profile-name-${p.id}`}
                className="settings-number settings-config-field"
                type="text"
                value={p.userName}
                placeholder="Ada Lovelace"
                onChange={(e) => updateProfile(p.id, { userName: e.target.value })}
              />
            </div>

            <div className="settings-control">
              <label className="settings-control-label" htmlFor={`profile-email-${p.id}`}>
                user.email
              </label>
              <input
                id={`profile-email-${p.id}`}
                className="settings-number settings-config-field"
                type="text"
                value={p.userEmail}
                placeholder="ada@example.com"
                onChange={(e) => updateProfile(p.id, { userEmail: e.target.value })}
              />
              {emailWarn !== null && (
                <p className="settings-config-hint settings-config-warn">{emailWarn}</p>
              )}
            </div>

            <div className="settings-control">
              <label className="settings-control-label" htmlFor={`profile-key-${p.id}`}>
                signing key
              </label>
              <input
                id={`profile-key-${p.id}`}
                className="settings-number settings-config-field"
                type="text"
                value={p.signingKey ?? ''}
                placeholder="(optional)"
                onChange={(e) =>
                  updateProfile(p.id, {
                    signingKey: e.target.value === '' ? null : e.target.value,
                  })
                }
              />
              <p className="settings-config-hint">
                If left empty, an existing repo signing key is kept untouched.
              </p>
            </div>

            <div className="settings-profile-actions">
              <button
                type="button"
                className="btn-secondary settings-toggle-btn"
                disabled={repoId === null || applying}
                onClick={() => void applyProfile(p)}
              >
                {applying ? 'Applying…' : 'Apply to current repo'}
              </button>
              <button
                type="button"
                className="btn-secondary settings-toggle-btn"
                disabled={applying}
                onClick={() => deleteProfile(p.id)}
              >
                Delete
              </button>
              {applied && <span className="settings-profile-applied">Applied</span>}
            </div>

            {repoId === null && (
              <p className="settings-config-hint">Open a repository to apply a profile.</p>
            )}
            {err !== undefined && <p className="settings-config-error">{err}</p>}
          </div>
        );
      })}

      <div className="settings-config-add-row">
        <button type="button" className="btn-secondary settings-toggle-btn" onClick={addProfile}>
          Add profile
        </button>
      </div>
    </section>
  );
}
