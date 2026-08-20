# P76 — Native USER CHECKPOINT automation harness

**Status:** PROPOSED (design contract; no code written). Architect: contract only.
**Depends on:** existing mock e2e (`playwright.config.ts`, `e2e/`), the `window.__bonsai.*` JS seams
(`scrollSweep`, `p7`, `p7SelfTest`), the `recentRepos`/`openRepos`/`activeRepo` session-reopen path
(`src-tauri/src/settings.rs`, `src-tauri/src/commands/repo.rs`, `src/App.tsx:221-243`), the committed
`claude` stub (`crates/bonsai-core/tests/fixtures/claude_stub.*`, `BONSAI_CLAUDE_BIN`/`BONSAI_STUB_MODE`),
and the `BONSAI_GIT_BIN` git-resolver seam (P70).

## 0. Problem and goal

~11 code-complete milestones sit at `awaiting USER CHECKPOINT` (P62, P63, P64, P65, P67, P68, P70,
P71, P72, P73, P74). The gate splits every milestone into an **AI gate** and a **USER CHECKPOINT**;
the orchestrator may never self-declare the second half, so the backlog never closes. The *reason* so
much lands in the human bucket is a harness limitation, not a genuine human requirement: the browser
harness composites at 0×0, so `document.visibilityState === "hidden"`, `requestAnimationFrame` is
paused, and no canvas pixel is ever produced (see `headless-harness-no-raf` memory + P65/P67/P68
checklists). A **real** window under WebDriver removes that limitation for everything except items that
genuinely need real network / real credentials / real signed installers / human perception / macOS.

**Goal:** a native-smoke harness that drives the *real* Bonsai window and reclassifies each pending
checkpoint item into (A) automatable via tauri-driver, (B) automatable via a real `cargo` integration
test, or (C) irreducibly human. A green native-smoke run lets the orchestrator legitimately close the
**A**+**B** half of a milestone and present a short, explicit **C** remainder to the user.

---

## 1. Module boundaries and file responsibilities

New, self-contained. **No application code changes are required** by the recommended design (§4.2);
the only production-adjacent additions are optional test seams flagged in §9.

| Path | Responsibility |
|---|---|
| `native-smoke/wdio.conf.ts` | WebdriverIO config: registers `tauri-driver` as a service, selects the per-OS WebDriver (msedgedriver / WebKitWebDriver), points the capability at the built binary, sets timeouts/retries. Skips itself with a clear message on macOS. |
| `native-smoke/helpers/launch.ts` | `launchBonsai()`, `seedSettings()`, `openScratchRepo()`, `readAppLog()` — process/config-dir isolation + repo injection (§4.2). |
| `native-smoke/helpers/fixtures.ts` | Thin TS wrappers that shell out to the fixture builders (§4.1): `smallRepo()`, `graphFixture(n)`, `conflictMergeRepo()`, `submoduleWedgeRepo()`, `githubOriginRepo(url)`. |
| `native-smoke/helpers/seams.ts` | Typed constants for the JS seams the specs call (`scrollSweep`, `p7`, `p7SelfTest`) + `runSeam()` around `browser.execute`. |
| `native-smoke/specs/*.native.ts` | One spec per milestone (`p65-graph.native.ts`, `p67-density.native.ts`, `p68-ai-stream.native.ts`, `p73-submodule.native.ts`, `p70-gitbin.native.ts`, `p62-forge.native.ts`, `p72-openurl.native.ts`). Each spec's file header lists the **C** items it deliberately does NOT cover. |
| `crates/bonsai-fixtures/` (new `[[bin]]`, optional — §4.1) | Deterministic large-graph fixture builder reusing the M2 lane topology via `git fast-import`. Emits `bonsai-fixtures graph20k <dest>` etc. Reuses `crates/bonsai-core/tests/common` scratch conventions. |
| `.github/workflows/native-smoke.yml` | Nightly + `workflow_dispatch` CI job (windows-latest + ubuntu-latest under xvfb). NOT a per-push gate (§6). |
| `docs/history/P76-native-coverage.md` | The living **per-milestone A/B/C ledger** the orchestrator reads to close backlog halves (§7). Curated by docs-curator once this ships. |

`package.json` gains one script: `"test:native": "wdio run native-smoke/wdio.conf.ts"`.

---

