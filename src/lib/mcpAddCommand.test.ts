import { describe, expect, it } from 'vitest';

import { buildClaudeAddCommand } from './mcpAddCommand';

const URL = 'http://127.0.0.1:8765/mcp';
const TOKEN = 'tok-abc123';

describe('buildClaudeAddCommand', () => {
  it('places the URL before the variadic --header (the ordering fix)', () => {
    const cmd = buildClaudeAddCommand({ url: URL, token: TOKEN, scope: 'user' });
    const urlAt = cmd.indexOf(URL);
    const headerAt = cmd.indexOf('--header');
    expect(urlAt).toBeGreaterThanOrEqual(0);
    expect(headerAt).toBeGreaterThanOrEqual(0);
    expect(urlAt).toBeLessThan(headerAt);
    // The server name also precedes --header.
    expect(cmd.indexOf('bonsai')).toBeLessThan(headerAt);
  });

  it('user scope has no cd prefix even when a repoPath is supplied', () => {
    const cmd = buildClaudeAddCommand({
      url: URL,
      token: TOKEN,
      scope: 'user',
      repoPath: 'D:/Repos/thing',
    });
    expect(cmd.startsWith('claude mcp add')).toBe(true);
    expect(cmd).toContain('--scope user');
    expect(cmd).not.toContain('cd ');
  });

  it('local scope with a repoPath is cd-prefixed to target the open repo', () => {
    const repoPath = 'D:/Repos/thing';
    const cmd = buildClaudeAddCommand({ url: URL, token: TOKEN, scope: 'local', repoPath });
    // `;` (not `&&`) so the pasted command works in Windows PowerShell 5.1.
    expect(cmd.startsWith(`cd "${repoPath}"; claude mcp add`)).toBe(true);
    expect(cmd).not.toContain('&&');
    expect(cmd).toContain('--scope local');
  });

  it('local scope without a repoPath falls back to no cd prefix', () => {
    const cmd = buildClaudeAddCommand({ url: URL, token: TOKEN, scope: 'local', repoPath: null });
    expect(cmd.startsWith('claude mcp add')).toBe(true);
    expect(cmd).not.toContain('cd ');
  });
});
