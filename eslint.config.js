// Bonsai — ESLint v9 flat config (frontend only).
//
// Scope: `src/` (React app + IPC layer) and `e2e/` (Playwright specs). Rust is
// covered by `cargo clippy -D warnings`; `src-tauri/`, `dist/`, `coverage/` and
// `target/` are ignored here.
//
// Severity policy — this config was introduced on an existing ~15k-LOC tree, so
// it is deliberately split in two:
//   * `error`   = genuine correctness bugs (hook rules, unsafe negation,
//                 accidental assignment in a condition, etc). CI fails on these.
//                 The tree is at ZERO errors and must stay there.
//   * `warn`    = style/strictness rules that fire broadly across code written
//                 before linting existed. They are visible in `pnpm lint` output
//                 as a cleanup backlog but do NOT fail CI.
// CI runs `pnpm lint:ci` = `eslint . --max-warnings 40`. The tree currently
// reports 30 warnings, so the budget leaves ~10 of headroom: a single new
// warning never blocks an unrelated PR, but a warning explosion (or a
// newly-enabled noisy rule) does fail the build. Lower the number as the
// backlog is cleaned up.
//
// No Prettier/formatting rules on purpose: the project has never had a
// formatter and adding one would rewrite every file.

import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';
import globals from 'globals';

export default tseslint.config(
  {
    // Build output, deps, Rust target dir, test artifacts.
    ignores: [
      'dist/**',
      'coverage/**',
      'node_modules/**',
      'target/**',
      'src-tauri/**',
      'crates/**',
      'playwright-report/**',
      'test-results/**',
      'public/**',
      'docs/**',
    ],
  },

  // ---------------------------------------------------------------------------
  // Frontend sources + Playwright specs.
  // ---------------------------------------------------------------------------
  {
    files: ['src/**/*.{ts,tsx}', 'e2e/**/*.{ts,tsx}'],
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: { ...globals.browser, ...globals.es2022 },
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    plugins: {
      'react-hooks': reactHooks,
      'react-refresh': reactRefresh,
    },
    rules: {
      // --- correctness: keep as errors -------------------------------------
      'react-hooks/rules-of-hooks': 'error',
      'no-unsafe-negation': 'error',
      'no-unsafe-optional-chaining': 'error',
      'no-cond-assign': ['error', 'always'],
      'no-dupe-else-if': 'error',
      'no-self-compare': 'error',
      'no-unmodified-loop-condition': 'error',
      'no-constant-binary-expression': 'error',
      eqeqeq: ['error', 'always', { null: 'ignore' }],
      '@typescript-eslint/no-misused-new': 'error',
      '@typescript-eslint/no-unsafe-declaration-merging': 'error',

      // --- future cleanup: warn only ---------------------------------------
      // `exhaustive-deps` has real findings in the older containers; fixing
      // them changes effect timing, so it is a reviewed cleanup, not a sweep.
      'react-hooks/exhaustive-deps': 'warn',
      // Vite HMR hygiene: mixed component/constant exports break fast refresh.
      'react-refresh/only-export-components': ['warn', { allowConstantExport: true }],
      // `any` is still used at a few IPC/serde boundaries.
      '@typescript-eslint/no-explicit-any': 'warn',
      '@typescript-eslint/no-unused-vars': [
        'warn',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_', caughtErrors: 'none' },
      ],
      '@typescript-eslint/no-empty-object-type': 'warn',
      // Flags `new Promise(r => setTimeout(r, ms))`, the sleep idiom used in
      // mock IPC + tests. Real bugs would look the same, so keep it visible.
      'no-promise-executor-return': 'warn',
      // High false-positive rate on `ref.current` / plain-object mutation after
      // an await; every current hit is a guarded ref flag. Warn, don't block.
      'require-atomic-updates': 'warn',

      // --- disabled: fires broadly on pre-existing, intentional code -------
      // Non-null assertions are used pervasively after explicit guards.
      '@typescript-eslint/no-non-null-assertion': 'off',
      // Canvas/graph code uses bitwise ops and `void` returns idiomatically.
      'no-bitwise': 'off',
      // The React Compiler rule set shipped in eslint-plugin-react-hooks v7
      // (purity / immutability / static-components / set-state-in-effect / …)
      // flags long-standing patterns across the whole app. Enabling it is its
      // own milestone; left off so this config can land green.
      'react-hooks/purity': 'off',
      'react-hooks/immutability': 'off',
      'react-hooks/static-components': 'off',
      'react-hooks/set-state-in-effect': 'off',
      'react-hooks/set-state-in-render': 'off',
      'react-hooks/refs': 'off',
      'react-hooks/globals': 'off',
      'react-hooks/use-memo': 'off',
      'react-hooks/preserve-manual-memoization': 'off',
      'react-hooks/error-boundaries': 'off',
      'react-hooks/incompatible-library': 'off',
      'react-hooks/unsupported-syntax': 'off',
      // `no-empty` on intentional swallow-and-fallback catch blocks (mock IPC,
      // localStorage probes). Future cleanup: replace with explicit comments.
      'no-empty': 'warn',
      // Only fires on fixture/sample source text: conflict-marker fixtures and
      // the conflict-editor self-test embed literal `${...}` inside
      // single-quoted lines on purpose. Not a real "meant to use a backtick".
      'no-template-curly-in-string': 'off',
    },
  },

  // ---------------------------------------------------------------------------
  // Playwright specs are not React: the fixture signature `async ({ page },
  // use) => { await use(...) }` makes the react-hooks heuristics think `use` is
  // the React `use()` hook called outside a component. Turn those off here.
  // ---------------------------------------------------------------------------
  {
    files: ['e2e/**/*.{ts,tsx}'],
    rules: {
      'react-hooks/rules-of-hooks': 'off',
      'react-hooks/exhaustive-deps': 'off',
      'react-refresh/only-export-components': 'off',
    },
  },

  // ---------------------------------------------------------------------------
  // Node-side tooling: this config file + repo scripts.
  // ---------------------------------------------------------------------------
  {
    files: ['eslint.config.js', 'scripts/**/*.mjs', '*.config.ts'],
    extends: [js.configs.recommended],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: { ...globals.node },
    },
  },
);
