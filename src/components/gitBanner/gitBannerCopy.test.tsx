/** P70 UI §5: the copy table — all three OS variants, both banner variants, the
 *  `sourceLabel` map, and the technical block. Pure functions, zero DOM. */
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  ANNOUNCE_STILL_UNAVAILABLE,
  announceAvailable,
  BANNER_EXPLANATION,
  BANNER_TITLE,
  bannerCopy,
  bannerRemedy,
  bannerVariant,
  buildAnnouncement,
  buildTechnicalDetails,
  CAPABILITY_ROWS,
  checkedAtLine,
  gitAvailableToastText,
  otherRemedies,
  resolveOsFamily,
  sourceLabel,
} from './gitBannerCopy';
import type { GitAvailability, GitBinSource } from '../../ipc';
import { osFamily, type OsFamily } from '../../utils/platform';

const ALL_OS: OsFamily[] = ['windows', 'mac', 'linux'];

const MISSING: GitAvailability = {
  found: false,
  path: null,
  version: null,
  source: 'fallback',
  detail: 'the rust payload',
};

describe('variant selection', () => {
  it('keys purely on `path !== null` (ratified decision 1)', () => {
    expect(bannerVariant(null)).toBe('notFound');
    expect(bannerVariant(MISSING)).toBe('notFound');
    expect(bannerVariant({ ...MISSING, path: 'C:\\git.exe', source: 'override' })).toBe(
      'unrunnable',
    );
  });
});

describe('per-OS copy', () => {
  it('the not-found remedy names the right launcher on each OS', () => {
    expect(bannerRemedy('notFound', 'windows', 'fallback')).toContain('Start menu');
    expect(bannerRemedy('notFound', 'mac', 'fallback')).toContain('Applications');
    expect(bannerRemedy('notFound', 'linux', 'fallback')).toContain('application menu');
  });

  it('the unrunnable remedy leads with BONSAI_GIT_BIN only when that is the source', () => {
    for (const os of ALL_OS) {
      expect(bannerRemedy('unrunnable', os, 'override')).toContain('BONSAI_GIT_BIN');
      expect(bannerRemedy('unrunnable', os, 'registry')).toMatch(/^Reinstall Git/);
    }
    expect(bannerRemedy('unrunnable', 'windows', 'registry')).toContain('Git for Windows');
  });

  it('the disclosure lists an install route per OS, and drops the env row when redundant', () => {
    expect(otherRemedies('notFound', 'windows', 'fallback')).toHaveLength(2);
    expect(otherRemedies('notFound', 'mac', 'fallback')[0]).toContain('xcode-select');
    expect(otherRemedies('notFound', 'linux', 'fallback')[0]).toContain('apt install git');
    // Variant B + override: BONSAI_GIT_BIN is already the headline remedy.
    expect(otherRemedies('unrunnable', 'windows', 'override')).toHaveLength(1);
  });

  it('every string is sentence case and free of banner-forbidden jargon', () => {
    const strings = [
      ...Object.values(BANNER_TITLE),
      ...Object.values(BANNER_EXPLANATION),
      ...ALL_OS.flatMap((os) => [
        bannerRemedy('notFound', os, 'fallback'),
        bannerRemedy('unrunnable', os, 'registry'),
        ...otherRemedies('notFound', os, 'fallback'),
      ]),
    ];
    for (const s of strings) {
      expect(s).not.toMatch(/authentication/i);
      expect(s).not.toMatch(/cached credentials/i);
    }
  });
});

describe('capability rows (ratified decision 5)', () => {
  it('promise SSH keeps working and blame only HTTPS sign-in', () => {
    const works = CAPABILITY_ROWS.find((r) => r.tone === 'works');
    const broken = CAPABILITY_ROWS.find((r) => r.tone === 'broken');
    expect(works?.text).toMatch(/SSH also keep working/);
    // The pre-ratification wording claimed fetch/pull/push were dead — that is
    // false for ssh-agent users and must never come back.
    expect(broken?.text).not.toMatch(/fetch, pull and push/i);
    expect(broken?.text).toMatch(/HTTPS remotes/);
  });
});

describe('technical details', () => {
  it('is omitted when there is nothing truthful to show', () => {
    expect(buildTechnicalDetails(null)).toBeNull();
  });

  it('is the Rust payload plus the resolved rung and path', () => {
    const block = buildTechnicalDetails({ ...MISSING, path: 'C:\\git.exe', source: 'registry' });
    expect(block).toBe('the rust payload\nResolved from: Windows registry\nPath: C:\\git.exe');
    expect(buildTechnicalDetails(MISSING)).toContain('Path: (none)');
  });

  it('maps every rung to a human label', () => {
    const map: Record<GitBinSource, string> = {
      override: 'BONSAI_GIT_BIN',
      path: 'PATH',
      registry: 'Windows registry',
      wellKnown: 'standard install folder',
      fallback: 'not found',
    };
    for (const [source, label] of Object.entries(map)) {
      expect(sourceLabel(source as GitBinSource)).toBe(label);
    }
  });
});

