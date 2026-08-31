# T1 — Test Infrastructure (testing campaign, Phase T1)

Scope: component-test capability (vitest + jsdom + RTL), Playwright e2e against the mock-IPC
harness, Rust coverage baseline, CI wiring, and a strict-mode guard so CI can never silently skip
git-dependent tests. **No application code changes** except the 15 `have_git()` bodies (§4).
One increment, one commit.

Verified facts this contract is built on: vitest `^4.1.10` configured inline in `vite.config.ts`
(`environment: 'node'`, include `src/**/*.test.{ts,tsx}`); React `^19`; pnpm `11.17.0`;
all 18 existing test files are `*.test.ts` (zero `.test.tsx` exist — the split below breaks
nothing); `tsconfig.json` includes only `src` (so `e2e/` and `playwright.config.ts` are invisible
to `pnpm build`'s tsc); `src/ipc/index.ts` picks mock vs tauri via top-level-await dynamic import
on `import.meta.env.VITE_MOCK_IPC === '1'`; mock layer is a directory `src/ipc/mock/`.

---

## 1. Vitest component testing

### 1.1 devDependencies to add (pnpm add -D)

| package | pin | note |
|---|---|---|
| `jsdom` | `^26` | accept what pnpm resolves; bump to newer major only if vitest 4 peer range demands it |
| `@testing-library/react` | `^16.3.0` | React 19 supported since 16.1 |
| `@testing-library/dom` | `^10` | explicit — pnpm does not auto-hoist RTL's peer |
| `@testing-library/user-event` | `^14.6.0` | |
| `@testing-library/jest-dom` | `^6.9.0` | import the `/vitest` entry |
| `@vitest/coverage-v8` | `^4.1.10` | MUST match the installed vitest minor exactly; if pnpm errors on version mismatch, pin both to the identical resolved version |
| `@playwright/test` | `^1.54` | §2 |

### 1.2 Environment split — ONE convention

- **`*.test.ts` → `node` project** (all 18 existing files, all future pure-logic tests).
- **`*.test.tsx` → `dom` project (jsdom)** — components, hooks via `renderHook`, anything
  touching DOM/localStorage. Hook tests that contain no JSX are still named `.test.tsx`;
  that is the convention. No `__dom__/` directories, no per-file `@vitest-environment` pragmas.

### 1.3 `vite.config.ts` — replace the current `test:` block (vitest 4 `projects`)

```ts
test: {
  coverage: {                       // root-level: aggregates across both projects
    provider: 'v8',
    reporter: ['text', 'html', 'json-summary'],
    reportsDirectory: 'coverage',
    include: ['src/**/*.{ts,tsx}'],
    exclude: [
      'src/**/*.test.{ts,tsx}',
      'src/test/**',
      'src/ipc/tauri.ts',           // requires the Tauri runtime; e2e/native territory
      'src/**/*.d.ts',
    ],
    // NO thresholds in T1 — baseline first; record numbers in COVERAGE.md.
  },
  projects: [
    {
      extends: true,
      test: { name: 'node', environment: 'node', include: ['src/**/*.test.ts'] },
    },
    {
      extends: true,
      test: {
        name: 'dom',
        environment: 'jsdom',
        include: ['src/**/*.test.tsx'],
        setupFiles: ['src/test/setup.ts'],
        env: { VITE_MOCK_IPC: '1' },   // see §1.5
      },
    },
  ],
},
```

Keep `envPrefix`, `server`, plugins unchanged.

### 1.4 `src/test/setup.ts` (new file)

Exact contents (implement verbatim; no `globals: true` is configured, so cleanup is explicit):

```ts
import '@testing-library/jest-dom/vitest';
import { afterEach, vi } from 'vitest';
import { cleanup } from '@testing-library/react';

afterEach(() => {
  cleanup();
  localStorage.clear();   // jsdom provides localStorage; keep tests isolated
});

// Canvas 2D stub — jsdom has no canvas backend; GraphCanvas et al. need a
// tolerant 2D context. Proxy returns no-op fns for anything not overridden.
const ctx2d = new Proxy(
  {
    canvas: null as unknown,
    measureText: (s: string) => ({ width: s.length * 7 }),
    getImageData: () => ({ data: new Uint8ClampedArray(4), width: 1, height: 1 }),
    createLinearGradient: () => ({ addColorStop: () => {} }),
  },
  { get: (t, p) => (p in t ? (t as any)[p] : () => undefined),
    set: () => true },
);
HTMLCanvasElement.prototype.getContext = vi.fn(() => ctx2d) as never;

// ResizeObserver stub
class ResizeObserverStub {
  observe() {} unobserve() {} disconnect() {}
}
globalThis.ResizeObserver = ResizeObserverStub as never;

// matchMedia stub (theme detection)
window.matchMedia ??= ((query: string) => ({
  matches: false, media: query, onchange: null,
  addEventListener: () => {}, removeEventListener: () => {},
  addListener: () => {}, removeListener: () => {}, dispatchEvent: () => false,
})) as never;
```

### 1.5 Mock IPC in component tests — DECISION

**Chosen: env-based selection, not `vi.mock`.** The `dom` project sets
`env: { VITE_MOCK_IPC: '1' }`, so `import { ipc } from '../ipc'` inside components resolves to
the real mock layer at module load — identical to the browser harness, zero test boilerplate,
and it exercises the mock handlers (which Phase T3 must test anyway). The dynamic import
guarantees `@tauri-apps/*` never loads.

Per-test behavior overrides: import the concrete object and spy —
`import { mockIpc } from '../ipc/mock'; vi.spyOn(mockIpc, 'getStatus').mockResolvedValue(...)`.
(`ipc` from `src/ipc/index.ts` and `mockIpc` are the same object reference.) Deterministic
fixture state resets via `localStorage.clear()` in setup plus any exported mock reset helper if
one exists — senior-dev: check `src/ipc/mock/` for a reset/seed export and document it in the
first `.test.tsx`. Rejected alternative: `vi.mock('../ipc')` — duplicates the mock surface and
drifts from the harness.

### 1.6 Scripts (`package.json`)

```json
"test": "vitest run",
"test:watch": "vitest",
"test:coverage": "vitest run --coverage",
"test:e2e": "playwright test"
```

Add to `.gitignore`: `coverage/`, `test-results/`, `playwright-report/`.

### 1.7 Proof component test (new file, part of this increment)

`src/test/harness.test.tsx` — renders a trivial existing presentational component (senior-dev
picks the smallest, e.g. a badge/pill component) with RTL, asserts via a jest-dom matcher
(`toBeInTheDocument`), and asserts `HTMLCanvasElement.prototype.getContext('2d')` returns the
stub (truthy, `measureText` works). This is the T1 acceptance artifact; real component tests are
Phase T3.

---

## 2. Playwright e2e

### 2.1 `playwright.config.ts` (repo root — outside tsconfig `include`, so tsc/build unaffected)

```ts
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
      ? { name: 'chromium', use: { browserName: 'chromium' } }          // bundled chromium (ubuntu)
      : { name: 'msedge', use: { browserName: 'chromium', channel: 'msedge' } }, // installed Edge — no download, ASR-safe
  ],
  webServer: {
    command: 'pnpm dev:mock',
    port: 1420,
    reuseExistingServer: !CI,
    timeout: 120_000,
  },
});
```

Local-Windows rule: never download browsers to C:. `channel: 'msedge'` downloads nothing. If a
browser install is ever needed locally, set `PLAYWRIGHT_BROWSERS_PATH=D:\Temp\ms-playwright`
first (document this as a comment at the top of `playwright.config.ts`).

### 2.2 `e2e/fixtures.ts` — shared fixture forbidding console errors

Exports `test` and `expect`. `test` extends `@playwright/test`'s base with an auto `page`
wrapper: before use, attach `page.on('console', ...)` collecting messages with `type() ===
'error'` and `page.on('pageerror', ...)`; after the test body, assert both collections are
empty (fail with the collected texts). Provide an opt-out per test via a fixture option
`allowConsoleErrors: string[]` (substring allowlist, default `[]`). All specs import from
`./fixtures`, never from `@playwright/test` directly.

### 2.3 `e2e/smoke.spec.ts` — first spec (T1 deliverable; journeys are Phase T4)

1. `page.goto('/')` — app boots against the mock harness.
2. Graph canvas visible: `expect(page.locator('canvas').first()).toBeVisible()` (senior-dev may
   tighten to a stable selector/test-id if one exists — do NOT add test-ids in T1).
3. Sidebar populated: an element containing the fixture default branch (`main` or whatever the
   mock fixture's branch list shows — verify against the mock fixture, assert on that exact text).
4. Console stays clean (enforced by the fixture automatically).

---

## 3. Rust coverage baseline (measurement only — NOT a CI gate)

One-time local setup: `rustup component add llvm-tools-preview` then
`cargo install cargo-llvm-cov --locked` (compiled locally → ASR-safe; never cargo-binstall).

Baseline run (PowerShell, sequential — never concurrent with any other cargo command):

```powershell
$env:TMP = 'D:\Temp'; $env:TEMP = 'D:\Temp'
cargo llvm-cov --workspace --summary-only            # numbers for COVERAGE.md
cargo llvm-cov --workspace --html --output-dir D:\Temp\bonsai-llvm-cov   # browsable detail, kept off the repo & off C:
```

Record per-crate line/region % into `docs/testing-campaign-2026-08/COVERAGE.md` (create the file
and `FINDINGS.md` stub in this increment, per the campaign plan's log convention). Note in
COVERAGE.md that llvm-cov uses its own profile dir — expect a full rebuild; do not interleave
with `cargo test`/`clippy`.

---

## 4. `require_git!` strict mode — CI can never silently skip

Fact: `require_git!` is a per-file macro (≈18 test files + `src-tauri/src/mcp.rs`), every copy
delegating to a local or shared `have_git()`. There are exactly **15 `have_git()` definitions**:

- `crates/bonsai-core/tests/common/mod.rs:79` (shared by most integration files)
- `crates/bonsai-core/tests/status_porcelain.rs:17`
- `crates/bonsai-mcp/tests/mcp_stdio.rs:92`
- `src-tauri/src/mcp.rs:719`
- inline `#[cfg(test)]` copies: `bonsai-core/src/{health.rs:610, fixture.rs:294}` and
  `bonsai-core/src/git/{branches.rs:2071, compose_apply.rs:314, exec.rs:131, hooks.rs:355,
  maintenance.rs:76, remote.rs:1576, search.rs:461, stale.rs:463, undo.rs:432}`

Change every one of the 15 to this exact body (keep each fn's existing visibility/signature):

```rust
fn have_git() -> bool {
    let ok = std::process::Command::new("git").arg("--version").output().is_ok();
    if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
        panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
    }
    ok
}
```

Semantics: locally unchanged (skip with eprintln); in CI (`BONSAI_REQUIRE_GIT_STRICT=1`) a
missing git turns every would-be skip into a loud failure. No macro changes needed — all copies
funnel through `have_git()`. `git_version_at_least` skips stay allowed (version-conditional, not
absence).

---

## 5. CI wiring (`.github/workflows/ci.yml`)

**Rust job** — keep steps identical, plus:
- Add `env: { BONSAI_REQUIRE_GIT_STRICT: '1' }` to the `cargo test` step.
- Change the test step to surface skip/ignore counts:
  ```yaml
  - name: cargo test
    env:
      BONSAI_REQUIRE_GIT_STRICT: '1'
    run: |
      set -o pipefail
      cargo test --workspace 2>&1 | tee /tmp/cargo-test.log
  - name: Surface ignored/skipped test counts
    run: |
      echo '## cargo test summary' >> "$GITHUB_STEP_SUMMARY"
      grep -E '^test result:' /tmp/cargo-test.log >> "$GITHUB_STEP_SUMMARY"
      grep -c 'skipping:' /tmp/cargo-test.log | xargs -I{} echo 'runtime skips: {}' >> "$GITHUB_STEP_SUMMARY" || true
  ```
  (Known baseline: 3 `#[ignore]`d perf gates + 1 `#[ignore]`d rebase known-bug — visible in the
  summary, not failed on.)

**Frontend job** — replace `pnpm test` with `pnpm test:coverage` (runs both vitest projects +
produces `coverage/`); add an artifact upload of `coverage/coverage-summary.json`
(`actions/upload-artifact@v4`, `if: always()`). Keep `pnpm build` unchanged.

**New `e2e` job** (parallel to the other two):

```yaml
e2e:
  name: E2E — Playwright vs mock harness
  runs-on: ubuntu-22.04
  steps:
    - uses: actions/checkout@v4
    - uses: pnpm/action-setup@v4
    - uses: actions/setup-node@v4
      with: { node-version: 22, cache: pnpm }
    - run: pnpm install --frozen-lockfile
    - run: pnpm exec playwright install --with-deps chromium
    - run: pnpm test:e2e        # CI env is set by Actions → chromium project + webServer autostart
    - uses: actions/upload-artifact@v4
      if: failure()
      with: { name: playwright-report, path: playwright-report/ }
```

---

## 6. Acceptance criteria (AI-gate)

1. `pnpm test` green, output shows BOTH projects (`node`: existing ~197 cases unchanged; `dom`:
   `harness.test.tsx` passing with jest-dom matcher + canvas stub assertion).
2. `pnpm test:coverage` produces `coverage/` (text summary + html + json-summary); numbers
   recorded in `docs/testing-campaign-2026-08/COVERAGE.md` alongside the cargo llvm-cov baseline.
3. `pnpm test:e2e` passes locally on Windows using the `msedge` channel against auto-started
   `pnpm dev:mock` (nothing written to C:); smoke spec proves boot + canvas + sidebar + clean
   console.
4. `pnpm build` (tsc + vite) unaffected; `cargo test --workspace` green with the 15 `have_git()`
   edits; `BONSAI_REQUIRE_GIT_STRICT=1 cargo test -p bonsai-core --test status_porcelain` still
   green on a machine with git (strict mode is a no-op when git exists); `cargo clippy -D
   warnings` clean.
5. `ci.yml` parses (actionlint or a dry push to a branch) — rust/frontend jobs modified as §5,
   e2e job added.

## Flagged for orchestrator

- `@vitest/coverage-v8` must equal the installed vitest version; if the lockfile has vitest at a
  newer 4.x than 4.1.10, pin coverage to that same resolved version.
- The 15-site `have_git()` edit touches `#[cfg(test)]` code in application files — within T1's
  "no app code" spirit but technically edits `src/` files; approved here as test-only bodies.
- Smoke-spec selectors assume the mock fixture shows a branch named `main` and at least one
  `<canvas>`; senior-dev must verify against the actual mock fixture before asserting.
