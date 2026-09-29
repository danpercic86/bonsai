// P125-ui §7 — the Settings consent confirm: a primary `ConfirmDialog` over the
// shared `DepConsentCopy`. Opened on every off → on flip of the Settings
// switch; the switch flips only on confirm. `ConfirmDialog` lands initial focus
// on Cancel, so a stray Enter never confirms, and Esc cancels.
import { ConfirmDialog } from '../ConfirmDialog';
import { CONSENT_CONFIRM, CONSENT_TITLE } from './depsCopy';
import { DepConsentCopy } from './DepConsentCopy';

export interface DepConsentDialogProps {
  open: boolean;
  intervalHours: number;
  onConfirm(): void;
  onCancel(): void;
}

export function DepConsentDialog({ open, intervalHours, onConfirm, onCancel }: DepConsentDialogProps) {
  return (
    <ConfirmDialog
      open={open}
      title={CONSENT_TITLE}
      confirmLabel={CONSENT_CONFIRM}
      confirmVariant="primary"
      cardClass="deps-consent-card"
      busy={false}
      onConfirm={onConfirm}
      onCancel={onCancel}
    >
      <DepConsentCopy intervalHours={intervalHours} />
    </ConfirmDialog>
  );
}