## 2. Tooling — tauri-driver + WebdriverIO

Tauri v2 exposes a **cross-platform WebDriver** binary, `tauri-driver`, that proxies to the platform's
native WebView WebDriver. It drives the *webview DOM* (Selenium/WebdriverIO commands, `execute`,
`takeScreenshot`) of the **built** app binary. It does **not** reach native OS chrome — file pickers,
Credential Manager, MSI/NSIS UI are all outside its reach (§3, §5 rely on this fact).

**Driver stack (locked recommendation): WebdriverIO v9.** Chosen over raw Selenium because the tauri
docs' reference integration is WDIO, its `execute` maps cleanly onto the existing `window.__bonsai`
seams, and its service model registers `tauri-driver` as a managed subprocess.

### Dependencies / versions
- `tauri-driver` — `cargo install tauri-driver --locked` (v2 line; pin the installed version in CI).
- WDIO — `@wdio/cli`, `@wdio/local-runner`, `@wdio/mocha-framework`, `@wdio/spec-reporter` (`^9`),
  added as `devDependencies` (kept out of the default `pnpm install` critical path is not required;
  they are dev-only and small).
- The app must be **built** first: `pnpm tauri build --debug` (produces the unbundled binary +
  `frontendDist`). Capability points at `src-tauri/target/debug/bonsai(.exe)`.

### Windows setup (WebView2)
- WebView2 Runtime (Evergreen) is already required by the app.
- Driver = **msedgedriver** whose version **must match the installed WebView2 runtime major version**
  (this is the version-coupling risk, §10). Install to `D:\Temp` (never C: — ASR + full-disk rules);
  set the WDIO capability `"tauri:options": { "application": "<abs path to bonsai.exe>" }` and point
  `tauri-driver --native-driver <path to msedgedriver.exe>`.
- No Playwright/chromium reuse here — that harness is DOM-only mock; this is the native binary.

### Linux setup (WebKitGTK)
- Driver = **WebKitWebDriver**, from the distro's `webkit2gtk-driver` package (Ubuntu:
  `apt-get install -y webkit2gtk-driver xvfb`).
- Headless: run the whole `wdio` invocation under `xvfb-run -a` (a real X server → real compositor →
  real rAF, which is exactly what the browser pane lacked).
- `tauri-driver --native-driver /usr/bin/WebKitWebDriver`.

### macOS — UNSUPPORTED (hard constraint)
`tauri-driver` has **no macOS support**: Apple's WKWebView ships no WebDriver, and there is no
`safaridriver` path into a Tauri webview. **Every macOS-specific checkpoint stays human**, forever, by
platform limitation — not a gap we can close later. The harness must `skip` on `process.platform ===
'darwin'` with an explicit log line so a macOS contributor is never misled into thinking it ran. The
per-milestone C-lists (§7) each carry an explicit "macOS: human on all items" line.

---

## 3. The scratch-repo + native-window fixture

### 3.1 Fixture repos (never thousands of `git commit` calls)
- **Small / behavioural repos** (conflict merge, submodule wedge, github-origin): built by shelling
  `git` from `native-smoke/helpers/fixtures.ts`, mirroring the exact scripts already proven in
  `crates/bonsai-core/tests/*_cli.rs` (e.g. `submodule_reconnect_cli.rs` reproduces the P73 wedged
  state offline with no network). Reuse those command sequences verbatim.
- **Large graph repos** (20k / 200k): built by `git fast-import` from a generated stream, via the
  optional `crates/bonsai-fixtures` bin (§4.1) OR a `.mjs` stream generator. Reuses the M2 fixture
  lane topology so `scrollSweep` numbers are comparable to the M2 gate. Target build time < a few
  seconds for 20k.
- **Scratch root:** honor the standing mandate — Windows uses `D:\Temp\bonsai-scratch`, macOS/Linux
  `std::env::temp_dir()/bonsai-scratch`, exactly as `crates/bonsai-core/src/testutil.rs` and
  `tests/common/mod.rs` already do. The TS helper reads `BONSAI_SCRATCH_ROOT` (default per-OS as
  above) so CI can redirect it.

### 3.2 Launch / drive / teardown lifecycle (per spec, per fixture)
1. **Build fixture** → absolute path under the scratch root.
2. **Isolate config** — point the app's config dir at a fresh temp dir so `settings.json` is
   pristine and pre-seedable: set `APPDATA=<tmp>` (Windows) / `XDG_CONFIG_HOME=<tmp>` (Linux) in the
   launched process env. This is the zero-production-code repo-injection seam (§4.2).
