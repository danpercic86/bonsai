// P112 §6: the external-tool picker's IPC surface. Thin `invoke` wrappers — all
// detection, validation and the native dialog live in Rust.
//
// `pickExternalTool` deliberately takes only a `kind`: the dialog is opened BY
// THE BACKEND and its result never crosses IPC inbound, so there is no path
// parameter to add here. Do NOT "helpfully" switch this to
// `@tauri-apps/plugin-dialog` + send the path down (the `openRepo` shape) —
// that makes the dialog advisory and re-opens the exact hole P112 closes.
import { invoke } from './invoke';
import type { DetectedTool, ExternalToolKind, ExternalToolScan } from '../types';

export const toolsCommands = {
  listExternalTools(refresh: boolean): Promise<ExternalToolScan> {
    return invoke<ExternalToolScan>('list_external_tools', { refresh });
  },

  // The backend has already persisted the selection when this resolves non-null;
  // the caller re-reads settings rather than patching, or it races that write.
  pickExternalTool(kind: ExternalToolKind): Promise<DetectedTool | null> {
    return invoke<DetectedTool | null>('pick_external_tool', { kind });
  },
};
