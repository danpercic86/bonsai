// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from 'vitest';
import { REGIONS, isRegion, regionOf } from './regions';

describe('regionOf (T-F1)', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('nearest [data-obs-region] ancestor wins', () => {
    document.body.innerHTML =
      '<div data-obs-region="details"><div data-obs-region="diff"><span id="t">x</span></div><b id="d">y</b></div>';
    expect(regionOf(document.getElementById('t'))).toBe('diff');
    expect(regionOf(document.getElementById('d'))).toBe('details');
  });

  it('text nodes resolve via their parent element', () => {
    document.body.innerHTML = '<aside data-obs-region="sidebar"><p id="p">text</p></aside>';
    const text = document.getElementById('p')?.firstChild ?? null;
    expect(regionOf(text)).toBe('sidebar');
  });

  it('no attribute, unknown value, or null → other', () => {
    document.body.innerHTML = '<div id="a"></div><div data-obs-region="C:/x"><i id="b"></i></div>';
    expect(regionOf(document.getElementById('a'))).toBe('other');
    expect(regionOf(document.getElementById('b'))).toBe('other');
    expect(regionOf(null)).toBe('other');
  });

  it('works on a detached subtree', () => {
    const root = document.createElement('main');
    root.setAttribute('data-obs-region', 'graph');
    const child = document.createElement('canvas');
    root.appendChild(child);
    expect(regionOf(child)).toBe('graph');
  });

  it('REGIONS is the closed vocabulary', () => {
    expect([...REGIONS]).toEqual(['sidebar', 'graph', 'details', 'diff', 'dock', 'other']);
    expect(isRegion('graph')).toBe(true);
    expect(isRegion('/etc/passwd')).toBe(false);
  });
});
