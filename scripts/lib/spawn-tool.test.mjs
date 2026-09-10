/** scripts/lib/spawn-tool.mjs — the shell-free launcher the gate spawns through.
 *
 *  The point of the module is that NOTHING goes through a shell (Node DEP0190:
 *  `shell: true` concatenates the args array into one command line without
 *  escaping). These tests pin the three resolution branches, including the
 *  Windows batch-shim last resort, whose hand-built command line is the one
 *  place quoting is ours to get right.
 */
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { delimiter, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';

import { resolveTool, runTool } from './spawn-tool.mjs';

const isWin = process.platform === 'win32';

/** This file is a real `.mjs` on disk — a stand-in for pnpm's own JS entry. */
const SELF = fileURLToPath(import.meta.url);

/** Temp dirs created by a test, removed after it. */
const dirs = [];
function mkTempDir() {
  const dir = mkdtempSync(join(tmpdir(), 'bonsai-spawn-'));
  dirs.push(dir);
  return dir;
}

let saved;
beforeEach(() => {
  saved = {
    PATH: process.env.PATH,
    Path: process.env.Path,
    npm_execpath: process.env.npm_execpath,
    npm_node_execpath: process.env.npm_node_execpath,
  };
});
afterEach(() => {
  for (const [k, v] of Object.entries(saved)) {
    if (v === undefined) delete process.env[k];
    else process.env[k] = v;
  }
  while (dirs.length > 0) rmSync(dirs.pop(), { recursive: true, force: true });
});

function setPath(dir) {
  process.env.PATH = dir;
  if (isWin) process.env.Path = dir;
}

describe('resolveTool', () => {
  it('runs pnpm as a JS entry under the current node — never a shell', () => {
    process.env.npm_execpath = SELF;
    process.env.npm_node_execpath = process.execPath;

    const r = resolveTool('pnpm', ['lint:size']);

    expect(r.file).toBe(process.execPath);
    expect(r.args).toEqual([SELF, 'lint:size']);
    expect(r.verbatim).toBe(false);
  });

  it('ignores a non-JS npm_execpath (a .cmd shim is not runnable by node)', () => {
    process.env.npm_execpath = join('C:', 'nope', 'pnpm.cmd');
    setPath(mkTempDir());

    const r = resolveTool('pnpm', ['build']);

    expect(r.args[0]).not.toMatch(/\.cmd$/i);
  });

  it('passes an unresolvable tool through so the caller gets the usual ENOENT', () => {
    delete process.env.npm_execpath;
    setPath(mkTempDir());

    expect(resolveTool('definitely-not-a-tool', ['--version'])).toEqual({
      file: 'definitely-not-a-tool',
      args: ['--version'],
      verbatim: false,
    });
  });

  it('prefers a real executable over a batch shim of the same name', () => {
    const dir = mkTempDir();
    // `.cmd` sorts after `.exe` in PATHEXT, but the preference must not depend
    // on that — it is an explicit "not a batch file" filter.
    writeFileSync(join(dir, 'toolx.cmd'), '');
    writeFileSync(join(dir, 'toolx.exe'), '');
    setPath(dir);

    const r = resolveTool('toolx', ['a']);

    if (isWin) {
      expect(r.file.toLowerCase()).toBe(join(dir, 'toolx.exe').toLowerCase());
      expect(r.verbatim).toBe(false);
    } else {
      // Off Windows there is no PATHEXT: `toolx` itself is not on this PATH.
      expect(r.file).toBe('toolx');
    }
  });

  it('resolves a name that already carries its extension', () => {
    // PATHEXT never contains an empty entry, so probing `toolx.exe` used to try
    // `toolx.exe.COM`, `toolx.exe.EXE`, … and find nothing.
    const dir = mkTempDir();
    writeFileSync(join(dir, isWin ? 'toolx.exe' : 'toolx'), '');
    if (!isWin) chmodSync(join(dir, 'toolx'), 0o755);
    setPath(dir);

    const r = resolveTool(isWin ? 'toolx.exe' : 'toolx', ['a']);

    expect(r.file.toLowerCase()).toBe(join(dir, isWin ? 'toolx.exe' : 'toolx').toLowerCase());
    expect(r.verbatim).toBe(false);
  });

  it('never resolves to a directory that merely has an executable name', () => {
    const dir = mkTempDir();
    mkdirSync(join(dir, isWin ? 'toolx.exe' : 'toolx'));
    setPath(dir);

    // No file hit at all → the bare name falls through to the ENOENT path.
    expect(resolveTool('toolx', ['a']).file).toBe('toolx');
  });

  it.runIf(isWin)('ignores an extensionless sibling of a Windows shim', () => {
    // npm/corepack ship a POSIX shell script beside every `.cmd` shim
    // (`pnpm`, `pnpm.cmd`, `pnpm.ps1`); CreateProcess cannot run it, so it must
    // never be picked as the "real executable".
    const dir = mkTempDir();
    writeFileSync(join(dir, 'toolx'), '#!/bin/sh\nexit 0\n');
    writeFileSync(join(dir, 'toolx.cmd'), '@echo off\r\n');
    setPath(dir);
    delete process.env.npm_execpath;

    const r = resolveTool('toolx', ['a']);

    // The batch shim is the only usable hit → ComSpec last resort, not the script.
    expect(r.verbatim).toBe(true);
    expect(r.args[3].toLowerCase()).toContain('toolx.cmd');
  });
});

describe.runIf(isWin)('Windows batch-shim last resort', () => {
  /** A `.cmd` that echoes the argv it received, as JSON, via node. */
  function shimDir() {
    const dir = mkTempDir();
    const probe = join(dir, 'probe.cjs');
    writeFileSync(probe, 'console.log(JSON.stringify(process.argv.slice(2)));\n');
    writeFileSync(
      join(dir, 'echoargs.cmd'),
      `@echo off\r\n"${process.execPath}" "${probe}" %*\r\n`,
    );
    chmodSync(join(dir, 'echoargs.cmd'), 0o755);
    return dir;
  }

  it('builds an explicit ComSpec command line rather than concatenating blindly', () => {
    const dir = shimDir();
    setPath(dir);
    delete process.env.npm_execpath;

    const r = resolveTool('echoargs', ['plain', 'has space', 'a&b']);

    expect(r.verbatim).toBe(true);
    expect(r.args.slice(0, 3)).toEqual(['/d', '/s', '/c']);
    // Only the arguments that need quoting get quoted.
    expect(r.args[3]).toContain(' plain "has space" "a&b"');
  });

  it('delivers those arguments to the shim intact', () => {
    const dir = shimDir();
    const previous = process.env.PATH;
    setPath(`${dir}${delimiter}${previous ?? ''}`);
    delete process.env.npm_execpath;

    const r = runTool('echoargs', ['plain', 'has space', 'a&b'], { encoding: 'utf8' });

    expect(r.error).toBeUndefined();
    expect(r.status).toBe(0);
    expect(JSON.parse(r.stdout.trim())).toEqual(['plain', 'has space', 'a&b']);
  });
});
