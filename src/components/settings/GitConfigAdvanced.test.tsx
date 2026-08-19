/**
 * P69h / Amendment A (AM-2) — the two aggregate `'group'` rows.
 *
 * `git-config.behaviour` and `git-config.custom-keys` stand for blocks whose
 * contents are REPO-DERIVED, so no static catalog can own them and the coverage
 * guard is deliberately blind inside them (AM-4b blindness #2). This suite is
 * AM-2's recommended compensating control: it asserts the rendered
 * `data-config-key` set against the keys the backend actually returned, which is
 * the per-key coverage the guard gives up.
 */
import { describe, expect, it, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { GitConfigAdvanced } from './GitConfigAdvanced';
import { findSettingsRow } from './settingsCatalog';
import type { ConfigEntry, CuratedConfigEntry } from '../../ipc';

const CURATED: CuratedConfigEntry[] = [
  {
    key: 'pull.rebase',
    kind: 'enum',
    enumValues: ['true', 'false'],
    effectiveValue: 'true',
    effectiveLevel: 'local',
    targetValue: 'true',
  },
  {
    key: 'core.autocrlf',
    kind: 'enum',
    enumValues: ['true', 'false', 'input'],
    effectiveValue: 'input',
    effectiveLevel: 'global',
    targetValue: null,
  },
  {
    key: 'init.defaultBranch',
    kind: 'text',
    enumValues: [],
    effectiveValue: 'main',
    effectiveLevel: 'global',
    targetValue: null,
  },
];

const ADVANCED: ConfigEntry[] = [
  { name: 'custom.thing', value: 'v1', level: 'local' },
  { name: 'bonsai.runHooks', value: 'false', level: 'local' },
];

function renderBlock(advanced: ConfigEntry[] = ADVANCED) {
  return render(
    <GitConfigAdvanced
      repoId="/repo"
      level="local"
      behaviourKeys={CURATED}
      advanced={advanced}
      drafts={{}}
      busyKey={null}
      fieldErrors={{}}
      onDraftChange={vi.fn()}
      onCommit={vi.fn()}
      onRemove={vi.fn()}
      onReload={vi.fn()}
    />,
  );
}

function keysIn(block: Element): string[] {
  return [...block.querySelectorAll<HTMLElement>('[data-config-key]')]
    .map((el) => el.dataset.configKey ?? '')
    .sort();
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('GitConfigAdvanced — the aggregate group rows (AM-2)', () => {
  it('stamps each block as a role="group" named by its own heading', () => {
    const { container } = renderBlock();

    for (const id of ['git-config.behaviour', 'git-config.custom-keys']) {
      const block = container.querySelector(`[data-setting-id="${id}"]`);
      expect(block, id).not.toBeNull();
      expect(block?.getAttribute('role')).toBe('group');
      const labelledBy = block?.getAttribute('aria-labelledby') ?? '';
      const heading = document.getElementById(labelledBy);
      // Byte-for-byte with the catalog label — British `Behaviour` included.
      expect(heading?.textContent).toBe(findSettingsRow(id)?.label);
    }
  });

  it('renders every curated Behaviour key the backend returned, and only those', () => {
    const { container } = renderBlock();
    const block = container.querySelector('[data-setting-id="git-config.behaviour"]');
    expect(keysIn(block as Element)).toEqual(['core.autocrlf', 'init.defaultBranch', 'pull.rebase']);
  });

  it('renders every custom key the backend returned, and only those', () => {
    const { container } = renderBlock();
    const block = container.querySelector('[data-setting-id="git-config.custom-keys"]');
    expect(keysIn(block as Element)).toEqual(['bonsai.runHooks', 'custom.thing']);
  });

  it('says so when the level holds no custom keys, rather than showing an empty block', () => {
    renderBlock([]);
    expect(screen.getByText('No other keys set at the local level.')).toBeInTheDocument();
  });

  it('keeps the blocks inside the collapsed Advanced group', () => {
    const { container } = renderBlock();
    const details = container.querySelector('details.settings-group');
    expect(details?.querySelector('.settings-group-title')?.textContent).toBe('Advanced');
    expect(details?.querySelector('[data-setting-id="git-config.behaviour"]')).not.toBeNull();
    // The <details> itself is NOT a catalogued row — the two blocks are.
    expect(details?.getAttribute('data-setting-id')).toBeNull();
  });
});
