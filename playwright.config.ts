// T1 e2e config — runs the smoke suite against the mock-IPC browser harness.
//
// Local-Windows rule: NEVER download browsers to C:. Windows Defender ASR blocks
// freshly-downloaded, low-prevalence executables, so a bundled-chromium install
// is both unwanted and unreliable there — the `msedge` channel reuses the
// system-installed Edge and downloads nothing. If a browser install is ever
// needed on Windows anyway, set PLAYWRIGHT_BROWSERS_PATH=D:\Data\Temp\ms-playwright
// first.
//
// That workaround is Windows-specific, NOT a cross-platform default: macOS and
// Linux contributors (and CI) get Playwright's bundled chromium, which they can
// install with `pnpm exec playwright install chromium`.
import { defineConfig } from '@playwright/test';
import { cpus } from 'node:os';

const CI = !!process.env.CI;
/** Local Windows only — see the ASR note above. */
const USE_EDGE_CHANNEL = !CI && process.platform === 'win32';

// P94 isolation: e2e gets its OWN port, separate from the 1420 dev server a
// human/orchestrator drives by hand. Two reasons, both observed:
//   1. `reuseExistingServer` on 1420 silently adopts whatever is listening —
//      including a plain `pnpm dev` (no VITE_MOCK_IPC), which boots the app
//      with the real Tauri IPC and leaves every spec on the empty state.
//   2. Sharing one server with an interactive harness session means the suite
//      competes with a live browser tab for the same dev-server transform
//      pipeline, which is exactly the resource the suite is bottlenecked on.
// PORT still overrides, so a second worktree can pick another free port.
const DEV_PORT = Number(process.env.PORT) || 1430;

export default defineConfig({
  testDir: 'e2e',
  fullyParallel: true,
  forbidOnly: CI,
  retries: CI ? 1 : 0,
  // P94: cap LOCAL parallelism. Playwright's default is half the CPU cores; on a
  // 22-core dev box that is 11 concurrent Edge instances plus 11 page loads
  // against a single Vite DEV server, which oversubscribes the machine badly —
  // measured on the full suite: 11 workers => 7-12 failures/run in 5.8-6.3 min
  // (CDP "session closed" mid-test, `page.goto` timing out at "load", whole
  // spec files dying in cascade, failure identity shifting run to run);
  // 6 workers => 3 failures in 3.6 min; 4 workers => 0-1 in ~4.5 min (and the
  // residual one was a per-spec row-map race, since hardened in helpers.ts's
  // clickGraphRowUntilVisible, not a worker-count problem).
  // So the cap is not a throughput trade at all — it is at worst break-even and
  // it is what makes the suite deterministic. Nothing is serialised: 4 workers
  // still run 4 specs at a time, and `--workers=N` overrides this.
  // CI keeps Playwright's default: CI runners have 2-4 cores, so the default is
  // already 1-2 workers and this cap would never bind.
  // Adaptive so the cap is portable: it must never EXCEED Playwright's own
  // default (half the cores), or it would oversubscribe a small box — which is
  // the exact failure this cap exists to prevent.
  workers: CI ? undefined : Math.max(1, Math.min(4, Math.floor(cpus().length / 2))),
  // 'list' always; add the HTML report + trace zips in CI so a CI-only
  // failure (one that doesn't reproduce locally) is actually diagnosable
  // from the uploaded artifact instead of just the text log.
  reporter: CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: `http://localhost:${DEV_PORT}`,
    trace: 'on-first-retry',
  },
  projects: [
    USE_EDGE_CHANNEL
      ? { name: 'msedge', use: { browserName: 'chromium', channel: 'msedge' } } // installed Edge — no download, ASR-safe
      : { name: 'chromium', use: { browserName: 'chromium' } }, // bundled chromium (CI + local macOS/Linux)
  ],
  webServer: {
    command: 'pnpm dev:mock',
    // vite.config.ts reads PORT (strictPort), so passing it here is what pins
    // the spawned server to the e2e port instead of the shared 1420 one.
    env: { PORT: String(DEV_PORT) },
    port: DEV_PORT,
    // Safe now that the port is e2e-only: anything already listening there was
    // started by this same `dev:mock` command, not by a hand-run `pnpm dev`.
    reuseExistingServer: !CI,
    timeout: 120_000,
  },
});