describe('status strings', () => {
  it('zero-pads the checked-at time', () => {
    expect(checkedAtLine(new Date(2026, 7, 19, 9, 4))).toBe('Still not found — checked 09:04.');
  });

  it('names the version in the success toast, and degrades gracefully without one', () => {
    expect(gitAvailableToastText({ ...MISSING, found: true, version: '2.47.1' })).toBe(
      'Git is available again — Bonsai found Git 2.47.1.',
    );
    expect(gitAvailableToastText({ ...MISSING, found: true })).toContain('on this computer');
  });
});

describe('resolveOsFamily — the ?os= harness override (UI §11.3)', () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    window.history.replaceState({}, '', '/');
  });

  it('is INERT outside the mock build, whatever the query string says', () => {
    vi.stubEnv('VITE_MOCK_IPC', '0');
    window.history.replaceState({}, '', '/?os=mac');
    expect(resolveOsFamily()).toBe(osFamily);
  });

  it('accepts exactly the three families under VITE_MOCK_IPC=1', () => {
    vi.stubEnv('VITE_MOCK_IPC', '1');
    for (const os of ALL_OS) {
      window.history.replaceState({}, '', `/?os=${os}`);
      expect(resolveOsFamily()).toBe(os);
    }
  });

  it('falls back to the detected family for junk or an absent value', () => {
    vi.stubEnv('VITE_MOCK_IPC', '1');
    window.history.replaceState({}, '', '/?os=solaris');
    expect(resolveOsFamily()).toBe(osFamily);
    window.history.replaceState({}, '', '/');
    expect(resolveOsFamily()).toBe(osFamily);
  });
});

describe('buildAnnouncement (UI §5.7)', () => {
  const OVERRIDE: GitAvailability = {
    ...MISSING,
    path: 'C:\\Users\\dev\\AppData\\Local\\Programs\\Git\\cmd\\git.exe',
    source: 'override',
  };

  it('composes title + explanation + remedy from the SAME copy the bar renders', () => {
    const a = bannerCopy(MISSING, 'windows');
    expect(buildAnnouncement(a)).toBe(
      'Git is not available. ' +
        "Bonsai couldn't find a runnable git program on this computer. Your saved credentials " +
        'are fine — Bonsai never got as far as checking them. ' +
        'Quit Bonsai and reopen it from the Start menu — an in-app update can leave Bonsai ' +
        'running without your full PATH.',
    );

    const b = bannerCopy(OVERRIDE, 'windows');
    expect(buildAnnouncement(b)).toBe(
      "Git couldn't be started. " +
        "Bonsai found a git program but couldn't run it. Your saved credentials are fine — " +
        'Bonsai never got as far as checking them. ' +
        "BONSAI_GIT_BIN points at a program Bonsai can't run. Correct it or clear it, then " +
        'restart Bonsai.',
    );
  });

  it('is structurally title-first / remedy-last for every variant and OS', () => {
    for (const os of ALL_OS) {
      for (const status of [MISSING, OVERRIDE, { ...OVERRIDE, source: 'registry' as const }]) {
        const copy = bannerCopy(status, os);
        const text = buildAnnouncement(copy);
        expect(text.startsWith(copy.title)).toBe(true);
        expect(text.endsWith(copy.remedy)).toBe(true);
      }
    }
  });

  it('never carries the resolved path — that stays in the technical block', () => {
    const copy = bannerCopy(OVERRIDE, 'windows');
    expect(copy.triedPath).toBe(OVERRIDE.path);
    expect(buildAnnouncement(copy)).not.toContain(OVERRIDE.path);
    expect(buildTechnicalDetails(OVERRIDE)).toContain(OVERRIDE.path);
  });

  it('a null status resolves to the Variant A copy (the latch case)', () => {
    const copy = bannerCopy(null, 'mac');
    expect(copy.variant).toBe('notFound');
    expect(copy.triedPath).toBeNull();
    expect(buildAnnouncement(copy)).toContain('Applications');
  });
});

describe('recovery + retry announcements (UI §5.7)', () => {
  it('names the version, and degrades gracefully without one', () => {
    expect(announceAvailable({ ...MISSING, found: true, version: '2.47.1' })).toBe(
      'Git is available. Bonsai found Git 2.47.1.',
    );
    expect(announceAvailable({ ...MISSING, found: true })).toBe(
      'Git is available. Bonsai found Git on this computer.',
    );
  });

  it('the retry line is short — diagnosis and remedy were already announced', () => {
    expect(ANNOUNCE_STILL_UNAVAILABLE).toBe('Git is still not available.');
  });
});
