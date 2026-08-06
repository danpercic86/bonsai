// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { LineSelection, StatusEntry } from '../types';

export function matchesAny(entry: StatusEntry, paths: string[]): boolean {
  return paths.includes(entry.path) || (entry.origPath !== null && paths.includes(entry.origPath));
}

export function sortByPath(entries: StatusEntry[]): void {
  entries.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
}

/** P17: line-array equality for the three-way model (index vs head/workdir). */
export function linesEqual(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((l, i) => l === b[i]);
}

export const MAIN_RS_PATH = 'src/main.rs';

/** P17: split a wire selection into add (by newNo) / del (by oldNo) sets;
 *  stray Context elements are ignored (kept in both directions). */
export function collectSelection(selection: LineSelection[]): {
  selAdd: Set<number>;
  selDel: Set<number>;
} {
  const selAdd = new Set<number>();
  const selDel = new Set<number>();
  for (const s of selection) {
    if (s.kind === 'add' && s.newNo !== null) selAdd.add(s.newNo);
    else if (s.kind === 'del' && s.oldNo !== null) selDel.add(s.oldNo);
  }
  return { selAdd, selDel };
}

/** Removes matching entries from `from` and returns them. */
export function takeMatching(from: StatusEntry[], paths: string[]): StatusEntry[] {
  const taken = from.filter((e) => matchesAny(e, paths));
  const kept = from.filter((e) => !matchesAny(e, paths));
  from.length = 0;
  from.push(...kept);
  return taken;
}

/** Upserts into `into`, deduping by `path` (new entry wins). */
export function upsert(into: StatusEntry[], entry: StatusEntry): void {
  const idx = into.findIndex((e) => e.path === entry.path);
  if (idx !== -1) into.splice(idx, 1);
  into.push(entry);
}
