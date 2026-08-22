// P84 reveal-in-graph shared types (architect contract §"New shared type").
//
// A reveal target identifies which graph row a sidebar single-click should scroll
// to + flash. Refs (local branches, remotes as `origin/name`, tags) resolve by
// their `RefLabel.name`; stashes (not ref-labelled in the graph) resolve by oid.

export type RevealTarget =
  | { kind: 'ref'; name: string } // RefLabel.name: "main" | "origin/main" | "v1.0"
  | { kind: 'oid'; oid: string; label?: string }; // full 40-hex (stashes); label e.g. "stash@{0}"

/** Human-facing label for a reveal target, used in the a11y announcement + toast. */
export function revealTargetLabel(t: RevealTarget): string {
  return t.kind === 'ref' ? t.name : t.label ?? t.oid.slice(0, 7);
}

/**
 * Nonce-carrying flash descriptor threaded to GraphCanvas. A NEW `nonce`
 * (re)starts the flash even when `index` is unchanged (re-revealing the same
 * row); `null` means no flash.
 */
export interface RevealFlash {
  /** Layout row index to flash (same basis as `selectedIndex`). */
  index: number;
  /** Monotonic counter — a fresh value re-triggers the flash animation. */
  nonce: number;
}
