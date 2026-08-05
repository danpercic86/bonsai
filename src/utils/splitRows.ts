import type { DiffLine, Hunk } from '../ipc';

// P46 Workstream 1: pure, unit-testable pairing of a hunk's unified lines into
// side-by-side rows. No React, no CSS — DiffViewSplit consumes the result and
// DiffView keeps ALL selection state.

/** One row of the side-by-side view. `left` = OLD-side cell, `right` = NEW-side cell.
 *  `null` = filler (empty cell). Both-null never occurs. For a context line the SAME
 *  DiffLine object is placed in both cells (one global index; identity preserved). */
export interface SplitRow {
  left: DiffLine | null;
  right: DiffLine | null;
}

/**
 * Pair a hunk's unified lines into side-by-side rows.
 *
 * Rules:
 *  - `context`  → flush the pending change block, then push `{ left: line, right: line }`
 *    (same object reference in both cells).
 *  - `del`      → buffer into `dels`.
 *  - `add`      → buffer into `adds`.
 *  - At each context boundary AND at end-of-hunk, flush: emit `max(dels,adds)` rows,
 *    `{ left: dels[i] ?? null, right: adds[i] ?? null }`. Surplus dels → `{left, right:null}`,
 *    surplus adds → `{left:null, right}`.
 *  - Empty hunk (`hunk.lines.length === 0`) → returns `[]`.
 * The returned cells reference the EXACT `hunk.lines[*]` objects (no copies), so
 * `globalIndexByLine.get(cell)` resolves via object identity.
 */
export function pairSplitRows(hunk: Hunk): SplitRow[] {
  const out: SplitRow[] = [];
  let dels: DiffLine[] = [];
  let adds: DiffLine[] = [];

  const flush = () => {
    const n = Math.max(dels.length, adds.length);
    for (let i = 0; i < n; i++) {
      out.push({ left: dels[i] ?? null, right: adds[i] ?? null });
    }
    dels = [];
    adds = [];
  };

  for (const line of hunk.lines) {
    if (line.kind === 'del') {
      dels.push(line);
    } else if (line.kind === 'add') {
      adds.push(line);
    } else {
      flush();
      out.push({ left: line, right: line });
    }
  }
  flush();
  return out;
}
