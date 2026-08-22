// T1 e2e config — runs the smoke suite against the mock-IPC browser harness.
//
// Local-Windows rule: NEVER download browsers to C:. Windows Defender ASR blocks
// freshly-downloaded, low-prevalence executables, so a bundled-chromium install
// is both unwanted and unreliable there — the `msedge` channel reuses the
// system-installed Edge and downloads nothing. If a browser install is ever
// needed on Windows anyway, set PLAYWRIGHT_BROWSERS_PATH=D:\Temp\ms-playwright
// first.
//
// That workaround is Windows-specific, NOT a cross-platform default: macOS and
// Linux contributors (and CI) get Playwright's bundled chromium, which they can
// install with `pnpm exec playwright install chromium`.
import { defineConfig } from '@playwright/test';

const CI = !!process.env.CI;
/** Local Windows only — see the ASR note above. */
const USE_EDGE_CHANNEL = !CI && process.platform === 'win32';

// PORT override mirrors vite.config.ts, so a second agent session running in a
// separate worktree can drive its own dev server + e2e on a free port instead of
// silently reusing the default-1420 server of another checkout.
const DEV_PORT = Number(process.env.PORT) || 1420;

export default defineConfig({
  testDir: 'e2e',
  fullyParallel: true,
  forbidOnly: CI,
  retries: CI ? 1 : 0,
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
    port: DEV_PORT,
    reuseExistingServer: !CI,
    timeout: 120_000,
  },
});
