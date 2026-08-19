// P69d: the one curated-key editor (text/enum + "inherited from <level>" hint),
// extracted from SettingsGitConfigSection so the Identity sub-section (which stays in
// that file — the `configMissing` deep link focuses its `user.name` input) and the
// Advanced sub-form (`GitConfigAdvanced.tsx`) share ONE control instead of the parent
// growing a 74-line inline render function.
//
// Behaviour is verbatim: text commits on blur (only when the draft diverges from the
// target value) and Enter blurs; enum commits on change; `user.email` warns when it
// has no `@`; a field error renders under the control.
import type { Ref } from 'react';

import type { ConfigLevelName, CuratedConfigEntry } from '../../ipc';

const LEVEL_LABEL: Record<ConfigLevelName, string> = {
  local: 'local',
  global: 'global',
  system: 'system',
  other: 'other',
};

/** Muted "inherited from <level>: <value>" hint when a key is unset at the target
 *  level but has an effective value from another level. */
export function InheritedHint({ entry }: { entry: CuratedConfigEntry }) {
  if (entry.targetValue !== null) return null;
  if (entry.effectiveValue === null || entry.effectiveLevel === null) return null;
  return (
    <p className="settings-config-hint">
      inherited from {LEVEL_LABEL[entry.effectiveLevel]}: {entry.effectiveValue}
    </p>
  );
}

export interface CuratedConfigControlProps {
  entry: CuratedConfigEntry;
  /** The editable draft for this key ('' = unset at the target level). */
  draft: string;
  /** A write for this key is in flight. */
  busy: boolean;
  /** Server-side (or shape) error for this key. */
  error?: string;
  /** Focus target for the `configMissing` deep link (user.name only). */
  inputRef?: Ref<HTMLInputElement>;
  onDraftChange(key: string, value: string): void;
  /** Commit `value` for `key`; `hadTarget` decides unset-vs-set for an empty value. */
  onCommit(key: string, value: string, hadTarget: boolean): void;
}

export function CuratedConfigControl({
  entry,
  draft,
  busy,
  error,
  inputRef,
  onDraftChange,
  onCommit,
}: CuratedConfigControlProps) {
  const hadTarget = entry.targetValue !== null;

  if (entry.kind === 'enum') {
    return (
      <div className="settings-control">
        <label className="settings-control-label" htmlFor={`cfg-${entry.key}`}>
          {entry.key}
        </label>
        <select
          id={`cfg-${entry.key}`}
          className="settings-number settings-config-select"
          value={draft}
          disabled={busy}
          onChange={(e) => {
            onDraftChange(entry.key, e.target.value);
            onCommit(entry.key, e.target.value, hadTarget);
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
        {error !== undefined && <p className="settings-config-error">{error}</p>}
      </div>
    );
  }

  const emailWarn =
    entry.key === 'user.email' && draft.trim() !== '' && !draft.includes('@')
      ? 'This does not look like an email address (missing @).'
      : null;

  return (
    <div className="settings-control">
      <label className="settings-control-label" htmlFor={`cfg-${entry.key}`}>
        {entry.key}
      </label>
      <input
        id={`cfg-${entry.key}`}
        ref={inputRef}
        className="settings-number settings-config-field"
        type="text"
        value={draft}
        disabled={busy}
        placeholder={entry.effectiveValue ?? ''}
        onChange={(e) => onDraftChange(entry.key, e.target.value)}
        onBlur={() => {
          if (draft !== (entry.targetValue ?? '')) onCommit(entry.key, draft, hadTarget);
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            e.currentTarget.blur();
          }
        }}
      />
      <InheritedHint entry={entry} />
      {emailWarn !== null && <p className="settings-config-hint settings-config-warn">{emailWarn}</p>}
      {error !== undefined && <p className="settings-config-error">{error}</p>}
    </div>
  );
}
