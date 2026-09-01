// The webServer Playwright spawns for the e2e suite. Two modes, one process:
//
//   default        -> Vite DEV server on 1430 (what the suite always used)
//   E2E_BUNDLE=1   -> `vite build --mode mock` + `vite preview` on 1440
//
// Why bundle mode exists: the dev server's per-request transform pipeline is
// what the suite is bottlenecked on. Serving a prebuilt bundle removes it —
// measured on the full 161-test suite, the summed per-test time drops
// 565s -> 326s.
//
// Mode matters: the specs need the mock IPC layer, which `src/ipc/index.ts`
// selects on `VITE_MOCK_IPC=1` — supplied by `.env.mock`, i.e. only by
// `--mode mock`. A plain `pnpm build` is REAL mode and boots the app against
// the Tauri IPC, so the gate's `tsc + vite build` artifact is NOT reusable
// here. `tsc` is deliberately skipped: the gate already type-checks via
// `pnpm build`, and duplicating it would eat the speedup.
//
// Vite's JS API, NOT a spawned CLI — this is the load-bearing detail. Measured
// on Windows, the previous `command: 'pnpm dev:mock'` left the dev server
// ORPHANED when Playwright tore the webServer down: the port stayed bound and
// `playwright test` hung for MINUTES after the last test finished (full-suite
// wall clock 446s against 178s of actual testing). Playwright's process-tree
// kill does not reach a grandchild behind the pnpm + .cmd shim layers. Hosting
// the server inside THIS process means the kill always takes the server with
// it. Bundle mode had the identical bug for the same reason.
import { build, createServer, preview } from 'vite';
import { resolve } from 'node:path';

const bundle = process.env.E2E_BUNDLE === '1';

// Teardown, measured on Windows: Playwright logs "Terminating the WebServer",
// then WAITS for the server process to exit. A Node process holding a listening
// socket does not exit on its own, and the signal Playwright sends does not
// reliably reach it through the shell layer it was spawned behind — the port
// stayed bound and `playwright test` hung for MINUTES after the last test
// (full-suite wall clock 446s for 178s of actual testing; that dead time is
// most of the "e2e takes 5.8 min" figure on the board). Two belts:
//   1. explicit signal handlers, paired with `gracefulShutdown` in
//      playwright.config.ts so a signal is actually sent;
//   2. stdin EOF — Playwright closes the pipe on teardown, so this fires even
//      if no signal is delivered at all.
const die = () => process.exit(0);
process.on('SIGTERM', die);
process.on('SIGINT', die);
process.on('SIGHUP', die);
process.stdin.on('end', die);
process.stdin.on('close', die);
process.stdin.resume();

/** Port allocation on this project, all explicit: 1420 = hand-driven
 *  `dev:mock` harness, 1430 = e2e dev server, 1440 = e2e built bundle.
 *  Separate ports mean the two e2e modes never adopt each other's server. */
const port = Number(bundle ? process.env.E2E_BUNDLE_PORT : process.env.PORT) || (bundle ? 1440 : 1430);

/** Bundle output defaults into the repo (gitignored); override to put it on
 *  another volume (the local rule is "build output on the Defender-excluded
 *  drive") without hard-coding a machine path into committed config. */
const outDir = resolve(process.env.E2E_BUNDLE_DIR ?? 'dist-mock');

// strictPort in both modes: fail loudly on a stale listener rather than
// silently serving a different (possibly real-mode) app from a neighbour port.
if (bundle) {
  // 'warn': the default build reporter prints a ~170-line chunk table into
  // Playwright's webServer log on every run, which buries anything useful.
  await build({ mode: 'mock', logLevel: 'warn', build: { outDir, emptyOutDir: true } });
  const server = await preview({ mode: 'mock', build: { outDir }, preview: { port, strictPort: true } });
  server.printUrls();
} else {
  const server = await createServer({ mode: 'mock', server: { port, strictPort: true } });
  await server.listen();
  server.printUrls();
}
