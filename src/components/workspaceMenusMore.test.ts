// T3.1 — workspaceMenus part 2: externalToolsItems, stash/submodule/worktree/
// tag/remote menus, and the clipboard-copy toast wiring. Part 1 (branch/reset/
// commit/context routing) lives in workspaceMenus.test.ts.
import { afterEach, describe, expect, it, vi } from 'vitest';

import { createWorkspaceMenus, externalToolsItems } from './workspaceMenus';
import type { ExternalToolsHandlers } from './workspaceMenus';
import {
  OID_OTHER,
  itemByLabel,
  labelsOf,
  makeDeps,
  makeSubmodule,
  makeWorktree,
} from '../test/workspaceMenusFixtures';

const EXT_LABELS = ['Open in terminal', 'Reveal in file manager', 'Open in editor'];

describe('externalToolsItems (standalone)', () => {
  it('builds the always-enabled trio and passes the path to each handler', () => {
    const h: ExternalToolsHandlers = {
      onOpenInTerminal: vi.fn(),
      onRevealInFileManager: vi.fn(),
      onOpenInEditor: vi.fn(),
    };
    const items = externalToolsItems('C:/repo', h);
    expect(labelsOf(items)).toEqual(EXT_LABELS);
    expect(items.every((i) => i.disabled === false)).toBe(true);
    items.forEach((i) => i.onSelect?.());
    expect(h.onOpenInTerminal).toHaveBeenCalledWith('C:/repo');
    expect(h.onRevealInFileManager).toHaveBeenCalledWith('C:/repo');
    expect(h.onOpenInEditor).toHaveBeenCalledWith('C:/repo');
  });

  it('the factory-bound variant uses the deps handlers', () => {
    const deps = makeDeps();
    const items = createWorkspaceMenus(deps).externalToolsItems('/x');
    itemByLabel(items, 'Open in editor').onSelect?.();
    expect(deps.onOpenInEditor).toHaveBeenCalledWith('/x');
  });
});

describe('stashMenuItems', () => {
  it('Apply/Pop/Drop wired to the index', () => {
    const deps = makeDeps();
    const items = createWorkspaceMenus(deps).stashMenuItems(3);
    expect(labelsOf(items)).toEqual(['Apply', 'Pop', 'Drop']);
    itemByLabel(items, 'Apply').onSelect?.();
    itemByLabel(items, 'Pop').onSelect?.();
    itemByLabel(items, 'Drop').onSelect?.();
    expect(deps.handleApplyStash).toHaveBeenCalledWith(3);
    expect(deps.handlePopStash).toHaveBeenCalledWith(3);
    expect(deps.setPendingDropStash).toHaveBeenCalledWith(3);
  });

  it('opActive gates Apply/Pop but NOT Drop; mutating gates all three', () => {
    const op = createWorkspaceMenus(makeDeps({ opActive: true })).stashMenuItems(0);
    expect(op.map((i) => i.disabled)).toEqual([true, true, false]);
    const mut = createWorkspaceMenus(makeDeps({ mutating: true })).stashMenuItems(0);
    expect(mut.map((i) => i.disabled)).toEqual([true, true, true]);
  });

  it('index 0 (the newest stash) is passed through unmangled', () => {
    const deps = makeDeps();
    itemByLabel(createWorkspaceMenus(deps).stashMenuItems(0), 'Apply').onSelect?.();
    expect(deps.handleApplyStash).toHaveBeenCalledWith(0);
  });
});

