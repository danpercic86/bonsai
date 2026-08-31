/**
 * P68g §2.2 — the one-time AI consent dialog.
 *
 * It lives in its own file because its body is ~140 words of load-bearing, testable
 * text, and because the copy it replaces was FACTUALLY FALSE (security audit M2). That
 * copy claimed the payload was nothing but the conflicted files' bodies — the model
 * also chooses other repository files to read, and those bytes go to Anthropic — and
 * that nothing is changed until the user reviews it, which `autoResolve` contradicts
 * by writing and staging with no review step at all (N files per click since P68f).
 * The exact retired sentences are spelled only in this component's test, which fails
 * if either ever reappears anywhere under `src/`.
 *
 * The four blocks state four facts in the order the user cares about: what runs and
 * where · what leaves this machine · what Claude cannot do · when Bonsai writes.
 *
 * `confirmVariant="primary"`, not the ConfirmDialog default `danger`: this is a
 * reversible opt-in, and destructive styling is reserved for operations that lose
 * work. The honesty lives in the body text, not in a red button.
 */
import { ConfirmDialog } from '../ConfirmDialog';

export interface AiConsentDialogProps {
  open: boolean;
  onConfirm(): void;
  onCancel(): void;
}

export function AiConsentDialog({ open, onConfirm, onCancel }: AiConsentDialogProps) {
  return (
    <ConfirmDialog
      open={open}
      title="Enable AI features?"
      confirmLabel="Enable"
      confirmVariant="primary"
      cardClass="ai-consent-card"
      busy={false}
      onConfirm={onConfirm}
      onCancel={onCancel}
    >
      <div>
        Bonsai resolves conflicts with the Claude Code CLI installed on this machine, under your
        Claude subscription. Nothing is sent to Bonsai's own servers.
      </div>
      <p className="dialog-body-detail">
        Claude receives the conflicting versions of the files you choose — and it can read other
        files in this repository while it works, which is what lets it match your surrounding code.
        Whatever it reads is sent to Anthropic with the request.
      </p>
      <p className="dialog-body-detail">
        Its tools are read-only: it cannot write files, stage anything, or run commands, and reads
        outside this repository folder are refused. Refused reads appear in the AI activity dock.
      </p>
      <p className="dialog-body-detail">
        {
          "Bonsai changes your files only when you apply a result. The exception is “Resolve automatically” under Settings → AI assistance, which writes and stages Claude's results with no review step."
        }
      </p>
    </ConfirmDialog>
  );
}
