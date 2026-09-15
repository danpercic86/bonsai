/**
 * P112 §3 / §16.4 / §16.4a — the external-tool scan, its Rescan, and Browse.
 *
 * It lives beside `useOutcomeNotes.ts` rather than in `src/hooks/`, deliberately
 * (§16.2): a hook in `src/hooks/` that renders Settings rows and takes an outcome
 * callback is precisely the blind spot that let five toast call sites through
 * P113's first sweep, and no `files:` glob can express it. Here it is inside the
 * path lint's reach.
 *
 * **It takes no `pushToast` and no launcher callback, ever** (§16.5 / UA17). This
 * hook CONFIGURES the launchers; it never invokes one. `openInTerminal` /
 * `revealInFileManager` / `openInEditor` all need a repository path, and Settings
 * has none in hand — so the structural claim is checkable by reading
 * `ToolScanOutcomeOps` instead of by reasoning about reachability.
 *
 * Six ordering rules are load-bearing and each has a defect behind it:
 *
 *   1. **Never `setScan(null)`** — not when a refresh starts, not when one
 *      rejects. The last good scan is what keeps a rescan from blanking a picker
 *      that already knows its own value (§16.6 7b), and what lets a failed scan
 *      leave the pickers exactly as they were (§16.4 R5).
 *   2. **The Browse adopt commits the settings and the scan in ONE update.** The
 *      moment `editorTool` becomes `'custom'` the held scan still predates the
 *      pick and contains no `custom` row, so the input would render blank — the
 *      §12.13 defect, on the SUCCESS path (§16.4a, UA18).
 *   3. **A superseded response is discarded by EPOCH, not by `scannedAtMs`.**
 *      `refresh: false` does not advance that field (`tools.rs:60-64`), so the
 *      Browse follow-up read returns the same timestamp as the cached scan while
 *      carrying the new row — a freshness comparison would throw it away. The
 *      epoch is bumped by every `load` dispatch **and by the Browse adopt**,
 *      which is the only overlap that can actually happen: loads are serialised
 *      by `scanInFlight`, so without the Browse bump the compare was
 *      unreachable — dead as written.
 *   4. **A recovery retracts its own message.** `BROWSE_STALE` names Rescan, so
 *      the scan that completes the adopt must also `begin` the slot that error
 *      sits in. Notes clear only through `begin` or unmount (there is no
 *      auto-dismiss), so an adopt that skips it leaves the row showing the
 *      recovered label AND a standing "Press Rescan." — the third layer of one
 *      defect (§17.2's string, its recovery, and this).
 *   5. **An owed adopt outlives the component.** §17.2 grants recovery "from
 *      either the button **or a remount**", and a per-mount ref cannot do that:
 *      switching category away from General and back dropped the owed adopt AND
 *      its note, silently, leaving the picked value only on disk. So it sits
 *      beside `cachedScan` — and inside the same test reset seam, or it becomes
 *      the next cross-suite leak. A mount that finds one owed restores the note
 *      and completes the adopt; the ordinary warm remount still costs zero
 *      round trips.
 *   6. **An explicit pick supersedes that kind's owed adopt.** An owed adopt
 *      means "the disk holds a value the UI has not picked up yet"; if the user
 *      then picks something for that kind, the owed adopt is stale by
 *      definition — there is nothing left to adopt, because the user has just
 *      told us what they want. This is why the pick is routed THROUGH this hook
 *      (`changeTool`): a call site patching `terminalTool`/`editorTool` itself
 *      could not clear it, and the next scan would then put the browsed value
 *      back over the newer pick while that pick reached disk anyway — screen and
 *      disk disagreeing until the next launch.
 */
import { useCallback, useEffect, useRef, useState } from 'react';

import { ipc } from '../../ipc';
import {
  BROWSE_ERR,
  BROWSE_STALE,
  EDITOR_SLOT,
  RESCAN_SLOT,
  SCAN_ERR,
  TERMINAL_SLOT,
  announceBrowsed,
  scanCounts,
} from './toolPickerCopy';
import type { ExternalToolKind, ExternalToolScan, UiSettings } from '../../ipc';
import type { SettingsOutcome } from './SettingsOutcomeNote';
import type { ToolSelection } from '../../hooks/useUiSettings';

