// P74 §"Harness states": a browser-harness seam that makes all four toast tones
// visible at once so the contrast/shape acceptance criteria are AI-gate
// verifiable. Two of the four tones (`warning`, `info`) are otherwise
// unreachable without driving unrelated flows; only `error`
// (?submodule=notEmpty|urlMismatch|fail) and `success` (a successful submodule
// op) can be raised directly today.
//
// This is NOT an IpcApi handler — toasts are App-owned UI state, not IPC data.
// It is a plain function that `Toasts.tsx` reaches through a DYNAMIC import
// inside a mount effect, gated on `import.meta.env.VITE_MOCK_IPC`, so no
// production module ever statically imports mock internals. Pushes go through
// App's real `pushToast`, which means the 5-toast cap, the sticky-error rule and
// the P70 dedupe key are exercised, not bypassed.
//
// MEASURING IN THE HARNESS: only `error` toasts are sticky. `?toasts=demo`
// contains three non-sticky tones (info/success/warning) that auto-dismiss after
// 5 s, so its readings must be taken immediately after load (or from the e2e
// spec, which controls timing). `?toasts=cap` and `?toasts=dedupe` push errors
// only and therefore persist indefinitely.
import type { PushToast } from '../../../ToastContext';
import type { ToastTone } from '../../../components/Toasts';
import { LONG_SUBMODULE_PATH, LONG_SUBMODULE_URL } from '../../fixtures/submodules';
import { query } from '../repoState';
import { msgDirtyWorkdir, msgUrlMismatch } from './submodules';

/** P73's pathological case at full length: the 91-char submodule path fed
 *  through the real failure prefix from `useSubmoduleActions.runRowOp`
 *  (`Couldn't <verb> <name>. `) plus the verbatim `urlMismatch` body. Composed
 *  from the shared helpers so it can never drift from the copy a real refusal
 *  produces. */
const LONG_TEXT = `Couldn't check out ${LONG_SUBMODULE_PATH}. ${msgUrlMismatch(
  LONG_SUBMODULE_PATH,
  LONG_SUBMODULE_URL,
)}`;

const DEMO: ReadonlyArray<readonly [ToastTone, string]> = [
  ['info', 'Already up to date'],
  ['success', 'Checked out vendor/libcore'],
  ['warning', 'Pre-commit hook printed a warning'],
  ['error', `Couldn't check out vendor/libcore. ${msgDirtyWorkdir('vendor/libcore')}`],
];

/** Two pushes sharing one key => a single toast, replaced in place (§10.1). */
const DEDUPE_KEY = 'submodule:vendor/libcore';

type SeamPush = readonly [ToastTone, string, string?];

function seamToasts(seam: string): ReadonlyArray<SeamPush> {
  if (seam === 'demo') return DEMO;
  if (seam === 'long') return [['error', LONG_TEXT]];
  // `cap` pushes six so the §10.1 five-toast cap is observable post-restyle.
  // All six are `error` on purpose: sticky, so the surviving five persist for
  // as long as the measurement takes.
  if (seam === 'cap') {
    return Array.from(
      { length: 6 },
      (_, i) => ['error', `Fetched origin (${i + 1} of 6)`] as const,
    );
  }
  // `dedupe` proves the P70 key path: same key, different text => one toast
  // showing the SECOND text.
  if (seam === 'dedupe') {
    return [
      ['error', "Couldn't check out vendor/libcore. First attempt.", DEDUPE_KEY],
      ['error', "Couldn't check out vendor/libcore. Second attempt (replaced in place).", DEDUPE_KEY],
    ];
  }
  return [];
}

/** Mock-only: replay the `?toasts=demo|long|cap|dedupe` fixture stack once.
 *  Idempotent per page load — the module-level latch survives the StrictMode
 *  mount/unmount/mount cycle (module instances are not re-created), so the
 *  stack is never pushed twice. */
let replayed = false;

export function replayToastSeam(pushToast: PushToast): void {
  if (replayed) return;
  const seam = query('toasts');
  if (!seam) return;
  replayed = true;
  for (const [tone, text, key] of seamToasts(seam)) pushToast(tone, text, key);
}
