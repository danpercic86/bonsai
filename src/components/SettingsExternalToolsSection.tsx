// P49b: Settings "External tools" section (own file, mirrors SettingsUpdatesSection).
// Two free-form command templates — terminal + editor — used when launching the
// OS terminal / editor at a repo/worktree/submodule/tab path. Empty ⇒ the backend
// auto-detects a per-OS default (VS Code family for the editor). Controlled by the
// App's UiSettings state: every edit fires `onChange` (App updates state live +
// debounces the persist, exactly like the other sections).
//
// P69g: re-skinned onto the canonical stacked row (UI §5.1). The dedicated
// "Reset to auto-detect" button is gone — one reset idiom app-wide is the row `↺`,
// which is ABSENT (not disabled) at the default. The section keeps its own props
// (§2.3 leaf boundary), so it also passes its own reset source: with no provider
// above it in its unit suite, the catalog descriptor has no values to read.

import { DEFAULT_UI_SETTINGS } from '../settings/defaults';
import { SettingsGroup } from './settings/SettingsGroup';
import { SettingsRow } from './settings/SettingsRow';
import { settingsRowHelpId } from './settings/settingsCatalog';
import type { UiSettingsPatch } from '../ipc';

export interface SettingsExternalToolsSectionProps {
  /** Current terminal template ('' ⇒ auto-detect). */
  terminalCommand: string;
  /** Current editor template ('' ⇒ auto-detect VS Code family). */
  editorCommand: string;
  /** Same debounced patch channel the other sections use (App owns the persist). */
  onChange(patch: UiSettingsPatch): void;
}

/** One labeled command-template row. Controlled by the parent value; edits and the
 *  row `↺` both fire `onChange`. */
function CommandRow({
  rowId,
  id,
  value,
  defaultValue,
  onValue,
}: {
  rowId: string;
  id: string;
  value: string;
  /** The production default for this key — the ↺ target AND the "is default" test. */
  defaultValue: string;
  onValue(next: string): void;
}) {
  return (
    <SettingsRow
      id={rowId}
      controlId={id}
      stacked
      reset={{ isDefault: value === defaultValue, onReset: () => onValue(defaultValue) }}
    >
      <input
        id={id}
        className="settings-text"
        type="text"
        spellCheck={false}
        autoComplete="off"
        value={value}
        placeholder="Auto-detect"
        title={value === '' ? undefined : value}
        aria-describedby={settingsRowHelpId(rowId)}
        onChange={(e) => onValue(e.target.value)}
      />
    </SettingsRow>
  );
}

export function SettingsExternalToolsSection({
  terminalCommand,
  editorCommand,
  onChange,
}: SettingsExternalToolsSectionProps) {
  return (
    <SettingsGroup id="general-external-tools" title="External tools">
      <CommandRow
        rowId="general.terminal-command"
        id="settings-terminal-command"
        value={terminalCommand}
        defaultValue={DEFAULT_UI_SETTINGS.terminalCommand}
        onValue={(v) => onChange({ terminalCommand: v })}
      />
      <CommandRow
        rowId="general.editor-command"
        id="settings-editor-command"
        value={editorCommand}
        defaultValue={DEFAULT_UI_SETTINGS.editorCommand}
        onValue={(v) => onChange({ editorCommand: v })}
      />
      <p className="settings-group-note">
        Use <code>{'{path}'}</code> for the folder — it is passed as a separate argument, never
        through a shell.
      </p>
    </SettingsGroup>
  );
}
