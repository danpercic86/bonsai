/**
 * P87a — the git-activity subscribe handler for the mock IPC layer.
 *
 * MINIMAL: registers the long-lived callback (the full `runMockActivity` emitter
 * + the `?prePushHook` / `?prePushFail` / `?fetchSlow` query seams that wrap the
 * push/commit/fetch handler bodies are P87b). Resolves immediately, like the real
 * `git_activity_subscribe`.
 */
import { subscribeGitActivity } from '../gitActivity';
import type { GitActivityEvent, IpcApi } from '../../types';

export const activityHandlers = {
  async gitActivitySubscribe(onEvent: (e: GitActivityEvent) => void): Promise<void> {
    subscribeGitActivity(onEvent);
  },
} satisfies Partial<IpcApi>;
