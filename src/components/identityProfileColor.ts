// P82 — identity profile color palette + display-fallback helpers (UI §2/§6).
//
// The color is a DISPLAY attribute; auto-distinct assignment lives entirely in
// this UI layer and never rewrites persisted state (architect's "no rewrite on
// load" rule). `resolveProfileColor` centralizes the single `undefined → neutral`
// read-through; `autoDistinctColors` is the render-time fallback that makes
// pre-P82 (color-less) profiles look distinct by array index.

import type { IdentityProfile, ProfileColor } from '../ipc';

/** Canonical order — the ui-reference §12.8 table order. */
export const PROFILE_COLORS: readonly ProfileColor[] = [
  'neutral',
  'slate',
  'blue',
  'teal',
  'green',
  'amber',
  'orange',
  'purple',
  'pink',
];

/** The 8 assignable hues (palette minus `neutral`), in table order. */
export const ASSIGNABLE_COLORS: readonly ProfileColor[] = PROFILE_COLORS.filter(
  (c) => c !== 'neutral',
);

const LABELS: Record<ProfileColor, string> = {
  neutral: 'Neutral',
  slate: 'Slate',
  blue: 'Blue',
  teal: 'Teal',
  green: 'Green',
  amber: 'Amber',
  orange: 'Orange',
  purple: 'Purple',
  pink: 'Pink',
};

/** Title-Case display name for ARIA / screen readers. */
export function profileColorLabel(c: ProfileColor): string {
  return LABELS[c];
}

/** The single place `undefined → 'neutral'` is defined. */
export function resolveProfileColor(p: IdentityProfile): ProfileColor {
  return p.color ?? 'neutral';
}

/**
 * The first assignable hue (table order) not already used by any profile's
 * resolved color; if all 8 are taken, the least-used, ties broken by table
 * order. Used by the create flow and the header save-as draft.
 */
export function nextFreeHue(profiles: readonly IdentityProfile[]): ProfileColor {
  const counts = new Map<ProfileColor, number>();
  for (const c of ASSIGNABLE_COLORS) counts.set(c, 0);
  for (const p of profiles) {
    const c = resolveProfileColor(p);
    if (c !== 'neutral') counts.set(c, (counts.get(c) ?? 0) + 1);
  }
  let best = ASSIGNABLE_COLORS[0];
  let bestCount = Number.POSITIVE_INFINITY;
  for (const c of ASSIGNABLE_COLORS) {
    const n = counts.get(c) ?? 0;
    if (n < bestCount) {
      best = c;
      bestCount = n;
    }
  }
  return best;
}

/**
 * Display-fallback (UI §6): a color per profile for RENDERING only — never
 * persisted. A profile with an explicit `color` (including a deliberate
 * `'neutral'`) is honoured; a color-less (pre-P82) profile at index `i` gets
 * `ASSIGNABLE_COLORS[i % 8]` so legacy lists look distinct on upgrade.
 */
export function autoDistinctColors(
  profiles: readonly IdentityProfile[],
): ProfileColor[] {
  return profiles.map((p, i) =>
    p.color === undefined ? ASSIGNABLE_COLORS[i % ASSIGNABLE_COLORS.length] : p.color,
  );
}
