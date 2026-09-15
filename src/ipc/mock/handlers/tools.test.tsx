// P112 AC14 (the checkable half) — the ten `?tools=` harness seams serve the
// payload SHAPES `docs/contracts/P112-external-tool-detection.md` §6 specifies.
//
// Per-state UI assertions belong to `P112-ui.md` §13 and sub-increment 4; what
// is pinned here is the mock↔backend parity a stale mock would silently break:
// the DTO fields, the label maps being populated even for an empty scan, the
// `present` flag, the verbatim refusal string, and `pickExternalTool` writing
// the selection itself.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PATHOLOGICAL_CUSTOM_PATH } from '../../fixtures/externalTools';
import type { AppError, DetectedTool } from '../../types';

/** Point `window.location.search` at one seam, then import a FRESH module graph
 *  (the mock reads the query string per call, but `persistence` caches). */
async function withSeam(seam: string) {
  vi.resetModules();
  window.history.replaceState({}, '', seam ? `/?tools=${seam}` : '/');
  window.localStorage.clear();
  const tools = await import('./tools');
  return tools;
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  window.history.replaceState({}, '', '/');
});

/** `delay()` is a real timer in the mock; run the promise under fake timers. */
async function settle<T>(p: Promise<T>): Promise<T> {
  await vi.runAllTimersAsync();
  return p;
}

function ids(rows: DetectedTool[]): string[] {
  return rows.map((r) => r.id);
}

describe('P112 §6 mock — listExternalTools', () => {
  it('serves the default populated Windows scan with both label maps', async () => {
    const { toolsHandlers } = await withSeam('');
    const scan = await settle(toolsHandlers.listExternalTools(false));
    expect(ids(scan.terminals)).toEqual(['windows-terminal', 'powershell', 'cmd']);
    expect(ids(scan.editors)).toEqual(['vscode', 'sublime', 'notepadpp']);
    // `detail` is the resolved absolute path, or the literal 'built in'.
    expect(scan.terminals[1].source).toBe('builtIn');
    expect(scan.terminals[1].detail).toBe('built in');
    expect(scan.editors[0].detail).toBe('C:\\Program Files\\Microsoft VS Code\\Code.exe');
    // Every probe-derived row is present by construction (AMEND-3).
    expect([...scan.terminals, ...scan.editors].every((r) => r.present)).toBe(true);
    expect(scan.editorLabels.zed).toBe('Zed');
    expect(scan.terminalLabels['x-terminal-emulator']).toBe('System terminal');
  });

  it('?tools=none is an empty scan whose label maps are STILL populated', async () => {
    const { toolsHandlers } = await withSeam('none');
    const scan = await settle(toolsHandlers.listExternalTools(false));
    expect(scan.terminals).toEqual([]);
    expect(scan.editors).toEqual([]);
    // The stale state needs them even with nothing detected.
    expect(Object.keys(scan.editorLabels).length).toBeGreaterThan(5);
  });

  it('?tools=mac serves appBundle rows with .app details', async () => {
    const { toolsHandlers } = await withSeam('mac');
    const scan = await settle(toolsHandlers.listExternalTools(false));
    expect(scan.terminals.every((r) => r.source === 'appBundle')).toBe(true);
    expect(scan.editors[0].detail).toBe('/Applications/Visual Studio Code.app');
  });

  it('?tools=custom lists the remembered browsed row with present: true', async () => {
    const { toolsHandlers } = await withSeam('custom');
    const scan = await settle(toolsHandlers.listExternalTools(false));
    const custom = scan.editors.at(-1);
    expect(custom?.id).toBe('custom');
    expect(custom?.source).toBe('custom');
    expect(custom?.detail).toBe(PATHOLOGICAL_CUSTOM_PATH);
    expect(custom?.present).toBe(true);
    // The pathological fixture is the 118-char path §12 names.
    expect(PATHOLOGICAL_CUSTOM_PATH).toHaveLength(118);
  });

  it('?tools=customgone lists it anyway, with present: false', async () => {
    const { toolsHandlers } = await withSeam('customgone');
    const scan = await settle(toolsHandlers.listExternalTools(false));
    const custom = scan.editors.at(-1);
    expect(custom?.id).toBe('custom');
    expect(custom?.present).toBe(false);
    expect(custom?.detail).toBe(PATHOLOGICAL_CUSTOM_PATH);
  });

  it('?tools=stale persists an editorTool absent from the list, nameable from the map', async () => {
    const { applyToolSeam, toolsHandlers } = await withSeam('stale');
    const { DEFAULT_UI_SETTINGS } = await import('../../../settings/defaults');
    const settings = applyToolSeam(structuredClone(DEFAULT_UI_SETTINGS));
    expect(settings.editorTool).toBe('zed');
    const scan = await settle(toolsHandlers.listExternalTools(false));
    expect(ids(scan.editors)).not.toContain('zed');
    expect(scan.editorLabels.zed).toBe('Zed');
  });

  it('?tools=longlabels serves a 64-char label and a 180-char detail', async () => {
    const { toolsHandlers } = await withSeam('longlabels');
    const scan = await settle(toolsHandlers.listExternalTools(false));
    expect(scan.editors[0].label).toHaveLength(64);
    expect(scan.editors[0].detail).toHaveLength(180);
    expect(scan.editors[1].detail.startsWith('/usr/local/lib/')).toBe(true);
  });

  it('refresh: true advances scannedAtMs (the freshness identity)', async () => {
    const { toolsHandlers } = await withSeam('');
    const first = await settle(toolsHandlers.listExternalTools(false));
    const second = await settle(toolsHandlers.listExternalTools(true));
    expect(second.scannedAtMs).toBeGreaterThan(first.scannedAtMs);
  });
});

