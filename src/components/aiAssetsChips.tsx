import type { AgentAsset, AgentAssetKind, DriftEntry } from '../ipc';

/** The three agent-asset groups, in display order (skill<agent<command). */
export const AGENT_GROUPS: { kind: AgentAssetKind; title: string; newLabel: string }[] = [
  { kind: 'skill', title: 'Skills', newLabel: 'New skill' },
  { kind: 'agent', title: 'Subagents', newLabel: 'New subagent' },
  { kind: 'command', title: 'Slash commands', newLabel: 'New command' },
];

/** True when the asset's frontmatter is complex (read-only). Keys off the
 *  structural `complex` flag (authoritative); falls back to the legacy Error
 *  message for resilience. */
function isComplexAsset(asset: AgentAsset): boolean {
  return (
    asset.complex ||
    asset.validation.issues.some(
      (i) => i.severity === 'error' && i.message.includes('multi-line YAML'),
    )
  );
}

/** The validation chip for one agent asset (§7.1): complex read-only, green
 *  valid, or amber N issue(s). */
export function agentValidationChip(asset: AgentAsset) {
  if (isComplexAsset(asset)) {
    return <span className="asset-chip asset-chip-muted">complex — read-only</span>;
  }
  if (asset.validation.valid) {
    return <span className="asset-chip asset-chip-sync">valid</span>;
  }
  const count = asset.validation.issues.length;
  return (
    <span className="asset-chip asset-chip-drifted">
      {count} issue{count === 1 ? '' : 's'}
    </span>
  );
}

/** The sync chip for one managed instruction file (§8.1). */
export function syncChip(entry: DriftEntry, canonicalId: string | null) {
  if (entry.assetId === canonicalId) {
    return <span className="asset-chip asset-chip-canonical">canonical</span>;
  }
  if (!entry.exists) return <span className="asset-chip asset-chip-missing">missing</span>;
  if (entry.inSync) return <span className="asset-chip asset-chip-sync">in sync</span>;
  return <span className="asset-chip asset-chip-drifted">drifted</span>;
}
