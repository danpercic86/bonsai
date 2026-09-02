/**
 * P16c — the stronger, second consent dialog: letting the connected AI client
 * WRITE to any open repository.
 *
 * Separate from `McpConsentDialog` because it is a separate, escalating decision with
 * its own persisted gate (`mcpWriteConsented`) — and because its body is the only
 * place the "no per-action prompt" fact is stated to the user.
 */
import { ConfirmDialog } from '../ConfirmDialog';

export interface McpWriteConsentDialogProps {
  open: boolean;
  onConfirm(): void;
  onCancel(): void;
}

export function McpWriteConsentDialog({ open, onConfirm, onCancel }: McpWriteConsentDialogProps) {
  return (
    <ConfirmDialog
      open={open}
      title="Allow AI to modify repositories?"
      confirmLabel="Allow write access"
      busy={false}
      onConfirm={onConfirm}
      onCancel={onCancel}
    >
      <div>
        This grants the connected AI client the ability to <strong>modify</strong> any
        repository you have open in Bonsai — staging, committing, merging, resolving conflicts,
        and other write operations run without a per-action prompt. Changing this setting
        restarts the server and drops any active connection. Allow write access?
      </div>
    </ConfirmDialog>
  );
}
