# Bonsai — Milestone TODO

> Single source of truth for session resume. Keep the "Current step:" line of the
> in-progress milestone updated at every workflow transition.

Environment: Rust 1.97.1 stable-msvc, VS Build Tools 2022 17.14, pnpm 11.17.0, Node 24, WebView2.
Cargo not on default PATH — `$HOME/.cargo/bin`. Browser harness: `pnpm dev:mock` (port 1420).
Avoid tauri "test" feature on this machine (STATUS_ENTRYPOINT_NOT_FOUND); use runtime-free
inner functions for command tests.
Harness traps (cost a session each): the hidden Browser pane reports `innerWidth/innerHeight = 0`, so
every `vh`/`vw` rule evaluates to 0 — call `resize_window` (1440×900) before any layout measurement;
`setTimeout` is throttled to ~1 s in a hidden page, so batch tool dispatches instead of many `await`s;
To drive the git dock in the harness you must seed `bonsai.mockUiSettings` (`{onboardingSeen:true}`)
and `bonsai.mockSession` (`{openRepos:['C:\mock\bonsai-fixture']}`) into localStorage before load —
otherwise you land on the no-repo empty state; and select the toolbar Push button by its `title`, not
its `aria-label` (the label changes with the ahead-count once a push has landed).
never remove React-owned DOM nodes to "reset" a menu (throws `removeChild` on the next render) —
dismiss with Escape; headless preview pauses `requestAnimationFrame`, so canvas repaint/scroll-feel
ACs can only be checked in the native window.
**USER MANDATE (2026-07-28, updated 2026-08-04 for cross-platform support): on Windows, never use
C: for temp/scratch/mock repos — C: is critically full. Use `D:\Data\Temp\bonsai-scratch`; when running
cargo tests set TMP/TEMP to `D:\Data\Temp` (tempfile honors them). On macOS/Linux, `scratch_dir()` now
falls back to the OS temp dir (`std::env::temp_dir()/bonsai-scratch`) automatically — no special
handling needed there. Include the Windows-specific guidance in every subagent prompt that runs
tests or creates repos only when running on a Windows machine.**

## Board conventions

Status vocabulary: `pending` · `in-progress` · `done` · `awaiting USER CHECKPOINT` · `deferred`
(deferred always carries a one-line reason). A milestone is `done` only when the AI gate **and** the
native USER CHECKPOINT have both passed — the orchestrator never self-declares the second half.

**History is archived, not deleted.**

---

---

## Where the rest of the board went

Full detail for everything compacted out of this file is in `docs/history/` — start at
`docs/history/README.md`. The **Archive** table at the bottom is the short form. Nothing below was
closed by the curator: a pending USER CHECKPOINT, an owed AI-gate item and an open follow-up all
stay here however old they are.

**2026-09-14 pass — Parts 62-70.** On **2026-09-11** the user ruled **all 22** open FOR-USER items
(17 + a second round of 5) and the work those rulings created landed the same day. Archived: the
stale 2026-09-10 resume block + FU-1 residue (62) · the FOR-USER *evidence* blocks, now that the
rulings are the record (63) · the `IN FLIGHT` queue + the 2026-09-11 closures (64) · `SEC-2026-09-11`
and `SEC-2026-09-11b` **including their verified-CLEAN registers** (65, 66) · P108 `AC11`, closed by
ruling #13 (67) · the happy-dom narrative (68) · the open follow-ups as they stood pre-condensation
(69) · superseded curator bookkeeping (70). **Not archived:** both ruling blocks in full, the durable
rules, the accepted decisions, every open follow-up, the four user actions, the ruling queue.

**Earlier passes.** 2026-09-10 → Parts 54-61 (the eight confirmed native checkpoints); 2026-09-03 →
Parts 36-53, plus a staleness sweep that found **11 of 35 open entries had drifted**; 2026-09-01 →
Parts 22-35. The board's own record of being wrong is kept deliberately.

---

## ⏸ RESUME HERE — updated 2026-09-14

**Branch `feat/post-p91-rulings`, no upstream — 18 commits ahead of `origin/dev` (`8b88efd`),
unpushed.** Last commit `8026622` (2026-09-11).

