#!/usr/bin/env node
// Bonsai — file-size ratchet.
//
// CLAUDE.md sets a ~500-line soft limit per file so no module becomes a
// god-file that every session must read in full. The tree already violates
// that limit in many places, so this check is a RATCHET rather than a gate:
//
//   * a file NOT in the baseline may never exceed the limit;
//   * a file IN the baseline may never grow beyond its recorded line count;
//   * shrinking a baselined file always passes (and is reported as reclaimed).
//
// Run:
//   node scripts/check-file-size.mjs                 # check (exit 1 on regression)
//   node scripts/check-file-size.mjs --update-baseline
//
// Plain Node ESM, zero dependencies, no shell built-ins — identical behaviour
// on Windows, macOS and Linux (all paths are normalised to forward slashes).

import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const LIMIT = 500;

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const BASELINE_PATH = join(REPO_ROOT, 'scripts', 'file-size-baseline.json');

/** Roots to scan, paired with the extensions that matter in each. */
const SCAN_TARGETS = [
  { root: 'crates', extensions: ['.rs'] },
  { root: 'src-tauri/src', extensions: ['.rs'] },
  { root: 'src', extensions: ['.ts', '.tsx'] },
  { root: 'e2e', extensions: ['.ts', '.tsx'] },
];

/** Directory names skipped anywhere in the tree. */
const EXCLUDED_DIRS = new Set(['target', 'node_modules', 'dist', 'coverage', '.git']);

/** Repo-relative path with forward slashes, so baselines are OS-independent. */
function toRepoPath(absolutePath) {
  return relative(REPO_ROOT, absolutePath).split(sep).join('/');
}

function hasExtension(name, extensions) {
  return extensions.some((extension) => name.endsWith(extension));
}

/** Recursively collect matching files under `absoluteDir`. */
function collectFiles(absoluteDir, extensions, out) {
  let entries;
  try {
    entries = readdirSync(absoluteDir, { withFileTypes: true });
  } catch {
    return out; // Root absent in this checkout — nothing to scan.
  }
  for (const entry of entries) {
    const absolute = join(absoluteDir, entry.name);
    if (entry.isDirectory()) {
      if (EXCLUDED_DIRS.has(entry.name)) continue;
      collectFiles(absolute, extensions, out);
    } else if (entry.isFile() && hasExtension(entry.name, extensions)) {
      out.push(absolute);
    }
  }
  return out;
}

/**
 * Line count, matching what an editor shows: the number of newline-separated
 * lines, ignoring a single trailing newline. CRLF and LF count the same.
 */
function countLines(absolutePath) {
  const text = readFileSync(absolutePath, 'utf8');
  if (text.length === 0) return 0;
  const withoutTrailingNewline = text.endsWith('\n') ? text.slice(0, -1) : text;
  let lines = 1;
  for (let i = 0; i < withoutTrailingNewline.length; i += 1) {
    if (withoutTrailingNewline.charCodeAt(i) === 10) lines += 1;
  }
  return lines;
}

/** Every scanned file as `{ path, lines }`, sorted by path. */
function scanTree() {
  const seen = new Set();
  const files = [];
  for (const { root, extensions } of SCAN_TARGETS) {
    const absoluteRoot = join(REPO_ROOT, ...root.split('/'));
    let stats;
    try {
      stats = statSync(absoluteRoot);
    } catch {
      continue;
    }
    if (!stats.isDirectory()) continue;
    for (const absolute of collectFiles(absoluteRoot, extensions, [])) {
      const path = toRepoPath(absolute);
      if (seen.has(path)) continue; // e.g. nested scan roots
      seen.add(path);
      files.push({ path, lines: countLines(absolute) });
    }
  }
  files.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  return files;
}

function readBaseline() {
  try {
    const parsed = JSON.parse(readFileSync(BASELINE_PATH, 'utf8'));
    return parsed && typeof parsed === 'object' && parsed.files && typeof parsed.files === 'object'
      ? parsed.files
      : {};
  } catch {
    return {}; // No baseline yet: every over-limit file is a new offender.
  }
}

function writeBaseline(overLimitFiles) {
  const files = {};
  for (const { path, lines } of overLimitFiles) files[path] = lines;
  const payload = {
    _comment:
      'Ratchet baseline for scripts/check-file-size.mjs — line counts of files already over the 500-line soft limit. These may only shrink. Regenerate with `pnpm lint:size -- --update-baseline`.',
    limit: LIMIT,
    files,
  };
  writeFileSync(BASELINE_PATH, `${JSON.stringify(payload, null, 2)}\n`, 'utf8');
}

function main() {
  const updateBaseline = process.argv.slice(2).includes('--update-baseline');
  const files = scanTree();
  const overLimit = files.filter((file) => file.lines > LIMIT);

  if (updateBaseline) {
    writeBaseline(overLimit);
    const excess = overLimit.reduce((sum, file) => sum + (file.lines - LIMIT), 0);
    console.log(
      `Baseline written: ${overLimit.length} file(s) over ${LIMIT} lines, ${excess} excess lines total.`,
    );
    console.log(`  ${toRepoPath(BASELINE_PATH)}`);
    return 0;
  }

  const baseline = readBaseline();
  const newOffenders = [];
  const grown = [];
  const shrunk = [];

  for (const { path, lines } of files) {
    const recorded = baseline[path];
    if (typeof recorded !== 'number') {
      if (lines > LIMIT) newOffenders.push({ path, lines });
      continue;
    }
    if (lines > recorded) grown.push({ path, lines, recorded });
    else if (lines < recorded) shrunk.push({ path, lines, recorded });
  }

  const totalExcess = overLimit.reduce((sum, file) => sum + (file.lines - LIMIT), 0);
  const worst = [...overLimit].sort((a, b) => b.lines - a.lines).slice(0, 5);

  console.log(`file-size ratchet — limit ${LIMIT} lines, ${files.length} files scanned`);
  console.log(`  offenders over limit: ${overLimit.length} (total excess ${totalExcess} lines)`);
  for (const file of worst) console.log(`    ${file.lines}  ${file.path}`);

  for (const file of shrunk) {
    console.log(
      `  shrunk: ${file.path} ${file.recorded} -> ${file.lines} (${file.recorded - file.lines} lines reclaimed)`,
    );
  }
  const reclaimed = shrunk.reduce((sum, file) => sum + (file.recorded - file.lines), 0);
  if (reclaimed > 0) {
    console.log(
      `  ${reclaimed} line(s) reclaimed across ${shrunk.length} file(s) — run \`pnpm lint:size -- --update-baseline\` to lock the win in.`,
    );
  }

  if (newOffenders.length === 0 && grown.length === 0) {
    console.log('OK — no file exceeded the limit and no baselined file grew.');
    return 0;
  }

  console.error('');
  console.error('FAIL — file-size ratchet regression:');
  for (const file of newOffenders) {
    console.error(
      `  NEW OFFENDER  ${file.path}: ${file.lines} lines (limit ${LIMIT}). Split it into smaller, focused files.`,
    );
  }
  for (const file of grown) {
    console.error(
      `  GREW          ${file.path}: ${file.recorded} -> ${file.lines} lines (+${file.lines - file.recorded}). Already over the limit; it may only shrink.`,
    );
  }
  console.error('');
  console.error(
    'Baselined files may shrink but never grow. If a split legitimately moves lines around,',
  );
  console.error('re-run with `pnpm lint:size -- --update-baseline` and commit the baseline.');
  return 1;
}

process.exit(main());
