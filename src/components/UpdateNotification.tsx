// P42b: non-modal "an update is available" banner. Presentational — the hook
// (useUpdateController) owns availability + dismissal; App renders this only while
// `notificationVisible`. "What's new" opens the UpdateDialog; ✕ dismisses.

export interface UpdateNotificationProps {
  /** Target version, e.g. "0.2.0". */
  version: string;
  /** Open the UpdateDialog (release notes + download flow). */
  onView(): void;
  /** Hide the banner until the next check finds an update again. */
  onDismiss(): void;
}

export function UpdateNotification({ version, onView, onDismiss }: UpdateNotificationProps) {
  return (
    <div className="update-banner" role="status" aria-live="polite">
      <span className="update-banner-text">Bonsai v{version} is available</span>
      <div className="update-banner-actions">
        <button type="button" className="btn-primary update-banner-view" onClick={onView}>
          {"What's new"}
        </button>
        <button
          type="button"
          className="btn-icon update-banner-dismiss"
          aria-label="Dismiss"
          title="Dismiss"
          onClick={onDismiss}
        >
          {'✕'}
        </button>
      </div>
    </div>
  );
}
