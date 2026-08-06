// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { agentRelPath, mockValidateAsset, requireValidAssetName, sortAgentAssets } from '../../fixtures/agentAssets';
import { SINGLE_FILE_PATHS, recomputeDrift } from '../../fixtures/profiles';
import { AI_OFF, applyMockWrite, delay, requireRepo } from '../repoState';
import { requireEligibleWorktree, tabWorktreeKey, worktreeDriftCounts, worktreeFilesFor, worktreesFor } from '../worktreeState';
import type { AgentAsset, AgentAssetInput, AgentAssetInventory, AgentAssetKind, AiAssetInventory, AiGeneratedAsset, AppError, AssetContent, ContextProfile, ProfileActivation, ProfilePreviewEntry, ProfileStore, TargetWriteAction, TargetWriteResult, WorktreeContextStatus } from '../../types';

export const assetsHandlers = {
  async listAiAssets(repoId: string, canonical?: string): Promise<AiAssetInventory> {
    await delay(120);
    const state = requireRepo(repoId);
    return {
      assets: structuredClone(state.inventory.assets),
      drift: recomputeDrift(state.inventory, canonical),
    };
  },

  async readAiAsset(repoId: string, path: string): Promise<AssetContent> {
    await delay(80);
    const state = requireRepo(repoId);
    const content = state.assetContent[path];
    return content !== undefined
      ? { path, exists: true, content }
      : { path, exists: false, content: null };
  },

  // P26: agent-asset (skills / subagents / slash commands) read path.
  async listAgentAssets(repoId: string): Promise<AgentAssetInventory> {
    await delay(100);
    const state = requireRepo(repoId);
    return { assets: sortAgentAssets(structuredClone(state.agentAssets)) };
  },

  async readAgentAsset(
    repoId: string,
    kind: AgentAssetKind,
    name: string,
  ): Promise<AgentAsset> {
    await delay(80);
    const state = requireRepo(repoId);
    requireValidAssetName(name);
    const found = state.agentAssets.find((a) => a.kind === kind && a.name === name);
    if (found) {
      return structuredClone(found);
    }
    // A missing asset resolves to an `exists:false` shell (matches Rust §3).
    return {
      kind,
      name,
      path: agentRelPath(kind, name),
      exists: false,
      frontmatter: [],
      body: '',
      complex: false,
      validation: {
        valid: false,
        issues: [{ severity: 'error', message: 'file does not exist' }],
      },
    };
  },

  // P26b: agent-asset write path (stateful upsert / delete).
  async saveAgentAsset(repoId: string, asset: AgentAssetInput): Promise<AgentAssetInventory> {
    await delay(120);
    const state = requireRepo(repoId);
    // Name guard fires first (throws invalidName on separators / `..` / bad
    // charset) — mirrors the backend; validation issues do NOT block a save.
    requireValidAssetName(asset.name);
    // Structural re-guard (mirrors the Rust save): refuse to overwrite an
    // existing COMPLEX asset — rebuilding from flat fields would drop its YAML.
    const existing = state.agentAssets.find(
      (a) => a.kind === asset.kind && a.name === asset.name,
    );
    if (existing?.complex) {
      const err: AppError = {
        kind: 'other',
        message:
          'cannot overwrite complex YAML frontmatter from the editor — edit this file directly',
      };
      throw err;
    }
    const saved: AgentAsset = {
      kind: asset.kind,
      name: asset.name,
      path: agentRelPath(asset.kind, asset.name),
      exists: true,
      frontmatter: structuredClone(asset.frontmatter),
      body: asset.body,
      // A flat `AgentAssetInput` is never complex (single-line editor values).
      complex: false,
      validation: mockValidateAsset(asset.kind, asset.name, asset.frontmatter, asset.body),
    };
    const idx = state.agentAssets.findIndex(
      (a) => a.kind === asset.kind && a.name === asset.name,
    );
    if (idx >= 0) {
      state.agentAssets[idx] = saved;
    } else {
      state.agentAssets.push(saved);
    }
    return { assets: sortAgentAssets(structuredClone(state.agentAssets)) };
  },

  async deleteAgentAsset(
    repoId: string,
    kind: AgentAssetKind,
    name: string,
  ): Promise<AgentAssetInventory> {
    await delay(120);
    const state = requireRepo(repoId);
    requireValidAssetName(name);
    // Skill vs single-file removal is a no-op distinction in the mock (§6).
    state.agentAssets = state.agentAssets.filter(
      (a) => !(a.kind === kind && a.name === name),
    );
    return { assets: sortAgentAssets(structuredClone(state.agentAssets)) };
  },

  // P24b: context-profile store (stateful CRUD + preview + activate).
  async listProfiles(repoId: string): Promise<ProfileStore> {
    await delay(80);
    const state = requireRepo(repoId);
    return structuredClone(state.profiles);
  },

  async saveProfile(repoId: string, profile: ContextProfile): Promise<ProfileStore> {
    await delay(120);
    const state = requireRepo(repoId);
    // Validate name: blank / leading '-' / path separators / control chars.
    const name = profile.name;
    const badName =
      name.trim() === '' ||
      name.startsWith('-') ||
      name.includes('/') ||
      name.includes('\\') ||
      [...name].some((c) => {
        const code = c.charCodeAt(0);
        // C0 controls, plus DEL and the C1 range (0x7f–0x9f) to match Rust's
        // char::is_control.
        return code < 0x20 || (code >= 0x7f && code <= 0x9f);
      });

    if (badName) {
      const err: AppError = { kind: 'invalidName', message: `invalid profile name: '${name}'` };
      throw err;
    }
    // Every target must be a single-file descriptor id.
    for (const t of profile.targets) {
      if (!(t.assetId in SINGLE_FILE_PATHS)) {
        const err: AppError = {
          kind: 'invalidName',
          message: `invalid profile target asset: '${t.assetId}'`,
        };
        throw err;
      }
    }
    const idx = state.profiles.profiles.findIndex((p) => p.name === name);
    if (idx >= 0) {
      state.profiles.profiles[idx] = structuredClone(profile);
    } else {
      state.profiles.profiles.push(structuredClone(profile));
    }
    return structuredClone(state.profiles);
  },

  async deleteProfile(repoId: string, name: string): Promise<ProfileStore> {
    await delay(100);
    const state = requireRepo(repoId);
    state.profiles.profiles = state.profiles.profiles.filter((p) => p.name !== name);
    if (state.profiles.activeProfile === name) {
      state.profiles.activeProfile = null;
    }
    return structuredClone(state.profiles);
  },

  async previewProfile(repoId: string, name: string): Promise<ProfilePreviewEntry[]> {
    await delay(120);
    const state = requireRepo(repoId);
    const profile = state.profiles.profiles.find((p) => p.name === name);
    if (profile === undefined) {
      const err: AppError = { kind: 'other', message: `profile '${name}' not found` };
      throw err;
    }
    return profile.targets.map((t) => {
      const path = SINGLE_FILE_PATHS[t.assetId] ?? t.assetId;
      const current = state.assetContent[path] ?? null;
      return { assetId: t.assetId, path, current, proposed: t.content, changed: current !== t.content };
    });
  },

  async activateProfile(repoId: string, name: string): Promise<ProfileActivation> {
    await delay(160);
    const state = requireRepo(repoId);
    const profile = state.profiles.profiles.find((p) => p.name === name);
    if (profile === undefined) {
      const err: AppError = { kind: 'other', message: `profile '${name}' not found` };
      throw err;
    }
    const results: TargetWriteResult[] = profile.targets.map((t) => {
      const path = SINGLE_FILE_PATHS[t.assetId] ?? t.assetId;
      const current = state.assetContent[path];
      let action: TargetWriteAction;
      if (current === undefined) {
        action = 'created';
      } else if (current === t.content) {
        action = 'unchanged';
      } else {
        action = 'written';
      }
      if (action !== 'unchanged') {
        applyMockWrite(state, t.assetId, path, t.content);
      }
      return { assetId: t.assetId, path, action };
    });
    // D5: record the activation under the TAB'S OWN worktree key; the legacy
    // `activeProfile` field mirrors only the "@main" entry (P31 D4).
    const key = tabWorktreeKey(state);
    state.profiles.worktreeActivations = {
      ...state.profiles.worktreeActivations,
      [key]: name,
    };
    if (key === '@main') {
      state.profiles.activeProfile = name;
    } else {
      // Keep the shared per-worktree file map in step so the matrix agrees.
      const files = worktreeFilesFor(key);
      for (const t of profile.targets) {
        files[SINGLE_FILE_PATHS[t.assetId] ?? t.assetId] = t.content;
      }
    }
    return { profile: name, results, store: structuredClone(state.profiles) };
  },

  // P31 §6: per-worktree AI contexts. The matrix derives "@main" counts from
  // the tab's stateful inventory (same math as listAiAssets) and linked-row
  // counts from the shared per-worktree file maps; activation mutates ONLY the
  // target worktree's map, so a re-list flips that row's chips alone.
  async listWorktreeContexts(repoId: string): Promise<WorktreeContextStatus[]> {
    await delay(150);
    const state = requireRepo(repoId);
    const rows = worktreesFor(state);
    const hasMatch = rows.some((w) => w.absPath === state.path);
    return rows.map((w) => {
      const key = w.isMain ? '@main' : w.name;
      const activatable = w.valid && !w.prunable && !w.locked;
      let blockedReason: string | null = null;
      if (!w.valid) {
        blockedReason = 'worktree is invalid (working directory missing or broken)';
      } else if (w.prunable) {
        blockedReason = 'worktree is stale (prunable)';
      } else if (w.locked) {
        blockedReason =
          w.lockReason === null ? 'worktree is locked' : `worktree is locked: ${w.lockReason}`;
      }
      let drifted = 0;
      let missing = 0;
      if (activatable) {
        if (w.isMain) {
          const entries = recomputeDrift(state.inventory).entries;
          drifted = entries.filter((e) => e.comparable && e.exists && !e.inSync).length;
          missing = entries.filter((e) => e.comparable && !e.exists).length;
        } else {
          ({ drifted, missing } = worktreeDriftCounts(worktreeFilesFor(key)));
        }
      }
      return {
        worktreeKey: key,
        name: w.name,
        absPath: w.absPath,
        branch: w.branch,
        isMain: w.isMain,
        isCurrent: hasMatch ? w.absPath === state.path : w.isMain,
        locked: w.locked,
        prunable: w.prunable,
        valid: w.valid,
        activeProfile: state.profiles.worktreeActivations?.[key] ?? null,
        driftedCount: drifted,
        missingCount: missing,
        activatable,
        blockedReason,
      };
    });
  },

  async previewWorktreeProfile(
    repoId: string,
    worktreeKey: string,
    name: string,
  ): Promise<ProfilePreviewEntry[]> {
    await delay(120);
    const state = requireRepo(repoId);
    const profile = state.profiles.profiles.find((p) => p.name === name);
    if (profile === undefined) {
      const err: AppError = { kind: 'other', message: `profile '${name}' not found` };
      throw err;
    }
    const row = requireEligibleWorktree(state, worktreeKey); // D6
    const files = row.isMain ? state.assetContent : worktreeFilesFor(worktreeKey);
    return profile.targets.map((t) => {
      const path = SINGLE_FILE_PATHS[t.assetId] ?? t.assetId;
      const current = files[path] ?? null;
      return {
        assetId: t.assetId,
        path,
        current,
        proposed: t.content,
        changed: current !== t.content,
      };
    });
  },

  async activateWorktreeProfile(
    repoId: string,
    worktreeKey: string,
    name: string,
  ): Promise<ProfileActivation> {
    await delay(160);
    const state = requireRepo(repoId);
    const profile = state.profiles.profiles.find((p) => p.name === name);
    if (profile === undefined) {
      const err: AppError = { kind: 'other', message: `profile '${name}' not found` };
      throw err;
    }
    const row = requireEligibleWorktree(state, worktreeKey); // D6
    const files = row.isMain ? state.assetContent : worktreeFilesFor(worktreeKey);
    const results: TargetWriteResult[] = profile.targets.map((t) => {
      const path = SINGLE_FILE_PATHS[t.assetId] ?? t.assetId;
      const current = files[path];
      let action: TargetWriteAction;
      if (current === undefined) {
        action = 'created';
      } else if (current === t.content) {
        action = 'unchanged';
      } else {
        action = 'written';
      }
      if (action !== 'unchanged') {
        if (row.isMain) {
          // Keep the tab's inventory in step so AiAssetsPanel chips flip too.
          applyMockWrite(state, t.assetId, path, t.content);
        } else {
          files[path] = t.content;
        }
      }
      return { assetId: t.assetId, path, action };
    });
    state.profiles.worktreeActivations = {
      ...state.profiles.worktreeActivations,
      [worktreeKey]: name,
    };
    if (worktreeKey === '@main') {
      state.profiles.activeProfile = name; // legacy mirror (D4)
    }
    return { profile: name, results, store: structuredClone(state.profiles) };
  },

  // P24e: translate one instruction file into another agent's flavor. Writes
  // NOTHING — returns canned proposed text after a delay. Gated on the mock's
  // AI-off convention (mirrors generateCommitMessage's AI_OFF handling): `?ai=off`
  // simulates disabled AI → aiUnavailable, else always succeed.
  async aiGenerateAsset(
    repoId: string,
    sourceAssetId: string,
    targetAgent: string,
    guidance?: string,
  ): Promise<AiGeneratedAsset> {
    await delay(500);
    requireRepo(repoId);
    if (AI_OFF) {
      const err: AppError = {
        kind: 'aiUnavailable',
        message: 'AI features are disabled — enable them in Settings',
      };
      throw err;
    }
    const guidanceLine =
      guidance !== undefined && guidance.trim() !== ''
        ? `\n\n> Guidance applied: ${guidance.trim()}`
        : '';
    return {
      targetAgent,
      content:
        `# ${targetAgent} instructions\n\n` +
        `…(translated from \`${sourceAssetId}\`)…\n\n` +
        `Preserve the original guidance; adapt tone and format for ${targetAgent}.` +
        guidanceLine +
        '\n',
    };
  },

  // P42 (INV-2): stateful update seam keyed on `?update=available|none|error`.
} satisfies Partial<IpcApi>;
