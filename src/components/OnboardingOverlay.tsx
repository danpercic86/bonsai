// P43a: first-run onboarding overlay (container). Owns the step state machine
// (Welcome → Open/Clone → Identity → Tour, contract §2.2/§2.3), the identity
// getConfig/setConfig effect (global level, P40), and Next/Back/Skip wiring.
// Persistence (`onboardingSeen`) is done by App in `onClose` — this component
// never writes settings directly. Chrome mirrors ShortcutOverlay/SettingsPanel
// (backdrop + ✕; Esc handled by App's global overlay-Esc effect).

import { useCallback, useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';

import { ipc } from '../ipc';
import type { ConfigView, RecentRepo } from '../ipc';
import { errorMessage } from '../utils/errors';
import { IdentityStep, OpenRepoStep, TourStep, WelcomeStep } from './OnboardingSteps';

export type OnboardingStep = 'welcome' | 'openRepo' | 'identity' | 'tour';

const ORDER: OnboardingStep[] = ['welcome', 'openRepo', 'identity', 'tour'];

const STEP_TITLES: Record<OnboardingStep, string> = {
  welcome: 'Welcome to Bonsai',
  openRepo: 'Open a repository',
  identity: 'Your Git identity',
  tour: 'Find your way around',
};

export interface OnboardingOverlayProps {
  open: boolean;
  /** Called on Skip/Finish/Esc/✕. App persists seen=true and closes. */
  onClose: () => void;
  /** Null until a repo is opened during (or before) the flow. */
  activeRepo: string | null;
  recents: RecentRepo[];
  loading: boolean;
  /** Reused P21 handlers, owned by App. */
  onOpenRepository: () => void;
  onCloneOpen: () => void;
  onInitRepository: () => void;
  onOpenRecent: (path: string) => void;
}

function curatedValue(view: ConfigView, key: string): string | null {
  const entry = view.curated.find((c) => c.key === key);
  return entry ? entry.effectiveValue : null;
}

export function OnboardingOverlay({
  open,
  onClose,
  activeRepo,
  recents,
  loading,
  onOpenRepository,
  onCloneOpen,
  onInitRepository,
  onOpenRecent,
}: OnboardingOverlayProps) {
  const [step, setStep] = useState<OnboardingStep>('welcome');

  // Identity (§2.3 identity step) — populated on entry to the identity step.
  const [identityLoading, setIdentityLoading] = useState(false);
  const [identityError, setIdentityError] = useState<string | null>(null);
  const [identityReady, setIdentityReady] = useState(false);
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const idx = ORDER.indexOf(step);

  // Fresh open (first-run OR Settings re-trigger) always restarts at welcome.
  useEffect(() => {
    if (open) {
      setStep('welcome');
      setSaveError(null);
    }
  }, [open]);

  // Auto-advance openRepo → identity when a repo becomes active DURING the flow
  // (a null→non-null transition while on the openRepo step, contract §2.3).
  const prevActiveRef = useRef(activeRepo);
  useEffect(() => {
    const prev = prevActiveRef.current;
    prevActiveRef.current = activeRepo;
    if (open && step === 'openRepo' && prev === null && activeRepo !== null) {
      setStep('identity');
    }
  }, [open, step, activeRepo]);

  // Load the global identity when the identity step is shown with a repo open.
  useEffect(() => {
    if (!open || step !== 'identity' || activeRepo === null) return;
    let cancelled = false;
    setIdentityLoading(true);
    setIdentityError(null);
    ipc
      .getConfig(activeRepo, 'global')
      .then((view) => {
        if (cancelled) return;
        const gotName = curatedValue(view, 'user.name');
        const gotEmail = curatedValue(view, 'user.email');
        setName(gotName ?? '');
        setEmail(gotEmail ?? '');
        setIdentityReady(
          gotName !== null && gotName !== '' && gotEmail !== null && gotEmail !== '',
        );
        setIdentityLoading(false);
      })
      .catch((e) => {
        if (cancelled) return;
        setIdentityError(errorMessage(e));
        setIdentityReady(false);
        setIdentityLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, step, activeRepo]);

  // Both nav helpers read the current step from the render closure (`idx`) and
  // call setters directly in the click handler — NOT from inside a setState
  // updater. A setState updater runs during React's render phase, so calling
  // `onClose` (an App setter) there triggers "Cannot update a component (App)
  // while rendering a different component (OnboardingOverlay)".
  const goNext = useCallback(() => {
    if (idx >= ORDER.length - 1) {
      onClose();
      return;
    }
    setStep(ORDER[idx + 1]);
  }, [idx, onClose]);

  const goBack = useCallback(() => {
    if (idx <= 0) return;
    setStep(ORDER[idx - 1]);
  }, [idx]);

  const handleSaveIdentity = useCallback(() => {
    if (activeRepo === null) return;
    const trimmedName = name.trim();
    const trimmedEmail = email.trim();
    if (trimmedName === '' || trimmedEmail === '') return;
    setSaving(true);
    setSaveError(null);
    Promise.all([
      ipc.setConfig(activeRepo, 'global', 'user.name', trimmedName),
      ipc.setConfig(activeRepo, 'global', 'user.email', trimmedEmail),
    ])
      .then(() => {
        setSaving(false);
        goNext();
      })
      .catch((e) => {
        setSaving(false);
        setSaveError(errorMessage(e));
      });
  }, [activeRepo, name, email, goNext]);

  if (!open) return null;

  const isLast = idx >= ORDER.length - 1;
  const nextLabel = step === 'welcome' ? 'Get started' : isLast ? 'Finish' : 'Next';

  let body: ReactNode;
  switch (step) {
    case 'welcome':
      body = <WelcomeStep />;
      break;
    case 'openRepo':
      body = (
        <OpenRepoStep
          activeRepo={activeRepo}
          recents={recents}
          loading={loading}
          onOpenRepository={onOpenRepository}
          onCloneOpen={onCloneOpen}
          onInitRepository={onInitRepository}
          onOpenRecent={onOpenRecent}
        />
      );
      break;
    case 'identity':
      body = (
        <IdentityStep
          activeRepo={activeRepo}
          loading={identityLoading}
          error={identityError}
          ready={identityReady}
          name={name}
          email={email}
          saving={saving}
          saveError={saveError}
          onNameChange={setName}
          onEmailChange={setEmail}
          onSave={handleSaveIdentity}
        />
      );
      break;
    case 'tour':
      body = <TourStep />;
      break;
  }

  return (
    <div
      className="dialog-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="dialog-card onboarding-card" role="dialog" aria-label="Welcome to Bonsai">
        <div className="shortcut-header">
          <h2 className="dialog-title shortcut-title">{STEP_TITLES[step]}</h2>
          <button
            type="button"
            className="btn-icon shortcut-close"
            aria-label="Close"
            title="Close"
            onClick={onClose}
          >
            {'×'}
          </button>
        </div>

        <div className="onboarding-progress" aria-hidden="true">
          {ORDER.map((s, i) => (
            <span
              key={s}
              className={`onboarding-dot${i === idx ? ' is-current' : ''}${
                i < idx ? ' is-done' : ''
              }`}
            />
          ))}
        </div>

        <div className="onboarding-content">{body}</div>

        <div className="onboarding-footer">
          <button type="button" className="onboarding-skip" onClick={onClose}>
            {isLast ? 'Close' : 'Skip'}
          </button>
          <div className="onboarding-nav">
            <button
              type="button"
              className="btn-secondary"
              onClick={goBack}
              disabled={idx === 0}
            >
              {'Back'}
            </button>
            <button type="button" className="btn-primary" onClick={goNext}>
              {nextLabel}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
