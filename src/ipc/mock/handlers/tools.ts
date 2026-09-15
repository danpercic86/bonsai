// P112 §6 — external-tool picker mock (`VITE_MOCK_IPC=1`).
//
// Serves the ten `?tools=` seams `P112-ui.md` §12 enumerates. The payload shapes
// mirror `bonsai_core::tools` exactly; the fixture tables live in
// `../../fixtures/externalTools.ts`.
//
// **This mock is NOT a security seam.** Rust owns the dialog,
// `validate_custom_program` and `coerce_tool_id` (the `validate_web_url`
// precedent) — there is no dialog in the harness and nothing here decides what
// may launch. What it DOES have to mirror faithfully is the backend's *write*:
// `pickExternalTool` persists `<kind>Tool = 'custom'` itself, so the frontend
// must never patch that key after a browse. A mock that silently skipped the
// write would hide exactly that bug.
import {
  CUSTOM_ID,
  EDITOR_LABELS,
  LONG_EDITORS,
  MAC_EDITORS,
  MAC_TERMINALS,
  PATHOLOGICAL_CUSTOM_PATH,
  TERMINAL_LABELS,
  WIN_EDITORS,
  WIN_TERMINALS,
} from '../../fixtures/externalTools';
import {
  readCustomToolPaths,
  readUiSettings,
  writeCustomToolPaths,
  writeUiSettings,
} from '../persistence';
import { delay, query } from '../repoState';
import type {
  AppError,
  DetectedTool,
  ExternalToolKind,
  ExternalToolScan,
  IpcApi,
  UiSettings,
} from '../../types';

/** The `?tools=` seam, read per call so a harness navigation takes effect. */
function seam(): string {
  return query('tools') ?? '';
}

/** The two browsed paths, persisted under their OWN localStorage key — never on
 *  the `UiSettings` blob, which has no field able to carry a path (AC15(d)).
 *  Backed by storage rather than module memory so Browse-then-RELOAD lists the
 *  row, as the backend does; module memory would leave `<kind>Tool: 'custom'`
 *  persisted with no row and no label, a state the backend cannot produce. */
function storedCustomPath(kind: ExternalToolKind): string {
  return readCustomToolPaths()[kind];
}

/** The seam's initial browsed path, applied only while nothing has been picked. */
function seededCustomPath(kind: ExternalToolKind): string {
  const stored = storedCustomPath(kind);
  if (stored) return stored;
  const s = seam();
  if (kind === 'editor' && (s === 'custom' || s === 'customgone')) return PATHOLOGICAL_CUSTOM_PATH;
  return '';
}

/** The remembered browsed row (AMEND-3): LISTED even when its path is gone, so
 *  the UI can render "custom path gone" instead of an unexplained empty picker. */
function customRow(kind: ExternalToolKind, path: string): DetectedTool {
  const stem = path.split(/[\\/]/).pop() ?? '';
  return {
    id: CUSTOM_ID,
    label: stem.replace(/\.[^.]+$/, ''),
    kind,
    source: 'custom',
    detail: path,
    // `?tools=customgone` is the one seam whose stored path no longer resolves;
    // a path the harness actually browsed to is present again.
    present: seam() !== 'customgone' || storedCustomPath(kind) !== '',
  };
}

function baseLists(): { terminals: DetectedTool[]; editors: DetectedTool[] } {
  switch (seam()) {
    case 'none':
      return { terminals: [], editors: [] };
    case 'mac':
      return { terminals: MAC_TERMINALS, editors: MAC_EDITORS };
    case 'longlabels':
      return { terminals: WIN_TERMINALS, editors: LONG_EDITORS };
    default:
      return { terminals: WIN_TERMINALS, editors: WIN_EDITORS };
  }
}

/** Monotonic freshness identity — bumped by `refresh: true` so the frontend can
 *  see that a Rescan landed (DEC-3: never displayed). */
let scannedAtMs = Date.UTC(2026, 8, 14, 9, 30, 0);

function buildScan(): ExternalToolScan {
  const { terminals, editors } = baseLists();
  const withCustom = (kind: ExternalToolKind, rows: DetectedTool[]): DetectedTool[] => {
    const path = seededCustomPath(kind);
    return path ? [...rows, customRow(kind, path)] : [...rows];
  };
  return {
    terminals: withCustom('terminal', terminals),
    editors: withCustom('editor', editors),
    // Populated even for `?tools=none`: the stale state needs them.
    terminalLabels: { ...TERMINAL_LABELS },
    editorLabels: { ...EDITOR_LABELS },
    scannedAtMs,
  };
}

/**
 * The seam's PERSISTED selection, layered under the harness's stored settings.
 *
 * Applied only where the stored value is still `''`, so a pick or a patch made
 * in the harness sticks instead of being re-overridden on the next read. Both
 * `getUiSettings` and `setUiSettings`' `current` go through this — otherwise
 * patching the *other* key would silently wipe the seam.
 */
export function applyToolSeam(s: UiSettings): UiSettings {
  const wanted = seam();
  let editorTool = s.editorTool;
  if (editorTool === '') {
    if (wanted === 'stale') editorTool = 'zed';
    else if (wanted === 'custom' || wanted === 'customgone') editorTool = CUSTOM_ID;
  }
  return editorTool === s.editorTool ? s : { ...s, editorTool };
}

export const toolsHandlers = {
  async listExternalTools(refresh: boolean): Promise<ExternalToolScan> {
    // `?tools=slow` drives the scanning state (UI state 7); a Rescan is slower
    // than a first read everywhere, so the delay applies to both.
    await delay(seam() === 'slow' ? 1200 : 150);
    if (refresh) scannedAtMs += 1000;
    return buildScan();
  },

  async pickExternalTool(kind: ExternalToolKind): Promise<DetectedTool | null> {
    // 600 ms so the in-flight state (UI state 8) is observable; there is no
    // dialog here, and the real one's timing is the user's.
    await delay(600);
    if (seam() === 'browsecancel') return null;
    if (seam() === 'browseerr') {
      // VERBATIM from `bonsai_core::tools::custom::refuse()` — the harness must
      // not invent a refusal string the backend never emits. That file
      // (`crates/bonsai-core/src/tools/custom.rs`, `fn refuse`) is the source of
      // truth, and `tools.test.tsx` pins this copy against it by reading the
      // Rust source — so editing the wording on either side alone fails a test
      // instead of silently desyncing the harness.
      const err: AppError = {
        kind: 'externalToolFailed',
        message:
          'that selection cannot be used as a program — choose a program file on this machine (on Windows, a `.exe`)',
      };
      throw err;
    }
    const path = PATHOLOGICAL_CUSTOM_PATH;
    writeCustomToolPaths({ ...readCustomToolPaths(), [kind]: path });
    // Mirrors the backend writing BOTH keys in one settings cycle: the caller
    // re-reads settings and must NOT patch `<kind>Tool` itself.
    const current = applyToolSeam(readUiSettings());
    writeUiSettings(
      kind === 'terminal'
        ? { ...current, terminalTool: CUSTOM_ID }
        : { ...current, editorTool: CUSTOM_ID },
    );
    return customRow(kind, path);
  },
} satisfies Partial<IpcApi>;
