/**
 * P91 §13 — Developer (Dev-mode / observability) category rows.
 *
 * Groups exactly as rendered: `Dev mode` / `What is captured` /
 * `What Bonsai records` / `Log files`.
 *
 * Two deliberate deviations from §13, forced by the enforced DOM↔catalog guard
 * (`settingsCatalog.coverage.test.tsx`), which the contract's "label = accessible
 * name" claim predates:
 *
 *  1. The two action rows follow the `about.welcome-tour` precedent: the catalog
 *     `label` is the BUTTON text (a button row names itself), and the visible row
 *     title is the `rowLabel` prop at the call site. So `dev.logs` is labelled
 *     `Show in folder` (row title `Log files`) and `dev.delete-logs` is labelled
 *     `Delete all…` (row title `Delete logs and usage counts`). The lost search
 *     vocabulary (`export`, `session`, and the row title's own words) lives in
 *     keywords. NOT `delete all log files`: §6's AC7 retired that phrase from
 *     every one of its five sites, so putting it back here would make search
 *     match copy the app no longer shows.
 *
 *  2. `dev.session-info` and `dev.privacy-note` are control-LESS `group` rows
 *     (a live readout and a prose block); the guard's "a group contains form
 *     controls" sub-assertion is carved out for exactly these two ids.
 */
import type { SettingsIndexEntry } from '../types';
import { resetField } from './reset';

export const DEV_ENTRIES: readonly SettingsIndexEntry[] = [
  {
    id: 'dev.enabled',
    category: 'dev',
    group: 'Dev mode',
    label: 'Dev mode',
    help: 'Records what the app does to a file on this computer, so a problem can be diagnosed after it happens.',
    keywords: 'debug logging diagnostics troubleshoot verbose trace log jsonl',
    control: 'switch',
    reset: resetField('dev', 'enabled', 'Off'),
  },
  {
    id: 'dev.session-info',
    category: 'dev',
    group: 'Dev mode',
    label: 'Logging status',
    keywords: 'status recording session file records flagged',
    control: 'group',
  },
  {
    id: 'dev.level',
    category: 'dev',
    group: 'What is captured',
    label: 'Detail level',
    help: 'Higher levels record more and produce larger files.',
    keywords: 'verbose trace debug info detail level',
    control: 'segmented',
    reset: resetField('dev', 'level', 'Debug'),
  },
  {
    id: 'dev.capture-ipc',
    category: 'dev',
    group: 'What is captured',
    label: 'App requests',
    help: 'Commands sent to the Git engine, their timing and result.',
    keywords: 'ipc requests commands git engine timing',
    control: 'switch',
    reset: resetField('dev', 'captureIpc', 'On'),
  },
  {
    id: 'dev.capture-react',
    category: 'dev',
    group: 'What is captured',
    label: 'Screen updates',
    help: 'Which parts of the window re-render, and why.',
    keywords: 'react render screen updates redraw flicker',
    control: 'switch',
    reset: resetField('dev', 'captureReact', 'On'),
  },
  {
    id: 'dev.capture-frames',
    category: 'dev',
    group: 'What is captured',
    label: 'Frame timing',
    help: 'Drawing performance of the commit graph. Records a lot of data.',
    keywords: 'frame timing performance paint graph',
    control: 'switch',
    reset: resetField('dev', 'captureFrames', 'Off'),
  },
  {
    id: 'dev.include-raw-names',
    category: 'dev',
    group: 'What is captured',
    label: 'Include raw repository names',
    keywords: 'raw names privacy redact anonymous real branch tag repository',
    control: 'switch',
    reset: resetField('dev', 'includeRawNames', 'Off'),
  },
  {
    id: 'dev.privacy-note',
    category: 'dev',
    group: 'What Bonsai records',
    label: 'What Bonsai records',
    keywords: 'privacy contents redact anonymous network upload usage counts metrics retention 90 days',
    control: 'group',
  },
  {
    id: 'dev.logs',
    category: 'dev',
    group: 'Log files',
    label: 'Show in folder',
    keywords: 'export session zip folder reveal show open jsonl report bug',
    control: 'button',
  },
  {
    id: 'dev.delete-logs',
    category: 'dev',
    group: 'Log files',
    label: 'Delete all…',
    keywords:
      'delete remove clear purge erase wipe clean logs usage counts metrics disk space privacy free',
    control: 'button',
  },
];
