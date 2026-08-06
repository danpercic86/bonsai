// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import { MERGE_AUTH_OURS, MERGE_AUTH_TEXT, MERGE_AUTH_THEIRS, MERGE_README_TEXT } from '../fixtures/conflicts';
import type { MockRepoState } from './repoState';

/** Seeds (or clears) a repo's paused-op state + conflicted status rows. */
export function seedOpState(state: MockRepoState, op: 'merge' | 'rebase' | null): void {
  state.conflictTexts = new Map();
  if (op === 'rebase') {
    // Pre-seeded paused conflicted rebase at step 2/3 — the "resolve → continue
    // finishes" demo. `rebaseBranch` is the separate clean-rebase demo path.
    state.opState = {
      kind: 'rebase',
      headName: 'feature/topic',
      onto: '00'.repeat(20), // fixture full oid of the onto tip (base row 0's oid)
      currentStep: 2,
      totalSteps: 3,
    };
    state.conflicts = [
      { path: 'src/auth.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
    ];
    state.conflictTexts.set('src/auth.ts', {
      path: 'src/auth.ts',
      kind: 'bothModified',
      binary: false,
      tooLarge: false,
      missing: false,
      text: MERGE_AUTH_TEXT, // reuse the marker fixture
      ours: MERGE_AUTH_OURS,
      theirs: MERGE_AUTH_THEIRS,
    });
    state.status.conflicted = state.conflicts.map((c) => ({
      path: c.path,
      origPath: null,
      status: 'conflicted',
    }));
    return;
  }
  if (op !== 'merge') {
    state.opState = { kind: 'none' };
    state.conflicts = [];
    return;
  }
  state.opState = {
    kind: 'merge',
    incoming: 'feature/login',
    message: "Merge branch 'feature/login'\n\nConflicts:\n\tsrc/auth.ts\n\tREADME.md",
  };
  // Path-ascending, like the backend's list_conflicts.
  state.conflicts = [
    { path: 'README.md', kind: 'deletedByThem', hasBase: true, hasOurs: true, hasTheirs: false },
    { path: 'src/auth.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
  ];
  state.conflictTexts.set('src/auth.ts', {
    path: 'src/auth.ts',
    kind: 'bothModified',
    binary: false,
    tooLarge: false,
    missing: false,
    text: MERGE_AUTH_TEXT,
    ours: MERGE_AUTH_OURS,
    theirs: MERGE_AUTH_THEIRS,
  });
  // deletedByThem: the worktree keeps OUR version (no markers). ours/theirs are
  // editor-irrelevant for this quick-action kind (§0.5) → '' per §1.4.
  state.conflictTexts.set('README.md', {
    path: 'README.md',
    kind: 'deletedByThem',
    binary: false,
    tooLarge: false,
    missing: false,
    text: MERGE_README_TEXT,
    ours: '',
    theirs: '',
  });
  state.status.conflicted = state.conflicts.map((c) => ({
    path: c.path,
    origPath: null,
    status: 'conflicted',
  }));
  // README.md is conflicted, not plain-modified, while the merge is paused.
  state.status.unstaged = state.status.unstaged.filter((e) => e.path !== 'README.md');
}

/** P20 §8.4: seed a paused cherry-pick / revert with one conflicted path
 *  (src/app.ts), reusing the merge-conflict marker fixture. Mirrors the merge
 *  branch of seedOpState so getOpState + listConflicts + the OpBanner render
 *  from the mock with no extra plumbing. */
export function seedPickRevertConflict(state: MockRepoState, kind: 'cherryPick' | 'revert'): void {
  state.opState = { kind };
  state.conflicts = [
    { path: 'src/app.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
  ];
  state.conflictTexts = new Map();
  state.conflictTexts.set('src/app.ts', {
    path: 'src/app.ts',
    kind: 'bothModified',
    binary: false,
    tooLarge: false,
    missing: false,
    text: MERGE_AUTH_TEXT, // reuse the marker fixture
    ours: MERGE_AUTH_OURS,
    theirs: MERGE_AUTH_THEIRS,
  });
  state.status.conflicted = state.conflicts.map((c) => ({
    path: c.path,
    origPath: null,
    status: 'conflicted',
  }));
}
