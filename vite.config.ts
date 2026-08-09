/// <reference types="vitest/config" />
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  test: {
    // Root-level coverage aggregates across both projects (T1 contract §1.3).
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'json-summary'],
      reportsDirectory: 'coverage',
      include: ['src/**/*.{ts,tsx}'],
      exclude: [
        'src/**/*.test.{ts,tsx}',
        'src/test/**',
        'src/ipc/tauri.ts', // requires the Tauri runtime; e2e/native territory
        'src/**/*.d.ts',
      ],
      // NO thresholds in T1 — baseline first; numbers recorded in COVERAGE.md.
    },
    projects: [
      {
        // Pure-logic tests: *.test.ts run in node (all pre-T1 tests).
        extends: true,
        test: { name: 'node', environment: 'node', include: ['src/**/*.test.ts'] },
      },
      {
        // Component/hook tests: *.test.tsx run in jsdom with the RTL setup.
        // VITE_MOCK_IPC=1 makes `src/ipc/index.ts` resolve to the mock layer,
        // exactly like the browser harness — no per-test IPC mocking needed.
        extends: true,
        test: {
          name: 'dom',
          environment: 'jsdom',
          include: ['src/**/*.test.tsx'],
          setupFiles: ['src/test/setup.ts'],
          env: { VITE_MOCK_IPC: '1' },
        },
      },
    ],
  },
  server: {
    // PORT override lets a second dev server (e.g. another agent session) pick
    // a free port; 1420 stays the default that tauri.conf.json expects.
    port: Number(process.env.PORT) || 1420,
    strictPort: true,
    watch: {
      // Never watch the Rust workspace — cargo builds churn the target dir
      // and crash the dev-server watcher (EBUSY on locked .dll/.pdb files).
      // This is a cargo workspace: build output lands in the repo-root target/,
      // not src-tauri/target/, so both must be ignored.
      ignored: ['**/src-tauri/**', '**/target/**', '**/crates/*/target/**'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
});
