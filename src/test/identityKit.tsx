/**
 * P69i shared fixtures for the header identity tests (split per the ~500-line
 * rule, the `aiDockKit.tsx` idiom).
 *
 * Lives in `src/test/` so it stays out of coverage. Data + one render helper
 * only: every assertion stays in the test file that owns its concern.
 */
import { vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { IdentityMenu } from '../components/IdentityMenu';
import { ToastContext } from '../ToastContext';
import type { ConfigLevelName, ConfigView, IdentityProfile } from '../ipc';

export const WORK: IdentityProfile = {
  id: 'p-work',
  label: 'Work',
  userName: 'Ada Lovelace',
  userEmail: 'work@bonsai.dev',
  signingKey: null,
};
export const PERSONAL: IdentityProfile = {
  id: 'p-personal',
  label: 'Personal',
  userName: 'Ada Lovelace',
  userEmail: 'me@home.dev',
  signingKey: 'KEY1',
};

/** A `getConfig(repo, 'local')` reply whose identity is effective at `level` —
 *  `targetValue` set only when that level IS local, which is the whole point. */
export function identityView(name: string, email: string, level: ConfigLevelName): ConfigView {
  const local = level === 'local';
  return {
    targetLevel: 'local',
    curated: [
      {
        key: 'user.name',
        kind: 'text',
        enumValues: [],
        effectiveValue: name,
        effectiveLevel: level,
        targetValue: local ? name : null,
      },
      {
        key: 'user.email',
        kind: 'text',
        enumValues: [],
        effectiveValue: email,
        effectiveLevel: level,
        targetValue: local ? email : null,
      },
    ],
    advanced: [],
  };
}

export const EMPTY_VIEW: ConfigView = { targetLevel: 'local', curated: [], advanced: [] };

export interface IdentityHarness {
  pushToast: ReturnType<typeof vi.fn>;
  onOpenSettingsAt: ReturnType<typeof vi.fn>;
  onMenuOpenChange: ReturnType<typeof vi.fn>;
  onProfilesChange: ReturnType<typeof vi.fn>;
}

export function renderMenu(
  repoId: string,
  profiles: IdentityProfile[] = [WORK, PERSONAL],
): IdentityHarness {
  const pushToast = vi.fn();
  const onOpenSettingsAt = vi.fn();
  const onMenuOpenChange = vi.fn();
  const onProfilesChange = vi.fn();
  render(
    <ToastContext.Provider value={pushToast}>
      <IdentityMenu
        repoId={repoId}
        profiles={profiles}
        onProfilesChange={onProfilesChange}
        onOpenSettingsAt={onOpenSettingsAt}
        onMenuOpenChange={onMenuOpenChange}
      />
    </ToastContext.Provider>,
  );
  return { pushToast, onOpenSettingsAt, onMenuOpenChange, onProfilesChange };
}

/** The trigger, whatever state it is in — matched by its stable class. */
export function trigger(): HTMLElement {
  return document.querySelector('.identity-trigger') as HTMLElement;
}

export async function openMenu(): Promise<void> {
  fireEvent.click(trigger());
  await screen.findByRole('menu');
}
