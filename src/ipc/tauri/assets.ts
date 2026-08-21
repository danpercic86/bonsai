import { invoke } from '@tauri-apps/api/core';
import type { AgentAsset, AgentAssetInput, AgentAssetInventory, AgentAssetKind, AiAssetInventory, AiGeneratedAsset, AssetContent, ContextProfile, ProfileActivation, ProfilePreviewEntry, ProfileStore, WorktreeContextStatus } from '../types';

export const assetsCommands = {

  // P24: AI-asset inventory + drift.
  listAiAssets(repoId: string, canonical?: string): Promise<AiAssetInventory> {
    return invoke<AiAssetInventory>('list_ai_assets', { repoId, canonical });
  },

  readAiAsset(repoId: string, path: string): Promise<AssetContent> {
    return invoke<AssetContent>('read_ai_asset', { repoId, path });
  },

  // P26: agent-asset (skills / subagents / slash commands) read path.
  listAgentAssets(repoId: string): Promise<AgentAssetInventory> {
    return invoke<AgentAssetInventory>('list_agent_assets', { repoId });
  },

  readAgentAsset(
    repoId: string,
    kind: AgentAssetKind,
    name: string,
  ): Promise<AgentAsset> {
    return invoke<AgentAsset>('read_agent_asset', { repoId, kind, name });
  },

  saveAgentAsset(repoId: string, asset: AgentAssetInput): Promise<AgentAssetInventory> {
    return invoke<AgentAssetInventory>('save_agent_asset', { repoId, asset });
  },

  deleteAgentAsset(
    repoId: string,
    kind: AgentAssetKind,
    name: string,
  ): Promise<AgentAssetInventory> {
    return invoke<AgentAssetInventory>('delete_agent_asset', { repoId, kind, name });
  },

  // P24: context-profile store (CRUD + preview + activate).
  listProfiles(repoId: string): Promise<ProfileStore> {
    return invoke<ProfileStore>('list_profiles', { repoId });
  },

  saveProfile(repoId: string, profile: ContextProfile): Promise<ProfileStore> {
    return invoke<ProfileStore>('save_profile', { repoId, profile });
  },

  deleteProfile(repoId: string, name: string): Promise<ProfileStore> {
    return invoke<ProfileStore>('delete_profile', { repoId, name });
  },

  previewProfile(repoId: string, name: string): Promise<ProfilePreviewEntry[]> {
    return invoke<ProfilePreviewEntry[]>('preview_profile', { repoId, name });
  },

  activateProfile(repoId: string, name: string): Promise<ProfileActivation> {
    return invoke<ProfileActivation>('activate_profile', { repoId, name });
  },

  // P31: per-worktree AI contexts (matrix + worktree-targeted preview/activate).
  listWorktreeContexts(repoId: string): Promise<WorktreeContextStatus[]> {
    return invoke<WorktreeContextStatus[]>('list_worktree_contexts', { repoId });
  },

  previewWorktreeProfile(
    repoId: string,
    worktreeKey: string,
    name: string,
  ): Promise<ProfilePreviewEntry[]> {
    return invoke<ProfilePreviewEntry[]>('preview_worktree_profile', {
      repoId,
      worktreeKey,
      name,
    });
  },

  activateWorktreeProfile(
    repoId: string,
    worktreeKey: string,
    name: string,
  ): Promise<ProfileActivation> {
    return invoke<ProfileActivation>('activate_worktree_profile', {
      repoId,
      worktreeKey,
      name,
    });
  },

  // P24e: translate one instruction file into another agent's flavor. Writes
  // NOTHING — returns proposed text the user reviews before saving.
  aiGenerateAsset(
    repoId: string,
    sourceAssetId: string,
    targetAgent: string,
    guidance?: string,
  ): Promise<AiGeneratedAsset> {
    return invoke<AiGeneratedAsset>('ai_generate_asset', {
      repoId,
      sourceAssetId,
      targetAgent,
      guidance,
    });
  },
};