describe('submoduleMenuItems', () => {
  it('initialized submodule: Update live, "Initialize and check out" dead, open-in-tab enabled', () => {
    const deps = makeDeps();
    const items = createWorkspaceMenus(deps).submoduleMenuItems(makeSubmodule());
    expect(labelsOf(items)).toEqual([
      'Initialize and check out',
      'Update',
      'Sync',
      'Deinitialize…',
      'Remove…',
      'Open in new tab',
      ...EXT_LABELS,
    ]);
    // P73 §3.2: mutually exclusive — files are already on disk.
    expect(itemByLabel(items, 'Initialize and check out').disabled).toBe(true);
    expect(itemByLabel(items, 'Update').disabled).toBe(false);
    expect(itemByLabel(items, 'Deinitialize…').disabled).toBe(false);
    expect(itemByLabel(items, 'Open in new tab').disabled).toBe(false);
    expect(itemByLabel(items, 'Remove…').tone).toBe('danger');
    itemByLabel(items, 'Open in new tab').onSelect?.();
    expect(deps.onOpenRepoPath).toHaveBeenCalledWith('/repo/libs/dep');
  });

  it('uninitialized submodule flips every gate: init enabled, Update + Deinit + open-in-tab disabled', () => {
    const items = createWorkspaceMenus(makeDeps()).submoduleMenuItems(
      makeSubmodule({ status: 'uninitialized', wtOid: null }),
    );
    expect(itemByLabel(items, 'Initialize and check out').disabled).toBe(false);
    // P73 §3.2: Update is the exact inverse gate (no row to update yet).
    expect(itemByLabel(items, 'Update').disabled).toBe(true);
    expect(itemByLabel(items, 'Deinitialize…').disabled).toBe(true);
    expect(itemByLabel(items, 'Open in new tab').disabled).toBe(true);
  });

  it('gate disables git mutations but never the external trio', () => {
    const items = createWorkspaceMenus(makeDeps({ opActive: true })).submoduleMenuItems(
      makeSubmodule(),
    );
    expect(itemByLabel(items, 'Update').disabled).toBe(true);
    expect(itemByLabel(items, 'Initialize and check out').disabled).toBe(true);
    expect(itemByLabel(items, 'Sync').disabled).toBe(true);
    expect(itemByLabel(items, 'Remove…').disabled).toBe(true);
    for (const l of EXT_LABELS) expect(itemByLabel(items, l).disabled).toBe(false);
  });

  it('handlers receive the submodule NAME (stable key), not the path', () => {
    const deps = makeDeps();
    const sub = makeSubmodule({ name: 'the-name', path: 'other/path' });
    const items = createWorkspaceMenus(deps).submoduleMenuItems(sub);
    itemByLabel(items, 'Update').onSelect?.();
    expect(deps.handleUpdateSubmodule).toHaveBeenCalledWith('the-name');
    itemByLabel(items, 'Remove…').onSelect?.();
    expect(deps.setPendingRemoveSubmodule).toHaveBeenCalledWith('the-name');
  });
});

