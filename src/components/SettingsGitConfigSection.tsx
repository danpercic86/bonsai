// P40b §7.1: self-contained "Git config" Settings section. Owns its own IPC
// (getConfig / setConfig / unsetConfig) + form state so SettingsPanel stays a
// lean composer. Renders a Local | Global level toggle, an Identity sub-section
// (user.name / user.email), the remaining curated Behaviour keys (enum/text),
// and an Advanced add/edit/remove list of arbitrary section.key = value entries.
// Reads present the EFFECTIVE value + which level set it; writes target the
// chosen level. Server-side validation errors surface inline; every write
// refetches getConfig for the current level.

import { useCallback, useEffect, useRef, useState } from 'react';

import { ipc } from '../ipc';
import type { ConfigLevelArg, ConfigLevelName, ConfigView, CuratedConfigEntry } from '../ipc';
import { errorMessage } from '../utils/errors';

export interface SettingsGitConfigSectionProps {
  /** Open repo id (== workdir path). Null → a disabled "open a repo" note. */
  repoId: string | null;
  /** 'identity' → scroll/focus the Identity sub-section on mount (commit-error
   *  linkage). */
  initialFocus?: 'identity' | null;
}

const IDENTITY_KEYS = ['user.name', 'user.email'];

const LEVEL_LABEL: Record<ConfigLevelName, string> = {
  local: 'local',
  global: 'global',
  system: 'system',
  other: 'other',
};

/** Light client-side pre-check mirroring the Rust `validate_key` (§4.5). Returns
 *  an error string, or null when the shape is acceptable. */
function validateKeyShape(key: string): string | null {
  const trimmed = key.trim();
  if (trimmed === '') return 'config key must not be empty';
  const parts = trimmed.split('.');
  if (parts.length < 2) return 'config key must be section.key';
  const section = parts[0];
  if (section === '' || !/^[A-Za-z0-9-]+$/.test(section)) return 'invalid section name';
  const variable = parts[parts.length - 1];
  if (!/^[A-Za-z][A-Za-z0-9-]*$/.test(variable)) return 'invalid key name';
  for (const sub of parts.slice(1, parts.length - 1)) {
    if (sub === '' || /\s/.test(sub)) return 'invalid subsection';
  }
  return null;
}

/** Merge a fresh server-side draft map onto the current one, PRESERVING the
 *  local draft for any key the user is actively editing — i.e. whose input has
 *  focus, or whose current draft diverges from the freshly-loaded server value
 *  (an unsaved edit). Prevents a post-write refetch from clobbering a sibling
 *  field mid-keystroke (name→email identity flow). Keys absent from the server
 *  (e.g. just-removed advanced entries) are dropped. */
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

/** Muted "inherited from <level>: <value>" hint when a key is unset at the
 *  target level but has an effective value from another level. */
