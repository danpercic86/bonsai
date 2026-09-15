/**
 * P112 rule 5 / §16.7 — the external-tool scan state that OUTLIVES a mount, and
 * the one seam that clears it between test suites.
 *
 * Split out of `useExternalToolScan.ts` (489 of the 500-line ratchet, and the
 * increment that closed rule 6's entry points had to grow its header): the hook
 * is the scan's BEHAVIOUR, this is the two pieces of state that are deliberately
 * not React's, one concern per file.
 *
 * The split also makes a claim the hook could previously only assert in a
 * comment structural: "the ONE writer of `owedAdopt`" is now a module boundary.
 * Nothing outside this file can assign either variable.
 */
import type { ExternalToolKind, ExternalToolScan } from '../../ipc';

/**
 * The last good scan.
 *
 * Module-scoped, so switching category away from General and back does not cost
 * another round trip (§3). The backend caches too, but a second trip per visit
 * is still waste — and the 2.1 s cold scan is then paid at most once per app run
 * (§16.7).
 */
let cachedScan: ExternalToolScan | null = null;

export function getCachedScan(): ExternalToolScan | null {
  return cachedScan;
}

/** Only ever a LANDED scan — rule 1: no path sets this back to `null`, because
 *  the last good scan is what keeps a rescan from blanking a picker that already
 *  knows its own value and what lets a failed scan leave the pickers as they
 *  were. Clearing it is the test seam's business alone. */
export function setCachedScan(next: ExternalToolScan): void {
  cachedScan = next;
}

/**
 * A Browse landed on disk but its follow-up read did not (`BROWSE_STALE`), so
 * React still holds the OLD selection — this is the KIND that owes an adopt.
 *
 * It is what makes that note's "Press Rescan" TRUE. §16.4a specified the string
 * and the report but not the recovery: a Rescan refetches the LIST, which by
 * then contains the `custom` row, and without re-reading settings the picker
 * would still show the previous tool — copy naming an action with no visible
 * effect.
 *
 * Module-scoped by rule 5, not a ref: §17.2's recovery has to work across a
 * remount, and a ref dies with the mount that owns it.
 *
 * An OBJECT rather than a bare kind, because two comparisons mean different
 * things. A scan reads the owed adopt when it is dispatched and applies it when
 * it lands, and in between the user may select that kind's tool (rule 6, which
 * nulls it) and a second Browse may fail (which owes it again). Kind equality
 * cannot tell "still the same owed pick" from "a different one, owed since" —
 * reference identity can, and the second case must NOT be satisfied by settings
 * this scan read before that second pick was written. Same generation-token
 * logic as `scanEpoch`, one level over.
 */
export interface OwedAdopt {
  readonly kind: ExternalToolKind;
}
let owedAdopt: OwedAdopt | null = null;

export function getAdoptOwed(): OwedAdopt | null {
  return owedAdopt;
}

/** The ONE writer. Through a function because `require-atomic-updates` flags a
 *  module variable written after an `await` in a scope that read it before —
 *  which `load` does, by design — and because one place makes the flag's
 *  lifetime readable: owed when a Browse's follow-up read fails, cleared when
 *  the adopt lands, when a Browse succeeds for that kind, or when the user makes
 *  an explicit selection for that kind (rule 6). */
export function setAdoptOwed(owed: OwedAdopt | null): void {
  owedAdopt = owed;
}

/** Suites share a module graph, and a scan cached by one of them would satisfy
 *  the next one's mount effect — so the first-scan path would silently stop
 *  being covered. The owed adopt is reset here for the same reason and it is
 *  the sharper half: leaked, it makes the NEXT suite's first mount restore a
 *  `BROWSE_STALE` note and adopt a selection nobody in it ever browsed.
 *  Called from `beforeEach`, never from product code. */
export function resetExternalToolScanCacheForTests(): void {
  cachedScan = null;
  owedAdopt = null;
}
