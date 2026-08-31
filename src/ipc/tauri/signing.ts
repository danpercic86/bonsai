import { invoke } from '@tauri-apps/api/core';
import type { SigningStatus, VerifyResults } from '../types';

export const signingCommands = {

  signingStatus(repoId: string): Promise<SigningStatus> {
    return invoke<SigningStatus>('signing_status', { repoId });
  },

  verifyCommits(repoId: string, oids: string[]): Promise<VerifyResults> {
    return invoke<VerifyResults>('verify_commits', { repoId, oids });
  },
};
