import type { RecentRepo } from '../ipc';

export interface EmptyStateProps {
  loading: boolean;
  error: string | null;
  recents: RecentRepo[];
  onOpenRepository: () => void;
  onCloneOpen: () => void;
  onInitRepository: () => void;
  onOpenRecent: (path: string) => void;
}

function folderName(path: string): string {
  const segments = path.split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] ?? path;
}

/**
 * P43b: the no-repo-open landing view — extracted verbatim from App.tsx and
 * lightly restyled (friendlier headline/sub-headline + a small mark). The three
 * primary actions and the recents list keep the exact handlers they had inline,
 * so open/clone/init/recents behavior is unchanged.
 *
 * Deliberately NO identity CTA here: SettingsPanel's identity section is
 * repo-scoped and `activeRepo` is null in this state, so a "set identity" link
 * would dead-end. Identity is handled by the onboarding flow once a repo opens.
 */
export function EmptyState({
  loading,
  error,
  recents,
  onOpenRepository,
  onCloneOpen,
  onInitRepository,
  onOpenRecent,
}: EmptyStateProps) {
  return (
    <div className="empty-state">
      <div className="empty-hero">
        <span className="empty-mark" aria-hidden="true">
          {'🌱'}
        </span>
        <h1 className="empty-title">Bonsai</h1>
        <p className="empty-tagline">A tidy Git client</p>
        <p className="empty-subhead">
          Open a repository to get started, clone one from a remote, or start something new.
        </p>
      </div>
      {error !== null && <div className="error-banner">{error}</div>}
      <div className="empty-actions">
        <button
          type="button"
          className="btn-primary"
          onClick={onOpenRepository}
          disabled={loading}
        >
          {loading ? 'Opening…' : 'Open repository'}
        </button>
        <button type="button" className="btn-secondary" onClick={onCloneOpen} disabled={loading}>
          {'Clone repository…'}
        </button>
        <button
          type="button"
          className="btn-secondary"
          onClick={onInitRepository}
          disabled={loading}
        >
          {'New repository…'}
        </button>
      </div>
      {recents.length > 0 && (
        <div className="recents-list">
          <p className="section-label recents-label">Recent</p>
          {recents.map((r) => (
            <button
              key={r.path}
              type="button"
              className="recents-item"
              disabled={loading}
              onClick={() => onOpenRecent(r.path)}
            >
              <span className="recents-item-name">{folderName(r.path)}</span>
              <span className="recents-item-path" title={r.path}>
                {r.path}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
