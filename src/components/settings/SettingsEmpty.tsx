// P69h — UI §1.2 / ui-reference §8: the in-pane empty block.
//
// Deliberately NOT `EmptyState.tsx`: that component is the app-level no-repo
// screen (hero mark, tagline, recents list, three CTAs) and embedding it in a
// 640px pane would put a hero section inside a settings dialog. This follows §8's
// centred-pane idiom instead — title, one sentence, at most one action.
//
// Generic on purpose (UI §9.1 lists three variants): the Git-config no-repo case
// is the only call site today, and Identities / zero-search-results are next.

import type { ReactNode } from 'react';

export function SettingsEmpty({
  title,
  body,
  actionLabel,
  onAction,
}: {
  /** 13px/600 `--text-1`. A statement of fact, not an error. */
  title: string;
  /** 12px `--text-2` (never `--text-3` — this is instructional, not decorative). */
  body: ReactNode;
  /** Omitted ⇒ no action button. */
  actionLabel?: string;
  onAction?(): void;
}) {
  return (
    <div className="settings-empty">
      <p className="settings-empty-title">{title}</p>
      <p className="settings-empty-body">{body}</p>
      {actionLabel !== undefined && onAction !== undefined && (
        <button type="button" className="btn-secondary settings-empty-action" onClick={onAction}>
          {actionLabel}
        </button>
      )}
    </div>
  );
}
