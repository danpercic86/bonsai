// P82 — the identity color dot primitive (UI §2).
//
// A tiny presentational span reused by the avatar ring, menu rows, menu header
// and the settings card. The fill is chosen in CSS by the `data-profile-color`
// attribute selector — there is NO inline color and NO hex here or anywhere.
//
// `aria-hidden` everywhere EXCEPT inside the picker, where the color name is the
// accessible signal (the picker passes its own radio labels; this stays hidden
// there too since the swatch is decorative next to the real radio).

import type { ProfileColor } from '../ipc';

export function IdentityColorSwatch({
  color,
  size = 'md',
}: {
  color: ProfileColor;
  /** 'sm' = 8px (menu rows / compact); default 10px. */
  size?: 'sm' | 'md';
}) {
  return (
    <span
      className="identity-swatch"
      data-profile-color={color}
      data-swatch-size={size}
      aria-hidden="true"
    />
  );
}
