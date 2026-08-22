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

export interface GraphSelectionAnnouncerProps {
  graph: GraphLayout | null;
  selectedIndex: number | null;
}

/** Short human summary of a node's refs for the announcement, or '' when none. */
function refSummary(refs: GraphLayout['nodes'][number]['refs']): string {
  if (refs === undefined || refs.length === 0) return '';
  const names = refs.map((r) => r.name);
  return `Refs: ${names.join(', ')}.`;
}

/** Builds the settled-selection announcement, or '' when nothing is selected. */
export function selectionMessage(graph: GraphLayout | null, selectedIndex: number | null): string {
  if (graph === null || selectedIndex === null) return '';
  const node = graph.nodes[selectedIndex];
  if (node === undefined) return '';
  const total = graph.nodes.length;
  const rel = relativeDate(node.ts, Math.floor(Date.now() / 1000));
  const refs = refSummary(node.refs);
  const base = `${node.summary} — ${node.author}, ${rel}. Row ${selectedIndex + 1} of ${total}.`;
  return refs === '' ? base : `${base} ${refs}`;
}

export function GraphSelectionAnnouncer({ graph, selectedIndex }: GraphSelectionAnnouncerProps) {
  const [message, setMessage] = useState('');
  useEffect(() => {
    const id = window.setTimeout(() => {
      setMessage(selectionMessage(graph, selectedIndex));
    }, 150);
    return () => window.clearTimeout(id);
  }, [graph, selectedIndex]);
  return <RevealAnnouncer message={message} />;
}
