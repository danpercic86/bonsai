// Conflict/merge sample texts for the browser-harness mock (P12 §1.4).
// Static blobs only: the stateful mock (seedOpState / seedPickRevertConflict)
// wires these into paused merge/rebase/cherry-pick states.

export const MERGE_AUTH_TEXT = [
  'import { hash } from "./crypto";',
  '',
  'export interface Session {',
  '  user: string;',
  '  token: string;',
  '}',
  '',
  'export function login(user: string, password: string): Session {',
  '<<<<<<< HEAD',
  '  const token = hash(`${user}:${password}:v2`);',
  '  return { user, token };',
  '=======',
  '  const token = hash(password + user);',
  '  return { user: user.toLowerCase(), token };',
  '>>>>>>> feature/login',
  '}',
  '',
  'export function logout(session: Session): void {',
  '  void session;',
  '}',
  '',
].join('\n');

// P12 §1.4: the OURS / THEIRS blob sides for MERGE_AUTH_TEXT's single conflict
// region — the file with the region collapsed to its ours / theirs block
// (markers removed). Hand-written to match MERGE_AUTH_TEXT above.
export const MERGE_AUTH_OURS = [
  'import { hash } from "./crypto";',
  '',
  'export interface Session {',
  '  user: string;',
  '  token: string;',
  '}',
  '',
  'export function login(user: string, password: string): Session {',
  '  const token = hash(`${user}:${password}:v2`);',
  '  return { user, token };',
  '}',
  '',
  'export function logout(session: Session): void {',
  '  void session;',
  '}',
  '',
].join('\n');

export const MERGE_AUTH_THEIRS = [
  'import { hash } from "./crypto";',
  '',
  'export interface Session {',
  '  user: string;',
  '  token: string;',
  '}',
  '',
  'export function login(user: string, password: string): Session {',
  '  const token = hash(password + user);',
  '  return { user: user.toLowerCase(), token };',
  '}',
  '',
  'export function logout(session: Session): void {',
  '  void session;',
  '}',
  '',
].join('\n');

/**
 * P68d: a DEEP, long conflicted path for the merge fixture.
 *
 * Two jobs: (1) the paused merge now has TWO text-mergeable (`bothModified`)
 * conflicts, which is the minimum for the item-5 scenario ("start on file A, switch
 * to file B, come back") and for P68f's "Resolve all with AI" to appear at all;
 * (2) it is long enough to exercise path truncation in the dock header and the run
 * queue, which a 12-character `src/auth.ts` never could. It is also an i18n JSON
 * file on purpose — the user's actual item-6 repro.
 */
export const MERGE_DEEP_PATH =
  'src/features/internationalization/locales/de-DE/components/settings/advanced/notifications/messages.json';

export const MERGE_DEEP_TEXT = [
  '{',
  '  "notifications": {',
  '    "title": "Benachrichtigungen",',
  '<<<<<<< HEAD',
  '    "unreadCount": "{{count}} ungelesene Einträge",',
  '    "markAllRead": "Alle als gelesen markieren"',
  '=======',
  '    "unreadCount": "{{count}} ungelesene Eintraege",',
  '    "markAllRead": "Alles als gelesen markieren"',
  '>>>>>>> feature/login',
  '  }',
  '}',
  '',
].join('\n');

export const MERGE_DEEP_OURS = [
  '{',
  '  "notifications": {',
  '    "title": "Benachrichtigungen",',
  '    "unreadCount": "{{count}} ungelesene Einträge",',
  '    "markAllRead": "Alle als gelesen markieren"',
  '  }',
  '}',
  '',
].join('\n');

export const MERGE_DEEP_THEIRS = [
  '{',
  '  "notifications": {',
  '    "title": "Benachrichtigungen",',
  '    "unreadCount": "{{count}} ungelesene Eintraege",',
  '    "markAllRead": "Alles als gelesen markieren"',
  '  }',
  '}',
  '',
].join('\n');

export const MERGE_README_TEXT = [
  '# Bonsai fixture',
  '',
  'Our side kept this README while feature/login deleted it.',
  '',
].join('\n');
