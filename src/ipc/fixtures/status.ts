// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { StatusSnapshot } from '../types';

/** Exercises every status render path, incl. a file both staged AND modified. */
export const INITIAL_STATUS: StatusSnapshot = {
  staged: [
    { path: 'src/app.rs', origPath: null, status: 'added' },
    // src/main.rs is model-derived (P17 three-way): getStatus appends it to the
    // staged and/or unstaged sections from `state.mainRs`, not this snapshot.
    { path: 'docs/getting-started.md', origPath: 'docs/intro.md', status: 'renamed' },
    { path: 'src/shared/util.rs', origPath: null, status: 'modified' }, // also unstaged below
  ],
  unstaged: [
    { path: 'src/shared/util.rs', origPath: null, status: 'modified' },
    { path: 'README.md', origPath: null, status: 'modified' },
    { path: 'old-config.toml', origPath: null, status: 'deleted' },
    // M4 contract §5: exercise the binary + too-large diff placeholders.
    { path: 'assets/logo.png', origPath: null, status: 'modified' },
    { path: 'data/big-report.csv', origPath: null, status: 'modified' },
  ],
  untracked: [
    { path: 'notes/todo.txt', origPath: null, status: 'untracked' },
    // P3b §3.4: single-child chain — collapses to one "src/git" dir in tree mode.
    { path: 'src/git/status.rs', origPath: null, status: 'untracked' },
    { path: 'scratch.rs', origPath: null, status: 'untracked' },
  ],
  conflicted: [],
};
