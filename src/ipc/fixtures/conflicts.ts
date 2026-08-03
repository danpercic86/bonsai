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

export const MERGE_README_TEXT = [
  '# Bonsai fixture',
  '',
  'Our side kept this README while feature/login deleted it.',
  '',
].join('\n');