3. **Seed settings** — write `<config>/com.bonsai.app/settings.json` with the session block pointing
   at the fixture (§5.2) plus any UI settings the spec needs (e.g. `aiConsented:true`,
   `panelDensity:'compact'`), and any consent flags.
4. **Launch** the built binary under tauri-driver via the WDIO capability, passing per-spec env
   (`BONSAI_GIT_BIN`, `BONSAI_CLAUDE_BIN`, `BONSAI_STUB_MODE`).
5. **Drive** — DOM queries (reuse the role/text locators the Playwright e2e already uses for
   portability), `browser.execute()` into the `window.__bonsai` seams, `browser.takeScreenshot()` for
   canvas-pixel assertions.
6. **Teardown** — quit the session (WDIO closes the app), then `rm -rf` the fixture + temp config dir.
   A `try/finally` guarantees teardown so scratch dirs never accumulate on C:/D:.

---

## 4. Interface contracts

### 4.1 Fixture builder (Rust bin — optional, recommended for the graph fixtures)
```rust
// crates/bonsai-fixtures/src/main.rs  — bin "bonsai-fixtures"
// Usage: bonsai-fixtures <kind> <dest_dir>
//   kind ∈ { "small", "graph20k", "graph200k", "conflict-merge", "submodule-wedge" }
// Writes a ready-to-open repo at <dest_dir>; exits 0 on success, prints the path.
// graph* build via `git fast-import` reusing the M2 lane topology (NOT per-commit spawns).
```
If the orchestrator prefers zero new Rust, a `native-smoke/helpers/fixtures.mjs` stream generator is
an acceptable substitute for the graph fixtures (flagged in §9).

### 4.2 TS harness helpers (`native-smoke/helpers/launch.ts`)
```ts
export interface BonsaiSession {
  openRepos: string[];
  activeRepo: string | null;
}

/** UI-settings subset the harness pre-seeds. Mirror of the real settings.json shape;
 *  keep in sync with src/settings/uiSettingsDefaults.json (only the keys a spec touches). */
export interface SeedSettings {
  version: number;
  recentRepos: { path: string; lastOpened: number }[];
  openRepos?: string[];
  activeRepo?: string | null;
  aiEnabled?: boolean;
  aiConsented?: boolean;
  panelDensity?: 'cozy' | 'compact';
  // …any other flat UI key a spec needs; unknown keys must round-trip harmlessly
  //   (guaranteed by the existing additive-migration behaviour in settings.rs).
}

export interface LaunchOptions {
  /** Absolute path to a fixture repo to reopen on launch (seeds session + recents). */
  repo?: string;
  /** Extra settings merged over the repo-derived session block. */
  settings?: Partial<SeedSettings>;
  /** Per-process env: BONSAI_GIT_BIN, BONSAI_CLAUDE_BIN, BONSAI_STUB_MODE, BONSAI_SCRATCH_ROOT. */
  env?: Record<string, string>;
}

/** Builds an isolated config dir, seeds settings.json, launches the built binary under
 *  tauri-driver, and returns once the workspace shell (or empty state) has mounted. */
export function launchBonsai(opts: LaunchOptions): Promise<void>;

/** Writes <config>/com.bonsai.app/settings.json from a repo path + overrides. */
export function seedSettings(configDir: string, repo: string | null, over?: Partial<SeedSettings>): void;

/** Reads the app's settings.json back (for restart-persistence assertions). */
export function readSettings(configDir: string): SeedSettings;
```

**Repo-injection decision (recommended):** reopen the seeded repo through the existing
launch-reopen path (`getSession()` → `openRepos`/`activeRepo`, `src/App.tsx:221-243`) instead of
driving the folder picker — because the picker is a **native OS dialog** (`tauri_plugin_dialog`,
`src-tauri/src/lib.rs:37`) that WebDriver cannot touch. This needs **zero production code**. See §9 for
the fallback if config-dir redirection proves unreliable under tauri-driver.

