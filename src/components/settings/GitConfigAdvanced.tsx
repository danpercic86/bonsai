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
import { findSettingsRow } from './settingsCatalog';
import {
  useSettingsGroupVisible,
  useSettingsRowVisible,
  useSettingsSearch,
} from './SettingsSearchContext';

/** Amendment A (AM-2): the two blocks are aggregate `'group'` rows — one catalog
 *  entry standing for a whole dynamically-populated block, stamped on a
 *  `role="group"` element whose heading IS its accessible name. The heading text
 *  must equal the catalog label byte-for-byte (British `Behaviour` included); the
 *  coverage guard enforces exactly that. */
const BLOCKS = {
  behaviour: { row: 'git-config.behaviour', title: 'Behaviour', titleId: 'git-config-behaviour-title' },
  custom: { row: 'git-config.custom-keys', title: 'Custom keys', titleId: 'git-config-custom-keys-title' },
} as const;

if (import.meta.env.DEV) {
  // AM-8 #4: these two rows are stamped OUTSIDE `SettingsRow`, so they need the
  // same catalog tripwire it applies to every other row.
  for (const block of Object.values(BLOCKS)) {
    if (findSettingsRow(block.row) === undefined) {
      console.error(
        `GitConfigAdvanced block "${block.row}" has no catalog entry — the block is unsearchable and the coverage guard will fail.`,
      );
    }
  }
}

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

  /* P69k: the two blocks are catalogued rows stamped OUTSIDE `SettingsRow`, so
     they need its self-filter too — without it a search that hit git-config
     anywhere rendered the whole Advanced form (both blocks and the add row) as
     a "result". The <details> is the "Advanced" GROUP, so it disappears when
     neither block survived, and it must be forced OPEN while a search is
     running: a hit inside a collapsed disclosure is an invisible result. */
  const searching = useSettingsSearch() !== null;
  const groupVisible = useSettingsGroupVisible('Advanced');
  const behaviourVisible = useSettingsRowVisible(BLOCKS.behaviour.row);
  const customVisible = useSettingsRowVisible(BLOCKS.custom.row);

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

  if (!groupVisible) return null;

  return (
    /* The <details> IS the "Advanced" group (UI §1.3 files both blocks under it),
       so it carries the group classes the pane's other groups carry — the summary
       is its title. It is deliberately NOT stamped with a setting id: the two
       blocks inside it are the catalogued rows. */
    <details
      className="settings-group settings-config-advanced-details"
      /* Uncontrolled while not searching, so the user's own open/closed state is
         theirs; forced open while searching so a hit is never hidden. */
      open={searching ? true : undefined}
    >
      <summary className="settings-group-title settings-config-advanced-summary">Advanced</summary>

      {/* --- Behaviour --- */}
      {behaviourVisible && (
        <section
          className="settings-config-group"
          role="group"
          aria-labelledby={BLOCKS.behaviour.titleId}
          data-setting-id={BLOCKS.behaviour.row}
        >
          <h4 className="settings-config-subtitle" id={BLOCKS.behaviour.titleId}>
            {BLOCKS.behaviour.title}
          </h4>
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
        </section>
      )}

      {/* --- Custom keys --- */}
      {customVisible && (
        <section
          className="settings-config-group"
          role="group"
          aria-labelledby={BLOCKS.custom.titleId}
          data-setting-id={BLOCKS.custom.row}
        >
          <h4 className="settings-config-subtitle" id={BLOCKS.custom.titleId}>
            {BLOCKS.custom.title}
          </h4>
          {advanced.length === 0 && (
            <p className="settings-config-hint">No other keys set at the {level} level.</p>
          )}
          {advanced.map((entry) => {
            const disabled = busyKey === entry.name;
            const err = fieldErrors[entry.name];
            return (
              <div
                className="settings-config-advanced-row"
                key={entry.name}
                data-config-key={entry.name}
              >
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
        </section>
      )}
    </details>
  );
}