function InheritedHint({ entry }: { entry: CuratedConfigEntry }) {
  if (entry.targetValue !== null) return null;
  if (entry.effectiveValue === null || entry.effectiveLevel === null) return null;
  return (
    <p className="settings-config-hint">
      inherited from {LEVEL_LABEL[entry.effectiveLevel]}: {entry.effectiveValue}
    </p>
  );
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
  // Advanced "add entry" row.
  const [addName, setAddName] = useState('');
  const [addValue, setAddValue] = useState('');
  const [addError, setAddError] = useState<string | null>(null);

  const reqId = useRef(0);
  const identityRef = useRef<HTMLDivElement>(null);
  const nameInputRef = useRef<HTMLInputElement>(null);
  const focusedOnce = useRef(false);

  const load = useCallback(
    // `preserveEdits` (post-write refetch): keep the local draft for a
    // focused/dirty field so an in-flight sibling edit is not clobbered. On
    // mount / level change it is false → full reset for the new context.
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
      setFieldErrors((m) => {
        const n = { ...m };
        delete n[key];
        return n;
      });
      try {
        if (value === '') {
          if (hadTarget) await ipc.unsetConfig(repoId, level, key);
        } else {
          await ipc.setConfig(repoId, level, key, value);
        }
        await load(level, true);
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
      } catch (e) {
        setFieldErrors((m) => ({ ...m, [key]: errorMessage(e) }));
      } finally {
        setBusyKey(null);
      }
    },
    [repoId, level, load],
  );

  const addEntry = useCallback(async () => {
    if (repoId === null) return;
    const shapeErr = validateKeyShape(addName);
    if (shapeErr !== null) {
      setAddError(shapeErr);
      return;
    }
    setBusyKey('__add__');
    setAddError(null);
    try {
      await ipc.setConfig(repoId, level, addName.trim(), addValue.trim());
      await load(level, true);
      setAddName('');
      setAddValue('');
    } catch (e) {
      setAddError(errorMessage(e));
    } finally {
      setBusyKey(null);
    }
  }, [repoId, level, addName, addValue, load]);

  if (repoId === null) {
    return (
      <section className="settings-section">
        <h3 className="settings-section-title">Git config</h3>
        <p className="settings-section-desc">Open a repository to view and edit its Git config.</p>
      </section>
    );
  }

  const curated = view?.curated ?? [];
  const findCurated = (key: string): CuratedConfigEntry | undefined =>
    curated.find((c) => c.key === key);
  const behaviourKeys = curated.filter((c) => !IDENTITY_KEYS.includes(c.key));
  const advanced = view?.advanced ?? [];

  const renderCuratedControl = (entry: CuratedConfigEntry, inputRef?: React.Ref<HTMLInputElement>) => {
    const draft = drafts[entry.key] ?? '';
    const disabled = busyKey === entry.key;
    const err = fieldErrors[entry.key];
    const hadTarget = entry.targetValue !== null;

    if (entry.kind === 'enum') {
      return (
        <div className="settings-control" key={entry.key}>
          <label className="settings-control-label" htmlFor={`cfg-${entry.key}`}>
            {entry.key}
          </label>
          <select
            id={`cfg-${entry.key}`}
            className="settings-number settings-config-select"
            value={draft}
            disabled={disabled}
            onChange={(e) => {
              setDrafts((d) => ({ ...d, [entry.key]: e.target.value }));
              void write(entry.key, e.target.value, hadTarget);
            }}
          >
            <option value="">(inherit / unset)</option>
            {entry.enumValues.map((v) => (
              <option key={v} value={v}>
                {v}
              </option>
            ))}
          </select>
          <InheritedHint entry={entry} />
          {err !== undefined && <p className="settings-config-error">{err}</p>}
        </div>
      );
    }

    const emailWarn =
      entry.key === 'user.email' && draft.trim() !== '' && !draft.includes('@')
        ? 'This does not look like an email address (missing @).'
        : null;

    return (
      <div className="settings-control" key={entry.key}>
        <label className="settings-control-label" htmlFor={`cfg-${entry.key}`}>
          {entry.key}
        </label>
        <input
          id={`cfg-${entry.key}`}
          ref={inputRef}
          className="settings-number settings-config-field"
          type="text"
          value={draft}
          disabled={disabled}
          placeholder={entry.effectiveValue ?? ''}
          onChange={(e) => setDrafts((d) => ({ ...d, [entry.key]: e.target.value }))}
          onBlur={() => {
            if ((drafts[entry.key] ?? '') !== (entry.targetValue ?? '')) {
              void write(entry.key, drafts[entry.key] ?? '', hadTarget);
            }
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              e.currentTarget.blur();
            }
          }}
        />
        <InheritedHint entry={entry} />
        {emailWarn !== null && (
          <p className="settings-config-hint settings-config-warn">{emailWarn}</p>
        )}
        {err !== undefined && <p className="settings-config-error">{err}</p>}
      </div>
    );
  };

  const nameEntry = findCurated('user.name');
  const emailEntry = findCurated('user.email');

  return (
    <section className="settings-section">
      <h3 className="settings-section-title">Git config</h3>
      <p className="settings-section-desc">
        Read and edit Git configuration at the repository (Local) or user-wide (Global) level.
      </p>

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
          {/* --- Identity --- */}
          <div className="settings-config-group" ref={identityRef}>
            <h4 className="settings-config-subtitle">Identity</h4>
            {nameEntry !== undefined && renderCuratedControl(nameEntry, nameInputRef)}
            {emailEntry !== undefined && renderCuratedControl(emailEntry)}
          </div>

          {/* --- Behaviour --- */}
          <div className="settings-config-group">
            <h4 className="settings-config-subtitle">Behaviour</h4>
            {behaviourKeys.map((entry) => renderCuratedControl(entry))}
          </div>

          {/* --- Advanced --- */}
          <div className="settings-config-group">
            <h4 className="settings-config-subtitle">Advanced</h4>
            {advanced.length === 0 && (
              <p className="settings-config-hint">
                No other keys set at the {level} level.
              </p>
            )}
            {advanced.map((entry) => {
              const disabled = busyKey === entry.name;
              const err = fieldErrors[entry.name];
              return (
                <div className="settings-config-advanced-row" key={entry.name}>
                  <span className="settings-config-advanced-name" title={entry.name}>
                    {entry.name}
                  </span>
                  <input
                    id={`cfg-adv-${entry.name}`}
                    className="settings-number settings-config-field"
                    type="text"
                    value={drafts[entry.name] ?? ''}
                    disabled={disabled}
                    onChange={(e) => setDrafts((d) => ({ ...d, [entry.name]: e.target.value }))}
                    onBlur={() => {
                      if ((drafts[entry.name] ?? '') !== entry.value) {
                        void write(entry.name, drafts[entry.name] ?? '', true);
                      }
                    }}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        e.preventDefault();
                        e.currentTarget.blur();
                      }
                    }}
                  />
                  <button
                    type="button"
                    className="btn-secondary settings-toggle-btn"
                    disabled={disabled}
                    onClick={() => void removeKey(entry.name)}
                  >
                    Remove
                  </button>
                  {err !== undefined && <p className="settings-config-error">{err}</p>}
                </div>
              );
            })}

            <div className="settings-config-advanced-row settings-config-add-row">
              <input
                className="settings-number settings-config-field"
                type="text"
                placeholder="section.key"
                value={addName}
                disabled={busyKey === '__add__'}
                onChange={(e) => setAddName(e.target.value)}
              />
              <input
                className="settings-number settings-config-field"
                type="text"
                placeholder="value"
                value={addValue}
                disabled={busyKey === '__add__'}
                onChange={(e) => setAddValue(e.target.value)}
              />
              <button
                type="button"
                className="btn-secondary settings-toggle-btn"
                disabled={busyKey === '__add__' || addName.trim() === ''}
                onClick={() => void addEntry()}
              >
                Add entry
              </button>
              {addError !== null && <p className="settings-config-error">{addError}</p>}
            </div>
          </div>
        </>
      )}
    </section>
  );
}
