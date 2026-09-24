/** P119 §5.1 — the mock's `TargetArg` / line-funnel mirrors pin the same table
 *  as the Rust T-R2 (`activity_target_arg_tests.rs`), so the two backends agree
 *  on every target the dock can show. */
import { describe, expect, it } from 'vitest';

import {
  mockActivityLine,
  mockCommitTarget,
  mockPathLeaf,
  mockReasonLine,
  mockRefTarget,
  mockStashTarget,
  mockTagTarget,
} from './gitActivityTargetArg';

describe('TargetArg mirrors', () => {
  it('Commit: 40-hex → 7; a rev expression or short input → null', () => {
    expect(mockCommitTarget('9fceb02d0ae598e95dc970b74767f19372d61af8')).toBe('9fceb02');
    expect(mockCommitTarget('HEAD~2')).toBeNull();
    expect(mockCommitTarget('abc')).toBeNull();
  });

  it('Stash: stash@{N}', () => {
    expect(mockStashTarget(2)).toBe('stash@{2}');
  });

  it('PathLeaf: either separator, trailing separators ignored', () => {
    expect(mockPathLeaf('C:\\a\\repo\\')).toBe('repo');
    expect(mockPathLeaf('/a/b/')).toBe('b');
    expect(mockPathLeaf('/a/b')).toBe('b');
    expect(mockPathLeaf('C:/mixed\\sep/leaf')).toBe('leaf');
    expect(mockPathLeaf('single')).toBe('single');
    expect(mockPathLeaf('')).toBeNull();
    expect(mockPathLeaf('///')).toBeNull();
  });

  it('PathLeaf: URL-shaped input is refused outright (credentials never leak)', () => {
    for (const url of [
      'https://tok@host/r.git',
      'https://user:tok@host',
      'https://user:tok@host/',
      'ssh://git@host/r.git',
      'file:///C:/a/repo',
      'git@host',
      '/a/user:tok@host',
    ]) {
      expect(mockPathLeaf(url)).toBeNull();
    }
  });

  it('Ref / Tag strip exactly one prefix', () => {
    expect(mockRefTarget('refs/remotes/origin/main')).toBe('origin/main');
    expect(mockRefTarget('refs/heads/refs/tags/x')).toBe('refs/tags/x');
    expect(mockTagTarget('refs/tags/v1.0.0')).toBe('v1.0.0');
  });
});

describe('line funnels', () => {
  it('an ordinary line has controls stripped (not spaced) and a 2000-char cap', () => {
    expect(mockActivityLine('a\tb\u202ec')).toBe('abc');
    const capped = mockActivityLine('y'.repeat(2001));
    expect([...capped]).toHaveLength(2000);
    expect(capped.endsWith('…')).toBe(true);
  });

  it('the reason line turns every line break into ONE space first', () => {
    expect(mockReasonLine('a\r\nb\nc\rd')).toBe('a b c d');
  });
});
