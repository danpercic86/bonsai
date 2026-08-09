/**
 * Shared e2e fixture (T1 contract §2.2): every spec imports `test`/`expect`
 * from here, never from '@playwright/test' directly. The `page` fixture is
 * wrapped so any console error or uncaught page error fails the test — the
 * mock harness must stay console-clean.
 *
 * Opt-out per test via the `allowConsoleErrors` option (substring allowlist):
 *   test.use({ allowConsoleErrors: ['expected fragment'] });
 */
import { test as base, expect } from '@playwright/test';

interface ConsoleFixtures {
  allowConsoleErrors: string[];
}

export const test = base.extend<ConsoleFixtures>({
  allowConsoleErrors: [[], { option: true }],

  page: async ({ page, allowConsoleErrors }, use) => {
    const consoleErrors: string[] = [];
    const pageErrors: string[] = [];

    page.on('console', (msg) => {
      if (msg.type() !== 'error') return;
      const text = msg.text();
      if (allowConsoleErrors.some((allowed) => text.includes(allowed))) return;
      consoleErrors.push(text);
    });
    page.on('pageerror', (err) => {
      const text = err.message;
      if (allowConsoleErrors.some((allowed) => text.includes(allowed))) return;
      pageErrors.push(text);
    });

    await use(page);

    expect(consoleErrors, `console.error emitted:\n${consoleErrors.join('\n')}`).toEqual([]);
    expect(pageErrors, `uncaught page errors:\n${pageErrors.join('\n')}`).toEqual([]);
  },
});

export { expect };