describe('P112 §5.4 mock — pickExternalTool', () => {
  it('resolves a canned custom row AND persists the selection itself', async () => {
    const { toolsHandlers } = await withSeam('');
    const { readUiSettings } = await import('../persistence');
    const row = await settle(toolsHandlers.pickExternalTool('editor'));
    expect(row?.id).toBe('custom');
    expect(row?.kind).toBe('editor');
    expect(row?.detail).toBe(PATHOLOGICAL_CUSTOM_PATH);
    // Mirrors the backend writing both keys: the caller must NOT patch this.
    expect(readUiSettings().editorTool).toBe('custom');
    // …and the row now appears in the scan.
    const scan = await settle(toolsHandlers.listExternalTools(false));
    expect(ids(scan.editors).at(-1)).toBe('custom');
  });

  it('the browsed path survives a RELOAD, so the row is still listed', async () => {
    const { toolsHandlers } = await withSeam('');
    await settle(toolsHandlers.pickExternalTool('editor'));
    // A reload: fresh module graph, localStorage deliberately NOT cleared.
    vi.resetModules();
    const reloaded = await import('./tools');
    const { readUiSettings } = await import('../persistence');
    expect(readUiSettings().editorTool).toBe('custom');
    const scan = await settle(reloaded.toolsHandlers.listExternalTools(false));
    const custom = scan.editors.at(-1);
    // Without storage-backed paths this row would be absent, leaving a
    // persisted `editorTool: 'custom'` with nothing to name — a state the
    // backend cannot produce.
    expect(custom?.id).toBe('custom');
    expect(custom?.detail).toBe(PATHOLOGICAL_CUSTOM_PATH);
    expect(custom?.present).toBe(true);
  });

  it('?tools=browsecancel resolves null and writes nothing', async () => {
    const { toolsHandlers } = await withSeam('browsecancel');
    const { readUiSettings } = await import('../persistence');
    expect(await settle(toolsHandlers.pickExternalTool('editor'))).toBeNull();
    expect(readUiSettings().editorTool).toBe('');
  });

  it('?tools=browseerr rejects with the backend refusal VERBATIM', async () => {
    const { toolsHandlers } = await withSeam('browseerr');
    const err = await settle(
      toolsHandlers.pickExternalTool('editor').then(
        () => null,
        (e: AppError) => e,
      ),
    );
    expect(err?.kind).toBe('externalToolFailed');
    // Byte-identical to `bonsai_core::tools::custom::refuse()` — and that is not
    // taken on trust: the describe block below reads the Rust source.
    expect(err?.message).toBe(BROWSE_REFUSAL);
  });
});

/** The backend refusal, spelled here exactly as `custom.rs`'s `refuse()` spells
 *  it. One literal in this file, pinned in BOTH directions below. */
const BROWSE_REFUSAL =
  'that selection cannot be used as a program — choose a program file on this machine (on Windows, a `.exe`)';

/** `crates/bonsai-core/src/tools/custom.rs`, read as text by Vite (the
 *  `AiConsentDialog.test.tsx` technique — no `node:fs`, this tsconfig has no
 *  node types). A glob so the lookup fails loudly if the file is ever moved. */
const TOOLS_RS = import.meta.glob('../../../../crates/bonsai-core/src/tools/*.rs', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

// Cross-language pin. `tools.ts` carries its own copy of the refusal because the
// harness has no backend to ask — which means a Rust-side rewording would
// otherwise leave the mock showing users a string the app never emits, with
// every frontend test still green. This is the test that goes red instead.
describe('the browse refusal is the backend’s wording, not the harness’s', () => {
  it('appears verbatim in bonsai_core::tools::custom::refuse()', () => {
    const path = Object.keys(TOOLS_RS).find((p) => p.endsWith('/custom.rs'));
    expect(path, 'tools/custom.rs moved — repoint this pin').toBeDefined();
    const source = TOOLS_RS[path as string];
    // Guard against a VACUOUS pass: if `refuse()` is renamed or gone, fail here
    // rather than "assert" against a file that no longer defines the string.
    const start = source.indexOf('fn refuse() -> AppError {');
    expect(start, '`refuse()` was renamed or removed — repoint this pin').toBeGreaterThanOrEqual(0);
    // Scope the search to refuse()'s BODY. Unscoped, the old wording surviving
    // ANYWHERE else in this ~400-line file — a doc comment, a sibling helper —
    // would keep this green while `refuse()` itself emitted something else.
    const bodyEnd = source.slice(start).search(/\r?\n\}/);
    expect(bodyEnd, 'refuse()’s body has no closing brace — repoint this pin').toBeGreaterThan(0);
    const body = source.slice(start, start + bodyEnd);
    // Undo Rust's `\<newline><indent>` continuation: the literal is wrapped over
    // two source lines there and is one string at runtime.
    expect(body.replace(/\\\r?\n\s*/g, '')).toContain(BROWSE_REFUSAL);
  });
});