/**
 * Everything this hook is allowed to do to the page besides holding the scan.
 *
 * `report` / `begin` / `announceOnly` are `useOutcomeNotes`' — the ONE channel
 * P113 built, so no outcome here can raise a toast (which would render behind
 * the Settings overlay and be unclickable). `adoptToolSelection` is §16.16-5's
 * two-field non-writing setter; the pick has already been persisted by the
 * backend, so the renderer re-reads rather than patching, or it races that
 * write.
 */
export interface ToolScanOutcomeOps {
  begin(key: string): void;
  report(key: string, tone: SettingsOutcome['tone'], text: string): void;
  announceOnly(text: string): void;
  /**
   * Adopt a tool selection read from disk WITHOUT queueing a write.
   *
   * §16.16-5, not `hydrateUiSettings` (§17.3 reversed that recommendation): the
   * whole-struct hydrate set EVERY field from disk, and a settings write that
   * succeeds is never re-adopted (`useSettingsWriteQueue.ts:108-120`), so a
   * debounced patch still inside its 300 ms window was reverted **on screen
   * until the next launch** while the disk still got it — not "one debounce
   * window" as §16.4a priced it. This hook always sends exactly ONE field, the
   * kind whose adopt is owed, so neither the other picker nor any unrelated
   * setting can be reverted by somebody pressing Rescan.
   *
   * The one window this used to leave open is closed by rule 6: an explicit
   * pick for a kind clears that kind's owed adopt, so no adopt can ever land
   * over a newer user pick (which is why `changeToolSelection` below exists).
   */
  adoptToolSelection(selection: ToolSelection): void;
  /**
   * Persist an explicit user pick for ONE kind — the ordinary debounced settings
   * write (`SettingsActions.change`), narrowed to the two fields this hook owns.
   *
   * It is an op rather than a call-site concern because the hook owns the ORDER
   * (rule 6): clearing that kind's owed adopt and writing the pick are one act,
   * and a container that called `change` directly could do the second without
   * the first. That is semantics, not a race patch — an owed adopt is by
   * definition a value the user has not seen yet, and an explicit pick replaces
   * it.
   */
  changeToolSelection(selection: ToolSelection): void;
}

export interface ExternalToolScanState {
  /** The last good scan, or `null` while none has ever landed. */
  scan: ExternalToolScan | null;
  /** A scan is in flight — the first one or a Rescan. */
  scanning: boolean;
  /** A scan has FAILED with nothing cached: the one state where the picker has
   *  no options to show and no label map to name a stored id with, so the row
   *  must not claim to still be looking. */
  scanFailedCold: boolean;
  /** The kind whose native dialog is open, or `null`. */
  browsing: ExternalToolKind | null;
  /** Re-probe and replace the process cache (the Rescan button). */
  refresh(): void;
  /** Open the backend's native program picker for one kind. */
  browse(kind: ExternalToolKind): void;
  /**
   * The user picked an option from one row's list: clear that kind's owed adopt
   * (rule 6) and persist the pick, in that order, so no call site can do the
   * second without the first.
   *
   * It is the PICKER's write path and not the only writer of these two
   * settings: each row's `↺` reset patches the same key through the generic
   * `resetRow` (`useSettingsPanelAdapter.ts:370`), which cannot reach this hook.
   * **Known gap, unclosed:** a reset made while an adopt is owed is still
   * overridden by that adopt on the next scan — the same divergence rule 6
   * closes for a pick, and a reset is just as explicit. Closing it means routing
   * the `↺` for these two rows through here (or giving `resetRow` a seam),
   * which is a decision above this file.
   */
  changeTool(kind: ExternalToolKind, next: string): void;
}

/**
 * Module-scoped, so switching category away from General and back does not cost
 * another round trip (§3). The backend caches too, but a second trip per visit
 * is still waste — and the 2.1 s cold scan is then paid at most once per app run
 * (§16.7).
 */
let cachedScan: ExternalToolScan | null = null;

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
 * it lands, and in between the user may pick that kind's tool (rule 6, which
 * nulls it) and a second Browse may fail (which owes it again). Kind equality
 * cannot tell "still the same owed pick" from "a different one, owed since" —
 * reference identity can, and the second case must NOT be satisfied by settings
 * this scan read before that second pick was written. Same generation-token
 * logic as `scanEpoch`, one level over.
 */
interface OwedAdopt {
  readonly kind: ExternalToolKind;
}
let owedAdopt: OwedAdopt | null = null;

