/**
 * P87a/P123 — the git-activity subscribe/unsubscribe handlers for the mock IPC
 * layer. Mirrors `git_activity_subscribe` (per-repo subscription, resolves to a
 * minted id) and `git_activity_unsubscribe` (idempotent, never rejects).
 */
import { subscribeGitActivity, unsubscribeGitActivity } from '../gitActivityHub';
import type { GitActivityListener, IpcApi } from '../../types';

export const activityHandlers = {
  async gitActivitySubscribe(repoId: string, onEvent: GitActivityListener): Promise<string> {
    return subscribeGitActivity(repoId, onEvent);
  },
  async gitActivityUnsubscribe(subscriptionId: string): Promise<void> {
    unsubscribeGitActivity(subscriptionId);
  },
} satisfies Partial<IpcApi>;
