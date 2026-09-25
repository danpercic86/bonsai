/** P120 §3.2 — the closed `data-obs-region` vocabulary and node → region lookup. */
import type { StabilityRegion } from './types';

export const REGIONS = ['sidebar', 'graph', 'details', 'diff', 'dock', 'toolbar', 'other'] as const satisfies readonly StabilityRegion[];

// Compile-time exhaustiveness: every StabilityRegion literal is in REGIONS.
type Missing = Exclude<StabilityRegion, (typeof REGIONS)[number]>;
const exhaustive: [Missing] extends [never] ? true : Missing = true;
void exhaustive;

const REGION_SET: ReadonlySet<string> = new Set<string>(REGIONS);

export function isRegion(v: string): v is StabilityRegion {
  return REGION_SET.has(v);
}

/** Nearest `[data-obs-region]` ancestor wins; unknown value or none → `'other'`.
 *  Text nodes (layout-shift sources can be text) resolve via their parent. Works
 *  on detached subtrees too. Never reads layout. */
export function regionOf(node: Node | null): StabilityRegion {
  if (node === null) return 'other';
  const el = node instanceof Element ? node : node.parentElement;
  const v = el?.closest('[data-obs-region]')?.getAttribute('data-obs-region');
  return v !== undefined && v !== null && isRegion(v) ? v : 'other';
}
