// P43a: presentational step cards for the first-run onboarding overlay. Pure
// display + callbacks — all state, IPC, and the step machine live in the
// container `OnboardingOverlay.tsx`. Reordered flow (contract §2.2):
// Welcome → Open/Clone → Identity → Tour.

import type { JSX } from 'react';

import type { RecentRepo } from '../ipc';
import { GraphIcon, RobotIcon, ChartIcon } from './appIcons';

function folderName(path: string): string {
  const segments = path.split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] ?? path;
}

/** Step 1: static welcome + value prop. */
export function WelcomeStep() {
  return (
    <div className="onboarding-step onboarding-welcome">
      <div className="onboarding-hero" aria-hidden="true">
        🌱
      </div>
      <p className="onboarding-lead">
        Bonsai is a fast, native Git client built around a rich, multi-lane commit graph.
      </p>
      <p className="onboarding-body">
        This quick tour helps you open a repository, confirm your Git identity, and find your way
        around. It takes less than a minute — you can skip it at any time.
      </p>
    </div>
  );
}

export interface OpenRepoStepProps {
  activeRepo: string | null;
  recents: RecentRepo[];
  loading: boolean;
  onOpenRepository: () => void;
  onCloneOpen: () => void;
  onInitRepository: () => void;
  onOpenRecent: (path: string) => void;
}

/** Step 2: reuse App's open/clone/init entry points + recents. */
export function OpenRepoStep({
  activeRepo,
  recents,
  loading,
  onOpenRepository,
  onCloneOpen,
  onInitRepository,
  onOpenRecent,
}: OpenRepoStepProps) {
  return (
    <div className="onboarding-step onboarding-openrepo">
      {activeRepo !== null ? (
        <p className="onboarding-body onboarding-openrepo-ok" role="status">
          <span aria-hidden="true">✓ </span>
          {folderName(activeRepo)} is open. Continue to finish setting up.
        </p>
      ) : (
        <p className="onboarding-body">
          Open a repository from disk, clone one from a URL, or start a fresh repository. Bonsai
          works on one repository at a time.
        </p>
      )}
      <div className="onboarding-actions">
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
        <div className="recents-list onboarding-recents">
          <p className="section-label recents-label">Recent</p>
          {recents.slice(0, 5).map((r) => (
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

export interface IdentityStepProps {
  /** Null ⇒ informational variant ("open a repo first"). */
  activeRepo: string | null;
  /** True while getConfig is in flight for the active repo. */
  loading: boolean;
  /** Non-null when the identity read failed. */
  error: string | null;
  /** Both effective values present ⇒ read-only "Identity ready" card. */
  ready: boolean;
  name: string;
  email: string;
  saving: boolean;
  saveError: string | null;
  onNameChange: (value: string) => void;
  onEmailChange: (value: string) => void;
  onSave: () => void;
}

/** Step 3: read/write global user.name + user.email (P40 config, global level). */
export function IdentityStep({
  activeRepo,
  loading,
  error,
  ready,
  name,
  email,
  saving,
  saveError,
  onNameChange,
  onEmailChange,
  onSave,
}: IdentityStepProps) {
  if (activeRepo === null) {
    return (
      <div className="onboarding-step onboarding-identity">
        <p className="onboarding-body" role="note">
          Open a repository first to confirm the Git identity used for your commits. You can set it
          later from Settings.
        </p>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="onboarding-step onboarding-identity">
        <p className="onboarding-body">Checking your Git identity…</p>
      </div>
    );
  }

  return (
    <div className="onboarding-step onboarding-identity">
      <p className="onboarding-body">
        Commits are stamped with your name and email. These are stored in your global Git
        configuration and used across all repositories.
      </p>
      {error !== null && (
        <p className="onboarding-error" role="note">
          {error}
        </p>
      )}
      {ready ? (
        <dl className="onboarding-identity-ready">
          <div className="onboarding-identity-row">
            <dt>Name</dt>
            <dd>{name}</dd>
          </div>
          <div className="onboarding-identity-row">
            <dt>Email</dt>
            <dd>{email}</dd>
          </div>
        </dl>
      ) : (
        <div className="onboarding-identity-form">
          <label className="onboarding-field">
            <span className="onboarding-field-label">Name</span>
            <input
              type="text"
              className="onboarding-input"
              value={name}
              placeholder="Your Name"
              disabled={saving}
              onChange={(e) => onNameChange(e.target.value)}
            />
          </label>
          <label className="onboarding-field">
            <span className="onboarding-field-label">Email</span>
            <input
              type="email"
              className="onboarding-input"
              value={email}
              placeholder="you@example.com"
              disabled={saving}
              onChange={(e) => onEmailChange(e.target.value)}
            />
          </label>
          {saveError !== null && (
            <p className="onboarding-error" role="note">
              {saveError}
            </p>
          )}
          <button
            type="button"
            className="btn-primary onboarding-identity-save"
            onClick={onSave}
            disabled={saving || name.trim() === '' || email.trim() === ''}
          >
            {saving ? 'Saving…' : 'Save identity'}
          </button>
        </div>
      )}
    </div>
  );
}

interface TourCard {
  icon: () => JSX.Element;
  title: string;
  body: string;
}

const TOUR_CARDS: TourCard[] = [
  {
    icon: GraphIcon,
    title: 'Commit graph',
    body: 'The center pane draws your history as multi-colored branch lanes. Scroll it to explore, click a commit to inspect it.',
  },
  {
    icon: RobotIcon,
    title: 'AI assets',
    body: 'The AI assets button in the toolbar manages AI context files and agent assets, with drift detection across profiles.',
  },
  {
    icon: ChartIcon,
    title: 'Repository health',
    body: 'The health button opens a read-only dashboard of stats, branches, working state, and structure for the open repo.',
  },
];

/** Step 4: static feature-tour cards (coach-marks deferred, contract §2.3). */
export function TourStep() {
  return (
    <div className="onboarding-step onboarding-tour">
      <p className="onboarding-body">Here is where the key features live:</p>
      <ul className="onboarding-tour-cards">
        {TOUR_CARDS.map((c) => (
          <li key={c.title} className="onboarding-tour-card">
            <span className="onboarding-tour-icon" aria-hidden="true">
              <c.icon />
            </span>
            <div className="onboarding-tour-text">
              <h4 className="onboarding-tour-title">{c.title}</h4>
              <p className="onboarding-tour-body">{c.body}</p>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
