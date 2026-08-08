import { Fragment } from 'react';
import type { ReactNode } from 'react';
import type { DiffLine } from '../ipc';
import { segmentLine } from '../utils/intralineSegments';

// P61a: shared word-level (intraline) emphasis rendering for BOTH the unified
// (DiffView) and split (DiffViewSplit) renderers, so emphasis is identical in
// each. The "Highlight changes" toggle renders a CHANGED line's `.diff-content`
// from its backend-computed `spans` (D5: no syntax highlight on that line);
// context lines keep syntax highlighting and never reach here.

/** True when a line should render word-level emphasis: the toggle is on, the
 *  line is add/del (not context), and it carries at least one changed span. */
export function showIntraline(line: DiffLine, intraline: boolean): boolean {
  return intraline && line.kind !== 'context' && (line.spans?.length ?? 0) > 0;
}

/** Render one changed line's content as plain + emphasized segments. Only call
 *  when {@link showIntraline} is true (so `line.kind` is 'add' | 'del'). */
export function intralineNodes(line: DiffLine): ReactNode {
  const cls = line.kind === 'add' ? 'diff-intra diff-intra-add' : 'diff-intra diff-intra-del';
  return segmentLine(line.content, line.spans).map((seg, i) =>
    seg.changed ? (
      <span key={i} className={cls}>
        {seg.text}
      </span>
    ) : (
      <Fragment key={i}>{seg.text}</Fragment>
    ),
  );
}