describe('worktreeMenuItems', () => {
  it('linked, unlocked, valid worktree: everything available', () => {
    const deps = makeDeps();
    const items = createWorkspaceMenus(deps).worktreeMenuItems(makeWorktree());
    expect(labelsOf(items)).toEqual([
      'Open in new tab',
      'AI context…',
      'Lock…',
      'Unlock',
      'Remove…',
      ...EXT_LABELS,
    ]);
    expect(itemByLabel(items, 'Open in new tab').disabled).toBe(false);
    expect(itemByLabel(items, 'Lock…').disabled).toBe(false);
    expect(itemByLabel(items, 'Unlock').disabled).toBe(true); // not locked
    expect(itemByLabel(items, 'Remove…').disabled).toBe(false);
    itemByLabel(items, 'Remove…').onSelect?.();
    expect(deps.setPendingWorktreeRemove).toHaveBeenCalledWith({
      name: 'wt1',
      absPath: '/repo/.worktrees/wt1',
    });
  });

  it('main worktree: Lock and Remove disabled', () => {
    const items = createWorkspaceMenus(makeDeps()).worktreeMenuItems(makeWorktree({ isMain: true }));
    expect(itemByLabel(items, 'Lock…').disabled).toBe(true);
    expect(itemByLabel(items, 'Remove…').disabled).toBe(true);
  });

  it('locked worktree: Lock disabled, Unlock enabled, Remove disabled', () => {
    const deps = makeDeps();
    const items = createWorkspaceMenus(deps).worktreeMenuItems(makeWorktree({ locked: true }));
    expect(itemByLabel(items, 'Lock…').disabled).toBe(true);
    expect(itemByLabel(items, 'Unlock').disabled).toBe(false);
    expect(itemByLabel(items, 'Remove…').disabled).toBe(true);
    itemByLabel(items, 'Unlock').onSelect?.();
    expect(deps.handleUnlockWorktree).toHaveBeenCalledWith('wt1');
  });

  it('current or invalid worktree: open-in-tab disabled; current also blocks Remove', () => {
    const cur = createWorkspaceMenus(makeDeps()).worktreeMenuItems(makeWorktree({ isCurrent: true }));
    expect(itemByLabel(cur, 'Open in new tab').disabled).toBe(true);
    expect(itemByLabel(cur, 'Remove…').disabled).toBe(true);
    const invalid = createWorkspaceMenus(makeDeps()).worktreeMenuItems(makeWorktree({ valid: false }));
    expect(itemByLabel(invalid, 'Open in new tab').disabled).toBe(true);
  });

  it('AI context… is never gated, even mid-operation', () => {
    const deps = makeDeps({ mutating: true, opActive: true });
    const items = createWorkspaceMenus(deps).worktreeMenuItems(makeWorktree());
    const ai = itemByLabel(items, 'AI context…');
    expect(ai.disabled).toBe(false);
    ai.onSelect?.();
    expect(deps.setWorktreeContextOpen).toHaveBeenCalledWith(true);
    for (const l of EXT_LABELS) expect(itemByLabel(items, l).disabled).toBe(false);
  });
});

describe('tagMenuItems', () => {
  it('sidebar row (oid null): delete/copy/release-notes/push only — no commit actions', () => {
    const items = createWorkspaceMenus(makeDeps()).tagMenuItems('v1.0', null);
    expect(labelsOf(items)).toEqual([
      'Delete tag',
      'Copy tag name',
      'Release notes since previous tag',
      'Push tag to origin',
    ]);
  });

  it('graph pill (oid set): commit actions appended after the tag items', () => {
    const items = createWorkspaceMenus(makeDeps()).tagMenuItems('v1.0', OID_OTHER);
    expect(labelsOf(items)).toEqual([
      'Delete tag',
      'Copy tag name',
      'Release notes since previous tag',
      'Push tag to origin',
      'Create branch here',
      'Create tag here',
      'Compare with HEAD',
      'Explain this commit',
      'Cherry-pick onto current…',
      'Revert commit',
    ]);
  });

  it('one push item per remote: 0 remotes → none; 2 remotes → two', () => {
    const zero = createWorkspaceMenus(makeDeps({ remotes: [] })).tagMenuItems('t', null);
    expect(labelsOf(zero)).not.toContain('Push tag to origin');
    const deps = makeDeps({
      remotes: [
        { name: 'origin', url: 'u1' },
        { name: 'fork', url: 'u2' },
      ],
    });
    const two = createWorkspaceMenus(deps).tagMenuItems('v1.0', null);
    expect(labelsOf(two)).toContain('Push tag to origin');
    expect(labelsOf(two)).toContain('Push tag to fork');
    itemByLabel(two, 'Push tag to fork').onSelect?.();
    expect(deps.handlePushTag).toHaveBeenCalledWith('fork', 'v1.0');
  });

  it('release notes disabled unless aiEligible; fires runChangelog with sinceLastTag', () => {
    const off = createWorkspaceMenus(makeDeps()).tagMenuItems('v1.0', null);
    expect(itemByLabel(off, 'Release notes since previous tag').disabled).toBe(true);
    const deps = makeDeps({ aiEligible: true });
    const on = createWorkspaceMenus(deps).tagMenuItems('v1.0', null);
    const notes = itemByLabel(on, 'Release notes since previous tag');
    expect(notes.disabled).toBe(false);
    notes.onSelect?.();
    expect(deps.runChangelog).toHaveBeenCalledWith(
      { kind: 'sinceLastTag', target: 'v1.0' },
      'Release notes for v1.0',
    );
  });

  it('gate disables delete/push but not copy', () => {
    const items = createWorkspaceMenus(makeDeps({ mutating: true })).tagMenuItems('v1.0', null);
    expect(itemByLabel(items, 'Delete tag').disabled).toBe(true);
    expect(itemByLabel(items, 'Push tag to origin').disabled).toBe(true);
    expect(itemByLabel(items, 'Copy tag name').disabled).toBe(false);
  });
});

