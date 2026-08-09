import { describe, expect, it } from 'vitest';

import { buildPathTree, flattenTreeLeaves, type TreeDir, type TreeNode } from './pathTree';

interface Item {
  p: string;
}
const mk = (...paths: string[]): Item[] => paths.map((p) => ({ p }));
const build = (items: Item[], priorityPath?: string): TreeNode<Item>[] =>
  buildPathTree(items, (i) => i.p, priorityPath === undefined ? undefined : { priorityPath });

/** Compact shape helper: dirs as `name[...]`, leaves as `name`. */
function shape(nodes: readonly TreeNode<Item>[]): string {
  return nodes
    .map((n) => (n.kind === 'leaf' ? n.name : `${n.name}[${shape(n.children)}]`))
    .join(',');
}

describe('buildPathTree', () => {
  it('empty input → []', () => {
    expect(build([])).toEqual([]);
  });

  it('flat files → sorted leaves at root', () => {
    expect(shape(build(mk('b.txt', 'a.txt', 'C.txt')))).toBe('a.txt,b.txt,C.txt');
  });

  it('nested paths build dirs; dirs before leaves at each level', () => {
    const t = build(mk('readme.md', 'src/a.ts', 'src/b.ts'));
    expect(shape(t)).toBe('src[a.ts,b.ts],readme.md');
  });

  it('single-child dir chain collapses with joined name and deepest fullPrefix', () => {
    const t = build(mk('src/git/mod.rs', 'src/git/diff.rs'));
    expect(t).toHaveLength(1);
    const d = t[0] as TreeDir<Item>;
    expect(d.kind).toBe('dir');
    expect(d.name).toBe('src/git');
    expect(d.fullPrefix).toBe('src/git');
    expect(shape(d.children)).toBe('diff.rs,mod.rs');
  });

  it('chain does NOT collapse into a dir that has its own leaves', () => {
    const t = build(mk('src/index.ts', 'src/git/mod.rs'));
    expect(shape(t)).toBe('src[git[mod.rs],index.ts]');
    expect((t[0] as TreeDir<Item>).fullPrefix).toBe('src');
  });

  it('multi-level collapse across three segments', () => {
    const t = build(mk('a/b/c/x.ts', 'a/b/c/y.ts'));
    const d = t[0] as TreeDir<Item>;
    expect(d.name).toBe('a/b/c');
    expect(d.fullPrefix).toBe('a/b/c');
  });

  it('sorting is case-insensitive with case-sensitive tiebreak (upper first)', () => {
    expect(shape(build(mk('b', 'a', 'A', 'B')))).toBe('A,a,B,b');
  });

  it('dirs sorted case-insensitively too', () => {
    const t = build(mk('Zeta/x', 'alpha/y', 'Beta/z'));
    expect(shape(t)).toBe('alpha[y],Beta[z],Zeta[x]');
  });

  it('leading/trailing/double slashes are skipped as empty segments', () => {
    const t = build(mk('/src//a.ts', 'src/b.ts/'));
    expect(shape(t)).toBe('src[a.ts,b.ts]');
    // leaf.path keeps the ORIGINAL string
    const dir = t[0] as TreeDir<Item>;
    const paths = dir.children.map((c) => (c.kind === 'leaf' ? c.path : ''));
    expect(paths).toEqual(['/src//a.ts', 'src/b.ts/']);
  });

  it('path of only slashes / empty path is dropped entirely', () => {
    expect(build(mk('', '/', '///'))).toEqual([]);
  });

  it('duplicate paths produce duplicate leaves (documented, not defended)', () => {
    const t = build(mk('a.txt', 'a.txt'));
    expect(t).toHaveLength(2);
  });

  it('priorityPath floats the exact path to the front of its sibling leaves', () => {
    const t = build(mk('src/a.ts', 'src/z.ts', 'src/m.ts'), 'src/z.ts');
    expect(shape(t)).toBe('src[z.ts,a.ts,m.ts]');
  });

  it('priorityPath not matching anything changes nothing', () => {
    const t = build(mk('a', 'b'), 'nope');
    expect(shape(t)).toBe('a,b');
  });

  it('unicode segment names survive and sort deterministically', () => {
    const t = build(mk('ünïcode/файл.txt', 'ünïcode/绘图.svg'));
    const d = t[0] as TreeDir<Item>;
    expect(d.name).toBe('ünïcode');
    expect(d.children.map((c) => c.name)).toEqual(['файл.txt', '绘图.svg']);
  });

  it('large flat input (1000 paths) keeps every leaf', () => {
    const items = mk(...Array.from({ length: 1000 }, (_, i) => `f${i}.txt`));
    expect(flattenTreeLeaves(build(items))).toHaveLength(1000);
  });

  it('deep single path (50 segments) collapses to one dir + one leaf', () => {
    const p = Array.from({ length: 50 }, (_, i) => `d${i}`).join('/') + '/leaf.txt';
    const t = build(mk(p));
    expect(t).toHaveLength(1);
    const d = t[0] as TreeDir<Item>;
    expect(d.children).toHaveLength(1);
    expect(d.children[0].kind).toBe('leaf');
    expect(d.name.split('/')).toHaveLength(50);
  });
});

describe('flattenTreeLeaves', () => {
  it('empty → []', () => {
    expect(flattenTreeLeaves([])).toEqual([]);
  });

  it('yields leaves in pre-order dirs-first visual order', () => {
    const items = mk('readme.md', 'src/z.ts', 'src/a/deep.ts', 'app.ts');
    const flat = flattenTreeLeaves(build(items));
    expect(flat.map((i) => i.p)).toEqual(['src/a/deep.ts', 'src/z.ts', 'app.ts', 'readme.md']);
  });
});