/** The ONE writer of `owedAdopt`. Through a function because
 *  `require-atomic-updates` flags a module variable written after an `await` in
 *  a scope that read it before — which `load` does, by design — and because one
 *  place makes the flag's lifetime readable: owed when a Browse's follow-up read
 *  fails, cleared when the adopt lands, when a Browse succeeds for that kind, or
 *  when the user picks that kind's tool themselves. */
function setAdoptOwed(owed: OwedAdopt | null): void {
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

function rowsFor(scan: ExternalToolScan): string {
  return scanCounts(scan.terminals.length, scan.editors.length);
}

/** The dotted row id a kind's outcomes live in — resolved here so no call site
 *  re-derives it and the Browse report and its retraction cannot drift apart. */
function slotFor(kind: ExternalToolKind): string {
  return kind === 'terminal' ? TERMINAL_SLOT : EDITOR_SLOT;
}

/** The ONE field an adopt or a pick for `kind` may touch. Everything else a
 *  settings read returned is deliberately dropped (see `adoptToolSelection`). */
function selectionFor(kind: ExternalToolKind, id: string): ToolSelection {
  return kind === 'terminal' ? { terminalTool: id } : { editorTool: id };
}

/** The id `kind`'s field holds in a settings struct read from disk. */
function storedId(kind: ExternalToolKind, settings: UiSettings): string {
  return kind === 'terminal' ? settings.terminalTool : settings.editorTool;
}

/** The landed scan's label for an adopted id, or `null` when no row carries it
 *  — `''` (auto-detect) and an id the scan does not list both land here, and
 *  neither can be announced by name. */
function labelFor(scan: ExternalToolScan, kind: ExternalToolKind, id: string): string | null {
  const rows = kind === 'terminal' ? scan.terminals : scan.editors;
  return rows.find((row) => row.id === id)?.label ?? null;
}

export function useExternalToolScan(ops: ToolScanOutcomeOps): ExternalToolScanState {
  const { begin, report, announceOnly, adoptToolSelection, changeToolSelection } = ops;
  const [scan, setScan] = useState<ExternalToolScan | null>(cachedScan);
  const [scanning, setScanning] = useState(false);
  const [scanFailedCold, setScanFailedCold] = useState(false);
  const [browsing, setBrowsing] = useState<ExternalToolKind | null>(null);

  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  /**
   * Rule 3: the freshness epoch of the displayed scan. Bumped by every `load`
   * dispatch AND by the Browse adopt, which lands a scan of its own.
   *
   * The Browse bump is what makes the mechanism live. `scanInFlight` serialises
   * loads, so a later load can only start after the earlier one has settled —
   * a load can never supersede a load. Browse and Rescan hold SEPARATE in-flight
   * flags and therefore can overlap, and that overlap is the real hazard: a
   * Rescan response that predates the pick would put back a list with no
   * `custom` row, and `custom` is in neither label map (Rust's `label_map` is
   * catalog-ids-only), so the row would render the raw id `custom` plus
   * "custom isn’t installed right now…" — a literal id and a false claim.
   */
  const scanEpoch = useRef(0);
  /** A second press while a scan is in flight no-ops — `aria-disabled` keeps the
   *  button focusable, so the handler is what has to refuse (§16.7). */
  const scanInFlight = useRef(false);
  const browseInFlight = useRef(false);
  /** Why `scanEpoch.current += 1` in `browse` may stay inline after its `await`
   *  while the owed adopt goes through `setAdoptOwed`: `require-atomic-updates`
   *  fires on a value READ before the await and written after it, and the epoch
   *  bump reads nothing across that boundary (lint is 0/0 on this file). */

  // Sync outer function, async body: the in-flight flag is read and written in
  // the same synchronous frame, which is both what makes the guard sound and
  // what keeps `require-atomic-updates` satisfied.
  const load = useCallback(
    (refresh: boolean): void => {
      if (scanInFlight.current) return;
      scanInFlight.current = true;
      const epoch = scanEpoch.current + 1;
      scanEpoch.current = epoch;
      // §16.4 R1: `begin` on the START of every scan, the first one included —
      // otherwise a previous SCAN_ERR survives into the next scan and reads as
      // its result.
      begin(RESCAN_SLOT);
      setScanning(true);
      setScanFailedCold(false);
      // Read synchronously, and deliberately NOT cleared here: if this scan
      // fails the adopt is still owed, so the next Rescan retries it. The TOKEN
      // is kept, not its kind — see `OwedAdopt`; it is re-checked at landing.
      const owed = owedAdopt;
      void (async () => {
        try {
          // The settings read rides along only while an adopt is owed, so the
          // ordinary Rescan stays ONE round trip.
          const [next, settings] = await Promise.all([
            ipc.listExternalTools(refresh),
            owed !== null ? ipc.getUiSettings() : Promise.resolve(null),
          ]);
          // A Browse adopt landed while this was out: its list is newer than
          // this one, so this response is dropped BEFORE `cachedScan` — leaving
          // the stale list in the module cache would re-show it on the next
          // visit to General.
          if (!mounted.current || scanEpoch.current !== epoch) return;
          cachedScan = next;
          // Same one-commit rule as the Browse adopt, for the same reason: the
          // selection and the option that names it must arrive together.
          //
          // `owedAdopt === owed` is rule 6 at LANDING, and it is a second window,
          // not a repeat of `changeTool`'s: a Rescan takes up to 2.1 s, so the
          // user can pick that kind's tool WHILE it is out. The dispatch-time
          // snapshot would then adopt the pre-pick disk value over the newer
          // pick — the divergence rule 6 exists to prevent, one timing window
          // over. Reference identity also refuses an adopt re-owed by a second
          // Browse failure since dispatch, whose value these `settings` predate.
          let adopted: string | null = null;
          if (owed !== null && settings !== null && owedAdopt === owed) {
            setAdoptOwed(null);
            adopted = storedId(owed.kind, settings);
            adoptToolSelection(selectionFor(owed.kind, adopted));
            // Rule 4: retract `BROWSE_STALE`. It sits in the kind's own slot and
            // an outcome note clears only through `begin` — in THIS synchronous
            // block, so it batches into the one commit, and the `announceOnly`
            // below still wins the announcer afterwards.
            begin(slotFor(owed.kind));
          }
          setScan(next);
          // §16.4 R3: only a USER-pressed rescan announces. Announcing counts
          // because somebody opened Settings is unsolicited chatter — and §16.8:
          // it fires even when the counts are byte-identical, which is what
          // makes a button that finds the same tools not read as broken.
          if (refresh) announceOnly(rowsFor(next));
          // The ONE exception, and it is not chatter: a mount that completes an
          // owed adopt (rule 5) changes the selection with nobody having touched
          // this page, and the note retracted just above was the only thing
          // saying so — silence leaves a screen-reader user with an instruction
          // to press Rescan and then a value that changed unheard.
          // `announceBrowsed`, because this IS the browse's confirmation
          // arriving late; an id no row carries has no name to say.
          else if (owed !== null && adopted !== null) {
            const label = labelFor(next, owed.kind, adopted);
            if (label !== null) announceOnly(announceBrowsed(owed.kind, label));
          }
        } catch {
          if (!mounted.current || scanEpoch.current !== epoch) return;
          report(RESCAN_SLOT, 'error', SCAN_ERR);
          // Rule 1: the cached scan is KEPT. Only a cold failure has nothing.
          if (cachedScan === null) setScanFailedCold(true);
        } finally {
          // NO epoch check here, deliberately: `scanning` belongs to THIS load
          // and nothing else can clear it (loads are serialised), so a
          // superseded response must still drop the spinner instead of
          // stranding the Rescan button `aria-busy` forever.
          if (mounted.current) setScanning(false);
        }
      })().finally(() => {
        scanInFlight.current = false;
      });
    },
    [begin, report, announceOnly, adoptToolSelection],
  );

  // Once per mount. A cached scan is adopted without a round trip; `load` and
  // `report` are referentially stable (`useOutcomeNotes`' callbacks are), so
  // this runs once per mount and never re-fires on a re-render.
  useEffect(() => {
    // Rule 5. The owed adopt survived the unmount; its note did not, because
    // notes live in the category's own `useOutcomeNotes`. Both halves come back
    // together — restoring the value silently would leave the user looking at a
    // stale selection with nothing saying why, and restoring the note without
    // completing the adopt would be §17.2's unimplemented recovery all over
    // again. This is the ONE path that spends a round trip on a warm cache, and
    // only in an error-recovery state (`refresh: false`, so the backend's PATH
    // walk is not re-run).
    const owed = owedAdopt;
    if (owed !== null) {
      // A microtask, not a direct call: `report`'s `flushSync` is legal only
      // from a promise continuation (`useOutcomeNotes.ts`), and React warns when
      // it is reached from inside an effect — which StrictMode's second mount
      // run would do, the text being identical by then. It also re-checks the
      // token, so a recovery that has ALREADY landed (a load resolving before
      // this tick) restores no note.
      void Promise.resolve().then(() => {
        if (!mounted.current || owedAdopt !== owed) return;
        report(slotFor(owed.kind), 'error', BROWSE_STALE);
      });
      load(false);
      return;
    }
    if (cachedScan !== null) {
      setScan(cachedScan);
      return;
    }
    load(false);
  }, [load, report]);

  const refresh = useCallback((): void => {
    load(true);
  }, [load]);

  const browse = useCallback(
    (kind: ExternalToolKind): void => {
      if (browseInFlight.current) return;
      browseInFlight.current = true;
      const slot = slotFor(kind);
      begin(slot);
      setBrowsing(kind);
      void (async () => {
        try {
          const picked = await ipc.pickExternalTool(kind);
          if (!mounted.current) return;
          // Cancel: nothing is written, no message, no toast, note unchanged,
          // and focus never left `Browse…` (it is `aria-disabled`, not
          // `disabled`). A cancel is not an error.
          if (picked === null) return;
          try {
            // `refresh: false` on purpose: the custom row is derived from the
            // arguments `list_external_tools` reads out of the settings file the
            // pick just wrote, so a cache hit on the PATH walk still returns it.
            // `refresh: true` would re-walk PATH for 0.45 s to learn nothing.
            const [settings, next] = await Promise.all([
              ipc.getUiSettings(),
              ipc.listExternalTools(false),
            ]);
            if (!mounted.current) return;
            // Rule 3: a scan landed OUTSIDE `load`, so the epoch advances here —
            // a Rescan that is still in flight must not put its older list back
            // over this one.
            scanEpoch.current += 1;
            cachedScan = next;
            // An adopt owed for THIS kind is now satisfied — this read is the
            // one it was waiting for. Leaving it owed only mattered once the
            // flag outlived the mount (rule 5): a browse that recovered from an
            // earlier failed one would otherwise re-raise "Press Rescan." over a
            // correct value on the next visit to General. The other kind's owed
            // adopt is untouched; nothing here read its field.
            if (owedAdopt?.kind === kind) setAdoptOwed(null);
            // Rule 2 — ONE update: the new selection and the option that names
            // it must arrive in the same render, or the input blanks in between.
            adoptToolSelection(selectionFor(kind, storedId(kind, settings)));
            setScan(next);
            announceOnly(announceBrowsed(kind, picked.label));
          } catch {
            if (!mounted.current) return;
            // The selection IS on disk and the UI is still showing the old one,
            // so the next scan owes an adopt for THIS kind — which is what makes
            // the note's "Press Rescan" recover the value rather than just
            // refresh a list, and what tells that scan which slot to retract.
            // A FRESH token every time, so a scan dispatched before this failure
            // cannot satisfy it with settings it read earlier.
            setAdoptOwed({ kind });
            report(slot, 'error', BROWSE_STALE);
          }
        } catch {
          if (!mounted.current) return;
          report(slot, 'error', BROWSE_ERR);
        } finally {
          browseInFlight.current = false;
          if (mounted.current) setBrowsing(null);
        }
      })();
    },
    [begin, report, announceOnly, adoptToolSelection],
  );

  const changeTool = useCallback(
    (kind: ExternalToolKind, next: string): void => {
      // Rule 6, which is the state's own meaning and not a guard against a
      // timing window: an owed adopt is a value the user has not picked up yet,
      // and they have just said what they want instead.
      //
      // Conditional, because `begin` blanks the section's ONE announcer
      // (§8.2) — clearing it on every pick would truncate an unrelated
      // utterance. And it `begin`s at all for rule 4's reason in reverse:
      // `BROWSE_STALE` names a Rescan that can no longer change this row, so
      // the pick that superseded it retracts it.
      if (owedAdopt?.kind === kind) {
        setAdoptOwed(null);
        begin(slotFor(kind));
      }
      changeToolSelection(selectionFor(kind, next));
    },
    [begin, changeToolSelection],
  );

  return { scan, scanning, scanFailedCold, browsing, refresh, browse, changeTool };
}
