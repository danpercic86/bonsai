// P61a: browser-harness twin of the Rust `intraline` pass. Given a mock
// FileDiff, annotate each hunk's PAIRED del/add lines with word-level `spans`
// (code-point ranges) so the "Highlight changes" toggle shows the same emphasis
// in the harness as native. Mirrors crates/bonsai-core/src/git/intraline.rs
// (tokenize -> LCS over token text -> merge touching changed ranges), operating
// on `Array.from` code points so multibyte content shifts offsets correctly.
import type { DiffLine, FileDiff, Hunk } from '../types';

const MAX_INTRALINE_CHARS = 2000;

type Class = 'word' | 'space' | 'other';

function classOf(c: string): Class {
  if (c === '_' || /[\p{L}\p{N}]/u.test(c)) return 'word';
  if (/\s/u.test(c)) return 'space';
  return 'other';
}

interface Tok {
  text: string;
  start: number; // code-point index
  len: number; // code points
}

function tokenize(s: string): Tok[] {
  const chars = Array.from(s);
  const out: Tok[] = [];
  let i = 0;
  while (i < chars.length) {
    const cls = classOf(chars[i]);
    const start = i;
    i += 1;
    if (cls !== 'other') {
      while (i < chars.length && classOf(chars[i]) === cls) i += 1;
    }
    out.push({ text: chars.slice(start, i).join(''), start, len: i - start });
  }
  return out;
}

/** LCS over token TEXT equality; returns (a deleted mask, b inserted mask). */
function lcsMarks(a: Tok[], b: Tok[]): [boolean[], boolean[]] {
  const n = a.length;
  const m = b.length;
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i][j] =
        a[i].text === b[j].text ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const aDel = new Array<boolean>(n).fill(true);
  const bIns = new Array<boolean>(m).fill(true);
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i].text === b[j].text) {
      aDel[i] = false;
      bIns[j] = false;
      i += 1;
      j += 1;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      i += 1;
    } else {
      j += 1;
    }
  }
  return [aDel, bIns];
}

function mergeAdjacent(toks: Tok[], marks: boolean[]): [number, number][] {
  const out: [number, number][] = [];
  toks.forEach((t, k) => {
    if (!marks[k]) return;
    const last = out[out.length - 1];
    if (last !== undefined && last[0] + last[1] === t.start) {
      last[1] += t.len;
    } else {
      out.push([t.start, t.len]);
    }
  });
  return out;
}

function tokenDiff(a: string, b: string): [[number, number][], [number, number][]] {
  const ta = tokenize(a);
  const tb = tokenize(b);
  const [aDel, bIns] = lcsMarks(ta, tb);
  return [mergeAdjacent(ta, aDel), mergeAdjacent(tb, bIns)];
}

function annotateRun(run: DiffLine[]): void {
  const dels: number[] = [];
  const adds: number[] = [];
  run.forEach((l, k) => {
    if (l.kind === 'del') dels.push(k);
    else if (l.kind === 'add') adds.push(k);
  });
  const pairs = Math.min(dels.length, adds.length);
  for (let p = 0; p < pairs; p++) {
    const di = dels[p];
    const ai = adds[p];
    if (
      Array.from(run[di].content).length > MAX_INTRALINE_CHARS ||
      Array.from(run[ai].content).length > MAX_INTRALINE_CHARS
    ) {
      continue;
    }
    const [oldSpans, newSpans] = tokenDiff(run[di].content, run[ai].content);
    run[di].spans = oldSpans;
    run[ai].spans = newSpans;
  }
}

function annotateHunk(hunk: Hunk): void {
  const lines = hunk.lines;
  let i = 0;
  while (i < lines.length) {
    if (lines[i].kind === 'context') {
      i += 1;
      continue;
    }
    const start = i;
    while (i < lines.length && lines[i].kind !== 'context') i += 1;
    // slice() shares the same DiffLine object references, so setting `.spans`
    // mutates the lines in place (the caller passes an already-cloned FileDiff).
    annotateRun(lines.slice(start, i));
  }
}

/** Annotate every hunk's paired del/add lines in place; returns `fd` for
 *  chaining. No-op for binary / too-large diffs (they carry no hunks). */
export function annotateIntraline(fd: FileDiff): FileDiff {
  if (fd.binary || fd.tooLarge) return fd;
  for (const h of fd.hunks) annotateHunk(h);
  return fd;
}
