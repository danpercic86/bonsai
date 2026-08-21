// P44 §7.1 / P69i — the "Identities" pane body. Profiles are global app settings;
// CRUD is lifted to the parent via `onProfilesChange` (whole-array replace,
// persisted by App's debounced onChange). This section owns its Apply IPC; the
// card is `settings/IdentityProfileCard`.
//
// P69d: the "in use" pill no longer reads the repo's LOCAL config. Git resolves
// identity as local-overrides-global, so a repo with no local block still commits
// with the global identity, and the old exact-match-on-local logic left the pill
// dark in the ordinary case (UI D6). It reads the shared effective-identity store
// (§5.1) — the same one the header trigger reads, so the two cannot disagree.
//
// P69i: re-skinned onto `SettingsGroup` + `SettingsRow`, and SWITCHING moved to
// the header. This pane is the editor; `Use in this repository` stays here as the
// same action, through the same store.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { ipc } from '../ipc';
import type { IdentityProfile } from '../ipc';
import { errorMessage } from '../utils/errors';
import { invalidateEffectiveIdentity, useEffectiveIdentity } from '../hooks/useEffectiveIdentity';
import { matchProfile } from './identityCopy';
import { autoDistinctColors, nextFreeHue } from './identityProfileColor';
import { IdentityProfileCard } from './settings/IdentityProfileCard';
import { SettingsEmpty } from './settings/SettingsEmpty';
import { SettingsGroup } from './settings/SettingsGroup';
import { SettingsRow } from './settings/SettingsRow';

export interface SettingsProfilesSectionProps {
  /** Open repo id (== workdir path). Null → Apply disabled + "open a repo" note. */
  repoId: string | null;
  /** Current profiles (from UiSettings). */
  profiles: IdentityProfile[];
  /** P69i / UI §4.3 item 2: the card whose Label field takes focus on mount —
   *  the header menu's `Save “…” as an identity…` appends a draft, then names it
   *  here so the user lands ON it instead of hunting for it. */
  focusProfileId?: string | null;
  /** Persist the WHOLE next list (replace semantics). Parent maps to
   *  `onChange({ profiles: next })`; debounced persistence is upstream. */
  onProfilesChange(next: IdentityProfile[]): void;
}

export function SettingsProfilesSection({
  repoId,
  profiles,
  focusProfileId,
  onProfilesChange,
}: SettingsProfilesSectionProps) {
  const identity = useEffectiveIdentity(repoId);
  const [applyingId, setApplyingId] = useState<string | null>(null);
  const [appliedId, setAppliedId] = useState<string | null>(null);
  const [applyErrors, setApplyErrors] = useState<Record<string, string>>({});
  const appliedTimer = useRef<number | null>(null);

  // Unmount: drop the pending "Applied" flash (clearTimeout(undefined) is a no-op).
  useEffect(() => () => window.clearTimeout(appliedTimer.current ?? undefined), []);

  // At most one profile is "in use": the first whose userName + userEmail both
  // equal the repo's EFFECTIVE identity. A half-set, still-loading or unreadable
  // identity matches nothing — `matchProfile` is the single implementation, also
  // used by the header menu's `checked` rows.
  const activeId = useMemo<string | null>(
    () => matchProfile(identity, profiles)?.id ?? null,
    [identity, profiles],
  );

  // P82 (UI §6): render-time display colors (auto-distinct fallback for pre-P82
  // color-less profiles), aligned by index with `profiles`.
  const displayColors = useMemo(() => autoDistinctColors(profiles), [profiles]);

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
      // P82 (UI §6): a new profile gets the next free hue, not neutral.
      color: nextFreeHue(profiles),
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

  // Applies the profile's CURRENT in-memory fields (not an id): App's `profiles`
  // state updates synchronously on edit while only the persist is debounced, so the
  // live fields are always current — sidestepping the edit-then-Apply staleness race
  // that a settings-read-by-id would suffer.
  const applyProfile = useCallback(
    async (profile: IdentityProfile) => {
      if (repoId === null) return;
      const { id } = profile;
      setApplyingId(id);
      clearError(id);
      try {
        await ipc.applyIdentityProfile(
          repoId,
          profile.userName,
          profile.userEmail,
          profile.signingKey,
        );
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
    <SettingsGroup id="identities" title="Identities">
      {profiles.length === 0 && (
        // UI §6.2. Deliberately WITHOUT its own action button: `identities.add` is
        // a catalog row that must render in every fixture, and two buttons named
        // "Add identity" in one pane is a worse answer than one, immediately below.
        <SettingsEmpty
          title="No identities yet"
          body="Save the name and email you commit with, then switch between them from the toolbar."
        />
      )}

      {profiles.map((p, i) => (
        <IdentityProfileCard
          key={p.id}
          profile={p}
          displayColor={displayColors[i]}
          isActive={p.id === activeId}
          applying={applyingId === p.id}
          applied={appliedId === p.id}
          noRepo={repoId === null}
          autoFocusLabel={p.id === focusProfileId}
          error={applyErrors[p.id]}
          onChange={(patch) => updateProfile(p.id, patch)}
          onApply={() => void applyProfile(p)}
          onDelete={() => deleteProfile(p.id)}
        />
      ))}

      <SettingsRow id="identities.add" rowLabel="New identity">
        <button type="button" className="btn-secondary settings-toggle-btn" onClick={addProfile}>
          {'Add identity'}
        </button>
      </SettingsRow>
    </SettingsGroup>
  );
}
