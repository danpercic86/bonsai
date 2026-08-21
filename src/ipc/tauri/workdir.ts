import { invoke } from '@tauri-apps/api/core';
import type { CommitDiff, CommitResult, CompareDiff, FileDiff, ImageDiff, ImageDiffRequest, LineSelection } from '../types';

export const workdirCommands = {

  stage(repoId: string, paths: string[]): Promise<void> {
    return invoke<void>('stage', { repoId, paths });
  },

  unstage(repoId: string, paths: string[]): Promise<void> {
    return invoke<void>('unstage', { repoId, paths });
  },

  commit(
    repoId: string,
    message: string,
    sign: boolean | null = null,
    skipHooks = false,
  ): Promise<CommitResult> {
    return invoke<CommitResult>('commit', { repoId, message, sign, skipHooks });
  },

  getWorkdirFileDiff(
    repoId: string,
    path: string,
    origPath: string | null,
    staged: boolean,
    fullContext: boolean,
    intraline: boolean,
  ): Promise<FileDiff> {
    return invoke<FileDiff>('get_workdir_file_diff', {
      repoId,
      path,
      origPath,
      staged,
      fullContext,
      intraline,
    });
  },

  stagePartial(
    repoId: string,
    path: string,
    origPath: string | null,
    selection: LineSelection[],
  ): Promise<void> {
    return invoke<void>('stage_partial', { repoId, path, origPath, selection });
  },

  unstagePartial(
    repoId: string,
    path: string,
    origPath: string | null,
    selection: LineSelection[],
  ): Promise<void> {
    return invoke<void>('unstage_partial', { repoId, path, origPath, selection });
  },

  discardPartial(
    repoId: string,
    path: string,
    origPath: string | null,
    selection: LineSelection[],
  ): Promise<void> {
    return invoke<void>('discard_partial', { repoId, path, origPath, selection });
  },

  getCommitDiff(repoId: string, oid: string): Promise<CommitDiff> {
    return invoke<CommitDiff>('get_commit_diff', { repoId, oid });
  },

  getCommitFileDiff(
    repoId: string,
    oid: string,
    path: string,
    origPath: string | null,
    fullContext: boolean,
    intraline: boolean,
  ): Promise<FileDiff> {
    return invoke<FileDiff>('get_commit_file_diff', {
      repoId,
      oid,
      path,
      origPath,
      fullContext,
      intraline,
    });
  },

  compareWithHead(repoId: string, oid: string): Promise<CompareDiff> {
    return invoke<CompareDiff>('compare_with_head', { repoId, oid });
  },

  compareWithHeadFileDiff(
    repoId: string,
    oid: string,
    path: string,
    origPath: string | null,
    fullContext: boolean,
    intraline: boolean,
  ): Promise<FileDiff> {
    return invoke<FileDiff>('compare_with_head_file_diff', {
      repoId,
      oid,
      path,
      origPath,
      fullContext,
      intraline,
    });
  },

  getImageDiff(repoId: string, request: ImageDiffRequest): Promise<ImageDiff> {
    return invoke<ImageDiff>('get_image_diff', { repoId, request });
  },
};
