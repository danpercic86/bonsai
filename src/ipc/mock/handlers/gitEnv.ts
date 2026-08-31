// P70: the git-preflight mock + its `?git=` / `?gitDelay=` harness seams.
// Split out per the architect's contract (§4.5) and the UI contract (§11).
import { query } from '../repoState';
import type { AppError, GitAvailability } from '../../types';

/** The Rust `git_not_found_message()` Windows text, byte-identical (§3.3). It is
 *  what the banner shows inside TECHNICAL DETAILS, so drift here would hide a
 *  copy regression from the harness. */
export const MOCK_GIT_NOT_FOUND_MESSAGE =
  'Git is not available. Bonsai could not find a runnable `git` executable — it checked ' +
  'BONSAI_GIT_BIN, PATH, the Git for Windows registry key, and the standard install folders. ' +
  'This is NOT an authentication failure: your saved credentials were never consulted, ' +
  'because Bonsai could not start the credential helper. This affects HTTPS remotes (which ' +
  "resolve credentials through Git's credential helper) plus commit search and signing; SSH " +
  'remotes using an ssh-agent are unaffected. Fix: quit Bonsai and relaunch it from the Start ' +
  'menu (an in-app update can leave the app running with an incomplete PATH), or install Git ' +
  'for Windows, or set BONSAI_GIT_BIN to the full path of git.exe and restart.';

/** Which fixture `checkGitAvailability` serves. Read ONCE at module init
 *  (mirrors `AI_OFF` / `?update=`); unknown values fall back to `default`. */
export type GitMockMode = 'default' | 'missing' | 'registry' | 'badpath' | 'longpath';

const GIT_MODE: GitMockMode = ((): GitMockMode => {
  const q = query('git');
  return q === 'missing' || q === 'registry' || q === 'badpath' || q === 'longpath'
    ? q
    : 'default';
})();

/** `?gitDelay=<ms>`, clamped to [0, 10000]; NaN/absent ⇒ 0. Orthogonal to
 *  `?git=` and applied to the mount probe AND every re-check — the only way to
 *  observe `Checking…` and the 400 ms floor in the harness. */
const GIT_DELAY = ((): number => {
  const n = Number.parseInt(query('gitDelay') ?? '', 10);
  if (Number.isNaN(n)) return 0;
  return Math.min(Math.max(n, 0), 10000);
})();

const REGISTRY_PATH = 'C:\\Users\\dev\\AppData\\Local\\Programs\\Git\\cmd\\git.exe';

/** ≥250 chars with one ≥70-char segment, so the single-line ellipsis on the
 *  "Tried:" row is provably exercised (UI §11.1). */
const LONG_PATH =
  'C:\\Users\\dev\\AppData\\Local\\Programs\\' +
  'a-very-long-vendor-directory-name-used-to-prove-single-line-truncation\\' +
  'nested\\deeper\\even-deeper\\still-going\\almost-there\\finally\\here\\' +
  // The contract's literal is 186 chars; these two extra segments carry it past
  // the ≥250 the same section asks for (the ≥70-char segment above is intact).
  'one-more-level-because-real-vendor-trees-nest\\and-another-for-good-measure\\' +
  'Git\\cmd\\git.exe';

/** The not-found copy plus a ≥900-char PATH dump — proves the technical block's
 *  120px scroll and `overflow-wrap: anywhere`. */
const LONG_DETAIL = `${MOCK_GIT_NOT_FOUND_MESSAGE}\nPATH: ${Array.from(
  { length: 12 },
  (_unused, i) => `C:\\Program Files\\vendor-${String(i).padStart(2, '0')}`.padEnd(60, 'x'),
).join(';')}`;

const FIXTURES: Record<GitMockMode, GitAvailability> = {
  default: {
    found: true,
    path: '/usr/bin/git',
    version: '2.47.1',
    source: 'path',
    detail: 'Git 2.47.1 — /usr/bin/git (path)',
  },
  registry: {
    found: true,
    path: REGISTRY_PATH,
    version: '2.47.1.windows.1',
    source: 'registry',
    detail: `Git 2.47.1.windows.1 — ${REGISTRY_PATH} (registry)`,
  },
  missing: {
    found: false,
    path: null,
    version: null,
    source: 'fallback',
    detail: MOCK_GIT_NOT_FOUND_MESSAGE,
  },
  badpath: {
    found: false,
    path: REGISTRY_PATH,
    version: null,
    source: 'override',
    detail: MOCK_GIT_NOT_FOUND_MESSAGE,
  },
  longpath: {
    found: false,
    path: LONG_PATH,
    version: null,
    source: 'wellKnown',
    detail: LONG_DETAIL,
  },
};

/** Reject the way a real HTTPS remote op does when the credential helper cannot
 *  be launched — `?git=missing` only.
 *
 *  DELIBERATELY NOT MODELLED: an SSH remote. Per the backend contract §3.1 an
 *  ssh-agent remote authenticates entirely inside libgit2 and KEEPS WORKING with
 *  git absent, so there is nothing for the banner path to show. Do not "fix"
 *  this mock by rejecting SSH too — that would encode the exact regression P70
 *  was corrected to avoid. */
export function throwIfGitMocksMissing(): void {
  if (GIT_MODE !== 'missing') return;
  const err: AppError = { kind: 'gitNotFound', message: MOCK_GIT_NOT_FOUND_MESSAGE };
  throw err;
}

export const gitEnvHandlers = {
  async checkGitAvailability(): Promise<GitAvailability> {
    if (GIT_DELAY > 0) {
      await new Promise<void>((resolve) => {
        setTimeout(resolve, GIT_DELAY);
      });
    }
    return { ...FIXTURES[GIT_MODE] };
  },
};
