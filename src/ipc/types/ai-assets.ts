// P24 — AI-asset management (inventory + drift). Mirrors the Rust wire types
// in `crates/bonsai-core/src/assets/` exactly (camelCase).

/** Kind of an AI-asset target. Bare-string serde enum on the Rust side. */
export type AssetKind = 'singleFile' | 'rulesDir' | 'config';

export interface AssetFile {
  path: string;
  /** u64 on the wire; safe as a JS number here. */
  size: number;
  contentHash: string;
  normalizedHash: string;
  /** epoch seconds, or null when unavailable. */
  modified: number | null;
}

export interface AiAsset {
  id: string;
  agent: string;
  label: string;
  kind: AssetKind;
  path: string;
  managed: boolean;
  exists: boolean;
  files: AssetFile[];
}

export interface DriftEntry {
  assetId: string;
  exists: boolean;
  comparable: boolean;
  normalizedHash: string | null;
  inSync: boolean;
}

export interface DriftReport {
  canonicalId: string | null;
  canonicalHash: string | null;
  entries: DriftEntry[];
  inSync: boolean;
}

export interface AiAssetInventory {
  assets: AiAsset[];
  drift: DriftReport;
}

export interface AssetContent {
  path: string;
  exists: boolean;
  content: string | null;
}

// P26 — Agent-asset (skills / subagents / slash commands) manager. Mirrors the
// Rust wire types in `crates/bonsai-core/src/assets/bundle.rs` exactly (camelCase;
// bare-string enums).

/** Which `.claude/` agent-asset kind. Bare-string serde enum on the Rust side. */
export type AgentAssetKind = 'skill' | 'agent' | 'command';

/** Severity of a validation finding. Bare-string serde enum on the Rust side. */
export type IssueSeverity = 'error' | 'warning';

/** One frontmatter entry; `value` is the verbatim opaque scalar after `key: `. */
export interface FrontmatterField {
  key: string;
  value: string;
}

export interface AssetIssue {
  severity: IssueSeverity;
  message: string;
}

/** Validation verdict for one asset. `valid` iff no Error-severity issue. */
export interface Validation {
  valid: boolean;
  issues: AssetIssue[];
}

export interface AgentAsset {
  kind: AgentAssetKind;
  /** Directory name (skill) or file stem (agent/command). */
  name: string;
  /** Repo-relative file path, forward slashes (e.g. `.claude/agents/foo.md`). */
  path: string;
  exists: boolean;
  /** Parsed flat frontmatter, in file order, unknown keys preserved. */
  frontmatter: FrontmatterField[];
  /** Everything after the closing `---` fence (verbatim); whole file if none. */
  body: string;
  /** `true` when the frontmatter uses multi-line/sequence/nested YAML the flat
   *  parser can't round-trip (§4.3). The structural signal the editor uses to
   *  open the asset read-only; the backend also re-guards saves on it. */
  complex: boolean;
  validation: Validation;
}

export interface AgentAssetInventory {
  assets: AgentAsset[];
}

/** Save payload for `saveAgentAsset` (P26b) — no path/exists/validation, which
 *  the backend derives/computes. */
export interface AgentAssetInput {
  kind: AgentAssetKind;
  name: string;
  frontmatter: FrontmatterField[];
  body: string;
}

/** One profile target: which single-file asset to write, and its verbatim content. */
export interface ProfileTarget {
  assetId: string;
  content: string;
}

export interface ContextProfile {
  name: string;
  description?: string | null;
  model?: string | null;
  targets: ProfileTarget[];
}

/** The on-disk store (`.bonsai/profiles.json`) and the wire shape of
 *  list/save/delete/activate. */
export interface ProfileStore {
  version: number;
  profiles: ContextProfile[];
  /** LEGACY mirror of `worktreeActivations["@main"]` (P31 D4). */
  activeProfile?: string | null;
  /** P31 D3/D4: worktree key (`"@main"` | linked worktree name) → the profile
   *  last activated INTO that worktree. Omitted by serde when empty. */
  worktreeActivations?: Record<string, string>;
}

export interface ProfilePreviewEntry {
  assetId: string;
  path: string;
  current: string | null;
  proposed: string;
  changed: boolean;
}

/** What an activation did to one target's file. Bare-string serde enum on Rust. */
export type TargetWriteAction = 'created' | 'written' | 'unchanged';

export interface TargetWriteResult {
  assetId: string;
  path: string;
  action: TargetWriteAction;
}

export interface ProfileActivation {
  profile: string;
  results: TargetWriteResult[];
  /** The store after `activeProfile` was updated (frontend refreshes from this). */
  store: ProfileStore;
}

/** P31 §4. One row of the worktree × AI-context matrix. Wire mirror of the
 *  Rust `WorktreeContextStatus`. */
export interface WorktreeContextStatus {
  /** Store key + command argument: `"@main"` | linked worktree name (D3). */
  worktreeKey: string;
  /** Display name (main basename / linked name). */
  name: string;
  /** Absolute path, forward slashes. */
  absPath: string;
  branch: string | null;
  isMain: boolean;
  isCurrent: boolean;
  locked: boolean;
  prunable: boolean;
  valid: boolean;
  /** From `worktreeActivations` (v1 legacy `activeProfile` folded in for `"@main"`). */
  activeProfile: string | null;
  /** D10: drift entries `comparable && exists && !inSync` in THIS worktree. */
  driftedCount: number;
  /** Comparable descriptors with `exists === false` in THIS worktree. */
  missingCount: number;
  /** D6: `valid && !prunable && !locked`. */
  activatable: boolean;
  /** Human-readable reason when `!activatable`, else null. */
  blockedReason: string | null;
}

/** P24e. The AI-translate helper's proposed instruction file. NOT written
 *  anywhere — the user reviews it and pastes it into a profile target. */
export interface AiGeneratedAsset {
  targetAgent: string;
  content: string;
}
