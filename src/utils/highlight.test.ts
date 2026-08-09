import { describe, expect, it } from 'vitest';

import { ensureLanguage, highlightLine } from './highlight';

// NOTE: `ready` is module-level state shared across tests — order within this
// file matters for the "not ready yet" assertion, so it runs first.

describe('highlight registry', () => {
  it('highlightLine returns null before the grammar is loaded', () => {
    expect(highlightLine('json', '{"a": 1}')).toBeNull();
  });

  it('ensureLanguage resolves true and is idempotent', async () => {
    await expect(ensureLanguage('json')).resolves.toBe(true);
    await expect(ensureLanguage('json')).resolves.toBe(true);
  });

  it('concurrent ensureLanguage calls share one in-flight promise', async () => {
    const p1 = ensureLanguage('typescript');
    const p2 = ensureLanguage('typescript');
    expect(p1).toBe(p2);
    await expect(p1).resolves.toBe(true);
  });

  it('highlightLine returns hljs-marked HTML once ready', async () => {
    await ensureLanguage('json');
    const html = highlightLine('json', '{"key": 123}');
    expect(html).not.toBeNull();
    expect(html).toContain('hljs-');
  });

  it('output has HTML entities escaped (never raw <, >, &)', async () => {
    await ensureLanguage('typescript');
    const html = highlightLine('typescript', 'const a = x < y && y > "<b>";');
    expect(html).not.toBeNull();
    // No raw '<' except as tag starts of hljs spans.
    const stripped = (html as string).replace(/<\/?span[^>]*>/g, '');
    expect(stripped).not.toMatch(/[<>]/);
    expect(stripped).toContain('&lt;');
  });

  it('never throws on adversarial input (empty, huge, unicode, lone surrogate)', async () => {
    await ensureLanguage('typescript');
    expect(highlightLine('typescript', '')).toBe('');
    expect(typeof highlightLine('typescript', 'x'.repeat(10_000))).toBe('string');
    expect(typeof highlightLine('typescript', 'const 名前 = "🌳";')).toBe('string');
    expect(typeof highlightLine('typescript', '\uD800 broken')).toBe('string');
  });

  it('html alias registers under "html" and highlights markup', async () => {
    await expect(ensureLanguage('html')).resolves.toBe(true);
    const html = highlightLine('html', '<div class="x">hi</div>');
    expect(html).toContain('hljs-');
  });
});
