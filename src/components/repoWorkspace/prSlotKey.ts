// P93: the `pr:<baseOid>:<headOid>:<path>` diff-slot key for one changed file of
// a pull request (base…head). Kept in its own module so the parser is unit
// testable — `overlayMeta` lives inside the RepoWorkspace container.

export const PR_SLOT_PREFIX = 'pr:';

/** Build the slot key for one PR changed file. */
export function prSlotKey(baseOid: string, headOid: string, path: string): string {
  return `${PR_SLOT_PREFIX}${baseOid}:${headOid}:${path}`;
}

/** True for a `pr:` slot key. */
export function isPrSlotKey(key: string): boolean {
  return key.startsWith(PR_SLOT_PREFIX);
}

/** Extract the path from a `pr:` slot key. A path may itself contain `:`, so the
 *  key is NOT split naively: the two oid segments are dropped and the remainder
 *  is taken verbatim. Returns null when the key is malformed (fewer than three
 *  segments after the prefix). */
export function parsePrSlotPath(key: string): string | null {
  if (!isPrSlotKey(key)) return null;
  const rest = key.slice(PR_SLOT_PREFIX.length);
  const firstSep = rest.indexOf(':');
  if (firstSep < 0) return null;
  const secondSep = rest.indexOf(':', firstSep + 1);
  if (secondSep < 0) return null;
  return rest.slice(secondSep + 1);
}
