// P91 §2/§6/§8 — the Developer (Dev-mode / observability) category page.
//
// Container: reads `dev` from SettingsContext, holds the `LogSessionInfo` poll
// (2s while THIS page is visible and Dev mode is on — the page unmounts when the
// category is deselected, so mounting == visible), the reveal/export/delete
// handlers, and the confirm-dialog state. It composes the four small section
// components (§3). All render lives in those children.
//
// ONE polite live region per page (§5.3/§8.5.6): it serves the reveal, export
// and delete announcements. The header pill (§5) carries no region of its own —
// Dev mode can only be toggled from here.
//
// P113 — this page raises NO toasts. Settings renders inside `.dialog-overlay`
// (z-index 100) and `.toast-stack` is 90, so a toast raised here is not merely
// dimmed: `elementFromPoint` at its own centre returns the overlay and its ✕
// cannot be clicked. All five outcomes go into two inline slots owned by
// `SettingsDevLogsSection` (ui-reference §12.14), announced through the ONE
// region below, which `useOutcomeNotes` writes from the same call as the note.

import { useCallback, useEffect, useRef, useState } from 'react';

import type { DevSettings, LogSessionInfo } from '../../../ipc';
import { ipc } from '../../../ipc';
import { errorMessage } from '../../../utils/errors';
import { useSettingsActions, useSettingsValues } from '../SettingsContext';
import { SettingsDevModeSection } from '../SettingsDevModeSection';
import { SettingsDevCaptureSection } from '../SettingsDevCaptureSection';
import { SettingsDevPrivacySection } from '../SettingsDevPrivacySection';
import { SettingsDevLogsSection, type DevLogsBusy } from '../SettingsDevLogsSection';
import {
  DeleteLogsConfirmDialog,
  ExportConfirmDialog,
  RawNamesConfirmDialog,
} from '../DevConfirmDialogs';
import { deleteErrorText, deleteResultToast, exportErrorText } from '../devLogMessages';
import { useOutcomeNotes } from '../useOutcomeNotes';

/** Faster than the header pill's 3 s (`DevModePill`) ON PURPOSE — do not unify
 *  them. This card is the surface the user watches while reproducing something:
 *  its record/byte counters must visibly move. The pill is always-mounted chrome
 *  reading one bool and settles for the cheaper cadence, and the offset keeps the
 *  two polls from phase-locking into a single synchronised IPC burst. */
const POLL_MS = 2000;
const NO_BUSY: DevLogsBusy = { reveal: false, export: false, delete: false };

/** P113 §6.1 — the two outcome slots. `dev.logs` is shared by reveal and export
 *  (both `anyBusy`-gated, so they cannot collide); the delete row owns its own. */
const LOGS_SLOT = 'dev.logs';
const DELETE_SLOT = 'dev.delete-logs';

