import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { JobKind, JobStatus, JobStatusChangedPayload, RecentRepo, RepoChangedPayload, SessionState, UiSettings, UiSettingsPatch, Unsubscribe } from '../types';

export const appCommands = {

  getRecentRepos(): Promise<RecentRepo[]> {
    return invoke<RecentRepo[]>('get_recent_repos');
  },

  removeRecentRepo(path: string): Promise<RecentRepo[]> {
    return invoke<RecentRepo[]>('remove_recent_repo', { path });
  },

  onRepoChanged(cb: (p: RepoChangedPayload) => void): Promise<Unsubscribe> {
    return listen<RepoChangedPayload>('repo-changed', (e) => cb(e.payload));
  },

  onWindowFocus(cb: () => void): Promise<Unsubscribe> {
    return getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) cb();
    });
  },

  // P30: background-job scheduler.
  getJobStatus(repoId: string): Promise<JobStatus[]> {
    return invoke<JobStatus[]>('get_job_status', { repoId });
  },

  runJobNow(repoId: string, job: JobKind): Promise<void> {
    return invoke<void>('run_job_now', { repoId, job });
  },

  onJobStatusChanged(cb: (p: JobStatusChangedPayload) => void): Promise<Unsubscribe> {
    return listen<JobStatusChangedPayload>('job-status-changed', (e) => cb(e.payload));
  },

  getUiSettings(): Promise<UiSettings> {
    return invoke<UiSettings>('get_ui_settings');
  },

  setUiSettings(patch: UiSettingsPatch): Promise<UiSettings> {
    return invoke<UiSettings>('set_ui_settings', { patch });
  },

  openInTerminal(path: string): Promise<void> {
    return invoke<void>('open_in_terminal', { path });
  },

  revealInFileManager(path: string): Promise<void> {
    return invoke<void>('reveal_in_file_manager', { path });
  },

  openInEditor(path: string): Promise<void> {
    return invoke<void>('open_in_editor', { path });
  },

  openUrl(url: string): Promise<void> {
    return invoke<void>('open_url', { url });
  },

  getSession(): Promise<SessionState> {
    return invoke<SessionState>('get_session');
  },

  setSession(session: SessionState): Promise<void> {
    return invoke<void>('set_session', { session });
  },
};
