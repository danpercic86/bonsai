// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { AgentAsset, AgentAssetKind, AppError, AssetIssue, FrontmatterField, Validation } from '../types';

// --- P26: agent-asset (skills / subagents / slash commands) fixture ---------

/** Repo-relative path for an agent asset (mirrors Rust `rel_path`, §3.1). */
export function agentRelPath(kind: AgentAssetKind, name: string): string {
  switch (kind) {
    case 'skill':
      return `.claude/skills/${name}/SKILL.md`;
    case 'agent':
      return `.claude/agents/${name}.md`;
    case 'command':
      return `.claude/commands/${name}.md`;
  }
}

/** Deterministic (kind order skill<agent<command, then name) sort, matching
 *  `scan_agent_assets`. */
const AGENT_KIND_ORD: Record<AgentAssetKind, number> = { skill: 0, agent: 1, command: 2 };
export function sortAgentAssets(assets: AgentAsset[]): AgentAsset[] {
  return [...assets].sort(
    (a, b) => AGENT_KIND_ORD[a.kind] - AGENT_KIND_ORD[b.kind] || a.name.localeCompare(b.name),
  );
}

/** Required frontmatter keys per kind (mirrors Rust `required_keys`, §3.1). */
export function agentRequiredKeys(kind: AgentAssetKind): string[] {
  return kind === 'agent' ? ['name', 'description'] : [];
}

/** Recompute an asset's `Validation` on save, mirroring Rust `validate` (§4.5):
 *  required-key Errors + lowercase-hyphen / name-mismatch / empty-body Warnings.
 *  `complex` is never reachable — an `AgentAssetInput` is a flat field list. */
export function mockValidateAsset(
  kind: AgentAssetKind,
  name: string,
  frontmatter: FrontmatterField[],
  body: string,
): Validation {
  const issues: AssetIssue[] = [];
  for (const key of agentRequiredKeys(kind)) {
    const present = frontmatter.some((f) => f.key === key && f.value.trim() !== '');
    if (!present) {
      issues.push({ severity: 'error', message: `${kind} requires frontmatter field '${key}'` });
    }
  }
  if (!/^[a-z0-9][a-z0-9-]*$/.test(name)) {
    issues.push({
      severity: 'warning',
      message: 'name should be lowercase letters, digits, and hyphens',
    });
  }
  if (kind === 'skill' || kind === 'agent') {
    const nf = frontmatter.find((f) => f.key === 'name');
    if (nf && nf.value !== '' && nf.value !== name) {
      issues.push({
        severity: 'warning',
        message: `frontmatter name '${nf.value}' differs from the file name '${name}'`,
      });
    }
  }
  if ((kind === 'skill' || kind === 'command') && body.trim() === '') {
    issues.push({ severity: 'warning', message: 'body is empty — nothing will run' });
  }
  const valid = !issues.some((i) => i.severity === 'error');
  return { valid, issues };
}

/** Windows reserved device names (case-insensitive), matched on the base name
 *  before the first `.` — mirrors Rust `WINDOWS_RESERVED_NAMES`. */
const WINDOWS_RESERVED_NAMES = new Set([
  'con', 'prn', 'aux', 'nul',
  'com1', 'com2', 'com3', 'com4', 'com5', 'com6', 'com7', 'com8', 'com9',
  'lpt1', 'lpt2', 'lpt3', 'lpt4', 'lpt5', 'lpt6', 'lpt7', 'lpt8', 'lpt9',
]);

/** Name safety mirror of Rust `validate_asset_name` (§4.4); throws `invalidName`. */
export function requireValidAssetName(name: string): void {
  const base = name.split('.')[0];
  const bad =
    name.trim() === '' ||
    name === '.' ||
    name === '..' ||
    name.startsWith('-') ||
    name.includes('/') ||
    name.includes('\\') ||
    name.includes(':') ||
    [...name].some((c) => {
      const code = c.charCodeAt(0);
      return code < 0x20 || (code >= 0x7f && code <= 0x9f);
    }) ||
    [...name].some((c) => !/[A-Za-z0-9._-]/.test(c)) ||
    WINDOWS_RESERVED_NAMES.has(base.toLowerCase()) ||
    name.endsWith('.') ||
    name.endsWith(' ');
  if (bad) {
    const err: AppError = { kind: 'invalidName', message: `invalid asset name: '${name}'` };
    throw err;
  }
}
