// P44 §7.1: self-contained "Identity profiles" Settings section. Profiles are global app
// settings; CRUD is lifted to the parent via `onProfilesChange` (whole-array replace,
// persisted by App's debounced onChange). This section owns its Apply IPC; the card is
// `settings/IdentityProfileCard`.
// P69d: the "Active on this repo" match no longer reads the repo's LOCAL config. Git
// resolves identity as local-overrides-global, so a repo with no local block still commits
// with the global identity, and the old exact-match-on-local logic left the pill dark in
// the ordinary case (UI D6). It now reads the shared effective-identity store (§5.1).

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { ipc } from '../ipc';
import type { IdentityProfile } from '../ipc';
import { errorMessage } from '../utils/errors';
import { invalidateEffectiveIdentity, useEffectiveIdentity } from '../hooks/useEffectiveIdentity';
import { IdentityProfileCard } from './settings/IdentityProfileCard';

export interface SettingsProfilesSectionProps {
  /** Open repo id (== workdir path). Null → Apply disabled + "open a repo" note. */
  repoId: string | null;
  /** Current profiles (from UiSettings). */
  profiles: IdentityProfile[];
  /** Persist the WHOLE next list (replace semantics). Parent maps to
   *  `onChange({ profiles: next })`; debounced persistence is upstream. */
  onProfilesChange(next: IdentityProfile[]): void;
}

export function SettingsProfilesSection({
  repoId,
  profiles,
  onProfilesChange,
}: SettingsProfilesSectionProps) {
  const identity = useEffectiveIdentity(repoId);
  const [applyingId, setApplyingId] = useState<string | null>(null);
  const [appliedId, setAppliedId] = useState<string | null>(null);
  const [applyErrors, setApplyErrors] = useState<Record<string, string>>({});
  const appliedTimer = useRef<number | null>(null);

  // Unmount: drop the pending "Applied" flash (clearTimeout(undefined) is a no-op).
  useEffect(() => () => window.clearTimeout(appliedTimer.current ?? undefined), []);

  // At most one profile is "Active": the first whose userName + userEmail both equal the
  // repo's EFFECTIVE identity (trimmed, exact compare). A half-set, still-loading or
  // unreadable identity matches nothing.
  const activeId = useMemo<string | null>(() => {
    const name = (identity.name ?? '').trim();
    const email = (identity.email ?? '').trim();
    if (name === '' || email === '') return null;
    const match = profiles.find((p) => p.userName.trim() === name && p.userEmail.trim() === email);
    return match ? match.id : null;
  }, [identity, profiles]);

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

  const clearError = useCallback((id: string) => {
    setApplyErrors((m) => {
      const n = { ...m };
      delete n[id];
      return n;
    });
  }, []);

  const deleteProfile = useCallback(
    (id: string) => {
      onProfilesChange(profiles.filter((p) => p.id !== id));
      clearError(id);
    },
    [profiles, onProfilesChange, clearError],
  );

  // Applies the profile's CURRENT in-memory fields (not an id): App's `profiles` state
  // updates synchronously on edit while only the persist is debounced, so the live fields
  // are always current — sidestepping the edit-then-Apply staleness race that a
  // settings-read-by-id would suffer.
  const applyProfile = useCallback(
    async (profile: IdentityProfile) => {
      if (repoId === null) return;
      const { id } = profile;
      setApplyingId(id);
      clearError(id);
      try {
        await ipc.applyIdentityProfile(repoId, profile.userName, profile.userEmail, profile.signingKey);
        // The write bypasses `repo-changed` (setConfig does not emit it), so the shared
        // store is told explicitly.
        invalidateEffectiveIdentity(repoId);
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
    [repoId, clearError],
  );

  return (
    <section className="settings-section">
      <h3 className="settings-section-title">Identity profiles</h3>
      <p className="settings-section-desc">
        Named identities (e.g. Work, Personal). Apply one to write its user.name,
        user.email, and signing key to the open repository&rsquo;s Local Git config.
      </p>

      {profiles.length === 0 && (
        <p className="settings-config-hint">No profiles yet. Add one to get started.</p>
      )}

      {profiles.map((p) => (
        <IdentityProfileCard
          key={p.id}
          profile={p}
          isActive={p.id === activeId}
          applying={applyingId === p.id}
          applied={appliedId === p.id}
          noRepo={repoId === null}
          error={applyErrors[p.id]}
          onChange={(patch) => updateProfile(p.id, patch)}
          onApply={() => void applyProfile(p)}
          onDelete={() => deleteProfile(p.id)}
        />
      ))}

      <div className="settings-config-add-row">
        <button type="button" className="btn-secondary settings-toggle-btn" onClick={addProfile}>
          Add profile
        </button>
      </div>
    </section>
  );
}
