// P4e Step 2: lazy highlight.js registry. Import ONLY the core + per-language
// modules via dynamic import — never the `highlight.js` barrel (which bundles
// every grammar). Each language module is self-contained, so registration is a
// one-liner with no dependency graph to manage.

import hljs from 'highlight.js/lib/core';
import type { LangId } from './language';

const loaders: Record<LangId, () => Promise<{ default: unknown }>> = {
  typescript: () => import('highlight.js/lib/languages/typescript'),
  javascript: () => import('highlight.js/lib/languages/javascript'),
  json: () => import('highlight.js/lib/languages/json'),
  html: () => import('highlight.js/lib/languages/xml'), // alias
  xml: () => import('highlight.js/lib/languages/xml'),
  css: () => import('highlight.js/lib/languages/css'),
  scss: () => import('highlight.js/lib/languages/scss'),
  markdown: () => import('highlight.js/lib/languages/markdown'),
  rust: () => import('highlight.js/lib/languages/rust'),
  python: () => import('highlight.js/lib/languages/python'),
  csharp: () => import('highlight.js/lib/languages/csharp'),
  java: () => import('highlight.js/lib/languages/java'),
  go: () => import('highlight.js/lib/languages/go'),
  ruby: () => import('highlight.js/lib/languages/ruby'),
  php: () => import('highlight.js/lib/languages/php'),
  bash: () => import('highlight.js/lib/languages/bash'),
  yaml: () => import('highlight.js/lib/languages/yaml'),
  toml: () => import('highlight.js/lib/languages/ini'), // TOML handled by ini grammar
  sql: () => import('highlight.js/lib/languages/sql'),
  c: () => import('highlight.js/lib/languages/c'),
  cpp: () => import('highlight.js/lib/languages/cpp'),
  kotlin: () => import('highlight.js/lib/languages/kotlin'),
  swift: () => import('highlight.js/lib/languages/swift'),
};

const ready = new Set<LangId>();
const inflight = new Map<LangId, Promise<boolean>>();

export function ensureLanguage(id: LangId): Promise<boolean> {
  if (ready.has(id)) return Promise.resolve(true);
  const existing = inflight.get(id);
  if (existing) return existing;
  const p = loaders[id]()
    .then((mod) => {
      // hljs registers under the grammar's canonical name; register under `id`
      // too so highlight(text,{language:id}) resolves for aliases (html->xml).
      if (!hljs.getLanguage(id)) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        hljs.registerLanguage(id, mod.default as any);
      }
      ready.add(id);
      return true;
    })
    .catch(() => false)
    .finally(() => inflight.delete(id));
  inflight.set(id, p);
  return p;
}

/** Returns highlighted HTML (entities escaped by hljs) or null if grammar not
 *  ready. Never throws — highlight is best-effort presentation. */
export function highlightLine(id: LangId, text: string): string | null {
  if (!ready.has(id)) return null;
  try {
    return hljs.highlight(text, { language: id, ignoreIllegals: true }).value;
  } catch {
    return null;
  }
}
