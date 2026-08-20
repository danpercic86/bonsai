// P74 §"Harness states": a browser-harness seam that makes all four toast tones
// visible at once so the contrast/shape acceptance criteria are AI-gate
// verifiable. Three of the four tones (warning, info) otherwise require driving
// unrelated flows; only `error` (?submodule=notEmpty|urlMismatch|fail) and
// `success` (a successful submodule op) are reachable today.
//
// This is NOT an IpcApi handler — toasts are App-owned UI state, not IPC data —
// so it ships as a one-shot effect mounted by RepoWorkspace. Pushes go through
// App's real `pushToast`, which means the 5-toast cap, the sticky-error rule and
// the P70 dedupe key are exercised, not bypassed.
import { useEffect, useRef } from 'react';
import type { PushToast } from '../../../ToastContext';
import type { ToastTone } from '../../../components/Toasts';
import { query } from '../repoState';

const MOCK = import.meta.env.VITE_MOCK_IPC === '1';

/** P73's pathological string: 91-char path + both URLs, the wrapping check. */
const LONG_TEXT =
  "Couldn't check out vendor/third_party/libcore/experimental/rendering/backends/metal. " +
  "The recorded URL doesn't match the one in .gitmodules " +
  '(https://github.com/example-org/libcore-experimental-rendering.git vs ' +
  'git@github.com:example-org/libcore-experimental-rendering.git). Run Sync URLs, then try again.';

const DEMO: ReadonlyArray<readonly [ToastTone, string]> = [
  ['info', 'Already up to date'],
  ['success', 'Checked out vendor/libcore'],
  ['warning', 'Pre-commit hook printed a warning'],
  [
    'error',
    "Couldn't check out vendor/libcore. The folder already has files in it. " +
      "Move or delete everything inside 'vendor/libcore', then try again.",
  ],
];

function seamToasts(seam: string): ReadonlyArray<readonly [ToastTone, string]> {
  if (seam === 'demo') return DEMO;
  if (seam === 'long') return [['error', LONG_TEXT]];
  // `cap` pushes six so the §10.1 five-toast cap is observable post-restyle.
  if (seam === 'cap') {
    return Array.from({ length: 6 }, (_, i) => ['info', `Fetched origin (${i + 1} of 6)`] as const);
  }
  return [];
}

/** Mock-only: on mount, replay the `?toasts=demo|long|cap` fixture stack. */
export function useMockToastSeam(pushToast: PushToast): void {
  const done = useRef(false);
  useEffect(() => {
    if (!MOCK || done.current) return;
    done.current = true;
    const seam = query('toasts');
    if (!seam) return;
    for (const [tone, text] of seamToasts(seam)) pushToast(tone, text);
  }, [pushToast]);
}
