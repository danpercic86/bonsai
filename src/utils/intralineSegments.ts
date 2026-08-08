// P61a: pure helper that splits a diff line's `content` into plain + emphasis
// segments from the backend-computed `spans` (word-level changed ranges). No
// React, no CSS — DiffView / DiffViewSplit consume the result and wrap
// `changed` segments in an emphasis span.
//
// `spans` offsets are CODE POINTS (per the Rust contract), so slicing goes
// through `Array.from(content)` (code-point aware) rather than string indexing
// (which is UTF-16-unit based and would mis-slice astral characters).

export interface LineSegment {
  text: string;
  /** true => this run differs from the paired counterpart (render emphasized). */
  changed: boolean;
}

/**
 * Split `content` on `spans` into alternating unchanged/changed segments.
 *
 * - `spans` absent or empty => the whole line as one unchanged segment.
 * - `spans` are `[startCodePoint, lenCodePoints]`, expected ascending +
 *   non-overlapping; out-of-order / overlapping / out-of-range entries are
 *   clamped defensively so a bad payload can never throw or duplicate text.
 * - Zero-length spans are ignored.
 */
export function segmentLine(content: string, spans?: [number, number][]): LineSegment[] {
  if (spans === undefined || spans.length === 0) {
    return [{ text: content, changed: false }];
  }
  const chars = Array.from(content);
  const n = chars.length;
  const out: LineSegment[] = [];
  let cursor = 0;
  for (const [start, len] of spans) {
    const s = Math.min(Math.max(start, 0), n);
    // A span beginning inside already-emitted territory (out-of-order /
    // overlapping) is skipped rather than remapped, so it can't re-emphasize
    // text that was already placed.
    if (s < cursor) continue;
    const e = Math.min(s + Math.max(0, len), n);
    if (s > cursor) {
      out.push({ text: chars.slice(cursor, s).join(''), changed: false });
    }
    if (e > s) {
      out.push({ text: chars.slice(s, e).join(''), changed: true });
      cursor = e;
    }
  }
  if (cursor < n) {
    out.push({ text: chars.slice(cursor).join(''), changed: false });
  }
  return out;
}