### 4.3 JS seams the harness calls (already in the app)
```ts
// on window.__bonsai (real window under tauri-driver runs rAF, unlike the 0×0 browser pane):
scrollSweep(px: number): Promise<{ maxWindow5Avg: number; over100: number }>; // P65/M2 frame gate
p7: { /* HEAD-guideline geometry seam (P67a) */ };
p7SelfTest(): number; // 0 = pass
```
No new seams are mandated. Where a spec needs a DOM hook that no role/text query can express (e.g. a
canvas-overlay badge button), a small `data-testid` may be requested — flagged in §9 as the one place
senior-dev + ui-designer input is needed.

---

## 5. IPC / event / channel surface

**P76 adds no new Tauri command, event, or channel.** It exercises the *existing* surface against a
real window. The two data shapes it reads/writes are already-shipped:

### 5.1 Existing commands the specs drive (no change)
`open_repo`, `set_active_repo`, `getSession`/`setSession`, `stream_graph` (channel — P65 scroll),
`ai_resolve_conflict_stream` + `ai_cancel_run` + `ai_reply_run` (channel — P68, driven with the stub
CLI), `update_submodule`/`init_submodule` (P73), the git-availability preflight + `open_url` (P70/P72).

### 5.2 Seeded settings.json session block (existing shape — `settings.rs`)
```jsonc
{
  "version": 1,
  "recentRepos": [{ "path": "<abs fixture path>", "lastOpened": 0 }],
  "openRepos": ["<abs fixture path>"],
  "activeRepo": "<abs fixture path>"
  // + any UI keys the spec seeds
}
```
Because settings loading is additive-migration-tolerant, a partial seed is legal and safe.

---

## 6. Where it runs

- **Local, opt-in:** `pnpm test:native`. Requires a display (native window) + the built binary +
  `tauri-driver` + the OS driver installed. Never wired into `pnpm test` / pre-commit.
- **CI:** `.github/workflows/native-smoke.yml`, triggered `schedule` (nightly) + `workflow_dispatch`.
  Matrix: `windows-latest` (WebView2 preinstalled; install matching msedgedriver to `D:`-equivalent
  runner temp) and `ubuntu-latest` (install `webkit2gtk-driver xvfb`; run under `xvfb-run -a`). **No
  `macos-*` leg** (unsupported). Upload WDIO logs + failure screenshots as artifacts.
- **NOT a per-push gate.** This suite is slow (builds a real binary, launches a real window, streams
  large fixtures) and structurally flakier than unit tests (driver/runtime version coupling, window
  focus, timing). It is a **nightly signal + an on-demand backlog-closer**, run explicitly when the
  orchestrator wants to retire a checkpoint half — never a blocker on the fast inner loop.

---

## 7. Interaction with the workflow

1. The orchestrator (or a nightly run) runs `pnpm test:native` on Windows and/or Linux.
2. For each milestone with a passing native spec, the orchestrator may legitimately mark the milestone's
   **A**+**B** items **verified** — because they were exercised against the real window/binary, which is
   exactly what the USER CHECKPOINT existed to cover. The board note records *which OS* proved it
   (Windows/Linux) and that macOS + the **C** items remain.
3. The **C** remainder is presented to the user as a shrunken, explicit list per milestone (from the
   ledger below), so the human effort is bounded and non-recurring.
4. `docs/history/P76-native-coverage.md` is the living A/B/C ledger; a milestone is only fully `done`
   when the A/B half is native-smoke-green **and** the C half is user-confirmed. This does not weaken
   the rule that the orchestrator never self-declares C — it shrinks C.

### 7.1 Per-milestone checkpoint taxonomy (the ledger seed)

