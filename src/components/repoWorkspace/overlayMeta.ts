// Header metadata for the center diff overlay, derived from the open slot key +
// the current snapshot (P3a §2.3). Extracted from RepoWorkspace as a PURE
// function so the key-prefix ordering — which is load-bearing — is unit
// testable: `conflict:` / `ai-proposal:` / `pr:` must all be matched BEFORE the
// generic `<section>:<path>` fallback, which casts the prefix to a
// WorkdirSection. Never stored; recomputed each render.

import type { StatusSnapshot } from '../../ipc';
import type { DiffOverlayMeta } from '../DiffOverlay';
import { isPrSlotKey, parsePrSlotPath } from './prSlotKey';
import type { PrOverlayCtx } from './types';
import type { WorkdirSection } from '../StatusSection';

/** P96: user-visible stand-in for the path of a malformed `pr:` slot key. */
export const UNKNOWN_PR_PATH = '(unknown file)';

export function deriveOverlayMeta(
  key: string,
  status: StatusSnapshot | null,
  prOverlayCtx: PrOverlayCtx | null,
): DiffOverlayMeta {
  if (key.startsWith('conflict:')) {
    return {
      path: key.slice('conflict:'.length),
      origPath: null,
      status: 'conflicted',
      kind: 'conflict',
    };
  }
  // P13 §8.3: AI proposal review — reuses the conflict editor (seeded with the
  // markerless proposed body carried on diffSlot.conflict).
  if (key.startsWith('ai-proposal:')) {
    return {
      path: key.slice('ai-proposal:'.length),
      origPath: null,
      status: 'conflicted',
      kind: 'aiProposal',
    };
  }
  // P93: the path comes from the ctx side-channel (which also carries
  // status/origPath); the key parse is the fallback for a remount that lost it.
  if (isPrSlotKey(key)) {
    // P96: a malformed `pr:` key (fewer than three segments) has no recoverable
    // path — surface a clearly-degraded label rather than the raw key, matching
    // how the section fallback below degrades an unresolvable entry (status
    // null, no badge) instead of inventing data.
    return {
      path: prOverlayCtx?.path ?? parsePrSlotPath(key) ?? UNKNOWN_PR_PATH,
      origPath: prOverlayCtx?.origPath ?? null,
      status: prOverlayCtx?.status ?? null,
      kind: 'pr',
    };
  }
  const sep = key.indexOf(':');
  const section = key.slice(0, sep) as WorkdirSection;
  const path = key.slice(sep + 1);
  const entry = status?.[section].find((e) => e.path === path) ?? null;
  return {
    path,
    origPath: entry?.origPath ?? null,
    status: entry?.status ?? null,
    kind: section,
  };
}