export function DevCategory() {
  const { dev } = useSettingsValues();
  const { change } = useSettingsActions();
  const { notes, announce, begin, report } = useOutcomeNotes();

  const [info, setInfo] = useState<LogSessionInfo | null>(null);
  const [busy, setBusy] = useState<DevLogsBusy>(NO_BUSY);
  const [rawConfirm, setRawConfirm] = useState(false);
  const [exportConfirm, setExportConfirm] = useState(false);
  // The delete dialog carries the info snapshot taken WHEN IT OPENED, so the count
  // the user consents to is the count that gets deleted (§8.5.4).
  const [deleteInfo, setDeleteInfo] = useState<LogSessionInfo | null>(null);
  // §F6: openness is its OWN flag rather than `deleteInfo !== null`. The delete row
  // is no longer gated on logs existing, so a null session info is a legitimate
  // state — the usage counts are deletable whether or not any log file exists —
  // and keying `open` off the info would turn that into a silently dead click.
  const [deleteOpen, setDeleteOpen] = useState(false);

  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const patchDev = useCallback(
    (patch: Partial<DevSettings>) => change({ dev: { ...dev, ...patch } }),
    [change, dev],
  );

  const refresh = useCallback(async (): Promise<LogSessionInfo | null> => {
    try {
      const next = await ipc.logSessionInfo();
      if (mounted.current) setInfo(next);
      return next;
    } catch {
      return null;
    }
  }, []);

  // Mount read + 2s poll while Dev mode is on. Never polls when the page is not
  // mounted (category deselected / overlay closed) — this must not be background
  // load (§6).
  useEffect(() => {
    void refresh();
    if (!dev.enabled) return;
    const timer = window.setInterval(() => void refresh(), POLL_MS);
    return () => window.clearInterval(timer);
  }, [dev.enabled, refresh]);

  const onReveal = useCallback(async () => {
    begin(LOGS_SLOT);
    setBusy((b) => ({ ...b, reveal: true }));
    try {
      await ipc.logRevealDir();
    } catch {
      // P113 §8.1 — this failure used to be toast-only, so a screen-reader user
      // heard NOTHING. `report` writes the note and the announcement from one
      // call, with the same string: inline-only would have made it visible-only.
      report(LOGS_SLOT, 'error', "Couldn't open the logs folder. It may have been moved or deleted.");
    } finally {
      if (mounted.current) setBusy((b) => ({ ...b, reveal: false }));
    }
  }, [begin, report]);

  const runExport = useCallback(async () => {
    setExportConfirm(false);
    begin(LOGS_SLOT);
    setBusy((b) => ({ ...b, export: true }));
    try {
      await ipc.logExportSession();
      // The zip lands in `exports/`, but the page's `Show in folder` reveals
      // `logs/` — the sibling directory. This NOTE is the only place that gap is
      // closed, so it names the folder (P91 UI §8.3 step 5) — and inline it lands
      // directly beside the `Kept in {dir}` state note it contrasts with. The
      // live-region announcement stays bare: the location is not actionable by
      // voice, the one sanctioned trim (P91 §6.11.1).
      report(
        LOGS_SLOT,
        'success',
        'Session log exported. It is in the exports folder, next to your logs.',
        'Session log exported.',
      );
    } catch (e) {
      // Parity with delete failure (below): a screen-reader user must hear the
      // outcome, not just the sighted-only note text (7b design-review NIT).
      report(LOGS_SLOT, 'error', exportErrorText(errorMessage(e)), 'The log was not exported.');
    } finally {
      if (mounted.current) setBusy((b) => ({ ...b, export: false }));
      void refresh();
    }
  }, [begin, refresh, report]);

  const openDelete = useCallback(async () => {
    // Re-read on open (§8.5.4), not the last poll, so the count consented to is
    // the count deleted. The flag is set AFTER the read so the dialog never shows
    // a stale count first. Falling back to the last good poll beats discarding it.
    // `fresh ?? info` may still be null (every read failed, including the one on
    // mount) — the dialog gets that null AS null and renders its §6.8 R3
    // unknown-count copy. It must NOT be collapsed to 0 here or there: "0 files"
    // would name a smaller target than the command destroys.
    const fresh = await refresh();
    setDeleteInfo(fresh ?? info);
    setDeleteOpen(true);
  }, [refresh, info]);

  const runDelete = useCallback(async () => {
    // P113 §7 — cleared when the OPERATION starts, not when the dialog opens, so
    // cancelling the confirm leaves the previous outcome intact.
    begin(DELETE_SLOT);
    setBusy((b) => ({ ...b, delete: true }));
    try {
      const result = await ipc.logsDeleteAll();
      // The status card must describe the NEW session immediately — drive the
      // refresh off the delete's completion, not the 2s timer (§6).
      await refresh();
      // P113 §3.1.2 — `tone` is computed HERE, at runtime: one key press yields
      // success or error. That is why tone selects the RECIPE and never the
      // location. No `await` may be inserted between this and the `finally` that
      // closes the dialog, or the note would sit behind `.dialog-overlay` for the
      // gap (§2).
      const { tone, text, announce: msg } = deleteResultToast(result);
      report(DELETE_SLOT, tone, text, msg);
    } catch (e) {
      report(DELETE_SLOT, 'error', deleteErrorText(errorMessage(e)), 'Nothing was deleted.');
      // §6.8 R5: the action's scope is logs AND usage counts, so the failure
      // announcement may not name only logs. Accurate for every reachable error:
      // the Dev-ON branch's only in-thread failure is the pre-purge roll, so a
      // rejected `logsDeleteAll` means nothing was removed (see `obs_delete.rs`).
      void refresh();
    } finally {
      if (mounted.current) {
        setBusy((b) => ({ ...b, delete: false }));
        setDeleteOpen(false);
        setDeleteInfo(null);
      }
    }
  }, [begin, refresh, report]);

  return (
    <>
      <SettingsDevModeSection
        dev={dev}
        info={info}
        onToggleEnabled={(enabled) => patchDev({ enabled })}
      />
      <SettingsDevCaptureSection
        dev={dev}
        onPatch={patchDev}
        onRequestRawNames={() => setRawConfirm(true)}
      />
      <SettingsDevPrivacySection rawNames={dev.includeRawNames} />
      <SettingsDevLogsSection
        dev={dev}
        info={info}
        busy={busy}
        onReveal={() => void onReveal()}
        onExport={() => setExportConfirm(true)}
        onRequestDelete={() => void openDelete()}
        logsOutcome={notes.get(LOGS_SLOT) ?? null}
        deleteOutcome={notes.get(DELETE_SLOT) ?? null}
      />

      <RawNamesConfirmDialog
        open={rawConfirm}
        onConfirm={() => {
          patchDev({ includeRawNames: true });
          setRawConfirm(false);
        }}
        onCancel={() => setRawConfirm(false)}
      />
      <ExportConfirmDialog
        open={exportConfirm}
        info={info}
        busy={busy.export}
        onConfirm={() => void runExport()}
        onCancel={() => setExportConfirm(false)}
      />
      <DeleteLogsConfirmDialog
        open={deleteOpen}
        info={deleteInfo}
        devEnabled={dev.enabled}
        busy={busy.delete}
        onConfirm={() => void runDelete()}
        onCancel={() => {
          setDeleteOpen(false);
          setDeleteInfo(null);
        }}
      />

      <p className="sr-only" role="status" aria-live="polite">
        {announce}
      </p>
    </>
  );
}