**The P91 branch merge is DONE (2026-09-11, ruling #1)** — `feat/p91-observability` was
fast-forwarded onto `dev` and pushed. Every "DO NOT MERGE" / "unmerged by user instruction" line
this board used to carry is **void**; where one survives inside an archived part it is history, not
instruction. Curator-verified 2026-09-14: `dev` = `origin/dev` = `8b88efd`, and
`git rev-list --count cb70f4a..8b88efd` = **165** — the ledger's "164" was measured before `8b88efd`
(the jbcontext commit of ruling #2) existed. Both were true when measured.

**Current step: 2026-09-14 batch — `senior-dev` is implementing the two code items the 2026-09-11
`ui-designer` pass left OWED. Both are `in-progress`; neither is done; the orchestrator confirms.**

1. **D3** — `src/styles/dialogs.css:238`, `.op-worktree-warning`: `var(--danger-strong)` →
   **`var(--warning-strong)`** (ruling #7). Measured on `.dialog-card`'s `--bg-1`: **8.38:1 dark /
   6.65:1 light** against the shipped danger hue's 7.62/6.01 — the tone repaint **raises** contrast
   in both themes. No new token; no grep invariant moves.
2. **A3** — clear the two stale "pending A3 sign-off" comments at
   `src/components/SettingsAiRunSection.tsx:25` and `:46`. The copy was **SIGNED 2026-09-11** and no
   string in `src/` changes, so no test moves (detail under P69 below).

**Nothing was in flight at the 2026-09-11 session end** — architect, `ui-designer`, `senior-dev` and
`security-auditor` all finished that day. Narrative: archive Parts 62-68.

> **Observation, not a status (curator, 2026-09-14).** `git status` during this pass showed the
> working tree dirty **beyond** the two items above: new `src-tauri/src/obs/metrics_clear.rs`,
> `metrics_purge.rs`, `tests_metrics_clear.rs`, `tests_metrics_purge.rs`, plus edits across
> `src-tauri/src/obs/*`, `src/components/settings/*`, `src/ipc/*/obs.*` and
> `src/components/repoWorkspace/useTagSync.ts`. That shape is **F6** and possibly **P77** in
> progress. The queue below still reads "not started" because **only the orchestrator sets status**
> — reconcile it at the next commit rather than assuming either way, and do not clobber the tree.

### Next, in order — the queue the rulings created (detail one section down)

1. **P112 — remove user-supplied `terminalCommand` / `editorCommand`** (ruling #21) — `pending`
   (contracted): `P112-external-tool-detection.md` + `P112-tool-catalog.md` + `P112-ui.md`.
2. **F6 — `usage.json` 90-day window + deletable** (ruling #3) — `pending` (contracted):
   `P91-F6-usage-retention.md`. **But see the observation above** — the tree suggests it started.
3. **P77 — trigger `list_tag_sync` on auto-fetch completion** (ruling #11). Design settled.
4. **The e2e cold-timing MEASUREMENT** (ruling #9 — measure and report, **do NOT flip**).
5. ~~**The UNC / `\\wsl$` `canonicalize` check** on `216ca45` — ship-blocker.~~ **CLEARED 2026-09-14** by a real UNC probe; `\\wsl$` and OneDrive placeholders remain untested — see the section below.
6. **`h_ai` stub isolation** — until then run that binary with `--test-threads=1`.

### Four USER ACTIONS — only the user can clear these

- **Boot `pnpm tauri dev` with Dev mode ON** so P91's owed AI-gate item (a real `logs/*.jsonl` parse)
  can be done (ruling #16). Verified 2026-09-11: no Dev-mode key in the persisted `settings.json`, so
  it cannot be pre-set from disk, and the 2026-09-10 confirmation run produced no `logs/` directory.
  **Any boot is not enough — Dev mode must be on.**
- **Back up `.tauri/updater-prod.key`** (ruling #14, deferred). Gitignored and untracked, so it exists
  in exactly ONE place: this working copy. Losing it permanently breaks auto-update for every
  installed client. The committed `tauri.conf.json` pubkey was verified to match it. **P71 must not
  touch it.**
- **Identify the Dependabot moderate alert** (ruling #15 — "not now", and **no `gh` install
  authorised**, so the orchestrator cannot read the page). The *high* is the known `nanoid`
  GHSA-2v37-7h3g-55p8: build/test tooling only, deliberately ignored in `pnpm-workspace.yaml`.
- **macOS ad-hoc signing** — ⏸ PARKED 2026-09-11 as blocked-on-release, not open work (detail below).

### Verification state

- **Full 8-step gate green at `1d8c6f9` (2026-09-10, 452.5s)** — 2344 Rust tests, 185 e2e passed /
  1 skipped. Per step: nextest 133.2s · doctests 3.7s · clippy 22.8s · eslint 12.7s · size ratchet
  0.9s · vitest 82.9s · tsc+build 15.3s · e2e 181.1s. A later 8-step green (542.4s) closed the
  reviewer follow-ups over `e9d025d` + `7f9f16b`.
- **No full-gate run is recorded at or after `216ca45`, `1953c0a` or `9422e8b`.** The last recorded
  full-gate attempts under happy-dom are the **two that failed** on vitest (see the load-flake entry;
  `9422e8b` fixed two of the five affected tests). **Treat the gate state as unproven at HEAD.**
- Exit code 0 is not sufficient evidence, and neither is a piped log — see `### The gate-running
  rules` below, which those two facts earned.
- Port **1420 is free**. Keep it so: `strictPort: true` means a held port breaks `pnpm tauri dev`.

---

> **Curator note, 2026-09-14 — the ledger below is reproduced verbatim and is authoritative.** One
> thing around it changed: the evidence blocks its preamble points at ("the sections that follow")
> were archived to `docs/history/todo-archive-2026-09.md` **Part 63** once every item was ruled, and
> the second round of rulings was moved up to sit directly beneath it. Nothing inside either block
> was shortened.

## ✅ USER DECISION LEDGER — 2026-09-11 (all 17 open items ruled)

**Every FOR-USER decision below this section is now RULED.** The evidence that justified each
ruling is kept in place in the sections that follow; this ledger is the authoritative record of
*what was decided*. Do not re-open any of these without the user.

| # | Item | Ruling (user, 2026-09-11) |
|---|---|---|
| 1 | Branch `feat/p91-observability` | **MERGE to `dev`.** See the correction note below — the merge is a **fast-forward of 164 commits**, not 30. |
| 2 | Uncommitted `CLAUDE.md` + `context-explorer.md` | **COMMIT them** (jbcontext CLI→MCP migration), fixing the tool-list line that contradicts the agent's own frontmatter. |
| 3 | F6 `usage.json` | **Stay always-on, but 90-day window + deletable.** Statistics page stays viable. Disclosure copy still required (naming the `metrics` folder, not the file). |
| 4 | Security MEDIUM-2 `terminalCommand`/`editorCommand` | **Validate the shape now** (absolute path to an existing executable, no shell metachars, no arg injection) **and put removal of user-supplied commands on the roadmap.** |
| 5 | Username / home masking in raw log paths | **Mask the home prefix — and it MUST be cross-platform** (user's explicit addition): resolve the actual home dir per-OS so Windows `C:\Users\x`, macOS `/Users/x` and Linux `/home/x` all collapse. Do NOT pattern-match `C:\Users`. |
| 6 | Security LOW-1 cwd DLL search order | **Fix it in the same increment as #4's validation** (one call site, near-free while there). |
| 7 | D3 `.op-worktree-warning` | **Repaint as the warning hue.** Danger stays reserved for destructive/irreversible actions. |
| 8 | happy-dom | **ADOPT** (`docs/proposals/happy-dom.patch`) **and also do the lazy `window.location` fix** in `src/ipc/mock/repoState.ts:160`. jsdom stays installed so the shim self-guard stays meaningful. |
| 9 | e2e bundle default | **MEASURE and REPORT, do NOT flip.** One cold `E2E_BUNDLE=1` run vs the 162s dev figure *including build*. The number goes on the record; the default does not change without the user seeing it. |
| 10 | P69 A3 AI gate-note copy | **ui-designer finalises it** with the surrounding copy in view; its signed string ships. |
| 11 | P77 tag-sync check | **Fold into the existing auto-fetch cycle** (on, 5-min). No new trigger, no repo-open network call. |
| 12 | Process changes | **ALL THREE ADOPTED** — see the new rules block below. |
| 13 | P108 `AC11` | **ACCEPT the source-derived figures** for the two unreachable states; record the limitation and **close AC11**. |
| 14 | Back up `.tauri/updater-prod.key` | **Not now** — stays on the board as a user action. Still single-copy; still breaks auto-update for every installed client if lost. |
| 15 | Dependabot moderate alert | **Not now**, and **no `gh` install authorised** — so the orchestrator cannot read the page. Stays open as a user action. |
| 16 | P91 owed AI-gate item (real `logs/*.jsonl`) | **User will boot `pnpm tauri dev` with Dev mode ON.** Orchestrator parses the files once they exist. Verified 2026-09-11: no Dev-mode key in persisted `settings.json`, so it cannot be pre-set from disk. |
| 17 | macOS ad-hoc signing | **PARK as blocked-on-release.** Re-raise when a tag is next cut. Not open work. |

### Correction to the board's own commit count (verified 2026-09-11)

The RESUME block said "30 commits ahead of `5c2dcd2`". Measured:

- `dev` is at `cb70f4a` and is a **strict ancestor** of this branch → the merge is a **pure
  fast-forward**, no merge commit, nothing to resolve.
- `cb70f4a..5c2dcd2` = **126 commits**; `5c2dcd2..HEAD` = **38** (the board's "30" is stale by 8).
- Total landing on `dev`: **164 commits**, of which 126 predate the board's own reference point.
  The "30 commits" framing described only the recent P91 window, not what `dev` has never seen.
- Also on the remote: `origin/Dev` (capital D) at `691f48b`, a separate ref from `origin/dev`
  (`cb70f4a`). A case-collision artefact — not touched, but do not confuse the two.

### The three process rules adopted 2026-09-11 (user)

1. **Batch small P-tasks through ONE senior-dev spawn.** Every fresh subagent re-pays this
   CLAUDE.md + its agent def (~6-8k tokens) before doing anything.
2. **Skip the architect contract for single-component fixes.** Accepted cost: nothing survives on
   disk if the session dies mid-increment, so keep such increments short.
3. **Fold the board update into the feat commit.** Accepted cost: a docs-only commit can no longer
   be identified as such from the log.

---

## 🆕 SECOND ROUND OF USER RULINGS — 2026-09-11 (four more, from the review findings)

These came out of the reviews of the first increment, not from the original 17. Same authority.

| # | Item | Ruling |
|---|---|---|
| 18 | MCP audit scope (`stage_paths` HIGH + MEDIUM + 4 LOWs) | **DO EVERYTHING IN THE AUDIT.** Not just the symlink guard. **QUEUED, not started.** |
| 19 | MCP review gate | **Snapshot test + review trigger.** A test snapshotting `list_all()` tool descriptions, **and** a rule that any diff touching `crates/bonsai-mcp/src/server/tools_*.rs` requires a `security-auditor` pass regardless of the commit subject. **QUEUED, not started.** |
| 20 | macOS `open -a` / `.app` regression from the `{path}` removal | **Teach the ladder app bundles** — Bonsai supplies the arguments itself, so no injection surface. **SUPERSEDED IN PLACE by #21:** this moves into the *detection* logic of the removal milestone rather than into the configured-program path, which is being deleted. Recorded so the intent is not lost. |
| 21 | Security MEDIUM-2, after the auditor showed validation insufficient | **DO THE REMOVAL NOW**, as its own milestone — not the interim native-confirmation dialog. Free-text program entry is eliminated and replaced by a detected-list picker. Contract in flight. |
| 22 | Fail-open home masking + the LOW-1 doc claim | **TAKE BOTH.** |

### Why #21 — shape validation does NOT achieve its claimed property

The first increment's own comment claims "renderer compromise ≠ arbitrary local execution". The
security auditor refuted it with **three routes that survive validation**, all needing only script
execution in the renderer plus a repo that was cloned once:

1. **The absolute branch accepts any existing file** — `external_cmd.rs:145-155` gates on `is_file()`
   alone: not executability, not location, not trust. A hostile repo ships `payload.exe`; the
   renderer points `editorCommand` at that absolute path; it validates; and `external.rs:319` sets
   `hide_console = true`, so it runs under `CREATE_NO_WINDOW`. **PRECISION 2026-09-11 (architect
   refuted my looser wording):** that suppresses the console of a **console-subsystem** image only —
   a GUI payload still shows its own windows — and the file must be a PE or `.cmd`/`.bat` and contain
   no `is_shell_syntax` character. The route is real; "silent, invisible execution" was my overstatement. The
   increment's own test (`external_cmd_tests.rs:60-67`) documents the shape: it writes a stub named
   `my-editor.exe` containing `b"stub"` and asserts acceptance.
2. **Bare interpreter + repo as argument** — `node <repo>` executes the repo's own `package.json`
   `main` or `index.js` (`external.rs:244`). More likely than the `python` the doc cited, on a
   developer machine and in a JS project.
3. **Bare build tool as the TERMINAL command** — `make` / `nmake` / `just` / `msbuild` with
   cwd = repo and **zero arguments** (`external.rs:245`). This route **breaks the residual's own
   "at most one argument, which must be an existing directory" wording.**

Root cause: it is a **free-text program string the renderer can write** via `set_ui_settings`.
Validating its shape cannot fix that. The audit status is being downgraded from "Closed 2026-09-11"
to **partially closed** for both MEDIUM-2 and LOW-1.
Also: `src-tauri/src/commands/external.rs:89` `launch_inner` accepts **any existing directory the
renderer names**, not the selected repo — pre-existing P49 design, so the argument was never bounded
to the open repo either.

**Mitigating context (why this was MEDIUM, not HIGH):** the CSP at `src-tauri/tauri.conf.json:23`
is `script-src 'self'`, `object-src 'none'`, `base-uri 'none'`, which blocks inline script and
inline event handlers. Noted by the auditor as premise context: two `dangerouslySetInnerHTML` sites
render syntax-highlighted diff content (`DiffView.tsx:294`, `DiffViewSplit.tsx:162`) whose
highlighter escaping was **not** audited — pre-existing and CSP-mitigated, **not** opened as a
finding, but it is the premise the whole MEDIUM rests on.

### LOW-1's surviving rung is the Windows 10 DEFAULT, not an edge case

`wt` is **not present on stock Windows 10** (this machine is Win10 Enterprise), so the live rung is
`powershell` (`external.rs:278`), which keeps `cwd = <repo>`. A hostile repo shipping a hijackable
DLL at its root therefore has its DLL-planting primitive against the **default** Win10 "Open in
terminal". Keeping cwd is still correct — the alternative,
`powershell -Command "Set-Location '<path>'"`, turns a repo-authored path containing a quote into
PowerShell injection, a strictly worse trade. **`x-terminal-emulator` is being dropped from the
residual list:** neither the Linux dynamic linker nor macOS dyld searches the current directory by
default, so only the Windows rungs carry the risk.

### Home masking was FAIL-OPEN — fixed under #22

`obs/mod.rs:177-182`: if `app.path().home_dir()` returns `Err`, `set_home_dir` is never called,
`HOME` stays unset, `mask_home` returns the input untouched, and raw mode writes
`C:\Users\<account>\…` into every part that goes into the mailed export zip. The only signal was an
`eprintln!`, **which goes nowhere in a release GUI build.** This contradicted `scrub_home.rs:30`'s
own stated failure direction. Compounding it: **nothing tested the wiring** — deleting the
`set_home_dir` call would have failed no test. Fix: move the home string into `writer::WriterConfig`
(testable, no process-global `OnceLock`) and stamp `homeMasking: <bool>` into the session header so
an export reader knows whether to trust it.

### Verified CLEAN by the security auditor — do not re-audit

Every reader of the two settings (exactly two consumption sites, both behind validating entry
points; the MCP server and AI CLI driver read neither key) · validate-then-launch trim equivalence
(both sides `str::trim` on the same `&str`) · argument smuggling through the permissive absolute
branch is **structurally impossible** (the string becomes `LaunchSpec::program` only, never an arg;
`Command::args` with an argv vector; nothing reaches a shell) · UNC, drive-relative, rooted-no-drive,
control/bidi chars all refused and tested · ADS / 8.3 aliases / trailing dots / `..` / symlinks /
the `is_file()`→spawn TOCTOU are all **subsumed** by route 1 rather than separate findings · the cwd
carve-out is exactly the four rungs claimed, with an iff test · `procutil::resolve_program` searches
PATH only, never cwd · the export zip contains only `bonsai-*.jsonl` parts (no manifest, no metrics
file, no environment dump) and every part goes through `append_record`, so no emit site bypasses the
scrub order · `metrics/usage.json` and its `.bak` **cannot carry a path** (no path fields;
`metrics_keys.rs` rejects separators) and are not in the zip regardless · `set_home_dir` runs before
`Sink::start`, and `apply_dev_settings` is the sole production sink constructor.

**Could NOT verify (so the CLEAN register does not over-claim):** Rust std's Windows `resolve_exe`
`.exe`-suffix appending and its `.bat`/`.cmd` → `cmd.exe` wrapping — `rust-src` is not installed in
this sysroot. Medium-high confidence from the post-CVE-2024-24576 implementation; neither changes
route 1, which stands on `.exe` alone. No launch was executed; all findings are from code reading.

---

## 🔄 The 2026-09-11 ruling queue — what is NOT in a contract

The rulings are in the two ledgers above; the contracts are on disk and indexed in
`docs/contracts/INDEX.md`. Only what neither carries is repeated here. Narrative: archive Part 64.

### P112 — remove user-supplied `terminalCommand` / `editorCommand` (rulings #4 → #21)

- Contracts: `P112-external-tool-detection.md` (behaviour) · `P112-tool-catalog.md` (data) ·
  `P112-ui.md` (signed 2026-09-11). Status `pending` (contracted).
- Both settings are **empty strings** in the user's real `settings.json` — nothing installed depends
  on this capability.
- **`safe_cwd()` must STAY.** The board once claimed removal retires it; the architect refuted that —
  LOW-1 is about a hostile **repo** as cwd on the **auto** rungs, unrelated to user-supplied commands,
  and `external_url.rs:127` depends on it. P112 moves it verbatim into `procutil.rs` (contract §0/§7).
- `dc295c5`'s shape validation is **the stopgap, not the design** — its launch-path comment must keep
  saying so, and **MEDIUM-2 / LOW-1 stay `partially closed` until P112 ships.**

### F6 — `usage.json` 90-day window + deletable (ruling #3, contract `P91-F6-usage-retention.md`)

- Status `pending` (contracted). The contract's §2 SUPERSEDED pointers and §11 decision rows 34-36 were spliced
  into `P91-observability.md` by the curator 2026-09-14, so that file no longer reads stale.
- **The one failure mode a test will not catch:** the delete must reset the in-memory `MetricsState`
  as well as remove the `metrics` **directory** — otherwise the next flush rewrites the deleted data
  **and a test that only asserts the file is gone PASSES while the button does nothing.**
- **The board's own "always-on" claim was an OVERCLAIM:** always-on is **counts**; the **durations
  half is Dev-mode-only** (`metrics.rs:148-152`). `P91-privacy-copy-ui.md` §6.1.1 is the source of
  truth over this board. `RETAIN_DAYS` = `obs/metrics.rs:40`, still 400.

### P77 tag-sync — the design, settled by measurement rather than guesswork

- Auto-fetch **already downloads tags**: `src-tauri/src/scheduler/exec.rs:151` → `fetch_all()` →
  `crates/bonsai-core/src/git/remote_activity.rs:52`, `opts.download_tags(AutotagOption::Auto)`.
- **A purely local compare is NOT sufficient** — fetched tags land in `refs/tags/*` beside local ones,
  so classification needs the `ls-remote`: `crates/bonsai-core/src/git/tag_sync.rs:275-288` →
  `ls_remote_tags()` → `remote.list()` at `:132-173`.
- **So the increment is: trigger the existing `list_tag_sync` on auto-fetch completion.** That honours
  ruling #11 — it rides a cycle the user already opted into (5-min, enabled), adds no repo-open call
  and no new network policy. Trigger to replace/augment:
  `src/components/sidebar/TagsSection.tsx:165-172` → `RepoWorkspace.tsx:1878`
  `onTagsExpand={() => void refetchTagSync()}`, 10 s cache guard at
  `src/components/repoWorkspace/useTagSync.ts:57-60`.

### The e2e bundle default — MEASURE and REPORT, do NOT flip (ruling #9)

- The gap: **162 s dev vs 122 s bundle**, and it is **not established that the 122 s includes the
  bundle build**. If it does not, flipping makes the default gate *slower* — the opposite of the point.
- **Time one `E2E_BUNDLE=1` run from a cold build**, compare against the 162 s dev figure *including
  build*, and put the number on the record. **The default does not change without the user seeing it.**
- The flip, if ever authorised, is one line each: `playwright.config.ts:38` →
  `const BUNDLE = process.env.E2E_BUNDLE !== '0'`, plus inverting `--e2e-bundle` in `gate.mjs`.
  Bundle mode is equally green and higher-fidelity (P103 was visible only there). Context: Part 48.

### ✅ UNC `canonicalize` — ship-blocker on `216ca45` CLEARED by measurement 2026-09-14

`stage_paths` inherits `ensure_within_workdir`'s **`fs::canonicalize(workdir)`** dependence —
pre-existing for `discard` / `stage_partial` / `conflict`, newly extended to the highest-traffic
write primitive. The auditor's concern was that on a UNC, `\\wsl$` or cloud-placeholder workdir a
canonicalize failure becomes `AppError::Io` and **refuses EVERY stage, including from the UI.**

**Tested for real**, not reasoned about: a standalone replica of `ensure_within_workdir` (an exact
copy of the function as of `d46c98e`) run against a real UNC workdir via the `\\localhost\D$` admin
share.

| Case | Result |
|---|---|
| Drive workdir (control) | OK — `canonicalize` yields the `\\?\D:\…` long-path form |
| **UNC workdir, normal file** | **OK** — `canonicalize` yields `\\?\UNC\localhost\D$\…` |
| UNC, nested existing dir | OK |
| UNC, not-yet-created dir | OK — the walk-up loop terminates correctly |
| UNC, `../outside.txt` escape | **correctly REFUSED** |

`canonicalize` **succeeds** on UNC, and both `base` and `real` come back in the same `\\?\UNC\` form,
so `starts_with` compares consistently — the guard neither fails open nor fails
closed-on-everything, which was the actual worry.

**Honest limits of this check:** `\\wsl$` could **not** be tested (WSL is not installed on this host
— `Test-Path` on it returns false), and **cloud placeholders (OneDrive) were not tested**. Both go
through the same `canonicalize` call that UNC now demonstrably survives, so the *mechanism* is
exercised — but neither path is empirically confirmed, and this entry should not be read as saying
otherwise. The probe is kept at `D:\Data\Temp\claude\unc-test\unc_check.rs` for whoever has such a
host. Original review: archive Part 66.

---

## Durable lessons — the rules

The reusable rules. They are on the board, not in the archive, because every one was learned by a
claim that was green the whole time it was wrong. **The stories, worked numbers and measurement
narrative behind them are archive Part 53** — cite the rule here, read the story there.

### The six failed app-wide claims

Every one had the same shape: **a sentence claiming an app-wide property, with a call-site count that
nobody enumerated.** The first five failed for want of an enumeration. The sixth is worse — the
enumeration **existed** and was still blind, because it was **scoped by token name**.

1. **P95** — the enabled-control class; 3 escapes found by P101.
2. **P98** — "`--text-3` family closed"; 122 declarations were never classified.
3. **P74** — the hue-as-text sweep; became P105.
4. **`ui-reference.md` §2** — "6 live hue-over-own-tint instances"; the real population is **38**.
5. **P106's hand-over count of 48** for P108; the real inventory was **62**.
6. **P101** — an *exhaustive* `--text-3` audit recorded as CLOSED, which still missed a **2.96 light**
   glyph, because **`--badge-unknown` is byte-identical to `--text-3`**.

**The rule:** a bucket + verdict per call site, P101 §3 style, or it is not closed. Enumerate,
bucket, record a verdict per site, predict the post-fix residue, then verify the prediction.
**Do not accept a "~N call sites and it's fine" sentence as evidence.**

### The aliasing rule

- **An audit scoped by token NAME cannot see an alias. Scope by resolved VALUE.**
- Token aliasing has hidden instances three times: `--badge-good`/`--badge-warn` are byte-identical
  to `--success`/`--danger` (the first pair), then `--badge-unknown` to `--text-3`.
- A `var()` **fallback masking a missing token is invisible to any hue-name search**, because the hue
  name appears only in the fallback (`--warn`, which is defined nowhere).
- A naive probe of an **undefined** custom property returns the *inherited* value, not the value the
  contract cites — same trap class.

### The grep-counting rules

- **An acceptance grep counts *text*.** Prose comments and `var()` fallbacks inflate it. P106's R10
  read 0 predicted vs **12** measured: 7 hex literals quoted inside comments, 5 `var()` fallbacks.
- A residue prediction must state whether it counts **declarations or raw matches**.
- **A baseline must be measured against the real pre-fix tree**, never inherited from a prior
  contract. Both of P106's misses were the contract's *baselines* being wrong, not the fix.
- **When a grep and a prediction disagree, re-measure the baseline BEFORE touching code.**
- Where a prediction names `file:line`, **the count is the criterion** — comments added by the fix
  itself shift the lines (216/227 → 220/231 in P106; count unchanged).
- An implementer must deliberately avoid literal strings in its own comments: without that care two
  P106 counts would have read 11 and 2 instead of 8 and 1.

### The BASE rule

- **A contrast figure is meaningless without its composited base.** Every ratio must name the ink,
  the tint, **AND** the base.
- A figure naming only the tint is **incomplete evidence and may not be used to close an AC**.
- Proof: `--accent-strong` on a 14% accent tint measures **6.42/5.19 over `--bg-0`**, **5.85/4.87
  over `--bg-1`** (P107's figure) and **5.16/4.52 over `--bg-2`** (P106's) — one method, three bases.
  The two contracts never disagreed; **neither stated its base**, and the base alone accounts for
  **1.26** of dark-theme spread.

### The three P91 testing rules — now in the contract, not only in session memory

1. **A writer rule is covered only by a `LogRecord` → `append_record` → read-back round-trip.**
   Synthetic-`Value` tests are additive, never substitutive. This is what let W6 ship dead in
   production while its test passed — the one rule of six that skipped the round-trip.
2. **A validator's tests must use inputs the REAL producer emits**, and a cross-boundary vocabulary
   must be pinned by a drift test that re-derives it from the producer's own source. *A predicate
   that rejects everything is indistinguishable from one that works, unless something asserts a real
   input is ACCEPTED.* This is the `cmd.*` camelCase bug — the **entire `cmd.*` histogram family
   recorded NOTHING in production** while every test was green.
3. **A negative test must be PROVEN to fail on the unfixed code.** Saying "this must fail on today's
   code" is not proof. AC6 as originally written specified a payload the scrubber **already caught**,
   so it would have been green on the buggy code — a negative test that could not go red.

### Two more failure modes, each named after it cost a session

- **Evidence lost in transcription** (named at `ef06e6b`) — a figure or a qualifier that survives the
  measurement and dies in the summary. P108's AC11 is the live example: **source-derived and
  unverified** must travel with the number.
- **The specificity trap** — **a contrast fix that is out-specified by an existing rule is a no-op
  that still passes a grep.** `.diff-stage-float button` (0,1,1) out-specified `.diff-float-discard`
  (0,1,0), so the destructive discard button rendered in accent blue with no danger hue at all while
  passing every AC grep. Sibling to the child-rule trap; both are in `ui-reference.md` §2.

### The measurement rules (numbers: archive Part 46; the worked narrative: archive Part 53)

- **Optimising the measured-slowest test may not move wall clock, because a different test becomes
  the floor.** Net for the 2026-09-03 pass: `cargo nextest --workspace` **106.5s → 92.5s** with tests
  *increasing* 2298 → 2316.
- **Concurrent agents on this box produce outliers** — single-run numbers are worthless; use paired
  or repeated runs.
- **proptest regression seeds can be worse than useless**: proptest keys its persistence file **per
  source file, not per test fn**, and a `cc` seed regenerates values through the *current* strategy.
  Random cases wearing a regression label.
- **Do not run the e2e suite concurrently with other heavy jobs.** `gate.mjs` is strictly serial, so
  the gate itself is safe.
- **A flake that reproduces deterministically in a production bundle is not a flake** (P103).

### The gate-running rules (the gate states that earned them: archive Parts 51 and 58)

- **Before trusting any timing-sensitive failure, verify machine state AT THE TIME IT RAN** — sample
  CPU repeatedly, look for scratch/load processes, confirm no agent is mid-run.
- A slow timing number is **evidence about the machine** until proven otherwise. Timing analogue of
  the grep-counting rule: *measure the baseline, do not infer it.*
- **Verifying machine state before running the gate is a standing pre-gate step**, not a nicety.
- The e2e leg **will** fail intermittently on a loaded machine — expected, documented at P104 (Edge
  misses a hardcoded 30 s CDP close window, then a blocking `taskkill` runs).
  `FIRST_PAINT_TIMEOUT = 15 s` (`c6cd7dd`) removed the largest source, measured at 7.6× p99.
- **Run the gate on an otherwise-idle machine, or run e2e with `--workers=1`.**
- **Give subagents `--workspace`, not `-p <crate>`, whenever a change crosses a crate boundary.**
  A remediation brief scoped to `cargo nextest -p bonsai-core` never ran the `bonsai` crate's tests
  under `src-tauri/`, and one of them asserted the exact behaviour the change removed — the gate's
  first run failed on it.
- **Redirect the whole gate log to a file**; read the summary from there. Piping through `tail -60`
  lost the failure detail and cost a re-run of the Rust leg.
- **Read the `gate summary` block, never the exit status alone** — a background wrapper reported
  exit 0 while the gate had failed (that was the pipeline's exit code, not the gate's).
- **That rule applies to a bare `cargo test` too, not only to `pnpm gate` — 2026-09-11 cost proves
  it.** The orchestrator piped three `cargo test` runs through `head`/`tail`, read exit 0, saw a
  truncated log with six `FAILED` lines and no `test result:` summary, and reported six phantom
  `h_ai` failures as a possible regression. Serialized and unpiped, `h_ai` was **45/57 passed, 0
  failed** — the truncating pipe manufactured the failure *and* hid the evidence that would have
  disproved it. **A cargo run whose log has no `test result:` line has not finished; it has been
  cut off.**
- **SERIALIZE cargo. Concurrent `cargo` invocations queue on the build-directory lock, and a queued
  cargo is INDISTINGUISHABLE FROM A HANG** — several `cargo.exe`, **zero `rustc`**, negligible CPU,
  no output for 20+ minutes. That is what *waiting for a lock* looks like, not a deadlock. On
  2026-09-11 the orchestrator read that signature as wedged and killed the processes **twice**;
  they would have drained on their own, and each kill discarded build progress and forced a cold
  vendored-libgit2 rebuild. **Do not kill them. Run one cargo at a time and wait** — CLAUDE.md's
  "never conclude failure from a timeout" covers this exact case.

---

## Accepted decisions that must survive compaction

- **Accepted defaults (2026-08-08, "ACCEPTED AS-IS"; changeable any time):** P55 `undoLastMerge` =
  reset-to-first-parent (Mixed, rewrites history, confirm-gated) · P57 retriever = BM25 lexical, no
  embeddings · P61 image-diff base64 = hand-rolled, no new crate.
- **OD1 (confirmed):** AI stays **local-`claude`-CLI-only**; model tiers deferred.
- **Forge defaults (2026-08-08, accepted):** new Rust deps `reqwest{blocking,json,rustls-tls}` +
  `keyring` · auth = **PAT-only** v1 (OAuth device-flow deferred) · provider order GitLab →
  Bitbucket → Azure DevOps.
- **v1.0.0 shipped** 2026-08-18 (tag `bd52483`), unsigned; forge/PR flagged beta.
- **P62-P74 native USER CHECKPOINTs were WAIVED and marked `done` 2026-08-20** (user decision).
- All native USER CHECKPOINTs for **P2 → P61** are confirmed; P70, P77, P78/P79/P80, P80b/P81/P82,
  P83, P84, P85-P90, P92, P93, P95, P96, P97, P98, P99, P100, P101 and DEP REFRESH are confirmed too.
  Full per-milestone text → `docs/history/todo-archive-2026-08.md` Parts 17-21 and
  `todo-archive-2026-09.md` Parts 22-31, 47.
- **P100 + P101 checkpoints were recorded 2026-09-02 on the user's direct instruction**, not on a
  contemporaneous native run — the user was going away for an unattended session and scoped that
  authority **to those two milestones only**. It does **not** reach anything created that session.
- **P75 (IPC codegen) — HALTED 2026-08-21 (user decision).** Linking `tauri-specta` breaks app launch
  on Windows 10 (`kernel32!WaitOnAddress` not exported → `STATUS_ENTRYPOINT_NOT_FOUND`). Spike
  reverted; findings + crate pins kept in `docs/contracts/P75-ipc-codegen.md`. Revisit only if
  validated on Windows 11 or with a link-order fix.
- **P76 (native-checkpoint automation) — HELD as contract-only per user (2026-08-20).**
  `docs/contracts/P76-native-checkpoint-automation.md`.
- **Process change adopted (P100 §6-D).** `ui-reference.md` is ~1322 lines / ~40k tokens and
  `ui-designer` has **no `Edit` tool** — only whole-file `Write`, which **truncates mid-file** at that
  size. That is the structural cause of the P95 "silently unapplied patch". From P100 on: the designer
  supplies **verbatim line-anchored hunks**, the orchestrator applies them with `Edit`, and verifies
  line count + section count + tail sentinel + hunk confinement. **This deviates from CLAUDE.md's
  "no other agent edits `ui-reference.md`" — raise with the user whether to give the designer `Edit`
  or split the file.**
- **Do NOT run prettier in this repo** until a config exists — there is no `.prettierrc*`, no
  `prettier.config.*`, no `package.json` key and no `.editorconfig`, so prettier falls back to its
  defaults and rewrites whole files (74 insertions for a ~20-line edit; double quotes at 80 columns
  against the repo's ~100).
- **Do NOT run `cargo fmt`** as part of another change — see the open item below.
- Gate child processes use `D:\Data\Temp\bonsai-build`, not Defender-scanned `C:\Temp`.
- Ports: browser harness **1420** · e2e dev server **1430** · e2e built bundle **1440**.
- Velocity/gate-cost numbers: `docs/history/velocity-2026-09-01.md`. Ceremony — not machine time — is
  **~75-85%** of per-task wall clock, which is what CLAUDE.md's velocity-mode and batching rules
  exist to cut.

- **All eight native USER CHECKPOINTs were confirmed by the user on 2026-09-10** (`548cc0a`):
  P102+P105, P106, P107, P108, P91, P110, P109, and the `8dd5b24` CSP change — the last of these
  needed a native run because it applies to the Tauri webview only, so neither the harness nor e2e
  ever exercised it. Full per-milestone text: archive Parts 54-55, with the board's own record of
  the confirmation in Part 60. ~~**The confirmation does not reach two AI-gate items** (P108 `AC11`,
  P91's `logs/*.jsonl` parse) and **is not authorisation to merge `feat/p91-observability`**.~~
  **Superseded 2026-09-11 — see the dated additions below:** the user ruled MERGE (#1) and closed
  P108 `AC11` (#13). **Only P91's `logs/*.jsonl` parse is still owed**, and it is still true that no
  native confirmation can close it.

**P91 user decisions that must survive compaction (all 2026-08-27 unless noted).** Build diary:
archive Part 44; the milestone entry is archive Part 54.6.

- Redaction conservative by default, opt-in raw-names toggle; credentials never logged in any mode.
- Metrics storage = **rolled-up JSON**, not SQLite.
- React instrumentation = **SIX surfaces** — the user added the **left sidebar** to the original five.
- Logs are **NOT** auto-deleted when Dev mode goes off (prune by caps only); per-session log files;
  `metrics_reset` ships headless.
- Decision 7 — **"Delete all log files" ships in v1**, model = **roll-then-purge**, scope hard-limited
  to `logs/*.jsonl` + `.tmp`, never `metrics/` or `settings.json`. Success copy must say "Still
  recording" when `rolled:true`.
- **USER GATE (2026-08-27):** each increment requires its own explicit go from the user.
- ~~**Branch policy (USER, 2026-09-02):** everything this session lands on `feat/p91-observability`;
  local commits only, no push.~~ **Superseded 2026-09-11 (ruling #1):** the branch was merged to
  `dev` and pushed. The policy is history, not instruction.

**P91 — three architectural rulings not to re-open.**

- **NO global cap on metric keys.** Worst case is genuinely per-map-per-bucket, ~616k keys, accepted
  rather than glossed: a global cap would make *today's* recording depend on *history*. 512 is a
  **runaway stop, not a sizing parameter**. File-size pressure has a different lever (size-triggered
  early roll-up) — revisit trigger only, ~8 MB, no work now.
- **The writer's limit, recorded honestly in both contracts:** `raw_args.rs` enforces **shape +
  vocabulary, not semantics**. Content under an identifier-named key, ≤512 chars, single-line,
  **survives**. Its only defence is the positional drift guard in `rawArgPolicy.test.ts`.
  **Neither guard may be removed citing the other.**
- **The raw-args ruling (`834f2d1`):** raw mode widens **IDENTIFIER** fidelity (repo path, file
  paths, ref names, remote URLs, full SHAs) and **never CONTENT** fidelity. Free text and credentials
  are outside both modes, permanently.

**Added 2026-09-11 (the ruling session) — facts, not narrative. Detail: archive Parts 62-68.**

- **All 22 open FOR-USER items were RULED** (17 + a second round of 5). Both ruling blocks above are
  authoritative; do not re-open any of them without the user.
- **`feat/p91-observability` was MERGED to `dev` and pushed** (ruling #1), a pure fast-forward;
  `dev` = `origin/dev` = `8b88efd`, `cb70f4a..8b88efd` = **165** commits (curator-verified
  2026-09-14). Every "do not merge" instruction on this board is void.
- **P108 `AC11` — CLOSED by ruling #13:** the two source-derived **3.05** figures for the two
  unreachable states are ACCEPTED, limitation recorded. Verbatim text: archive Part 67.
- **happy-dom ADOPTED as the vitest DOM environment** (ruling #8, `1953c0a`); `jsdom` stays installed
  so the shim's self-guard stays meaningful and `docs/proposals/happy-dom.patch` stays on disk so a
  revert is one command. The shim is a **hand-maintained subset of the UA stylesheet** — an inline tag
  outside that set silently gets `display: block`, landing precisely in accessibility-name
  computation (715 `ByRole(…, { name })` call sites across 77 files). **That quiet failure mode is an
  ACCEPTED risk.** Measured win: −26% wall, −44% environment CPU.
- **Home/username masking is FAIL-CLOSED and cross-platform** (rulings #5/#22, `dc295c5`): the home
  string moved into `writer::WriterConfig` (testable, no process-global `OnceLock`) and every session
  header carries a `homeMasking: <bool>` stamp so an export reader knows whether to trust it. Folded
  into `P91-observability.md` §7.5. It was **fail-open** before, with `eprintln!` as its only signal —
  which goes nowhere in a release GUI build.
- **The MCP tool-description review trigger is now in CLAUDE.md** (ruling #19): any diff touching
  `crates/bonsai-mcp/src/server/tools_*.rs` requires a `security-auditor` pass **regardless of the
  commit subject**, with a description-snapshot test guarding text drift. `rmcp-macros` concatenates
  every `///` line into the JSON-Schema `description`, so those comments are the **tool contracts a
  model reads** before invoking worktree-destructive operations.
- **P94 has no contract file — confirmed, accepted as debt.**

> **The second round of user rulings (items 18-22) used to sit here, after this section.** It was
> moved up to sit directly beneath the first ledger on 2026-09-14 — see `## 🆕 SECOND ROUND OF USER
> RULINGS — 2026-09-11` above. Nothing in it was shortened.

---

## OPEN follow-ups (genuine unresolved items, not checkpoints)

Condensed to one line per item on 2026-09-03 and again 2026-09-14; pre-condensation text is archive
Part 50 and **Part 69**. Nothing here was closed by the curator.

### ✅ Closed 2026-09-11 by orchestrator verification (user assented) — one line each, Part 64

- **The four 2026-09-02 file-size refactor follow-ups — CLOSED:** `52c815e`, `6092eb3`, `338d71f`
  (overlay teardown), `1d9d9bf` (disarm armed dialogs), `734b310` (watchdog on an injectable clock).
- **The two 2026-09-01 velocity follow-ups — CLOSED as superseded** by `737cc4b` (band the two
  slowest proptests) and `5731d37` (workspace test wall −14%).
- **`P91-raw-args-privacy.md` — CLOSED, already done:** folded into `P91-observability.md` §7.4 by
  `12b0ab6`; the file is absent.
- **`P91-observability-ui.md:496`/`:951` — `INDEX.md` was right, the BOARD was stale**; `fc9c36e`
  fixed it. The architect's own `P91-observability.md` §6/§10 are the copies **still** stale.
- **P87b `FU-1..4` — three of four were NOT open.** FU-1 shipped `1d8c6f9`; **FU-2's premise is
  false** (`commitAmend` *is* activity-wrapped, `staging.rs:179`); FU-3 closed by `833f2f9`; FU-4
  answered by *rejecting* the change (`763866a`). **Do not re-open them — the board has been wrong
  about this entry three times.** Only the `AiActivityPanel` aria-label NIT is arguably live, and
  `AiActivityPanel.tsx:192-193` already carries `role="region"` + `aria-label="AI activity"`.
- **P94 has no contract file — CONFIRMED, accepted as debt.**

### Still open, short form

- **P87b contract-hygiene residue — filed, NOT verified closed.** Five corrections in
  `docs/contracts/P87b-FU1-run-target.md` (§1 line-count estimates · §3's drifted
  `remote_push_activity.rs` ranges · §4's wrong unborn-HEAD rationale — the `if head.unborn ||
  head.detached` guard is **load-bearing**, not redundant · §8's understated seams + its 83-char
  `MOCK_LONG_TARGET` · §8/§9's **literal U+202E / zero-width chars in the very section that forbids
  them**), plus `P87b-FU1-FU4-git-dock-ui.md` F-F(a). `2aa1e06` and `f00fad3` claim to have closed
  some; **which ones is unverified.** Hand the list to the next `architect` spawn on that file.
- **STANDING WARNING — do NOT ungate `''` from `hasExecExt` in `whichAll`**
  (`scripts/lib/spawn-tool.mjs`). npm/corepack install **extensionless POSIX shell scripts** beside
  every shim and `resolveTool` picks `hits.find(p => !isBatch(p))`, so an unconditional `''` selects
  an unrunnable script. 3 regression tests guard it.
- **STANDING WARNING — the toast auto-dismiss timer fix is deliberately REVERTED.**
  `React.StrictMode` makes cancel-on-unmount strand a toast on screen permanently;
  `useToastQueue.test.tsx` carries the finding. Reasoning: archive Part 52.4.
- **Deliberately not taken:** `src/obs/types.ts:68` still cites `A26 §D`, the dead lettered scheme
  retired inside `raw_args.rs`. A repo-wide letter→section migration is a decision, not a drive-by.

### P91 — open items (branch merged 2026-09-11; one AI-gate item still owed)

Milestone detail: Part 54.6 · security arc 42 · audit F1–F9 43 · build diary 44 · SHOULD-FIX full
text 45 · pre-condensation board text 69.1. User decisions + architectural rulings: see
`## Accepted decisions` above.

- **OWED AI-GATE ITEM — the real `logs/*.jsonl` parse** → the USER ACTIONS block above. A native
  checkpoint confirmation does not reach it.
- **F7 — LOW, mostly latent.** `redact_names` misses bare ref/file names and never touches JSON keys
  (`feature/acme-client-migration` would be written verbatim into a strict file). `strict::enforce`
  is the **sole** enforcement point for Rust *and* frontend records, so a gap there is a single point
  of failure. **F9 — INFO:** the two redactors cannot disagree because only one enforces (`redact.ts`
  has no `redact_names`) — which is why F7's gaps matter more than their reachability suggests.
- **SHOULD-FIX: `SAVE_LOCK` orders the rename pair but NOT the snapshot** (`metrics.rs:379-382`,
  `:409-425`) — two savers can snapshot A→B but lock B→A, so **a `metrics_reset` can be silently
  undone on disk**. **⚠️ CONTRADICTION flagged 2026-09-14, unresolved:** `P91-observability.md` §13
  **row 32** records this exact defect as **fixed in `bcb3720`** (monotone `rev`/`commit_rev` stamp)
  and §8.3 ratifies the design. Verify against the tree before doing any work here.
- **SHOULD-FIX: the `last_fire` prune assumes non-decreasing `ts`** (`window.rs:52-63`), which merges
  two unsynchronised clocks (`src/obs/log.ts:43`, `sink.rs:385`) with no monotonic clamp; blast radius
  is a **duplicate** anomaly record, never a missed one. **⚠️ Same contradiction class:** §13 **row
  33** records the premise as stated in code and a `cutoff` guard as **rejected on merit**.
- **NIT:** `dup_ipc_debounce_map_stays_bounded_over_a_long_session` spaces events 100 ms apart against
  a 300 ms window, making `len <= 4` nearly tautological; `last_fire` has **no numeric cap**, unlike
  `open_calls` (FIFO 1024) and `slow` (LRU 200).
- **Contract follow-ups owed to `architect`:** §6/§10 still specify the removed
  `log_export_session(dest?)` — **confirmed live**. The other three on the old list (`cmd.*`
  camelCase, `MAX_KEYS_PER_MAP` ratification, decision 25's counter-key shape) **appear already
  discharged** by §13 rows 28, 29 and 25 — **contradiction flagged 2026-09-14, not closed.**
- **`.forge-connect-link:hover` is a no-op** — the resting-underline MUST-FIX means hover declares the
  same underline, so the link has **no hover feedback at all**. → `ui-designer`.

### The security record — where it lives, and what is still open

- **`SEC-2026-09-11`** (MCP tool-contract audit of `2a0b8f1`: HIGH `stage_paths` symlink escape,
  MEDIUM `git add -f` over MCP, 4 LOWs, the PROCESS finding) — **implemented `216ca45`**. Report:
  `docs/audit-2026-09-11-mcp-tool-contracts.md`; board narrative **archive Part 65**.
  **`SEC-2026-09-11b`** (review of that implementation) — **archive Part 66**; its one open item is
  the UNC ship-blocker in the queue above.
- **Both parts carry a verified-CLEAN register — READ THEM BEFORE RE-AUDITING THIS GROUND**, together
  with each register's explicit "NOT checked, so this does not over-claim" list (in Part 65 that
  list is `merge_branch`/`rebase_branch` `operationInProgress`, merge autostash +
  `stashPopConflicts`, `create_stash` claims, stash apply/pop outcome tags, `unstage` atomicity, the
  `commit` `hookRejected` vs `configMissing` mapping, `rebase_skip`, `list_repos`/`select_repo`).
- **Pre-existing, deliberately out of scope:** the **webview** path —
  `src-tauri/src/commands/staging.rs:19-23` still passes frontend paths straight to `stage_paths`,
  keeping `git add -f` semantics with **no status-membership check**; the *escape* half is closed for
  that caller too. Also: `ensure_within_workdir` treats `.git` as inside the boundary.

### SEC-2026-09-03 — external-launch residue (remediated `0806596`)

Full narrative: archive Part 56. Report: `docs/audit-2026-09-03-external-launch.md` (`7e426c3`).

- **MEDIUM-2 + LOW-1 are `partially closed`** by `dc295c5`'s shape validation — the auditor showed it
  does **not** achieve the claimed property (three surviving routes, second-round ruling #21).
  **P112 is the real closure.**
- **INFO (CSP `form-action` / `base-uri` / `object-src`) — FIXED `8dd5b24`**, native half confirmed by
  the user 2026-09-10.
- **One residual, documented not closed:** a symlink introduced inside an already-checked-out
  superproject at a not-yet-created leaf bypasses the canonicalize recheck. Primary vectors are closed
  lexically regardless of filesystem state.
- **Test gap, partially closed — CONTRADICTION still open.** `c218258` added the file-target case and
  `151232d` covered UNC end to end; whether the set is now adequate is **unverified**.

### Residue of the archived `Queued housekeeping` section (archive Part 57)

The `src/styles/forge-pr.css` split is **DONE** (`e149382`, five modules, emitted stylesheet proven
byte-identical) and `.css` is now in `scripts/check-file-size.mjs` `SCAN_TARGETS`. Still open:

- Three items found during that split and deliberately NOT fixed (each would reorder the cascade or
  cross into another file): `context-menu.css:89` now points at a rule that lives in
  `forge-account.css`; a duplicate `.pr-create-actions` rule in `forge-pr-create.css`; and the
  generic `.btn-secondary-danger` sitting in `forge-pr-create.css` where it belongs with
  `controls.css`.
- **`image_diff_cli_2.rs`** numbered split still owed — renaming changes nextest IDs, so it needs its
  own increment where that IS the expected diff. Path re-verified 2026-09-03:
  `crates/bonsai-core/tests/diff/image_diff_cli_2.rs`.
- **The 90-char branch-name chip** becomes a 50 px two-line stadium at `border-radius: 999px` —
  pre-existing, newly visible because P102/P105 added the fixture that reaches it.
- **`ui-reference.md` is growing fast** (§2 now carries a 16-row evidence table) — worth its own
  curation pass.
- **Two velocity items filed and deliberately NOT taken** (2026-09-03): C1 could drop 17s → 11s by
  giving one surface its own test and its own corrupted repo — **not taken**, it changes the shape of
  a crash-safety test for ~6s; and `crates/bonsai-mcp/tests/common/mod.rs:131-133` still spawns 3
  `git config` calls (board said `:127`; re-measured 2026-09-03 — same fix applies verbatim; left
  alone to keep the blast radius in one crate).
- **The gate script emits Node `DEP0190`** — it passed args to a child with `shell: true`, which
  concatenates rather than escapes. **CONTRADICTION flagged 2026-09-10:** `833f2f9`'s commit message
  is "the gate stops concatenating argv", so this looks already closed. Not closed by the curator.

### P110 / P111 residue (milestones done; archive Parts 55 and 59.1)

- **P110 — latent pre-existing gap, NOT introduced by P110 (candidate follow-up):** op-state files
  (`MERGE_HEAD`, `REBASE_HEAD`, `rebase-merge/**`, `CHERRY_PICK_HEAD`) are excluded by the watcher
  filter and never triggered a refresh before or after P110. Op-state freshness during a conflicted
  rebase rides on incidental worktree churn. Deliberately left alone — changing it would widen which
  bursts fire.
- **P110 — deliberately NOT done:** the canvas selected-row highlight is not sticky. The highlight is
  row-index-based and there is no honest row to draw while the row is absent from the partial layout;
  anchoring it to a stale index is exactly the wrong-commit hazard the fix removes.
- **P111 — filed not fixed:** `.asset-chip` got the R1 line guard but no R2 `max-width`, so a model
  id far longer than the fixture would widen the chip rather than ellipsize.

### From the 2026-09-02 file-size refactor pass (archive Part 36; pre-condensation text 69.2)

Ratchet baseline moved **27 offenders / 6241 excess → 20 / 3528**; full gate green 8/8, 603s.

- **Fold-pill cursor is dead** in `GraphCanvas.handleMouseMove` — `src/graph/GraphCanvas.tsx:535`
  writes `foldCursorFor(...)` and `:551` unconditionally overwrites it with
  `next?.kind === 'overflow' ? 'pointer' : ''`, no guard between. Real regression; no vitest mounts
  `GraphCanvas`, so e2e is the only net. Re-verified 2026-09-03.
- **Shortcuts stay live during confirm dialogs**, and the old candidate-fix citation (`1d9d9bf`) was
  wrong. `anyDialogArmed` (`repoWorkspace/useWorkspaceDialogState.ts:265-300`) enumerates ~35 flags
  but **not** `pendingForcePush` (`:208`), `pendingCommitPush` (`:205`) or `abortConfirmOpen`
  (`:178`); `pendingBisectBad` lives outside the hook at `RepoWorkspace.tsx:299`. **Four flags.**
- **Contract divergences the tests document as bugs-in-the-contract:** rebase §3.1.5/§9.7
  unstaged-changes precondition; the libgit2-vs-CLI rename/delete conflict index-entry count; a
  near-tautological `expected_presence` oracle.
- **Duplicated helpers left visible, not merged** (behavior risk, not a move): atomic-write helpers in
  `assets/bundle/write.rs` + `assets/profiles/store.rs`; test helper families in `tests/diff/` and the
  four `tests/rebase_merge/*_support.rs`.
- **Three files deliberately stopped short of 500** (each further cut forwards 15-100 values to one
  consumer): `RepoWorkspace.tsx` **2264**, `src/graph/GraphCanvas.tsx` **784**, `src/App.tsx` **590**.

### Velocity follow-ups from the 2026-09-01 pass (`docs/history/velocity-2026-09-01.md`)

- **`submodule_cli::oracle_add_deinit_remove_roundtrip` 12-14s** — not a proptest (a git-CLI oracle
  roundtrip), so banding does not apply; needs its own look if the ~12s floor matters.
- **The vitest-environment item is DONE** (happy-dom, `1953c0a`). **Still owed from the same ruling
  #8: make `src/ipc/mock/repoState.ts:160` lazy** — it calls `new URLSearchParams(
  window.location.search)` at **module init**, the single root cause keeping 19 otherwise-DOM-free
  files in the DOM project. ~3 lines, worth ~70 s of CPU. **Whether `1953c0a` included it is
  unverified.**
- **`pnpm gate --quick` is 305s and only drops e2e** — not a fast tier; `cargo nextest --workspace`
  alone is 181s of it. Add a genuinely narrow tier or lean on `--rust` / `--frontend`.

### Hoisted off milestones archived 2026-09-01

- **keyring 3 → 4** needs a dedicated increment: 4.x moves onto `keyring-core`, renames every
  per-backend feature, drops `crypto-rust`, requires explicit credential-store registration — real
  changes to `crates/bonsai-forge/src/auth.rs`. (DEP REFRESH, archive Part 24.)
- ~~`no_proxy_client()` `.expect(...)`~~ — **CLOSED 2026-09-03:** it survives at
  `src-tauri/src/mcp/http_support.rs:221`, but the module is `#[cfg(test)]` (`mcp.rs:410-411`), so it
  never reaches a shipped binary. Detail: archive Part 52.5.
- **RepoWorkspace refactor** still stands for maintainability (not perf); P88's audit re-confirmed it.
- **P90.1 deferred:** per-check timing fields; header commit-summary text; command-palette
  `Refresh checks` / `Show checks`; mock fixtures for noForge/error reachable by click.
- **Known flake, untouched:** `watcher::tests::git_internals_filtered` (`src-tauri/src/watcher/
  tests.rs:127`); the flaky assertion at `:144` is `rx.recv_timeout(1500ms).unwrap_err() ==
  RecvTimeoutError::Timeout` — an `unwrap_err` on a **channel-recv Result**, not "on an `Instant`".
  The test now defends that negative window as sound after `watch_into_channel`'s sentinel sync
  (`29e72a7`), so whether it still flakes is **undetermined** (not re-run since 2026-09-03).
- **FLAG FOR USER (peer session, ended):** `repoWorkspace/useWorkspaceKeyboard.test.tsx` failed in
  ISOLATION on the committed baseline (1 graph-nav `defaultPrevented` case), introduced by the peer's
  graph-a11y commit `590f2ef`. Likely test-isolation flakiness. **Unverified since 2026-08-23.**

### Known load-flakes and test-budget fragility (full narrative: archive Part 68)

- **The 5-second default test budget is the real fragility, and the reporting mechanism reframes
  every "timeout" here.** vitest 4's `withTimeout` checks wall clock **on completion**
  (`@vitest/runner` `chunk-artifact.js:2288-2294`): a test that passed every assertion is still
  rejected with "Test timed out in 5000ms" once `performance.now() - startTime` crosses the budget —
  **proved with a purely synchronous 6000 ms busy-wait**. So **"timed out" does NOT imply a pending
  async chain.** In the gate's own condition (rust tier → vitest), **tail inflation is 1.3-2.8×** and
  **three tests already cross 5000 ms**, green only on explicit `20_000`/`30_000` budgets. Most
  exposed default-budget tests: `App.test.tsx` "Arrow-key pane nudge" (2394 ms), `Sidebar.churn`
  (2110 ms).
- **happy-dom's causal role in the two failed gate runs was NOT established** (2 failing runs vs 1
  passing jsdom run; all 112 tests in the five affected files pass under **both** environments).
  Correlation over three gate runs, not a demonstrated cause — `8026622` corrected the board's own
  overclaim. Two **real** defects were found and fixed (`9422e8b`): a click on a present-but-
  **disabled** checkbox (`SettingsGitConfigSection.test.tsx:337`, the only component in the repo with
  that inert-but-visible design) and a 1 s poll on a **microtask-only** boundary
  (`Sidebar.test.tsx:184`). Three of the five were deliberately **not** changed and their timeouts
  deliberately **not** raised.
- **`h_ai` is genuinely flaky in PARALLEL — a real defect, not a timing artifact.** **0 of 57 fail
  with `--test-threads=1`** (57 passed, 130s); under default threading it fails or stalls, because
  **57 tests concurrently spawn the `claude_stub.cmd` harness on Windows** — presenting as **stalls**
  (the 5 `ai_stream_bulk_cli` tests, 18.6s serially) **or cross-talk** (e.g. `ai_explain` receiving
  another test's `createBranch` stub body). Pre-existing; not from the 2026-09-11 security work.
  **Follow-up: isolate the AI stub per test.** Until then run `h_ai` with `--test-threads=1`.
  **A flake you cannot see the summary for is indistinguishable from a regression.**
- **`rust-lld: failed to write output … permission denied` on a stale `.exe`** hit an `h_ai` link
  twice on 2026-09-11; deleting the file fixed it. A lock/AV artifact, likely caused by force-killing
  cargo mid-link (see the serialize-cargo rule).
- `ai::session_tests::watchdog_tests::watchdog_does_not_fire_while_awaiting_input` — failed once under
  load, passed on re-run. The clock seam it needed **landed in `734b310`**
  (`crates/bonsai-core/src/ai/clock.rs`; `session_watchdog_tests.rs:41/67/115` drive `TestClock`), so
  re-verify before treating this as live.
- `src/App.test.tsx > App shell > an Arrow-key pane nudge persists the POST-nudge width` — failed once
  at 2662ms (`setUiSettings` never called), then 4/4 isolated and 2644/2644 on a full re-run.

### Residue of the two dated 2026-08-22 design reviews (archive Part 35)

- **`graph-design-review-2026-08-22.md` M1 is SUPERSEDED — do not implement.** Facts corrected
  2026-09-03; the verdict is unchanged but the citation was wrong twice over. `role="grid"`,
  `aria-rowcount`, `role="row"` and `aria-rowindex` are forbidden by `ui-reference.md` §4.1 — now at
  `:860-864`, **not** `:250-252` (§4.1's header is `:848`; `:250-252` is unrelated text today).
  **`aria-activedescendant` is NOT forbidden** — `ui-reference.md:865` says it "is kept and is
  valid" (amended 2026-09-02, spec-004 merge), so the board was directing sessions to remove a
  shipped, sanctioned attribute.
- **M2/M3/M4/S2/S3/N1/N2 — resolution unverified.** Not checked by the 2026-09-01 sweep; do not
  assume they landed.
- **`review-2026-08-22-ui.md` NIT-1 — Sidebar ignores `panelDensity`** (re-confirmed still open
  2026-09-03: `src/styles/sidebar.css:92` is `.branch-row { height: 24px; }`, a hard literal, and no
  file under `src/components/` that reads `panelDensity` is a sidebar file).
- **NIT-2 —** `src/components/OnboardingOverlay.tsx:229` is still `aria-label="Close"` (re-verified
  2026-09-03, line number still exact); the review preferred "Close the tour".
- SHOULD-3 (`--accent` text over `--selection` fails AA) is the **same item** as P69 **A9** below —
  A9 is the canonical entry.

### P80 forge follow-ups (SHOULD-FIX/NIT, non-blocking)

- (a) `forge_set_token_inner` validates before the `host.is_empty()` guard — guard host first.
  Still open, re-verified 2026-09-03: `src-tauri/src/commands/forge.rs:290` calls
  `validate_repo_token`, `:291` is the `host.is_empty()` guard.
- (b) keychain-write-then-settings ordering: a failed `settings::update` leaves an orphaned keychain
  token (currently `let _ =`) — surface the error.
- (c) re-connecting a migrated legacy `login:None` host creates a 2nd three-part account + orphans the
  bare-host keychain entry (contract §1.2 rekey, optional).
- (e) `ContextMenu` has no separator concept, so the switcher's account/command rows run contiguous.
- (f) Settings Accounts group ordering is alphabetical only (no repoId in scope for "current host
  first").
- (g) disabled Default radio's `aria-describedby` points at a `hidden` span — use a visually-hidden
  class.
- (h) switcher trigger has no busy affordance during a pin/reset write. (i) §1.1 wireframe middot
  between host and caption omitted (cosmetic).

### `cargo fmt` has never been run on this repo

- No `rustfmt.toml` anywhere, no fmt check in any hook or CI (re-verified 2026-09-03: zero
  `rustfmt.toml` in the tree).
- `cargo fmt --all --check` reports **1773 hunks across 221 files**; `--config
  use_small_heuristics=Max` is *worse* (2065). **These two numbers were NOT re-measured in the
  2026-09-03 staleness sweep** (cargo is not on the default PATH and running it is out of that
  sweep's scope) — treat them as of their original measurement date, not as current.
- Right shape: its own commit — pick a config, add `rustfmt.toml`, one-shot reformat, then add
  `cargo fmt --check` to the gate. **Do it between milestones, never inside one.**

### Audit #2 remainder (full audit `docs/audit-2026-08-18.md`; fix-batch mapping archive Part 16)

- **Sections 4.3-4.8 test gaps** — CommandPalette/NumberSlider pins, streaming-graph e2e, 08-stash
  conflicted-apply fixture, Linux case-sensitivity assertions, low-value untested units, and the
  missing journeys: updater / AI-PR-description / clone-init / worktrees.
- **Section 7's 13 NITs** — recorded in the audit, no action required.
- **Section 5.6** perf/visual ACs stay USER CHECKPOINT (the headless harness cannot observe
  rAF/compositing).

### P68 contract debt (P68 is done; its contracts are stale/oversized)

- `docs/contracts/P68e-ai-activity-dock.md` is **1123 lines** (re-measured 2026-09-03; the board and
  `docs/contracts/INDEX.md` both said 1064, 59 lines stale) and under-describes shipped code
  (P68g-1 added an untrusted-model-output attribution line, a fixed "Bonsai never asks for passwords
  or tokens" guard, and a two-id `aria-describedby`). Splice-ready replacements are in
  `docs/contracts/P68g-ui.md` §3.1-§3.5. **Needs: apply the splice, then split the file.**
- `docs/contracts/P68-ai-conflict-streaming.md:304` is one module level stale (`session_drain_tests.rs`
  is now a child of `session::session_drain`). **Invariants D1-D16 remain canonical — do NOT "fix"
  them back.**
- **P68 security follow-ups 7-11 still OPEN**; rationale in `docs/contracts/P68-security-audit.md`
  (canonical): the novel-content gate (structural defeat for H1), proposals shown as a diff, bulk
  path-count cap + per-batch reads + batch count in the dialog, process-group kill off Windows (the
  pid-zeroing half landed in `67539fd`), and a symlink-safe `resolve_conflict_text` write.
### P69 Settings follow-ups — A3 ✅ SIGNED 2026-09-11; A8/A9 still backlog

> **A3:** the user handed the gate-note copy to `ui-designer` to finalise with the surrounding copy
> in view (ruling #10); whatever it signed ships — and it signed the **shipped** string. **A8/A9 were
> deliberately NOT put to the user** — they are backlog, not blocked on a decision.


- **A8 — bundle the two specced-but-unimplemented items into one increment** (both `ui-designer` and
  the orchestrator recommend bundling): (a) the help-text highlight fallback
  (`docs/contracts/archive/P69-settings-ui.md` §3.2.1) — the flagship query `graph` returns 5 hits and
  highlights **nothing**; and (b) the half-landed draft-hint feature (§13). The draft-hint CSS is
  genuinely dead but costs no visible layout today.
- **A9 — a scoped a11y sweep of `color: var(--accent)` on text over `--selection`** (measured
  3.51-3.74:1). Now **prohibited** in `ui-reference.md` §2 so new code cannot add to the backlog. The
  one deviation P69k shipped: the rail hit-count is `--text-1`; the exact declaration to flip is
  marked in `settings-shell.css`.
- **A3 — ✅ SIGNED 2026-09-11, no code change owed to the copy.** `ui-designer` withdrew its own
  preferred reword (`These take effect once AI features are on.`) after finding the shipped string
  is a **pattern, not a string**: `SettingsAiSection.tsx:110` is byte-identical modulo `this`/`these`
  and `SettingsDevCaptureSection.tsx:52` is a third instance. The shipped
  `Turn on "Enable AI features" above to change these.` stays, signed into `ui-reference.md` §12.12 +
  `P68g-ui.md`. The only owed work is clearing two stale "pending A3 sign-off" comments — see
  RESUME HERE, `in-progress`.

### P77 tag-sync deferred follow-ups (full detail: `docs/history/todo-archive-2026-08.md` Part 18)

- **Collapsed-rollup needs first expand — ✅ RULED 2026-09-11: fold the check into the existing
  auto-fetch cycle** (enabled, 5-min interval). No repo-open network call and no new trigger; the
  rollup warning appears within one cycle. The old text: the ls-remote check only fires on the first
  Tags expand per session, so the rollup warning cannot appear until the user expands Tags once.
- NITs: rollup aria-label lacks singular/plural ("1 tags") · `useTagSync` re-hits network on rapid
  collapse-then-expand while `unavailable` · confirm dialogs close optimistically so `busy` never
  paints · the tag-filter box gate counts local tags only · item-7 "Delete tag on origin" also shows
  on remote-only ghost rows (coherent).
- Backend NITs: `delete_remote_tag` doesn't `evict_fresh_on_auth_fail` · `validate_tag_name` is
  duplicated from `tags.rs` — promote to shared if a 3rd caller appears.

### macOS ad-hoc code signing — ⏸ PARKED 2026-09-11 (user): blocked-on-release, NOT open work

- `bundle.macOS.signingIdentity: "-"` is in `src-tauri/tauri.conf.json` but **has not shipped**: the
  last tag is `v1.5.0` (2026-08-26), which predates the fix. Verify the sealed ad-hoc signature on
  the next tagged release.
- Not fixed by ad-hoc at all: Gatekeeper "unidentified developer"; a new version re-prompts once for
  TCC (cdhash changes). Full fix = Developer ID + notarization; the `APPLE_*` env block in
  `.github/workflows/release.yml` is already scaffolded. Full detail: archive Part 34.


---

## Archive

**Start at `docs/history/README.md`** — it is the navigable index of every archived milestone and
part number. The table below is the short form.

| File | Covers |
|---|---|
| `docs/history/README.md` | **The archive index** — which file/part holds which milestone. |
| `docs/history/todo-archive-2026-09.md` | **Parts 62-70 (moved 2026-09-14, after the user ruled all 22 FOR-USER items on 2026-09-11):** the stale 2026-09-10 resume block + FU-1 residue (62) · the FOR-USER evidence blocks for items 0-6 (63) · the `IN FLIGHT` queue, the 2026-09-11 orchestrator closures, the unreviewed-MCP-merge warning (64) · **`SEC-2026-09-11`**, the MCP tool-contract audit, with its verified-CLEAN register (65) · **`SEC-2026-09-11b`**, the review of that implementation, with its verified-CLEAN register (66) · P108 `AC11`, closed by ruling #13 (67) · the happy-dom load-flake narrative (68) · the open follow-ups as they stood pre-condensation (69.1 P91 · 69.2 SEC-2026-09-03 through the 2026-09-01 hoisted items · 69.3 P69 Settings) · superseded curator bookkeeping (70). **Parts 54-61 (moved 2026-09-10):** the whole USER-CHECKPOINT block — P102+P105, P106, P107, P108, P91 (54) · P110 + P109 (55) · the 2026-09-03 closures + SEC-2026-09-03 remediation (56) · `Queued housekeeping` incl. the `e149382` CSS-split proof (57) · the superseded `c218258` and earlier gate states (58) · the 2026-09-10 session: P111, FU-1, six reviewer-follow-up closures (59) · the board's record of the confirmation (60) · superseded curator bookkeeping (61). **Parts 51-53 (2026-09-03):** the `5c2dcd2` + `c6cd7dd` gate states and the e2e-contention mis-diagnosis · the full narratives of everything closed 2026-09-03 · the durable-lessons stories and worked numbers. **Parts 36-50 (2026-09-03):** the file-size refactor pass · P102+P105, P106, P107, P108 and the P91 security arc + audit + build diary · superseded pre-ship filings · the 2026-09-03 velocity pass · P99, P100, P101, P98, P95, P96, P97 · built-bundle e2e + P103 + P104 · the DX/velocity stubs · the pre-condensation open-follow-up text. **Parts 33-35 (2026-09-01):** the P84 record gap · macOS ad-hoc signing · the two 2026-08-22 design reviews. **Parts 22-32 (2026-09-01):** P94 · P93+P92 · DEP REFRESH · P90+P89 · P88 · the P85-P87 batch · P82+P83 · divergence reconcile + Release 1.1.0 · the DX dev-loop text · the confirmed-checkpoints block · the 2026-08-21 resolved follow-ups. |
| `docs/history/todo-archive-2026-08.md` | Parts 1-9: P65 to P28 build detail, the Phase 1-4 banners, resolved FOR-USER decisions, P69(1.0.0)/P67/P68 detail. Parts 10-16: the P62-P74 checkpoint waiver + P71-P74, the P69 Settings redesign, the Audit #2 fix batch. Parts 17-18: P70 and P77. Part 19: the follow-ups resolved 2026-08-21, verbatim. Part 20: P78/P79/P80. Part 21: P80b/P81/P82. |
| `docs/history/todo-archive.md` | P27 to P2, M0-M6 |
| `docs/history/milestones-mvp.md` | the M0-M6 AI-gate vs USER CHECKPOINT split |
| `docs/history/context-pollution-audit.md` | the context/token-cost audit |
| `docs/history/velocity-2026-09-01.md` | gate wall-clock, test-suite hotspots, inner-loop rebuild cost, ceremony-vs-machine-time split (2026-09-01) |
| `docs/contracts/INDEX.md` | one line per contract file — milestone, scope, status |

Move a milestone's section into the current dated archive file only once **both** halves of its gate
have passed (or the native half is explicitly waived). A milestone with a pending USER CHECKPOINT
stays on this board. **An owed AI-gate item also keeps its entry here** — P91's `logs/*.jsonl` parse
is the live example (P108's `AC11`, the other one, was closed by user ruling on 2026-09-11).

### Why this board is ~988 lines, not ~300 (curator note, 2026-09-14)

**1537 → 988**, the largest single archiving pass this board has had (**1215 lines extracted
verbatim** into Parts 62-70 — every range diffed byte-identical against the pre-pass file before
removal). It did **not** reach ~300, and the reason is worth stating plainly rather than
re-discovering next pass.

Residual composition, measured after this pass: header + conventions + navigation **60** ·
RESUME HERE incl. the four user actions **75** · the two ruling blocks, kept **verbatim and
authoritative** **147** · the ruling queue **61** · durable-lesson rules **130** · accepted decisions
**114** · open follow-ups **351** · archive + this note **45**.

**What sets the floor — three things, none of them curatable.** (1) The **two ruling blocks**: the
user's own record of 22 decisions, which may be moved but not shortened. (2) The **durable-lesson
rules**: operational instruction every future session depends on, kept on the board deliberately
because each one was learned by a claim that was green the whole time it was wrong. (3) The **open
follow-up backlog** — 351 lines carrying the `file:line` citations a cold resume needs most.
Everything resolved is already a pointer into `docs/history/`.

**So ~300 is reachable only by working the backlog down, not by curating** — P112, F6, P77 and the
`h_ai` stub isolation are the four largest items in it. The one structural option that would move
the number without deleting open work is to give the durable rules their own file under `docs/` and
leave a pointer here; that trades ~130 board lines for one more hop on every session's most
load-bearing content, so it is a **user/orchestrator call, not a curator one.**
