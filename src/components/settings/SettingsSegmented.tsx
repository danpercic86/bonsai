// P69g — UI §5.6 / ui-reference §12.3.2: the segmented control.
//
// A CSS skin over NATIVE `<input type="radio">` inside a `role="radiogroup"`
// labelled by the row label. Arrow-key navigation, `aria-checked` and
// `getByRole('radio', {name})` all come free; `role="tablist"` is never used.
//
// It replaces the self-labelling `btn-secondary` toggles (UI §5.3): a button
// reading `Dark` says "make it dark", the opposite of what it does. Selecting the
// already-selected segment fires NOTHING — a real behaviour change from the old
// button, which always flipped.

export interface SettingsSegmentedOption<T extends string> {
  value: T;
  label: string;
}

export function SettingsSegmented<T extends string>({
  name,
  value,
  options,
  labelledBy,
  describedBy,
  disabled,
  onChange,
}: {
  /** Radio group name — unique per row, so two groups never share a selection. */
  name: string;
  value: T;
  /** Max 3 (UI §5.6); beyond that the control is a Combobox. */
  options: readonly SettingsSegmentedOption<T>[];
  /** Id of the row's label element (`{rowId}-label`). */
  labelledBy: string;
  describedBy?: string;
  disabled?: boolean;
  onChange(next: T): void;
}) {
  if (import.meta.env.DEV && options.length > 3) {
    console.error(
      `SettingsSegmented "${name}" has ${options.length} options; UI §5.6 caps it at 3 — use a Combobox.`,
    );
  }
  return (
    <div className="settings-segmented" role="radiogroup" aria-labelledby={labelledBy}>
      {options.map((option) => (
        <label
          key={option.value}
          className={`settings-segment${option.value === value ? ' is-selected' : ''}`}
        >
          <input
            className="settings-segment-input"
            type="radio"
            name={name}
            value={option.value}
            checked={option.value === value}
            disabled={disabled}
            aria-describedby={describedBy}
            onChange={() => onChange(option.value)}
          />
          <span className="settings-segment-text">{option.label}</span>
        </label>
      ))}
    </div>
  );
}
