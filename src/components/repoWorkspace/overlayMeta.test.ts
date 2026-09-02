/** P96: pins the load-bearing PREFIX ORDERING of `deriveOverlayMeta` — the
 *  `conflict:` / `ai-proposal:` / `pr:` checks must run BEFORE the generic
 *  `<section>:<path>` fallback, which blindly casts the prefix to a
 *  WorkdirSection. Each case asserts a field the fallback could NOT produce, so
 *  a reordering regression fails here instead of only in a component test. */
import { describe, expect, it } from 'vitest';
import { deriveOverlayMeta, UNKNOWN_PR_PATH } from './overlayMeta';
import { prSlotKey } from './prSlotKey';
import type { StatusSnapshot } from '../../ipc';

const BASE = 'a'.repeat(40);
const HEAD = 'b'.repeat(40);

const snapshot: StatusSnapshot = {
  staged: [{ path: 'src/app.ts', origPath: 'src/old.ts', status: 'renamed' }],
  unstaged: [{ path: 'src/lib.ts', origPath: null, status: 'modified' }],
  untracked: [],
  conflicted: [],
};

describe('deriveOverlayMeta prefix ordering', () => {
  it('classifies a `conflict:` key as the conflict overlay, not a section lookup', () => {
    const meta = deriveOverlayMeta('conflict:src/app.ts', null, null);
    expect(meta.kind).toBe('conflict');
    // Discriminator: the prefix branch HARDCODES `conflicted`. The section
    // fallback would look `conflict` up in the snapshot and yield null.
    expect(meta.status).toBe('conflicted');
    expect(meta.path).toBe('src/app.ts');
    expect(meta.origPath).toBeNull();
  });

  it('classifies an `ai-proposal:` key as the aiProposal overlay', () => {
    const meta = deriveOverlayMeta('ai-proposal:src/app.ts', null, null);
    // Discriminator: the fallback would cast the prefix verbatim, giving the
    // kebab-case `ai-proposal` rather than the camelCase overlay kind.
    expect(meta.kind).toBe('aiProposal');
    expect(meta.status).toBe('conflicted');
    expect(meta.path).toBe('src/app.ts');
  });

  it('classifies a `pr:` key as the pr overlay and parses the file path out', () => {
    const meta = deriveOverlayMeta(prSlotKey(BASE, HEAD, 'src/app.ts'), null, null);
    expect(meta.kind).toBe('pr');
    // Discriminator: the fallback would take everything after the first colon,
    // i.e. `<base>:<head>:src/app.ts`. Only the prefix branch drops the oids.
    expect(meta.path).toBe('src/app.ts');
    expect(meta.path).not.toContain(BASE);
    expect(meta.status).toBeNull();
  });

  it('prefers the pr ctx side-channel over the key parse', () => {
    const meta = deriveOverlayMeta(prSlotKey(BASE, HEAD, 'src/app.ts'), null, {
      prNumber: 7,
      baseOid: BASE,
      headOid: HEAD,
      path: 'src/renamed.ts',
      origPath: 'src/app.ts',
      status: 'renamed',
    });
    expect(meta).toEqual({
      path: 'src/renamed.ts',
      origPath: 'src/app.ts',
      status: 'renamed',
      kind: 'pr',
    });
  });

  it('degrades a malformed `pr:` key instead of surfacing the raw key', () => {
    // Fewer than three segments after the prefix ⇒ parsePrSlotPath returns null.
    for (const key of [`pr:${BASE}`, 'pr:', `pr:${BASE}:`]) {
      const meta = deriveOverlayMeta(key, null, null);
      expect(meta.kind).toBe('pr');
      expect(meta.status).toBeNull();
      expect(meta.path).toBe(UNKNOWN_PR_PATH);
      expect(meta.path).not.toContain('pr:');
      expect(meta.path).not.toContain(BASE);
    }
  });

  it('still falls back to a snapshot lookup for a plain `<section>:<path>` key', () => {
    expect(deriveOverlayMeta('staged:src/app.ts', snapshot, null)).toEqual({
      path: 'src/app.ts',
      origPath: 'src/old.ts',
      status: 'renamed',
      kind: 'staged',
    });
    expect(deriveOverlayMeta('unstaged:src/lib.ts', snapshot, null)).toEqual({
      path: 'src/lib.ts',
      origPath: null,
      status: 'modified',
      kind: 'unstaged',
    });
  });

  it('degrades a section key whose entry is gone (no badge, path kept)', () => {
    expect(deriveOverlayMeta('staged:src/vanished.ts', snapshot, null)).toEqual({
      path: 'src/vanished.ts',
      origPath: null,
      status: null,
      kind: 'staged',
    });
  });
});
