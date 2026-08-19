// P69g — UI §5.5 / ui-reference §12.3.1: the settings switch.
//
// A CSS skin over a NATIVE `<input type="checkbox">` (UI D4). The implicit
// `checkbox` role, native Space toggling and every `getByRole('checkbox', {name})`
// query survive unchanged; `role="switch"` is deliberately NOT used.
//
// The wrapper is a `<span>`, not the `<label>` of UI §5.5: the row already owns a
// `<label htmlFor>` (SettingsRow), and a second, textless `<label>` around the
// input would contribute an empty string to the accessible-name computation for
// nothing. The input itself is the whole 36×24 hit target, so click-to-toggle on
// the control is unaffected, and clicking the row label still toggles via `for`.

export function SettingsSwitch({
  id,
  checked,
  disabled,
  describedBy,
  onChange,
}: {
  /** Must match the row label's `htmlFor` — that label IS the accessible name. */
  id: string;
  checked: boolean;
  disabled?: boolean;
  /** Id of the row's help paragraph (UI §5.1). */
  describedBy?: string;
  onChange(next: boolean): void;
}) {
  return (
    <span className="settings-switch">
      <input
        id={id}
        className="settings-switch-input"
        type="checkbox"
        checked={checked}
        disabled={disabled}
        aria-describedby={describedBy}
        onChange={(e) => onChange(e.target.checked)}
      />
      {/* Knob POSITION is the non-colour carrier (UI §5.5); the track hue never
          carries meaning on its own. Both are decorative to AT. */}
      <span className="settings-switch-track" aria-hidden="true">
        <span className="settings-switch-knob" />
      </span>
    </span>
  );
}
