// P4e Step 2: React binding for the lazy highlight registry. Returns a
// per-line highlight function once the grammar is loaded (null before then, so
// callers fall back to plain text and re-render when ready).

import { useEffect, useReducer } from 'react';
import type { LangId } from './language';
import { ensureLanguage, highlightLine } from './highlight';

export function useHighlighter(
  id: LangId | null,
): ((text: string) => string | null) | null {
  const [, force] = useReducer((n: number) => n + 1, 0);
  useEffect(() => {
    if (id === null) return;
    let cancel = false;
    void ensureLanguage(id).then((ok) => {
      if (ok && !cancel) force();
    });
    return () => {
      cancel = true;
    };
  }, [id]);
  if (id === null) return null;
  return (text: string) => highlightLine(id, text);
}