| Milestone | (A) tauri-driver | (B) cargo integration | (C) irreducibly human |
|---|---|---|---|
| **P62** forge foundation | github-origin repo → "connect" state renders; token-absent-from-settings.json after connect | keychain round-trip against the real OS credential store on the CI runners (Credential Manager / Secret Service) | real PAT accepted, real OPEN PRs list/detail/create/sign-out/rate-limit against real GitHub |
| **P63** forge graph signals | toggles present + default OFF + persist + graph redraws on enable; `?forge=off` silent | (badge geometry pure helpers already unit-tested) | badge **pixels/colours**, click→PR, hover tooltip **on real PR/CI data** (no native forge-fixture seam today — see §9) |
| **P64** providers + AI PR | (provider **detection** already cargo-tested) | per-provider URL detection | real GitLab/Bitbucket/Azure tokens; **live AI** PR-description generation |
| **P65** paged loading | **`scrollSweep(10000)` on the real 20k window → `maxWindow5Avg≤33 && over100≤3`** (the flagship conversion; rAF now runs); progressive fill on a 200k fast-import fixture; scrollbar grows; repo-switch mid-stream; selection across re-stream; truncation cap | lane-stability byte-identical across batch sizes (already green) | subjective "smoothness feel" beyond the numeric gate; the deferred-P66 first-paint latency judgement |
| **P67** UX polish | HEAD-guideline **geometry** via `p7`/`p7SelfTest` + presence via `takeScreenshot`; density row-count / measured heights; ⋯ menu; **Cozy/Compact persists across a real restart** | — | whether a density *looks* right / legible at the user's DPR, both themes; dash-crawl *shimmer* perception |
| **P68** AI conflict streaming | With `BONSAI_CLAUDE_BIN`=stub + `BONSAI_STUB_MODE`: full **plumbing** — stream event order, Cancel keeps the log, reply completes a run, dock appears/collapses/resizes/**persists across restart**, one-run bulk, per-file outcomes, settings patch-independence, consent copy on screen, out-of-repo refusal line (stub emits it), **no orphaned child after window close** | (classify/parse/session watchdog already cargo-tested) | **real `claude`**: past 90 s, live-log *reads as live*, real tool use, real ambiguous question, real cost, real refusal; native focus rings |
| **P70** git-exec resolution | With `BONSAI_GIT_BIN`=nonexistent: banner shows, **no** error toast, copy is correct; Re-check recovery false→true without restart; healthy launch shows no banner | resolver ladder + credential-fill spawn-vs-empty (already cargo-tested, incl. SSH-exhaustion guards #16/#18) | SSH-agent auth *succeeds while banner shows* on a **real** SSH remote; the **MSI-install / Machine-only-PATH** field repro; screen-reader pass; both-theme look |
| **P71** auto-update env | — | PATH-rehydration merge logic (already cargo-tested) | **real signed update round-trip**, NSIS/MSI installer, ASR/AV interaction, post-update `GitAvailability.source==path` probe — the whole trust chain is real-machine-only |
| **P72** forge connect fixes | clicking "Create a token" / "Open in browser ↗" invokes `open_url` (assert the IPC fires + no crash; on xvfb the launch may no-op gracefully) | Azure validate-then-identify + `validate_web_url` rejections (already cargo-tested vs FakeTransport) | real Azure Code-scoped PAT connects / bad PAT clear error; the **system browser actually opening** |
| **P73** submodule reconnect | drive Init(=init+checkout) and Update against a **locally-built wedged** superproject fixture → badge/toast agree, workdir repopulates | orphaned-gitdir reattach offline (already proven network-free in `submodule_reconnect_cli.rs`) | the real Azure DevOps superproject (low value — the fix is proven network-free) |
| **P74** a11y | (contrast ratios + ≥24px hit targets already asserted in the **mock** e2e — nothing new to convert natively) | — | the info-`●` glyph shape judgement; glyph optical alignment; sidebar rhythm "reads well" — pure perception, both themes |

**Rough conversion:** of the ~11 backlog milestones, **8** (P62, P63, P65, P67, P68, P70, P72, P73)
have a substantial automatable slice the harness newly unlocks — most importantly P65's flagship
`scrollSweep` gate and P68's entire streaming/plumbing surface (via the stub CLI), both previously
impossible in the 0×0 pane. At item granularity, roughly **60–70%** of the individual checkpoint lines
become machine-verifiable. The **irreducible-human core** is: real-network forge/AI (P62/P63/P64 data
paths, P68 real-model), the **real signed-updater trust chain** (P71, and P42's deferred half), real
**SSH-agent + MSI** env (P70), aesthetic/perception judgements (P67/P74) — **plus every macOS item**,
which stays human by the tauri-driver platform limitation.

---

## 8. Acceptance criteria

1. `pnpm test:native` launches the **built** Bonsai binary under tauri-driver on Windows (WebView2 +
   msedgedriver) and Linux (WebKitWebDriver + xvfb), opens a seeded scratch repo **without touching the
   native folder picker**, and tears everything down under a `finally` (no scratch left on C:/D:).
2. The P65 spec runs `window.__bonsai.scrollSweep(10000)` on a real 20k window and **asserts**
   `maxWindow5Avg ≤ 33 && over100 ≤ 3` — the exact gate that was USER-CHECKPOINT-only until now.
3. The P68 spec, with the committed stub CLI, drives a full streaming resolve: event order, Cancel
   keeps the log, reply completes the run, dock persists across a real restart, one-run bulk, and
   **no orphaned child process** after the window closes.
4. The P67 spec confirms Cozy/Compact **persists across a real quit+relaunch** and the P73 spec drives
   Init/Update on a locally-built wedged submodule fixture to a correct badge + repopulated workdir.
5. Each spec's header enumerates the **C** items it does NOT cover; `docs/history/P76-native-coverage.md`
   holds the full A/B/C ledger seeded from §7.1.
6. The CI workflow runs nightly + on dispatch on windows-latest and ubuntu-latest, is **not** a
   per-push gate, uploads logs + failure screenshots, and `skip`s macOS with an explicit message.
7. Zero new production Tauri commands/events/channels; no application code changed beyond (at most) the
   small `data-testid` additions flagged in §9, if the orchestrator approves them.

---

## 9. Flagged ambiguities / decisions for the orchestrator

- **[REPO INJECTION] Recommended: config-dir redirection + session seed (zero prod code).** Set
  `APPDATA`/`XDG_CONFIG_HOME` to a temp dir and pre-seed `settings.json` so the app reopens the fixture
  on launch. **Fallback if that proves unreliable under tauri-driver** (env may be awkward to pass
  through the WDIO capability): add a *dev/test-only, feature-gated* `BONSAI_E2E_OPEN_REPO` env seam in
  the launch bootstrap. This touches production code, so it is a fallback, not the default.
  **Recommendation: config-dir redirection.** Needs orchestrator sign-off before senior-dev picks one.
- **[FIXTURE BUILDER] New `crates/bonsai-fixtures` bin vs. a `.mjs` fast-import generator.**
  Recommend the Rust bin for the large graph fixtures (deterministic, reuses the M2 lane topology so
  `scrollSweep` numbers stay comparable); the `.mjs` path is acceptable if a new crate is unwelcome.
- **[SELECTORS] Canvas-overlay controls may need `data-testid` hooks.** The forge badge buttons (P63)
  and any canvas-anchored control have no role/text locator. If P63 badge click-routing is to be
  automated (currently marked C for pixel/data reasons), senior-dev + ui-designer must add stable
  `data-testid`s. Recommend deferring P63 badge automation until a **native forge-fixture seam** exists
  (there is none today — the native build has no `?forge=` mock), and keeping it **C** for now.
- **[msedgedriver PINNING] WebView2 is Evergreen (auto-updates).** msedgedriver must track the runtime
  major version. Recommend a CI pre-step that reads the installed WebView2 version and fetches the
  matching driver, and a local doc note. This coupling is the top flakiness source (§10).
- **[P71/P42] No automation path.** The signed-updater trust chain, NSIS/MSI, and ASR interaction are
  real-machine-only. Recommend NOT attempting to automate it and stating so explicitly in the ledger,
  so effort is not spent chasing an unclosable item.

---

## 10. Risks

1. **Driver/runtime version coupling + flakiness.** msedgedriver-vs-WebView2 drift breaks the Windows
   leg silently; native window focus + timing make WDIO less deterministic than jsdom/Playwright-mock.
   Mitigation: nightly/opt-in only (never a push gate), generous timeouts + a single retry, pinned
   driver install, artifact upload for post-mortem. A red native-smoke run is a *signal to investigate*,
   not an automatic milestone regression.
2. **macOS gap is permanent.** tauri-driver cannot drive WKWebView, so ~a third of each milestone's
   cross-platform matrix (and anything macOS-specific) stays human forever. Combined with the
   real-network / real-credential / real-AI / real-installer items, this harness **shrinks** the human
   backlog substantially but never eliminates it — stubs prove *plumbing*, not the real integration, and
   the ledger must keep that distinction honest so a green native-smoke is never mistaken for a passed
   **C** item.
3. **Fixture cost / disk.** Large fast-import fixtures + built binaries are heavy; enforce the D:/temp
   scratch mandate and `finally` teardown, and cap the 200k fixture to the specs that truly need it.
