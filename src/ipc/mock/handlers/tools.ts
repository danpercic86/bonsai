// P112 §6 — external-tool picker mock (`VITE_MOCK_IPC=1`).
//
// Serves the thirteen `?tools=` seams `P112-ui.md` §12 + §16.13 enumerate. The
// payload shapes mirror `bonsai_core::tools` exactly; the fixture tables live in
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

/** `?tools=browseadopterr` (§16.4a) — armed by a CONFIRMED pick, so the very next
 *  `listExternalTools` rejects: the Browse succeeded and was persisted, but the
 *  re-read that would show it did not land. Rejecting one half of the caller's
 *  `Promise.all` is enough, and it must fire ONCE — a permanently-failing list
 *  would be `?tools=scanerr`, which is a different state. */
let adoptFailArmed = false;

/** Armed by a CONFIRMED pick and consumed by the NEXT list call, so anything
 *  that ends the flow in between — a category switch, the end of a test, a seam
 *  change — would otherwise leak a spurious scan failure into the next mount.
 *  `useExternalToolScan.ts:91`'s `resetExternalToolScanCacheForTests` is the
 *  precedent and the same reason. Called from `beforeEach`, never from product
 *  code. (`tools.test.tsx` needs it only implicitly: it re-imports a fresh
 *  module graph per seam, which resets this too.) */
export function resetExternalToolMockForTests(): void {
  adoptFailArmed = false;
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

/** The refusal `listExternalTools` rejects with. Category-only: `AppError('other')`
 *  is what the command's contract declares (`ipc-api-tools.ts:13`), and nothing in
 *  it is actionable, which is why the UI shows its own `SCAN_ERR` copy instead. */
function scanFailure(): AppError {
  return { kind: 'other', message: 'tool detection failed' };
}

export const toolsHandlers = {
  async listExternalTools(refresh: boolean): Promise<ExternalToolScan> {
    // `?tools=scanerr` rejects on EVERY call, including the first — the only way
    // to reach §16.4 R5's cold-failure state, where the Rescan row has no state
    // note at all because `SCAN_NONE` would be a lie.
    if (seam() === 'scanerr') {
      await delay(150);
      throw scanFailure();
    }
    // The Browse follow-up read fails once (§16.4a `BROWSE_STALE`).
    if (adoptFailArmed) {
      adoptFailArmed = false;
      await delay(150);
      throw scanFailure();
    }
    // `?tools=slow` drives the COLD scanning state (7a): the first read is slow,
    // so a persisted selection has no label yet and the placeholder shows.
    // `?tools=slowrescan` is the 7b seam and the opposite case — the first read
    // is fast and only a `refresh: true` takes the measured 2.1 s cold number,
    // which is the only way to prove a rescan does NOT blank the picker.
    if (seam() === 'slowrescan' && refresh) await delay(2100);
    else if (seam() === 'slow') await delay(1200);
    else await delay(150);
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
    if (seam() === 'browseadopterr') adoptFailArmed = true;
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
