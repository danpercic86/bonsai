// P42b: the update modal. Reuses the `.dialog-overlay`/`.dialog-card` chrome
// (ConfirmDialog idiom). Its body is driven by the shared UpdateUiState: shows
// version + notes with a "Download & install" button, a progress bar while
// downloading, a "Restart now / Later" prompt when ready, and an error+retry
// state. Never installs on its own — every step is a user action.

import { useEffect, useRef } from 'react';
import type { ReactNode } from 'react';

import type { UpdateUiState } from '../hooks/useUpdateController';

export interface UpdateDialogProps {
  open: boolean;
  state: UpdateUiState;
  /** Start (or retry) the download+install of the pending update. */
  onDownload(): void;
  /** Relaunch to finish the update. */
  onRestart(): void;
  onClose(): void;
}

function fmtMb(bytes: number): string {
  return `${(bytes / 1_000_000).toFixed(1)} MB`;
}

export function UpdateDialog({ open, state, onDownload, onRestart, onClose }: UpdateDialogProps) {
  const closeRef = useRef<HTMLButtonElement>(null);
  const downloading = state.status === 'downloading';

  useEffect(() => {
    if (open) closeRef.current?.focus();
  }, [open, state.status]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      // Do not allow Esc to close mid-download (the install is running).
      if (downloading) return;
      // Capture + stop: App's global Esc-close must not also fire.
      e.stopPropagation();
      onClose();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [open, onClose, downloading]);

  if (!open) return null;

  let title = 'Software update';
  let body: ReactNode = null;
  let buttons: ReactNode = null;

  if (state.status === 'available') {
    const { info } = state;
    title = 'Update available';
    body = (
      <>
        <p className="update-version-line">
          Version <span className="mono">{info.version}</span>
          {info.currentVersion !== '' && (
            <span className="update-version-from"> (you have {info.currentVersion})</span>
          )}
        </p>
        {info.notes !== null && info.notes !== '' && (
          <pre className="update-notes">{info.notes}</pre>
        )}
      </>
    );
    buttons = (
      <>
        <button type="button" className="btn-secondary" ref={closeRef} onClick={onClose}>
          Later
        </button>
        <button type="button" className="btn-primary" onClick={onDownload}>
          Download &amp; install
        </button>
      </>
    );
  } else if (state.status === 'downloading') {
    const { info, progress } = state;
    const { downloadedBytes, contentLength, phase } = progress;
    const known = contentLength !== null;
    title = 'Downloading update';
    body = (
      <>
        <p className="update-version-line">
          Version <span className="mono">{info.version}</span>
        </p>
        <div className="update-progress" aria-live="polite">
          <progress
            className="clone-progress-bar"
            max={known ? contentLength : undefined}
            value={known ? downloadedBytes : undefined}
          />
          <p className="clone-progress-detail">
            {phase === 'finished'
              ? 'Installing…'
              : `${fmtMb(downloadedBytes)}${known ? ` / ${fmtMb(contentLength)}` : ''}`}
          </p>
        </div>
      </>
    );
    buttons = (
      <button type="button" className="btn-secondary" ref={closeRef} disabled>
        Later
      </button>
    );
  } else if (state.status === 'readyToRestart') {
    const { info } = state;
    title = 'Update ready';
    body = (
      <p>
        Bonsai <span className="mono">{info.version}</span> has been installed. Restart to finish
        updating.
      </p>
    );
    buttons = (
      <>
        <button type="button" className="btn-secondary" ref={closeRef} onClick={onClose}>
          Later
        </button>
        <button type="button" className="btn-primary" onClick={onRestart}>
          Restart now
        </button>
      </>
    );
  } else if (state.status === 'error') {
    title = 'Update failed';
    body = (
      <p className="update-error" role="alert">
        {state.message}
      </p>
    );
    buttons = (
      <>
        <button type="button" className="btn-secondary" ref={closeRef} onClick={onClose}>
          Close
        </button>
        <button type="button" className="btn-primary" onClick={onDownload}>
          Retry
        </button>
      </>
    );
  } else {
    // checking / idle / upToDate — the dialog is normally only opened while an
    // update is available; render a graceful placeholder just in case.
    body = <p>Checking for updates…</p>;
    buttons = (
      <button type="button" className="btn-secondary" ref={closeRef} onClick={onClose}>
        Close
      </button>
    );
  }

  return (
    <div className="dialog-overlay" onClick={downloading ? undefined : onClose}>
      <div
        className="dialog-card update-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="Software update"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="dialog-title">{title}</h2>
        <div className="dialog-body">{body}</div>
        <div className="dialog-buttons">{buttons}</div>
      </div>
    </div>
  );
}
