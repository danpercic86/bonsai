// P12 §2.1: pure helpers for parsing and rewriting Git conflict regions in a
// document. No React, no CodeMirror, no ipc — this module is fully testable and
// drives the ConflictEditor (P12b) plus the per-region widgets (P12c) and the
// side-by-side MergeView (P12d).

/** Marker detection: a `<<<<<<<`, `=======`, or `>>>>>>>` run of 7 at line start
 *  (mirrors DiffOverlay's `MARKER_RE`). */
const MARKER_RE = /^(<{7}|={7}|>{7})/;
const START_RE = /^<{7}/;
const SEP_RE = /^={7}/;
const END_RE = /^>{7}/;

/** One <<<<<<< ======= >>>>>>> block, located by line index within a document.
 *  Line indices are 0-based into `text.split('\n')`. Always parse FRESH from the
 *  current document before acting — indices shift as regions are resolved. */
export interface ConflictRegion {
  /** 0-based order within the file (0 = first region). */
  index: number;
  /** Line index of the `<<<<<<<` marker. */
  startLine: number;
  /** Line index of the `=======` separator. */
  sepLine: number;
  /** Line index of the `>>>>>>>` marker. */
  endLine: number;
  /** Text after `<<<<<<< ` on the start line (e.g. "HEAD"); '' if none. */
  oursLabel: string;
  /** Text after `>>>>>>> ` on the end line (e.g. "feature/login"); '' if none. */
  theirsLabel: string;
  /** Lines strictly between startLine and sepLine (the OURS body). */
  oursLines: string[];
  /** Lines strictly between sepLine and endLine (the THEIRS body). */
  theirsLines: string[];
}

/** Extract the label after a marker on a line (`<<<<<<< HEAD` → `HEAD`). The
 *  marker run is 7 chars; a single following space is the conventional
 *  separator, so trim leading whitespace before the label. */
function markerLabel(line: string): string {
  return line.slice(7).replace(/^\s+/, '');
}

/** Parse every well-formed conflict region in `text`. A region requires a
 *  `<<<<<<<` line, a later `=======` line, then a `>>>>>>>` line, in order, with
 *  no nested `<<<<<<<` between them. Malformed/partial markers are skipped (never
 *  throw). Regions are returned in document order with sequential `index`. */
export function parseConflictRegions(text: string): ConflictRegion[] {
  const lines = text.split('\n');
  const regions: ConflictRegion[] = [];

  // State machine: seeking-start -> seeking-sep -> seeking-end.
  let startLine = -1;
  let sepLine = -1;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (startLine === -1) {
      // seeking-start: only a `<<<<<<<` opens a region; stray `=`/`>` ignored.
      if (START_RE.test(line)) startLine = i;
      continue;
    }
    if (sepLine === -1) {
      // seeking-sep: a second `<<<<<<<` abandons the partial and restarts here.
      if (START_RE.test(line)) {
        startLine = i;
      } else if (SEP_RE.test(line)) {
        sepLine = i;
      }
      continue;
    }
    // seeking-end.
    if (START_RE.test(line)) {
      // A new start before an end abandons the partial and restarts.
      startLine = i;
      sepLine = -1;
      continue;
    }
    if (END_RE.test(line)) {
      regions.push({
        index: regions.length,
        startLine,
        sepLine,
        endLine: i,
        oursLabel: markerLabel(lines[startLine]),
        theirsLabel: markerLabel(line),
        oursLines: lines.slice(startLine + 1, sepLine),
        theirsLines: lines.slice(sepLine + 1, i),
      });
      startLine = -1;
      sepLine = -1;
    }
    // Any other line inside the theirs body is body content — keep seeking-end.
  }

  return regions;
}

/** Return a NEW document with `region`'s block (startLine..endLine inclusive)
 *  replaced by the chosen body:
 *   - 'ours'   -> region.oursLines
 *   - 'theirs' -> region.theirsLines
 *   - 'both'   -> region.oursLines followed by region.theirsLines (ours-block
 *                 THEN theirs-block, matching git marker order — §0.4)
 *  `region` MUST have been parsed from the SAME `text` passed here (indices are
 *  used directly). Preserves all other lines and the doc's trailing newline. */
export function applyResolution(
  text: string,
  region: ConflictRegion,
  choice: 'ours' | 'theirs' | 'both',
): string {
  const lines = text.split('\n');
  const body =
    choice === 'ours'
      ? region.oursLines
      : choice === 'theirs'
        ? region.theirsLines
        : [...region.oursLines, ...region.theirsLines];
  const next = [
    ...lines.slice(0, region.startLine),
    ...body,
    ...lines.slice(region.endLine + 1),
  ];
  return next.join('\n');
}

/** True if `text` still contains any conflict marker line (`<<<<<<<`, `=======`,
 *  or `>>>>>>>` at line start). Drives the Save/Stage-resolved gate. */
export function hasUnresolvedMarkers(text: string): boolean {
  return text.split('\n').some((line) => MARKER_RE.test(line));
}
