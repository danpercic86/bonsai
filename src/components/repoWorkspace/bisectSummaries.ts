// P39b (extracted for size, spec-003 ratchet): short summaries for the bisect
// banner's first-bad / current oids, resolved from the loaded graph (missing →
// the banner falls back to shortOid). Pure extraction; no behavior change.
import type { GraphLayout, RepoOpState } from '../../ipc';

export function bisectSummariesOf(
  opState: RepoOpState,
  graph: GraphLayout | null,
): Record<string, string> | undefined {
  if (opState.kind !== 'bisect') return undefined;
  const map: Record<string, string> = {};
  const nodes = graph?.nodes ?? [];
  for (const oid of [opState.current, opState.firstBad]) {
    if (oid === null) continue;
    const s = nodes.find((n) => n.id === oid)?.summary;
    if (s !== undefined) map[oid] = s;
  }
  return map;
}
