// The two updater surfaces, driven by ONE controller.
//
// Extracted from `App` (P112 §5.1 increment) because App renders them from a
// single cohesive bag — `useUpdateController` — and spelling out nine props for
// two components put fifteen lines of pure wiring in a container that sits on
// the file-size ratchet. Nothing here holds state or calls IPC: the dialog's
// lifecycle, the download and the dismissal all belong to the controller, so
// this is a placement decision and nothing else.
//
// The pair is one unit on purpose: `notificationVisible` and `dialogOpen` are
// two views of the same `state`, and the banner must disappear the moment the
// dialog takes over. Keeping both reads in one file is what makes that
// checkable at a glance.

import type { UpdateController } from '../hooks/useUpdateController';
import { UpdateDialog } from './UpdateDialog';
import { UpdateNotification } from './UpdateNotification';

export function AppUpdateSurfaces({ update }: { update: UpdateController }) {
  return (
    <>
      <UpdateDialog
        open={update.dialogOpen}
        state={update.state}
        onDownload={update.download}
        onRestart={update.restart}
        onClose={update.closeDialog}
      />
      {/* `status === 'available'` is re-checked here, not just trusted from
          `notificationVisible`: the banner names a VERSION, and only the
          available state carries one. */}
      {update.notificationVisible && update.state.status === 'available' && (
        <UpdateNotification
          version={update.state.info.version ?? ''}
          onView={update.openDialog}
          onDismiss={update.dismissNotification}
        />
      )}
    </>
  );
}
