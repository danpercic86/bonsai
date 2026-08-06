// P49b: Settings "External tools" section (own file, mirrors SettingsUpdatesSection).
// Two free-form command templates — terminal + editor — used when launching the
// OS terminal / editor at a repo/worktree/submodule/tab path. Empty ⇒ the backend
// auto-detects a per-OS default (VS Code family for the editor). Controlled by the
// App's UiSettings state: every edit fires `onChange` (App updates state live +
// debounces the persist, exactly like the other sections).

import type { UiSettingsPatch } from '../ipc';

export interface SettingsExternalToolsSectionProps {
  /** Current terminal template ('' ⇒ auto-detect). */
  terminalCommand: string;
  /** Current editor template ('' ⇒ auto-detect VS Code family). */
  editorCommand: string;
  /** Same debounced patch channel the other sections use (App owns the persist). */
  onChange(patch: UiSettingsPatch): void;
}

/** One labeled command-template input + "Reset to auto-detect". Controlled by the
 *  parent value; edits and the reset both fire `onChange`. */
function CommandInput({
  id,
  label,
  value,
  placeholder,
  onValue,
  onReset,
}: {
  id: string;
  label: string;
  value: string;
  placeholder: string;
  onValue(next: string): void;
  onReset(): void;
}) {
  return (
    <div className="settings-control">
      <label className="settings-control-label" htmlFor={id}>
        {label}
      </label>
      <div className="settings-control-inputs">
        <input
          id={id}
          className="settings-number settings-config-field"
          type="text"
          spellCheck={false}
          autoComplete="off"
          value={value}
          placeholder={placeholder}
          onChange={(e) => onValue(e.target.value)}
        />
        <button
          type="button"
          className="btn-secondary settings-toggle-btn"
          disabled={value === ''}
          onClick={onReset}
          title="Clear the template and use the per-OS default"
        >
          Reset to auto-detect
        </button>
      </div>
    </div>
  );
}

export function SettingsExternalToolsSection({
  terminalCommand,
  editorCommand,
  onChange,
}: SettingsExternalToolsSectionProps) {
  return (
    <section className="settings-section">
      <h3 className="settings-section-title">External tools</h3>
      <p className="settings-section-desc">
        Commands used to open a repository, worktree, or submodule in your terminal or editor.
        Leave a field blank to auto-detect a sensible default for this operating system.
      </p>
      <CommandInput
        id="settings-terminal-command"
        label="Terminal command"
        value={terminalCommand}
        placeholder="Leave blank to auto-detect"
        onValue={(v) => onChange({ terminalCommand: v })}
        onReset={() => onChange({ terminalCommand: '' })}
      />
      <CommandInput
        id="settings-editor-command"
        label="Editor command"
        value={editorCommand}
        placeholder="Leave blank to auto-detect"
        onValue={(v) => onChange({ editorCommand: v })}
        onReset={() => onChange({ editorCommand: '' })}
      />
      <p className="settings-section-desc">
        Use <code>{'{path}'}</code> for the folder — it is passed as a separate argument, never
        through a shell.
      </p>
    </section>
  );
}
