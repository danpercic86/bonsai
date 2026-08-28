// P91 §2/§6/§8 — the Developer (Dev-mode / observability) category page.
//
// Container: reads `dev` from SettingsContext, holds the `LogSessionInfo` poll
// (2s while THIS page is visible and Dev mode is on — the page unmounts when the
// category is deselected, so mounting == visible), the reveal/export/delete
// handlers, and the confirm-dialog state. It composes the four small section
// components (§3). All render lives in those children.
//
// ONE polite live region per page (§5.3/§8.5.6): it serves the export and delete
// announcements. The header pill (§5) carries no region of its own — Dev mode can
// only be toggled from here.

import { useCallback, useEffect, useRef, useState } from 'react';

import type { DevSettings, LogSessionInfo } from '../../../ipc';
import { ipc } from '../../../ipc';
import { usePushToast } from '../../../ToastContext';
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

const POLL_MS = 2000;
const NO_BUSY: DevLogsBusy = { reveal: false, export: false, delete: false };

export function DevCategory() {
  const { dev } = useSettingsValues();
  const { change } = useSettingsActions();
  const pushToast = usePushToast();

  const [info, setInfo] = useState<LogSessionInfo | null>(null);
  const [busy, setBusy] = useState<DevLogsBusy>(NO_BUSY);
  const [rawConfirm, setRawConfirm] = useState(false);
  const [exportConfirm, setExportConfirm] = useState(false);
  // The delete dialog carries the info snapshot taken WHEN IT OPENED, so the count
  // the user consents to is the count that gets deleted (§8.5.4).
  const [deleteInfo, setDeleteInfo] = useState<LogSessionInfo | null>(null);
  const [announce, setAnnounce] = useState('');

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
    setBusy((b) => ({ ...b, reveal: true }));
    try {
      await ipc.logRevealDir();
    } catch {
      pushToast(
        'error',
        "Couldn't open the logs folder. It may have been moved or deleted.",
        'dev-reveal',
      );
    } finally {
      if (mounted.current) setBusy((b) => ({ ...b, reveal: false }));
    }
  }, [pushToast]);

  const runExport = useCallback(async () => {
    setExportConfirm(false);
    setBusy((b) => ({ ...b, export: true }));
    try {
      await ipc.logExportSession();
      pushToast('success', 'Session log exported.', 'dev-export');
      setAnnounce('Session log exported.');
    } catch (e) {
      pushToast('error', exportErrorText(errorMessage(e)), 'dev-export');
    } finally {
      if (mounted.current) setBusy((b) => ({ ...b, export: false }));
      void refresh();
    }
  }, [pushToast, refresh]);

  const openDelete = useCallback(async () => {
    // Re-read on open (§8.5.4), not the last poll. The Delete button is only
    // enabled while logs exist, so `info` is non-null here as a fallback.
    const fresh = await refresh();
    setDeleteInfo(fresh ?? info);
  }, [refresh, info]);

  const runDelete = useCallback(async () => {
    setBusy((b) => ({ ...b, delete: true }));
    try {
      const result = await ipc.logsDeleteAll();
      // The status card must describe the NEW session immediately — drive the
      // refresh off the delete's completion, not the 2s timer (§6).
      await refresh();
      const { tone, text, announce: msg } = deleteResultToast(result);
      pushToast(tone, text, 'dev-delete');
      setAnnounce(msg);
    } catch (e) {
      pushToast('error', deleteErrorText(errorMessage(e)), 'dev-delete');
      setAnnounce('No log files were deleted.');
      void refresh();
    } finally {
      if (mounted.current) {
        setBusy((b) => ({ ...b, delete: false }));
        setDeleteInfo(null);
      }
    }
  }, [pushToast, refresh]);

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
        open={deleteInfo !== null}
        info={deleteInfo}
        devEnabled={dev.enabled}
        busy={busy.delete}
        onConfirm={() => void runDelete()}
        onCancel={() => setDeleteInfo(null)}
      />

      <p className="sr-only" role="status" aria-live="polite">
        {announce}
      </p>
    </>
  );
}
