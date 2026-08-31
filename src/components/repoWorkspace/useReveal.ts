import { useCallback, useMemo, useRef, useState } from 'react';
import type { GraphLayout } from '../../ipc';
import type { RevealTarget, RevealFlash } from '../../graph/reveal';
import { revealTargetLabel } from '../../graph/reveal';
import { revealedMessage, revealMissMessage } from '../RevealAnnouncer';
import type { PushToast } from '../../ToastContext';

interface UseRevealDeps {
  graph: GraphLayout | null;
  setSelectedIndex: (i: number | null) => void;
  /** P90: re-scope the Checks tab on a branch reveal (tags/stashes ⇒ no-op). */
  revealBranch: (t: RevealTarget) => void;
  pushToast: PushToast;
}

/** P84: reveal-in-graph — the flash descriptor (nonce-driven so re-revealing the
 *  same row re-flashes), the a11y announcement string, the reduced-motion flag
 *  read once (never per-frame), and the oid/refName→row lookups. Extracted
 *  verbatim from RepoWorkspace so the container stays a composition site. */
export function useReveal({ graph, setSelectedIndex, revealBranch, pushToast }: UseRevealDeps) {
  // P84: reveal-in-graph — flash descriptor (nonce-driven so re-revealing the
  // same row re-flashes), a11y announcement, and the reduced-motion flag read
  // once (never per-frame). `revealNonceRef` supplies a monotonic nonce.
  const [revealFlash, setRevealFlash] = useState<RevealFlash | null>(null);
  const [revealMessage, setRevealMessage] = useState('');
  const revealNonceRef = useRef(0);
  const reducedMotion = useMemo(
    () =>
      typeof window !== 'undefined' &&
      typeof window.matchMedia === 'function' &&
      window.matchMedia('(prefers-reduced-motion: reduce)').matches,
    [],
  );
  // oid→row and refName→row lookups, rebuilt once per graph layout (first wins).
  const revealIndex = useMemo(() => {
    const byRef = new Map<string, number>();
    const byOid = new Map<string, number>();
    const nodes = graph?.nodes ?? [];
    for (let i = 0; i < nodes.length; i++) {
      const node = nodes[i];
      if (!byOid.has(node.id)) byOid.set(node.id, i);
      for (const ref of node.refs ?? []) {
        if (!byRef.has(ref.name)) byRef.set(ref.name, i);
      }
    }
    return { byRef, byOid };
  }, [graph]);

  const handleReveal = useCallback(
    (t: RevealTarget) => {
      const i =
        t.kind === 'ref' ? revealIndex.byRef.get(t.name) ?? null : revealIndex.byOid.get(t.oid) ?? null;
      const label = revealTargetLabel(t);
      revealBranch(t); // P90: re-scope the Checks tab (tags/stashes ⇒ no-op)
      revealNonceRef.current += 1; // §6: bump first; both paths thread it so repeats re-announce (invisible marker)
      if (i === null) {
        setRevealMessage(revealMissMessage(label, revealNonceRef.current));
        pushToast(
          'info',
          `"${label}" isn't in the loaded history yet. Load more commits to reveal it.`,
          'reveal-miss',
        );
        return;
      }
      // Selection drives the scroll-into-view (GraphCanvas effect); the flash is
      // the transient attention cue on top. Nonce bump re-flashes even when the
      // row is already selected.
      setSelectedIndex(i);
      setRevealFlash({ index: i, nonce: revealNonceRef.current });
      const oid = graph?.nodes[i]?.id ?? '';
      setRevealMessage(revealedMessage(label, oid, revealNonceRef.current));
    },
    [revealIndex, graph, pushToast, revealBranch, setSelectedIndex],
  );

  return { revealFlash, revealMessage, reducedMotion, handleReveal };
}
