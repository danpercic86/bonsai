/**
 * P16 — the one-time consent dialog for starting the embedded MCP server.
 *
 * It lives in its own file for the same reason `AiConsentDialog` does: the body is
 * load-bearing security copy (what an external AI client may read, and on what
 * terms), and App's render body should wire dialogs rather than spell them.
 */
import { ConfirmDialog } from '../ConfirmDialog';

export interface McpConsentDialogProps {
  open: boolean;
  onConfirm(): void;
  onCancel(): void;
}

export function McpConsentDialog({ open, onConfirm, onCancel }: McpConsentDialogProps) {
  return (
    <ConfirmDialog
      open={open}
      title="Enable MCP server?"
      confirmLabel="Enable"
      busy={false}
      onConfirm={onConfirm}
      onCancel={onCancel}
    >
      <div>
        Bonsai will run a local MCP server on 127.0.0.1 that lets an external AI client (e.g.
        Claude Code) read <strong>any repository you have open in Bonsai</strong>. Access
        requires a secret token shown in Settings; nothing is exposed to the network. The server
        is read-only. Enable the MCP server?
      </div>
    </ConfirmDialog>
  );
}
