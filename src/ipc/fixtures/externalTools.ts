// P112 §6 — browser-harness fixtures for the external-tool picker.
//
// Data only: the seam LOGIC lives in `mock/handlers/tools.ts`, this is the
// table it serves. Shapes mirror `bonsai_core::tools::{DetectedTool,
// ExternalToolScan}` exactly — `detail` is the resolved absolute path for every
// non-`builtIn` row and the literal `'built in'` otherwise, and `present` is
// `true` for every probe-derived row (AMEND-3: only a remembered `custom` row
// whose path is gone is ever `false`).
//
// The label maps are ALL-OS on purpose (AMEND-1): a settings file synced from
// another machine names an id this host has no row for, and the picker must
// still be able to name it ("Zed — not installed", never `zed`).
import type { DetectedTool, ExternalToolKind } from '../types';

/** `'custom'` — the reserved pseudo-id, mirroring Rust's `CUSTOM_ID`. */
export const CUSTOM_ID = 'custom';

function tool(
  id: string,
  label: string,
  kind: ExternalToolKind,
  source: DetectedTool['source'],
  detail: string,
): DetectedTool {
  return { id, label, kind, source, detail, present: true };
}

/** The default (Windows-flavoured) populated scan — harness states 1 and 3.
 *
 *  P112 §16.9 / UA14: the `source: 'path'` rows carry the **`PATHEXT` spelling**
 *  of their extension (`wt.EXE`, `code.CMD`), because that is what a `PATH`
 *  resolution really returns — `PATHEXT` is conventionally uppercase and Windows
 *  paths are case-insensitive, so the resolution carries the `PATHEXT` casing
 *  while the on-disk name is lowercase (`procutil_tests.rs:26-33`). The renderer
 *  displays `detail` byte-identically, so the fixture has to be able to catch one
 *  that "tidies" it. Do NOT normalise these. */
export const WIN_TERMINALS: DetectedTool[] = [
  tool(
    'windows-terminal',
    'Windows Terminal',
    'terminal',
    'path',
    'C:\\Users\\dev\\AppData\\Local\\Microsoft\\WindowsApps\\wt.EXE',
  ),
  tool('powershell', 'Windows PowerShell', 'terminal', 'builtIn', 'built in'),
  tool('cmd', 'Command Prompt', 'terminal', 'builtIn', 'built in'),
];

export const WIN_EDITORS: DetectedTool[] = [
  // `Rung::OnPath` is the FIRST rung of the Windows `vscode` row
  // (`catalog_table.rs:133-140`), so the usual resolution is a `PATH` hit on
  // `code` — a `.cmd` shim, reported with the `PATHEXT` casing.
  tool(
    'vscode',
    'Visual Studio Code',
    'editor',
    'path',
    'C:\\Users\\dev\\AppData\\Local\\Programs\\Microsoft VS Code\\bin\\code.CMD',
  ),
  tool('sublime', 'Sublime Text', 'editor', 'path', 'C:\\Program Files\\Sublime Text\\subl.exe'),
  tool(
    'notepadpp',
    'Notepad++',
    'editor',
    'registry',
    'C:\\Program Files\\Notepad++\\notepad++.exe',
  ),
];

/** `?tools=mac` — bundle resolutions, `source: 'appBundle'`, `.app` details. */
export const MAC_TERMINALS: DetectedTool[] = [
  tool(
    'apple-terminal',
    'Terminal',
    'terminal',
    'appBundle',
    '/System/Applications/Utilities/Terminal.app',
  ),
  tool('iterm2', 'iTerm', 'terminal', 'appBundle', '/Applications/iTerm.app'),
];

export const MAC_EDITORS: DetectedTool[] = [
  tool(
    'vscode',
    'Visual Studio Code',
    'editor',
    'appBundle',
    '/Applications/Visual Studio Code.app',
  ),
  tool('zed', 'Zed', 'editor', 'appBundle', '/Applications/Zed.app'),
];

/** `?tools=longlabels` — one 64-char label + one 180-char detail path, and a
 *  deep unix path, so the row's truncation rules are visible (UI state 14). */
const LONG_LABEL = 'Some Portable Editor 2026 Edition — Nightly Channel Build 99'.padEnd(64, '.');
const LONG_DETAIL =
  'C:\\Users\\a.very.long.account.name\\AppData\\Local\\Programs\\Some Portable Editor 2026 Edition\\channels\\nightly\\bin\\'.padEnd(
    180 - 'editor-cli-launcher.exe'.length,
    'x',
  ) + 'editor-cli-launcher.exe';

/** A detail the BACKEND has already truncated: `sanitize_detail` cuts at 512
 *  content chars and appends `…`, so 513 chars ending in an ellipsis is a shape
 *  the renderer really receives (`tools/custom.rs:355-369`). Nothing else in the
 *  harness shows that `detail` is not reliably a well-formed path — and
 *  `ToolPathLabel` splits this string on its last separator. */
const TRUNCATED_DETAIL =
  'C:\\Users\\a.very.long.account.name\\AppData\\Local\\Programs\\'.padEnd(512, 'y') + '…';

export const LONG_EDITORS: DetectedTool[] = [
  tool('vscode', LONG_LABEL, 'editor', 'wellKnown', LONG_DETAIL),
  tool(
    'zed',
    'Zed',
    'editor',
    'path',
    '/usr/local/lib/vendor-tools/editors/zed/2026.09/bin/zed-cli-launcher',
  ),
  tool('sublime', 'Sublime Text', 'editor', 'path', TRUNCATED_DETAIL),
];

/** The pathological browsed path of `?tools=custom` / `?tools=customgone`
 *  (118 chars — `P112-ui.md` §12 names the exact string). */
export const PATHOLOGICAL_CUSTOM_PATH =
  'C:\\Users\\a.very.long.account.name\\AppData\\Local\\Programs\\Some Portable Editor 2026 Edition\\bin\\editor-cli-launcher.exe';

/** `id -> label` for EVERY catalog entry of that kind, on EVERY OS (AMEND-1).
 *  Mirrors `tools::label_map`, so ids absent from the lists above are still
 *  nameable — which is what `?tools=stale` exercises. */
export const TERMINAL_LABELS: Record<string, string> = {
  'windows-terminal': 'Windows Terminal',
  powershell: 'Windows PowerShell',
  pwsh: 'PowerShell 7',
  cmd: 'Command Prompt',
  'git-bash': 'Git Bash',
  'apple-terminal': 'Terminal',
  iterm2: 'iTerm',
  warp: 'Warp',
  kitty: 'kitty',
  'gnome-terminal': 'GNOME Terminal',
  konsole: 'Konsole',
  'kitty-linux': 'kitty',
  alacritty: 'Alacritty',
  wezterm: 'WezTerm',
  'xfce4-terminal': 'Xfce Terminal',
  'x-terminal-emulator': 'System terminal',
};

export const EDITOR_LABELS: Record<string, string> = {
  vscode: 'Visual Studio Code',
  'vscode-insiders': 'VS Code Insiders',
  vscodium: 'VSCodium',
  cursor: 'Cursor',
  sublime: 'Sublime Text',
  notepadpp: 'Notepad++',
  idea: 'IntelliJ IDEA',
  zed: 'Zed',
  kate: 'Kate',
  gedit: 'Text Editor',
};
