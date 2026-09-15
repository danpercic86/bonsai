import type { DetectedTool, ExternalToolKind, ExternalToolScan } from './common';

/**
 * P112 §6 — the external-tool picker surface, split out of `ipc-api.ts` for the
 * same reason `ipc-api-forge.ts` / `ipc-api-obs.ts` were: that file is over the
 * file-size limit and may only shrink. `IpcApi` extends this.
 */
export interface IpcApiTools {
  /** Detected tools + the remembered browsed tool + the AMEND-1 label maps, in
   *  ONE round trip. NEVER rejects for detection state — an empty scan is a
   *  normal result (the `checkGitAvailability` precedent). `refresh: true`
   *  re-probes and replaces the process cache (the picker's Rescan), so
   *  `scannedAtMs` advances. Rejects AppError('other'). */
  listExternalTools(refresh: boolean): Promise<ExternalToolScan>;
  /** P112 §5.4: opens a NATIVE dialog in the BACKEND, which also WRITES the
   *  selection (both `custom<Kind>Path` and `<kind>Tool = 'custom'`). There is
   *  no way to pass a path in — the renderer only asks for the dialog.
   *
   *  Resolves `null` on cancel (nothing written). A refused selection rejects
   *  AppError('externalToolFailed') and writes nothing either — not the path,
   *  and not the selection; the message is category-only and never echoes the
   *  path, so the UI shows its own copy (`P112-ui.md` §8 `BROWSE_ERR`).
   *
   *  After a non-null resolve, RE-READ settings; do NOT also patch
   *  `{ editorTool: 'custom' }` or you race the backend's own write. */
  pickExternalTool(kind: ExternalToolKind): Promise<DetectedTool | null>;
}
