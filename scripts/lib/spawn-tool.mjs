// Bonsai — shell-free launcher for the repo's Node tooling (used by scripts/gate.mjs).
//
// WHY: `spawnSync(cmd, argsArray, { shell: true })` is deprecated as of Node 22
// (DEP0190) and rightly so — with a shell, the args array is CONCATENATED into
// one command line without escaping, so any argument containing a shell
// metacharacter is interpreted rather than passed. That is the same
// argv-versus-shell class we audit for in the app itself.
//
// But `shell: true` was load-bearing on Windows: `pnpm` there is `pnpm.cmd`, a
// batch shim, and Windows' CreateProcess cannot execute a batch file — Node
// (since the CVE-2024-27980 fix) refuses to spawn `.cmd`/`.bat` without a shell.
// So the fix is not "drop the flag", it is "resolve the real executable".
//
// Resolution order, all shell-free:
//   1. `pnpm`: its own JS entry point.
//      a. `npm_execpath` — pnpm sets this for every `pnpm run` child, so the
//         documented entry point (`pnpm gate`) always takes this path;
//      b. otherwise `<dir-of-pnpm-shim>/node_modules/pnpm/bin/pnpm.cjs`.
//      Run with `npm_node_execpath` (the node that launched us) and a plain
//      argv array — no shell, nothing to escape.
//   2. A real executable on PATH (`cargo.exe`, `rustup.exe`, POSIX `pnpm`),
//      found by walking PATH × PATHEXT ourselves.
//   3. Last resort (a `.cmd`/`.bat` shim with no JS entry beside it — e.g. a
//      corepack shim invoked via bare `node scripts/gate.mjs`): ComSpec with
//      `windowsVerbatimArguments`, where WE build and quote the command line
//      instead of letting Node concatenate it.
//
// Zero dependencies; identical behaviour on Windows, macOS and Linux.

import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { delimiter, dirname, join } from 'node:path';

const isWin = process.platform === 'win32';

/** Extensions a bare command name may resolve to. `['']` off Windows. */
const PATH_EXTS = isWin
  ? (process.env.PATHEXT ?? '.COM;.EXE;.BAT;.CMD').split(';').filter(Boolean)
  : [''];

function pathDirs() {
  const raw = process.env.PATH ?? process.env.Path ?? '';
  return raw
    .split(delimiter)
    .map((d) => d.trim().replace(/^"|"$/g, ''))
    .filter(Boolean);
}

/** Every PATH hit for `name`, in PATH order then PATHEXT order. */
function whichAll(name) {
  const hits = [];
  for (const dir of pathDirs()) {
    for (const ext of PATH_EXTS) {
      const candidate = join(dir, name + ext);
      if (existsSync(candidate)) hits.push(candidate);
    }
  }
  return hits;
}

const isBatch = (p) => /\.(cmd|bat)$/i.test(p);

/** pnpm's JS entry, so we can run it with this node and no shell. */
function pnpmJsEntry() {
  const fromEnv = process.env.npm_execpath;
  if (fromEnv !== undefined && /\.[cm]?js$/i.test(fromEnv) && existsSync(fromEnv)) return fromEnv;
  for (const shim of whichAll('pnpm')) {
    for (const entry of ['pnpm.cjs', 'pnpm.mjs']) {
      const candidate = join(dirname(shim), 'node_modules', 'pnpm', 'bin', entry);
      if (existsSync(candidate)) return candidate;
    }
  }
  return null;
}

/**
 * Quote one argument for a cmd.exe command line we assemble ourselves
 * (`windowsVerbatimArguments`). Backslashes before a quote — and at the end of
 * a quoted run — must be doubled, per the CommandLineToArgvW rules.
 *
 * `%` is NOT in the trigger set, deliberately (reviewer, 2026-09-03). Quoting
 * cannot neutralise it: cmd.exe expands `%VAR%` *inside* double quotes, so
 * `%PATH%` would still reach the child expanded, and there is no escape for `%`
 * in a `cmd /c "…"` line. Listing it here would advertise protection this
 * function does not provide. `assertNoPercent` below is the real answer.
 */
function quoteWinArg(arg) {
  if (arg !== '' && !/[\s"&|<>^()!]/.test(arg)) return arg;
  const escaped = arg.replace(/(\\*)"/g, '$1$1\\"').replace(/(\\*)$/, '$1$1');
  return `"${escaped}"`;
}

/**
 * Resolve a tool name to something spawnable WITHOUT a shell.
 * @returns {{ file: string, args: string[], verbatim: boolean }}
 */
export function resolveTool(name, args) {
  if (name === 'pnpm') {
    const js = pnpmJsEntry();
    if (js !== null) {
      const node = process.env.npm_node_execpath ?? process.execPath;
      return { file: node, args: [js, ...args], verbatim: false };
    }
  }

  const hits = whichAll(name);
  const direct = hits.find((p) => !isBatch(p));
  if (direct !== undefined) return { file: direct, args, verbatim: false };

  const shim = hits[0];
  if (shim !== undefined) {
    // A batch shim and nothing better. Build the command line ourselves so the
    // quoting is explicit and auditable rather than a blind concatenation.
    // Reject rather than pretend: see `quoteWinArg` — cmd.exe expands `%VAR%`
    // inside quotes, so an arg containing `%` cannot be passed through this
    // branch intact. Every caller today passes literal args, so this throws for
    // nobody; it exists so a future one finds out at the call site instead of
    // silently receiving an expanded environment variable.
    for (const a of args) {
      if (a.includes('%')) {
        throw new Error(
          `spawn-tool: cannot pass an argument containing '%' to the batch-shim ` +
            `fallback (cmd.exe would expand it): ${a}`,
        );
      }
    }
    const line = [shim, ...args].map(quoteWinArg).join(' ');
    const comspec = process.env.ComSpec ?? process.env.COMSPEC ?? 'cmd.exe';
    return { file: comspec, args: ['/d', '/s', '/c', `"${line}"`], verbatim: true };
  }

  // Not on PATH at all — hand the bare name to spawn so the caller gets the
  // usual ENOENT (`have()` treats that as "tool absent").
  return { file: name, args, verbatim: false };
}

/** `spawnSync` for a repo tool, never through a shell. */
export function runTool(name, args, options = {}) {
  const { file, args: argv, verbatim } = resolveTool(name, args);
  return spawnSync(file, argv, {
    ...options,
    ...(verbatim ? { windowsVerbatimArguments: true } : {}),
  });
}
