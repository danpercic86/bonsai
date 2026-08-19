// P69d: the collapsed "Advanced" sub-form of the Git-config section — the curated
// Behaviour keys plus the arbitrary `section.key = value` list and its add row —
// extracted from SettingsGitConfigSection (436 lines) so that file stays a container.
//
// The Identity sub-section deliberately did NOT move: the `configMissing` deep link
// scrolls to it and focuses its `user.name` input, and that effect stays where the
// refs live.
//
// State ownership: the parent still owns drafts, per-key busy/errors and every
// curated/advanced write, so a write and its refetch remain one flow. Only the add
// row (name/value/error/in-flight) is local — it has no reader elsewhere. Adding an
// entry calls `setConfig` then asks the parent to refetch, exactly as before.
import { useCallback, useState } from 'react';

import { ipc } from '../../ipc';
import type { ConfigEntry, ConfigLevelArg, CuratedConfigEntry } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import { CuratedConfigControl } from './CuratedConfigControl';

/** Light client-side pre-check mirroring the Rust `validate_key` (P40 §4.5). Returns
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

export interface GitConfigAdvancedProps {
  repoId: string;
  /** The level every write in here targets. */
  level: ConfigLevelArg;
  /** Curated entries that are not identity. */
  behaviourKeys: CuratedConfigEntry[];
  /** Non-curated entries set at `level`. */
  advanced: ConfigEntry[];
  drafts: Record<string, string>;
  busyKey: string | null;
  fieldErrors: Record<string, string>;
  onDraftChange(key: string, value: string): void;
  onCommit(key: string, value: string, hadTarget: boolean): void;
  onRemove(key: string): void;
  /** Refetch the current level after a successful add. */
  onReload(): Promise<void> | void;
}

export function GitConfigAdvanced({
  repoId,
  level,
  behaviourKeys,
  advanced,
  drafts,
  busyKey,
  fieldErrors,
  onDraftChange,
  onCommit,
  onRemove,
  onReload,
}: GitConfigAdvancedProps) {
  const [addName, setAddName] = useState('');
  const [addValue, setAddValue] = useState('');
  const [addError, setAddError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  const addEntry = useCallback(async () => {
    const shapeErr = validateKeyShape(addName);
    if (shapeErr !== null) {
      setAddError(shapeErr);
      return;
    }
    setAdding(true);
    setAddError(null);
    try {
      await ipc.setConfig(repoId, level, addName.trim(), addValue.trim());
      await onReload();
      setAddName('');
      setAddValue('');
    } catch (e) {
      setAddError(errorMessage(e));
    } finally {
      setAdding(false);
    }
  }, [repoId, level, addName, addValue, onReload]);

  return (
    <details className="settings-config-advanced-details">
      <summary className="settings-config-advanced-summary">Advanced</summary>

      {/* --- Behaviour --- */}
      <div className="settings-config-group">
        <h4 className="settings-config-subtitle">Behaviour</h4>
        {behaviourKeys.map((entry) => (
          <CuratedConfigControl
            key={entry.key}
            entry={entry}
            draft={drafts[entry.key] ?? ''}
            busy={busyKey === entry.key}
            error={fieldErrors[entry.key]}
            onDraftChange={onDraftChange}
            onCommit={onCommit}
          />
        ))}
      </div>

      {/* --- Custom keys --- */}
      <div className="settings-config-group">
        <h4 className="settings-config-subtitle">Custom keys</h4>
        {advanced.length === 0 && (
          <p className="settings-config-hint">No other keys set at the {level} level.</p>
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
                onChange={(e) => onDraftChange(entry.name, e.target.value)}
                onBlur={() => {
                  if ((drafts[entry.name] ?? '') !== entry.value) {
                    onCommit(entry.name, drafts[entry.name] ?? '', true);
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
                onClick={() => onRemove(entry.name)}
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
            disabled={adding}
            onChange={(e) => setAddName(e.target.value)}
          />
          <input
            className="settings-number settings-config-field"
            type="text"
            placeholder="value"
            value={addValue}
            disabled={adding}
            onChange={(e) => setAddValue(e.target.value)}
          />
          <button
            type="button"
            className="btn-secondary settings-toggle-btn"
            disabled={adding || addName.trim() === ''}
            onClick={() => void addEntry()}
          >
            Add entry
          </button>
          {addError !== null && <p className="settings-config-error">{addError}</p>}
        </div>
      </div>
    </details>
  );
}
