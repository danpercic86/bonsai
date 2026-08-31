// M1 (graph-design-review 2026-08-22, ui-reference §4.1). The commit graph is a
// canvas — opaque to assistive tech — so the settled selection is announced via a
// polite live region. Reuses the P84 `RevealAnnouncer` live-region component (a
// sibling `role="status"` region dedicated to graph-grid selection, distinct from
// the sidebar-reveal announcer).
//
// Copy: `{summary} — {author}, {relative date}. Row {n+1} of {N}. {ref summary}`,
// debounced ~150 ms so a held arrow key does not flood the reader.

import { useEffect, useState } from 'react';
import { RevealAnnouncer } from './RevealAnnouncer';
import type { GraphLayout } from '../ipc';
import { relativeDate } from '../graph/dates';
import { rowForgeSignal } from '../graph/forgeBadges';
import type { CheckRollup } from '../ipc';
import type { GraphDisplayOptions } from '../graph/rightColumns';

/** SR words for each CI rollup — plain language, never the raw enum. `none` maps
 *  to '' because `ciBadgeVisual('none')` draws nothing, so nothing is announced. */
const CI_ROLLUP_WORDS: Record<CheckRollup, string> = {
  success: 'passed',
  failure: 'failing',
  error: 'failing',
  pending: 'pending',
  neutral: 'neutral',
  none: '',
};

export interface GraphSelectionAnnouncerProps {
  graph: GraphLayout | null;
  selectedIndex: number | null;
  /** PR-badge-placement §6: per-row display toggles/maps, so the settled
   *  announcement can carry the forge signal (canvas pill colour has no SR
   *  equivalent). Optional — omitted ⇒ no forge suffix. */
  display?: GraphDisplayOptions;
}

/** Short human summary of a node's refs for the announcement, or '' when none. */
function refSummary(refs: GraphLayout['nodes'][number]['refs']): string {
  if (refs === undefined || refs.length === 0) return '';
  const names = refs.map((r) => r.name);
  return `Refs: ${names.join(', ')}.`;
}

/** PR-badge-placement §6: the forge suffix (`" PR #{n} {state}." " Checks {r}."`)
 *  for the selected row, or '' when the row carries no forge signal / no display. */
function forgeSummary(
  node: GraphLayout['nodes'][number],
  display: GraphDisplayOptions | undefined,
): string {
  if (display === undefined) return '';
  const signal = rowForgeSignal(node.refs, node, display);
  if (signal === null) return '';
  let out = '';
  if (signal.pr !== null) {
    const state = signal.pr.isDraft ? 'draft' : signal.pr.state;
    out += ` PR #${signal.pr.number} ${state}.`;
  }
  if (signal.ci !== null) {
    const word = CI_ROLLUP_WORDS[signal.ci.rollup];
    // Suppress the 'none' rollup entirely — nothing is drawn, so nothing is said.
    if (word !== '') out += ` Checks ${word}.`;
  }
  return out;
}

/** Builds the settled-selection announcement, or '' when nothing is selected. */
export function selectionMessage(
  graph: GraphLayout | null,
  selectedIndex: number | null,
  display?: GraphDisplayOptions,
): string {
  if (graph === null || selectedIndex === null) return '';
  const node = graph.nodes[selectedIndex];
  if (node === undefined) return '';
  const total = graph.nodes.length;
  const rel = relativeDate(node.ts, Math.floor(Date.now() / 1000));
  const refs = refSummary(node.refs);
  const base = `${node.summary} — ${node.author}, ${rel}. Row ${selectedIndex + 1} of ${total}.`;
  const head = refs === '' ? base : `${base} ${refs}`;
  return `${head}${forgeSummary(node, display)}`;
}

export function GraphSelectionAnnouncer({ graph, selectedIndex, display }: GraphSelectionAnnouncerProps) {
  const [message, setMessage] = useState('');
  useEffect(() => {
    const id = window.setTimeout(() => {
      setMessage(selectionMessage(graph, selectedIndex, display));
    }, 150);
    return () => window.clearTimeout(id);
  }, [graph, selectedIndex, display]);
  // Distinct accessible name so this graph-selection region and the sidebar-reveal
  // region (both `role="status"` sr-only spans) are individually addressable.
  return <RevealAnnouncer message={message} label="Graph selection" />;
}
