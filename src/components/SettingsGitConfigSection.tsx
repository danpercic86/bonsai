// P40b §7.1: self-contained "Git config" Settings section. Owns its own IPC (getConfig /
// setConfig / unsetConfig) + form state so SettingsPanel stays a lean composer. Renders a Local |
// Global level toggle and the Identity sub-section (user.name / user.email); P69d split the
// collapsed Advanced sub-form out to `settings/GitConfigAdvanced.tsx` and the one curated-key
// editor to `settings/CuratedConfigControl.tsx`. Reads present the EFFECTIVE value + which level
// set it; writes target the chosen level, refetch, and surface validation errors inline.

import { useCallback, useEffect, useRef, useState } from 'react';

import { ipc } from '../ipc';
import type { ConfigLevelArg, ConfigView, CuratedConfigEntry } from '../ipc';
import { errorMessage } from '../utils/errors';
import { invalidateEffectiveIdentity } from '../hooks/useEffectiveIdentity';
import { SettingsHooksToggle } from './SettingsHooksToggle';
import { CuratedConfigControl } from './settings/CuratedConfigControl';
import { GitConfigAdvanced } from './settings/GitConfigAdvanced';

export interface SettingsGitConfigSectionProps {
  /** Open repo id (== workdir path). Null → a disabled "open a repo" note. */
  repoId: string | null;
  /** 'identity' → scroll/focus the Identity sub-section on mount (commit-error
   *  linkage). */
  initialFocus?: 'identity' | null;
}
const IDENTITY_KEYS = ['user.name', 'user.email'];
/** Merge a fresh server-side draft map onto the current one, PRESERVING the local draft for
 *  any key the user is actively editing — i.e. whose input has focus, or whose draft diverges
 *  from the freshly-loaded server value (an unsaved edit). Prevents a post-write refetch from
 *  clobbering a sibling field mid-keystroke (name→email identity flow); keys absent from the
 *  server (just-removed advanced entries) are dropped. */
function mergeDraftsPreservingEdits(
  prev: Record<string, string>,
  server: Record<string, string>,
): Record<string, string> {
  const activeId = (document.activeElement as HTMLElement | null)?.id ?? '';
  const merged: Record<string, string> = { ...server };
  for (const key of Object.keys(prev)) {
    const serverVal = server[key];
    if (serverVal === undefined) continue; // key gone on the server → drop draft
    const focused = activeId === `cfg-${key}` || activeId === `cfg-adv-${key}`;
    const dirty = prev[key] !== serverVal;
    if (focused || dirty) merged[key] = prev[key];
  }
  return merged;
}
/** A `user.*` write changes the identity every surface shows, and setConfig/unsetConfig
 *  emit no `repo-changed` — so the shared store is told explicitly (§5.1's exhaustive
 *  trigger list). Both levels count: global is effective whenever local is unset. */
function notifyIdentity(repoId: string, key: string): void {
  if (/^user\./i.test(key)) invalidateEffectiveIdentity(repoId);
}

