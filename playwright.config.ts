// T1 e2e config — runs the smoke suite against the mock-IPC browser harness.
//
// Local-Windows rule: NEVER download browsers to C:. The `msedge` channel uses
// the system-installed Edge, so nothing is downloaded. If a browser install is
// ever needed locally, set PLAYWRIGHT_BROWSERS_PATH=D:\Temp\ms-playwright first.
import { defineConfig } from '@playwright/test';

const CI = !!process.env.CI;

export default defineConfig({
  testDir: 'e2e',
  fullyParallel: true,
  forbidOnly: CI,
  retries: CI ? 1 : 0,
  use: {
    baseURL: 'http://localhost:1420',
    trace: 'on-first-retry',
  },
  projects: [
    CI
      ? { name: 'chromium', use: { browserName: 'chromium' } } // bundled chromium (ubuntu CI)
      : { name: 'msedge', use: { browserName: 'chromium', channel: 'msedge' } }, // installed Edge — no download, ASR-safe
  ],
  webServer: {
    command: 'pnpm dev:mock',
    port: 1420,
    reuseExistingServer: !CI,
    timeout: 120_000,
  },
});
