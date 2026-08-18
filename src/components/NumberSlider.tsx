// P51b: extracted verbatim from SettingsPanel so the Graph section
// (SettingsGraphSection) and SettingsPanel's other sliders share one control.

function clamp(v: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, v));
}

/** A labeled number input + range slider bound to the same value. Clamps and
 *  ignores non-numeric input (empty field) before calling `onChange`. */
export function NumberSlider({
  id,
  label,
  value,
  min,
  max,
  unit,
  disabled,
  describedBy,
  onChange,
}: {
  id: string;
  label: string;
  value: number;
  min: number;
  max: number;
  unit?: string;
  disabled?: boolean;
  /** P68g §1.6: id(s) of the hint paragraph(s) describing this control, wired onto
   *  the number input so the explanation is announced rather than orphaned. */
  describedBy?: string;
  onChange(next: number): void;
}) {
  const commit = (raw: string): void => {
    // An empty (or non-numeric — the platform blanks the field) input patches
    // NOTHING: `Number('')` is 0, so without this guard clearing the field would
    // silently snap the setting to `min`, which is what the doc comment above
    // always claimed but the code did not do (P68g §6.1 acceptance 5).
    if (raw.trim() === '') return;
    const n = Number(raw);
    if (Number.isNaN(n)) return;
    onChange(clamp(Math.round(n), min, max));
  };
  return (
    <div className={`settings-control${disabled === true ? ' is-disabled' : ''}`}>
      <label className="settings-control-label" htmlFor={id}>
        {label}
      </label>
      <div className="settings-control-inputs">
        <input
          className="settings-range"
          type="range"
          min={min}
          max={max}
          step={1}
          value={value}
          disabled={disabled}
          onChange={(e) => commit(e.target.value)}
          aria-label={label}
        />
        <input
          id={id}
          className="settings-number"
          type="number"
          min={min}
          max={max}
          step={1}
          value={value}
          disabled={disabled}
          aria-describedby={describedBy}
          onChange={(e) => commit(e.target.value)}
        />
        {unit !== undefined && <span className="settings-unit">{unit}</span>}
      </div>
    </div>
  );
}
