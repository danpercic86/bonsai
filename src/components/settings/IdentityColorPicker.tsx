// P82 — the identity color picker (UI §5).
//
// A 9-swatch chooser built on NATIVE `<input type="radio">` inside a
// `role="radiogroup"` (the SettingsSegmented idiom — arrow-key nav, roving focus,
// aria-checked and getByRole('radio',{name}) all come free). It is a distinct
// control from SettingsSegmented because that one is text-only and caps at 3.
//
// This is the ONE place the color name IS announced: each radio's accessible name
// is the color name via a visually-hidden span. The painted dot is aria-hidden;
// the fill comes from `--profile-*` via the `data-profile-color` selector — no hex.

import { PROFILE_COLORS, profileColorLabel } from '../identityProfileColor';
import type { ProfileColor } from '../../ipc';

export function IdentityColorPicker({
  name,
  value,
  labelledBy,
  describedBy,
  onChange,
}: {
  /** Radio group name — unique per profile so groups never share a selection. */
  name: string;
  value: ProfileColor;
  /** Id of the row's label element (`{rowId}-label`). */
  labelledBy: string;
  describedBy?: string;
  onChange(next: ProfileColor): void;
}) {
  return (
    <div
      className="identity-swatch-grid"
      role="radiogroup"
      aria-labelledby={labelledBy}
      aria-describedby={describedBy}
    >
      {PROFILE_COLORS.map((color) => (
        <label
          key={color}
          className={`identity-swatch-option${color === value ? ' is-selected' : ''}`}
        >
          <input
            className="identity-swatch-input"
            type="radio"
            name={name}
            value={color}
            checked={color === value}
            onChange={() => onChange(color)}
          />
          <span className="identity-swatch-dot" data-profile-color={color} aria-hidden="true" />
          <span className="sr-only">{profileColorLabel(color)}</span>
        </label>
      ))}
    </div>
  );
}