export function SettingsGitConfigSection({ repoId, initialFocus }: SettingsGitConfigSectionProps) {
  const [level, setLevel] = useState<ConfigLevelArg>('local');
  const [view, setView] = useState<ConfigView | null>(null);
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  // Editable per-key drafts (curated + advanced), seeded from the fetched view.
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});

  const reqId = useRef(0);
  const identityRef = useRef<HTMLDivElement>(null);
  const nameInputRef = useRef<HTMLInputElement>(null);
  const focusedOnce = useRef(false);

  const load = useCallback(
    // `preserveEdits` (post-write refetch): keep the local draft for a focused/dirty field so
    // an in-flight sibling edit is not clobbered. False on mount / level change → full reset.
    async (lvl: ConfigLevelArg, preserveEdits = false) => {
      if (repoId === null) return;
      const id = ++reqId.current;
      setLoading(true);
      setLoadError(null);
      try {
        const v = await ipc.getConfig(repoId, lvl);
        if (id !== reqId.current) return;
        setView(v);
        const d: Record<string, string> = {};
        for (const c of v.curated) d[c.key] = c.targetValue ?? '';
        for (const a of v.advanced) d[a.name] = a.value;
        setDrafts((prev) => (preserveEdits ? mergeDraftsPreservingEdits(prev, d) : d));
        setFieldErrors({});
      } catch (e) {
        if (id !== reqId.current) return;
        setView(null);
        setLoadError(errorMessage(e));
      } finally {
        if (id === reqId.current) setLoading(false);
      }
    },
    [repoId],
  );

  useEffect(() => {
    void load(level);
  }, [load, level]);

  // Commit-error linkage: scroll/focus the Identity sub-section once when opened
  // with initialFocus === 'identity' and the view is ready.
  useEffect(() => {
    if (initialFocus !== 'identity' || view === null || focusedOnce.current) return;
    focusedOnce.current = true;
    identityRef.current?.scrollIntoView({ block: 'center' });
    nameInputRef.current?.focus();
  }, [initialFocus, view]);

  // Write (or unset) a single key at the current level, then refetch.
  const write = useCallback(
    async (key: string, rawValue: string, hadTarget: boolean) => {
      if (repoId === null) return;
      const value = rawValue.trim();
      setBusyKey(key);
      setFieldErrors((m) => Object.fromEntries(Object.entries(m).filter(([k]) => k !== key)));
      try {
        if (value === '') {
          if (hadTarget) await ipc.unsetConfig(repoId, level, key);
        } else {
          await ipc.setConfig(repoId, level, key, value);
        }
        await load(level, true);
        notifyIdentity(repoId, key);
      } catch (e) {
        setFieldErrors((m) => ({ ...m, [key]: errorMessage(e) }));
      } finally {
        setBusyKey(null);
      }
    },
    [repoId, level, load],
  );

  const removeKey = useCallback(
    async (key: string) => {
      if (repoId === null) return;
      setBusyKey(key);
      try {
        await ipc.unsetConfig(repoId, level, key);
        await load(level, true);
        notifyIdentity(repoId, key);
      } catch (e) {
        setFieldErrors((m) => ({ ...m, [key]: errorMessage(e) }));
      } finally {
        setBusyKey(null);
      }
    },
    [repoId, level, load],
  );

  const onDraftChange = useCallback((k: string, v: string) => setDrafts((d) => ({ ...d, [k]: v })), []);
  const onCommit = useCallback((k: string, v: string, had: boolean) => void write(k, v, had), [write]);
  const reload = useCallback(() => load(level, true), [load, level]);

  if (repoId === null) {
    return (
      <section className="settings-section">
        <h3 className="settings-section-title">Git config</h3>
        <p className="settings-section-desc">Open a repository to view and edit its Git config.</p>
      </section>
    );
  }

  const curated = view?.curated ?? [];
  const behaviourKeys = curated.filter((c) => !IDENTITY_KEYS.includes(c.key));

  const renderCurated = (entry: CuratedConfigEntry, inputRef?: React.Ref<HTMLInputElement>) => (
    <CuratedConfigControl
      key={entry.key}
      entry={entry}
      draft={drafts[entry.key] ?? ''}
      busy={busyKey === entry.key}
      error={fieldErrors[entry.key]}
      inputRef={inputRef}
      onDraftChange={onDraftChange}
      onCommit={onCommit}
    />
  );
  const nameEntry = curated.find((c) => c.key === 'user.name');
  const emailEntry = curated.find((c) => c.key === 'user.email');

  return (
    <section className="settings-section">
      <h3 className="settings-section-title">Git config</h3>
      <p className="settings-section-desc">
        Read and edit Git configuration at the repository (Local) or user-wide (Global) level.
      </p>

      {/* P59a: repo-scoped "Run git hooks" toggle (always Local). */}
      <SettingsHooksToggle repoId={repoId} />

      <div className="settings-row">
        <span className="settings-control-label">Level</span>
        <div className="settings-control-inputs">
          {(['local', 'global'] as ConfigLevelArg[]).map((lvl) => (
            <button
              key={lvl}
              type="button"
              className={`btn-secondary settings-toggle-btn${level === lvl ? ' is-active' : ''}`}
              aria-pressed={level === lvl}
              onClick={() => setLevel(lvl)}
            >
              {lvl === 'local' ? 'Local' : 'Global'}
            </button>
          ))}
        </div>
      </div>

      {loadError !== null ? (
        <p className="settings-ai-status settings-ai-status-warn" role="note">
          {loadError}
        </p>
      ) : loading && view === null ? (
        <p className="settings-ai-status">Loading config…</p>
      ) : (
        <>
          {/* --- Identity (stays here: the deep-link focus effect owns these refs) --- */}
          <div className="settings-config-group" ref={identityRef}>
            <h4 className="settings-config-subtitle">Identity</h4>
            {nameEntry !== undefined && renderCurated(nameEntry, nameInputRef)}
            {emailEntry !== undefined && renderCurated(emailEntry)}
          </div>

          <GitConfigAdvanced
            repoId={repoId}
            level={level}
            behaviourKeys={behaviourKeys}
            advanced={view?.advanced ?? []}
            drafts={drafts}
            busyKey={busyKey}
            fieldErrors={fieldErrors}
            onDraftChange={onDraftChange}
            onCommit={onCommit}
            onRemove={(key) => void removeKey(key)}
            onReload={reload}
          />
        </>
      )}
    </section>
  );
}
