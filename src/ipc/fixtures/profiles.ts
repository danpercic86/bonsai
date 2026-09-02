// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { AiAssetInventory, DriftReport } from '../types';

/** Client-side mirror of the Rust drift algorithm (§4.3) so the `canonical`
 *  override is demonstrable in the browser harness. */
export const COMPARABLE_IDS = ['claude', 'agents', 'copilot', 'gemini', 'windsurf', 'cursorLegacy'];
export function recomputeDrift(inv: AiAssetInventory, canonical?: string): DriftReport {
  const byId = (id: string) => inv.assets.find((a) => a.id === id);
  const exists = (id: string) => byId(id)?.exists ?? false;
  const nhash = (id: string) => byId(id)?.files[0]?.normalizedHash ?? null;

  const canonicalId: string | null =
    canonical && COMPARABLE_IDS.includes(canonical) && exists(canonical)
      ? canonical
      : // Priority == table order for the comparable set.
        (COMPARABLE_IDS.find((id) => exists(id)) ?? null);
  const canonicalHash = canonicalId ? nhash(canonicalId) : null;

  const entries = COMPARABLE_IDS.map((id) => {
    const ex = exists(id);
    const normalizedHash = ex ? nhash(id) : null;
    const inSync = ex && canonicalHash !== null && normalizedHash === canonicalHash;
    return { assetId: id, exists: ex, comparable: true, normalizedHash, inSync };
  });
  const inSync = entries.filter((e) => e.exists).every((e) => e.inSync);
  return { canonicalId, canonicalHash, entries, inSync };
}
// --- P24b: profiles fixture + stateful activation helpers -------------------

/** The single-file (profile-target-eligible) descriptor ids → mapped repo path,
 *  mirroring the Rust taxonomy's SingleFile rows. Used to validate targets and
 *  resolve preview/activation paths in the mock. */
export const SINGLE_FILE_PATHS: Record<string, string> = {
  claude: 'CLAUDE.md',
  agents: 'AGENTS.md',
  copilot: '.github/copilot-instructions.md',
  gemini: 'GEMINI.md',
  windsurf: '.windsurfrules',
  cursorLegacy: '.cursorrules',
};

/** Deterministic 40-hex mock hash of a string (FNV-1a → repeated to 40 chars).
 *  Not git's SHA-1 — the mock only needs stable equality so drift recomputes
 *  correctly after an activation writes new content. */
export function mockHash(content: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < content.length; i += 1) {
    h ^= content.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  const hex = h.toString(16).padStart(8, '0');
  return hex.repeat(5); // 8 * 5 = 40 hex chars
}
