import { Channel } from '@tauri-apps/api/core';
import { invoke } from './invoke';
import type { GitActivityEvent, GitActivityListener } from '../types';

// P87/P123 git-activity observability. One subscription per mounted tab, scoped
// to that tab's repo: the backend emits a run only onto its own repo's channels
// (clone/init broadcast). Resolves to a backend-minted subscription id, which
// `gitActivityUnsubscribe` drops on unmount (idempotent).
export const activityCommands = {
  gitActivitySubscribe(repoId: string, onEvent: GitActivityListener): Promise<string> {
    const channel = new Channel<GitActivityEvent>();
    channel.onmessage = onEvent;
    // Tauri auto-serializes the Channel as the `on_event` command argument
    // (mirrors aiResolveConflictStream / streamGraph).
    return invoke<string>('git_activity_subscribe', { repoId, onEvent: channel });
  },
  gitActivityUnsubscribe(subscriptionId: string): Promise<void> {
    return invoke<void>('git_activity_unsubscribe', { subscriptionId });
  },
};
