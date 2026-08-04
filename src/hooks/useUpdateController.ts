// P42b: the update state machine, extracted so App stays slim. Owns the
// idle → checking → (available | upToDate | error) → downloading → readyToRestart
// flow behind Bonsai's IPC-triple (`checkForUpdate` / `downloadAndInstallUpdate` /
// `relaunchApp`). Never auto-installs: an available update only NOTIFIES; the user
// drives download and the (process-exiting) restart. See docs/contracts/P42.

import { useCallback, useRef, useState } from 'react';

import { ipc } from '../ipc';
import type { UpdateCheckResult, UpdateProgress } from '../ipc';
import { errorMessage } from '../utils/errors';

/** The full update UI state (contract §7.2). `info` carries the check result so
 *  the dialog can show current→target version + notes; `progress` is the latest
 *  byte tick during download. */
export type UpdateUiState =
  | { status: 'idle' }
  | { status: 'checking' }
  | { status: 'upToDate' }
  | { status: 'available'; info: UpdateCheckResult }
  | { status: 'downloading'; info: UpdateCheckResult; progress: UpdateProgress }
  | { status: 'readyToRestart'; info: UpdateCheckResult }
  | { status: 'error'; message: string };

export interface UpdateController {
  /** The single source of truth the notification, dialog, and Settings share. */
  state: UpdateUiState;
  /** Populated from any resolved check (`checkForUpdate` always returns it);
   *  `null` until the first successful check. */
  currentVersion: string | null;
  /** Show the non-modal banner: an update is available and not dismissed. */
  notificationVisible: boolean;
  dialogOpen: boolean;
  /** Check the endpoint. `silent` (auto-check-on-launch) suppresses the
   *  checking/up-to-date/error surfacing — only an AVAILABLE result is shown. */
  check(silent?: boolean): Promise<void>;
  /** Download + install the pending update, streaming progress into `state`. */
  download(): void;
  /** Restart to finish the update (process exits; no-op in the mock). */
  restart(): void;
  openDialog(): void;
  closeDialog(): void;
  dismissNotification(): void;
}

export function useUpdateController(): UpdateController {
  const [state, setState] = useState<UpdateUiState>({ status: 'idle' });
  const [currentVersion, setCurrentVersion] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(false);
  const [dialogOpen, setDialogOpen] = useState(false);
  // Retained across an error so a dialog "Retry" can re-drive the download.
  const infoRef = useRef<UpdateCheckResult | null>(null);
  // Re-entrancy guard: a launch auto-check + a manual check must not overlap.
  const checkingRef = useRef(false);

  const check = useCallback(async (silent = false): Promise<void> => {
    if (checkingRef.current) return;
    checkingRef.current = true;
    if (!silent) setState({ status: 'checking' });
    try {
      const res = await ipc.checkForUpdate();
      setCurrentVersion(res.currentVersion);
      if (res.available) {
        infoRef.current = res;
        setDismissed(false);
        setState({ status: 'available', info: res });
      } else {
        infoRef.current = null;
        setState(silent ? { status: 'idle' } : { status: 'upToDate' });
      }
    } catch (e) {
      // A silent launch check swallows failures (no surprise toast/dialog).
      setState(silent ? { status: 'idle' } : { status: 'error', message: errorMessage(e) });
    } finally {
      checkingRef.current = false;
    }
  }, []);

  const download = useCallback((): void => {
    const info = infoRef.current;
    if (info === null) return;
    setState({
      status: 'downloading',
      info,
      progress: { phase: 'started', downloadedBytes: 0, contentLength: null },
    });
    void ipc
      .downloadAndInstallUpdate((progress) => {
        setState({ status: 'downloading', info, progress });
      })
      .then(() => setState({ status: 'readyToRestart', info }))
      .catch((e) => setState({ status: 'error', message: errorMessage(e) }));
  }, []);

  const restart = useCallback((): void => {
    void ipc.relaunchApp().catch(() => {
      // Never resolves in practice (the process exits); a rejection is non-fatal.
    });
  }, []);

  const openDialog = useCallback(() => setDialogOpen(true), []);
  const closeDialog = useCallback(() => setDialogOpen(false), []);
  const dismissNotification = useCallback(() => setDismissed(true), []);

  return {
    state,
    currentVersion,
    notificationVisible: state.status === 'available' && !dismissed,
    dialogOpen,
    check,
    download,
    restart,
    openDialog,
    closeDialog,
    dismissNotification,
  };
}
