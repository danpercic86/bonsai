import { invoke, Channel } from '@tauri-apps/api/core';
import type { GitActivityEvent } from '../types';

// P87 git-activity observability. ONE long-lived subscription (Option B): the
// frontend registers once on app/repo mount and every git op's events arrive on
// this channel; re-invoked after an HMR/reload (the backend prunes stale
// channels on send failure). Resolves immediately.
export const activityCommands = {
  gitActivitySubscribe(onEvent: (e: GitActivityEvent) => void): Promise<void> {
    const channel = new Channel<GitActivityEvent>();
    channel.onmessage = onEvent;
    // Tauri auto-serializes the Channel as the `on_event` command argument
    // (mirrors aiResolveConflictStream / streamGraph).
    return invoke<void>('git_activity_subscribe', { onEvent: channel });
  },
};