describe('remoteMenuItems', () => {
  it('Rename / Edit URL / Remove wired with the resolved URL', () => {
    const deps = makeDeps();
    const items = createWorkspaceMenus(deps).remoteMenuItems('origin');
    expect(labelsOf(items)).toEqual(['Rename…', 'Edit URL…', 'Remove…']);
    itemByLabel(items, 'Rename…').onSelect?.();
    expect(deps.setPendingRenameRemote).toHaveBeenCalledWith({ name: 'origin' });
    itemByLabel(items, 'Edit URL…').onSelect?.();
    expect(deps.setPendingEditUrl).toHaveBeenCalledWith({
      name: 'origin',
      url: 'https://example.com/r.git',
    });
    itemByLabel(items, 'Remove…').onSelect?.();
    expect(deps.setPendingRemoveRemote).toHaveBeenCalledWith('origin');
  });

  it('unknown remote or null URL falls back to empty string', () => {
    const deps = makeDeps({ remotes: [{ name: 'origin', url: null }] });
    const menus = createWorkspaceMenus(deps);
    itemByLabel(menus.remoteMenuItems('origin'), 'Edit URL…').onSelect?.();
    expect(deps.setPendingEditUrl).toHaveBeenLastCalledWith({ name: 'origin', url: '' });
    itemByLabel(menus.remoteMenuItems('ghost'), 'Edit URL…').onSelect?.();
    expect(deps.setPendingEditUrl).toHaveBeenLastCalledWith({ name: 'ghost', url: '' });
  });

  it('all three are gated while mutating', () => {
    const items = createWorkspaceMenus(makeDeps({ mutating: true })).remoteMenuItems('origin');
    expect(items.map((i) => i.disabled)).toEqual([true, true, true]);
  });
});

describe('clipboard copy wiring (branch + tag)', () => {
  afterEach(() => vi.unstubAllGlobals());
  const flush = () => new Promise<void>((r) => setTimeout(r, 0));

  it('successful write → success toast', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal('navigator', { clipboard: { writeText } });
    const deps = makeDeps();
    const items = createWorkspaceMenus(deps).branchMenuItems('feature', 'localBranch');
    itemByLabel(items, 'Copy branch name').onSelect?.();
    await flush();
    expect(writeText).toHaveBeenCalledWith('feature');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Copied branch name');
  });

  it('rejected write → error toast with the failure message', async () => {
    vi.stubGlobal('navigator', {
      clipboard: { writeText: vi.fn().mockRejectedValue(new Error('denied')) },
    });
    const deps = makeDeps();
    itemByLabel(
      createWorkspaceMenus(deps).tagMenuItems('v1.0', null),
      'Copy tag name',
    ).onSelect?.();
    await flush();
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'Copy failed: denied');
  });

  it('missing clipboard API → "Clipboard unavailable" error toast (no throw)', async () => {
    vi.stubGlobal('navigator', {});
    const deps = makeDeps();
    itemByLabel(
      createWorkspaceMenus(deps).branchMenuItems('feature', 'localBranch'),
      'Copy branch name',
    ).onSelect?.();
    await flush();
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'Copy failed: Clipboard unavailable');
  });
});
