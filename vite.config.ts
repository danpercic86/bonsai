/// <reference types="vitest/config" />
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  test: {
    // Node environment is enough — the helpers under test are pure string logic.
    environment: 'node',
    include: ['src/**/*.test.{ts,tsx}'],
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
