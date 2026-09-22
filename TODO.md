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
closed by the curator: a pending USER CHECKPOINT, an owed AI-gate item and an open follow-up all stay
here however old they are.

**2026-09-22 pass — Parts 76-82. P112's native USER CHECKPOINT was confirmed and verified by the
user on 2026-09-22 (`ba4b9d3`), which is what finally made it and everything downstream archivable.**
Archived: **P112 in full** — its milestone entry, AI gate, the five checkpoint items as confirmed
(incl. item 4's `set_parent` sub-question) and the 41-file process-failure narrative (**76**) · the
completed 2026-09-14/15 queue, the superseded `5654eaa`/`934a280` gate states, ruling #24's scope
facts and the second-round rulings' evidence blocks, and the whole 2026-09-11 ruling queue (**77**) ·
the 2026-09-16 session — the real-log investigation, the nine-file second review pass and its
closure, the Settings-scrim finding, **P113 phases 1+2** (**78**) · the 2026-09-17 session — the
orphaned-credential arc, **P114**, contract hygiene, the rustfmt pass, three security audits and the
superseded `ea6d323`/`5f015be`/`3948478` greens (**79**) · the release block's pre-tightening text
(**80**) · P91's closed `logs/*.jsonl` parse, the `mono` defect it found (fixed `88a4004`) and the
superseded `cargo fmt` section (**81**) · superseded curator bookkeeping (**82**).

**Not archived, deliberately:** the live release block, the **two** remaining USER ACTIONS — the
macOS ad-hoc signature, verify-on-tag (ruling #17) and the Dependabot moderate (ruling #15, which
the release block argues a merge to `main` should clear) — **every open follow-up**, all four ruling ledgers, the durable rules, the
accepted decisions, the cross-platform-gap finding, both security CLEAN registers' pointers, and
**A4 finding 6** — a diagnostic regression with no commit that closes it.

**Order of operations, mandatory since `c5b3ea5`.** That pass cut 1950 lines from this file and wrote
them **nowhere**; `67e2ce6` had to restore them wholesale. **Extract → diff byte-identical against
`git show HEAD:TODO.md` → only then remove → leave a Part pointer.** 2026-09-22: **17 ranges, 2425
lines, all 17 verified byte-identical inside `todo-archive-2026-09.md` after the append**, plus a
whole-file check that every non-blank line of `git show HEAD:TODO.md` still exists in the board or
the archive. 2026-09-16: 20 ranges, 935 lines, all 20 verified.

**Earlier passes.** 2026-09-16 → Parts 71-75; 2026-09-14 → Parts 62-70 (the 22 FOR-USER rulings);
2026-09-10 → Parts 54-61 (the eight confirmed native checkpoints); 2026-09-03 → Parts 36-53, plus a
staleness sweep that found **11 of 35 open entries had drifted**; 2026-09-01 → Parts 22-35. The
board's own record of being wrong is kept deliberately.

---

## 🚀 RELEASE v1.6.0 — PREPARED 2026-09-18, **NOT PUBLISHED**

**Current step: v1.6.0 IS CLEAR TO PUBLISH — zero blockers.** Prep done and verified at the ≈CI
tier, the installer bundles, the branch is pushed (branch only, 2026-09-18), and on **2026-09-22 the
user confirmed the three remaining blockers done and verified**: the P112 native checkpoint, the
`updater-prod.key` backup, and the signing secrets. No agent task remains.

**Two things stay RECOMMENDED rather than required**, and neither is a blocker:
1. A manual **`workflow_dispatch` CI run on this branch** — still the only ubuntu/macOS verification
   that exists. **Actions → CI → Run workflow → branch `feat/post-p91-rulings`.** `.github/workflows/ci.yml`
   triggers on `push`/`pull_request` to `main` **and on `workflow_dispatch: {}`**, so it needs no PR:
   it runs `rust` on **ubuntu-22.04 + windows-latest + macos-latest**, `frontend` on two OSes, plus
   `e2e` and `audit`. It closes the whole cross-platform gap below and it would finally *execute* the
   **AMEND-8 host-bound test fix, which is still reasoned, not executed** (the unix accept chain was
   traced line by line, never run). **Highest-value action still available, and it costs one click.**
2. **Route the release through `main`.** `release.yml` is `workflow_dispatch` and tags `context.sha`,
   so Release is technically dispatchable from this branch today — but `main` would otherwise lag its
   own release, and the Dependabot alert sits on the **default branch**, so only a merge clears it.
   Note `release.yml` **only builds — it never tests**, which is why the CI run above, not the
   Release run, is what verifies ubuntu and macOS.

Pre-tightening verbatim text of this whole block: **archive Part 80** (269 lines → this form; every
number, SHA, path and caveat below was carried forward).

### 🔓 PUSHED 2026-09-18 — branch only, by explicit user choice. Ruling #25 SUPERSEDED for this branch

- Asked with three options laid out (PR to `main` / branch only / hold); the user chose **"Push
  branch only"**. `feat/post-p91-rulings` is on `origin` at **`b80dd36`**, tracking set.
- **No PR, no tag, nothing published** — `git ls-remote --tags origin` still ends at `v1.5.0`.
- **Ruling #25 ("DO NOT PUSH … do not raise this again") is superseded for this branch by that
  choice.** Recorded so no later session re-applies it: the ruling's *history* stands, its
  *instruction* does not. The branch now also exists off this machine, which it did not before.

### 🔎 RULING #15's DEPENDABOT MODERATE IS ALL BUT IDENTIFIED — and this branch already fixes it

- The push itself answered it. `git push` returned: *"GitHub found 1 vulnerability on
  danpercic86/bonsai's **default branch** (1 moderate)."*
- It is on the **default branch**, not on this one.
- `git show origin/main:Cargo.lock` carries **rustls 0.23.43** — precisely the version
  RUSTSEC-2026-0285 names. This branch carries **0.23.45**.
- The npm candidate the board always named is **ruled out**: `nanoid` is at the fixed **3.3.18** on
  `origin/main` as well as here. That also explains the count being **1**, not the one-high-plus-one-
  moderate the board used to record.
- **The one alternative NOT closed:** `git diff --stat origin/main HEAD -- pnpm-lock.yaml` is
  **94 insertions / 13 deletions**, so the npm trees do differ and an npm advisory unique to `main`'s
  lockfile cannot be excluded from here. What can be said: **our** npm tree is clean at `low` with
  zero suppressions, and `main` carries a **confirmed** Rust advisory this branch fixes.
  **Merging into `main` should clear the alert — the observation after the merge is the
  confirmation, not this reasoning.**

### What the prep changed

- **1.5.0 → 1.6.0** in the four places that carry it, found with `git grep` rather than from memory:
  `package.json:4`, `src-tauri/tauri.conf.json:4`, `src-tauri/Cargo.toml:3`, `README.md:19-20`
  (the "Status: shipping" line). `Cargo.lock` refreshed with `cargo check -p bonsai`.
- **The `1.5.0` strings in `src-tauri/src/obs/tests_*.rs` and `watcher/tests.rs` were deliberately
  NOT touched** — `obs/mod.rs:216` takes `app_version` from `app.package_info().version` at runtime,
  so those are arbitrary fixture values, not the shipped version.
- **`CHANGELOG.md`: `[Unreleased]` cut to `[1.6.0] — 2026-09-18`**, an empty `[Unreleased]` kept
  above it. Curated over the real range **`v1.5.0..HEAD`** — 315 non-merge commits, 74 with
  `feat|fix|perf|security` subjects — *not* over `origin/dev..HEAD`, which would have mis-scoped it.

### 🔐 TWO SUPPLY-CHAIN FAILURES THIS BRANCH HAD NEVER SEEN — both fixed (`230113a`)

`cargo-deny` lives in the **`audit` group, which runs only under `--full`/`--ci`**. The bare 9-step
`pnpm gate` the board ran for weeks **does not include it**, so neither had ever run against this
branch and **both would have failed CI's `audit` job on the first push:**

1. **`RUSTSEC-2026-0285` — `rustls 0.23.43`.** TLS 1.3 handshake messages accepted across encryption-
   level boundaries (functionally Go's CVE-2025-61730). Reached via **`bonsai-forge`** (the HTTPS
   client that carries forge tokens) **and via `tauri-plugin-updater`** (the auto-update download
   path). Fixed by `cargo update -p rustls` → **0.23.45**, the advisory's stated minimum.
2. **`libssh2-sys 0.3.2` was YANKED.** Fixed by `cargo update -p libssh2-sys` → **0.3.3**. This is
   git2's SSH transport, so it ships in the app — not tooling.

- `cargo deny --all-features check` now reports **`advisories ok, bans ok, licenses ok, sources ok`**.
  The two surviving `license-not-encountered` warnings are an over-broad allowlist in `deny.toml`
  (`BSD-2-Clause`, `CDLA-Permissive-2.0`) — not findings.
- **The lesson is the board's own rule earning itself again: a green bare gate is not a green CI.** A
  verification block must say which **tier** it means, and the audit tier has to run before a release.

### ✅ AI GATE — `pnpm gate --full` (≈CI tier), **ALL 11 STEPS GREEN**

- **510.6s, exit 0, zero FAIL lines.** Log: `D:/Data/Temp/claude/bonsai-gate/gate-v160-final.log`.
- nextest **2605 run / 2605 passed / 11 skipped** (206.1s, **0 LEAK lines** — the known intermittent
  `external_spawn::detached_spawn_ignores_nonzero_exit` did not reproduce) · doctests 3.1s ·
  `cargo fmt --check` 1.5s · clippy 44.9s · eslint 9.5s (**36 warnings**, ceiling 50) · size ratchet
  581ms · vitest **3019 passed / 275 files** (65.7s, coverage mode) · tsc+build 9.2s · e2e
  **185 passed** (166.4s) · **cargo-deny 2.8s** · **pnpm audit 723ms**.
- Against the `5654eaa` green: Rust **2605 vs 2563 (+42)**, vitest **3019 vs 3007 (+12)**, e2e
  **185, unchanged**.
- **What that green covers — measured, not asserted (this wording is the corrected one).** The first
  draft claimed the run was against the finished prep tree, and the changelog cut had not landed when
  the gate launched. `git diff --name-only 230113a HEAD` returns **nine `.md` files plus
  `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`** — and those three carried
  1.6.0 **before both gate runs**, which the log proves on its own by printing `Compiling bonsai
  v1.6.0`. `Cargo.lock` with both dependency fixes was likewise in place, proven by cargo-deny
  passing clean in the same run. **No gate step reads a file that differs between that run and HEAD.**
  The prose commits (`51d8b48`, `80a91d0`) are markdown only.
- **Plus the one check no gate tier runs: `cargo build --release -p bonsai` → `Finished release
  profile [optimized] in 4m 19s`**, producing `target/release/bonsai.exe` (32,152,064 bytes). The
  gate compiles the **dev** profile only, so this is the first proof the *shipped* binary compiles
  under the release profile at all.
- **It is NOT evidence that the `pub(crate)`-widened `#[cfg(test)] testutil` stays out of the
  binary** — an earlier draft claimed that and it is unearned: `#[cfg(test)]` is off in dev and
  release alike, so a release build cannot distinguish the two.

### ✅ THE INSTALLER BUNDLES — the last unproven link in the release chain

- `pnpm tauri build --config '{"bundle":{"createUpdaterArtifacts":false}}'` → **exit 0**,
  `Finished 1 bundle`: **`target/release/bundle/nsis/Bonsai_1.6.0_x64-setup.exe`, 7,874,889 bytes**,
  release compile 2m35s. Log: `D:/Data/Temp/claude/bonsai-gate/tauri-build.log`.
- **Why the `--config` override:** `createUpdaterArtifacts: true` makes the bundler sign the updater
  payload, which needs `TAURI_SIGNING_PRIVATE_KEY` **and its password** — a secret the orchestrator
  must not handle.
- **What it therefore does NOT prove:** the **`.sig` / `latest.json` step**, which can only run where
  the key lives — the release workflow. That step is exactly what the `publish-release` guard checks
  for, so a failure there cannot produce a published half-release. Nothing about macOS or Linux
  bundling is proven here either.
- **Release notes for the draft body are extracted to
  `D:/Data/Temp/claude/bonsai-gate/release-notes-v1.6.0.md`** — 314 lines, 26.6 KB, well inside
  GitHub's 125 KB body limit. `release.yml` creates the draft with a **placeholder** body ("See the
  assets below to download and install this version."), so **unless that is replaced the published
  release carries no notes at all.** Editable in the GitHub UI while the release is a draft, which is
  why `release.yml` was left alone rather than taught to read `CHANGELOG.md` before its first use.

### Changelog and README polish applied after the curator's pass

- **Two internal-tooling sub-bullets removed** from the dependency-refresh entry: the
  `pnpm lint:ci --max-warnings 40 → 50` note — also **factually stale**, claiming 42 warnings where
  the gate measured **36** — and the "TypeScript 7 deliberately not adopted" note. Contributor
  toolchain policy belongs in `CONTRIBUTING.md`. The `reqwest`/OS-certificate-store and `keyring` 3.x
  notes were **kept**: both have real user-visible consequences.
- **`README.md` now names the `.rpm`.** The Install note covered `.AppImage` and `.deb` only, while
  `bundle.targets` ships an `.rpm` that `publish-release` **requires**.
- **Judgment calls decided and deliberately left as they are:** the P110 entry stays under **Fixed**
  (it reads as a regression fix and its copy was already approved); and **no "verification in
  progress" note was added** to the P112 picker entry, because the changelog describes what the code
  does while the native checkpoint is our process. That dependency was recorded in BLOCKS RELEASE #1
  instead — and it is now moot, the checkpoint having passed.

### ⚠ THE CROSS-PLATFORM GAP IS REAL AND CANNOT BE CLOSED ON THIS MACHINE

- I tried to close it and **failed** — recorded so nobody repeats the attempt the same way.
- `rustup target add aarch64-apple-darwin x86_64-unknown-linux-gnu` **succeeded**, and both
  `cargo clippy --target` steps then failed for a **missing C cross-compiler**: `cc` for darwin,
  `x86_64-linux-gnu-gcc` for linux, because `alloca` and `libz-sys` (→ libgit2) are **C** crates.
- **`gate.mjs:105`'s claim that "only the pure crates cross-compile cleanly from any host" is false
  for this workspace** — `bonsai-core` depends on `git2`, so it needs a per-target C toolchain too.
- Both targets were **removed again**, restoring `--full` to its designed behaviour (warn + defer to
  CI). **Three-platform verification therefore requires CI, which requires the `workflow_dispatch`
  run above.**
- **🆕 FILED, not fixed:** `scripts/gate.mjs:105`'s comment + the tier docs misdescribe this
  workspace, which is why `--full`/`--ci-parity` advertise a cross-target check they cannot deliver
  here. A comment and tier-doc fix, not a code change, and not release-blocking.

### Release machinery — verified by inspection, since no test covers it

- **The updater pubkey matches the signing key.** `tauri.conf.json`'s `plugins.updater.pubkey` is
  byte-identical to `.tauri/updater-prod.key.pub` (minisign key id **`B4E84ADA465319A8`**) and is
  **not** the dev key. A mismatch here makes every installed client reject the update as a bad
  signature. *(Method note: my first comparison decoded one side and not the other and reported a
  false mismatch — the `.pub` file already stores the base64 form. Compare like with like.)*
- **`bundle.targets`** (`nsis, app, dmg, deb, rpm, appimage`) plus `createUpdaterArtifacts: true`
  produce **exactly the 10 assets** `release.yml`'s `publish-release` guard demands. No gap.
- **Hygiene:** `.tauri/` and `dist*/` are gitignored **and untracked** — no key or build output in
  the tree. **`v1.6.0` does not exist as a tag**; `release.yml` derives it from `package.json`.
- **Unsigned on Windows** (`certificateThumbprint: null`), **ad-hoc on macOS**
  (`signingIdentity: "-"`) — the locked v1 decision (`docs/code-signing.md:3`), not a gap.
- **npm side: `pnpm audit --audit-level low` → no known vulnerabilities, and since 2026-09-18 that is
  with ZERO suppressions.** The "ignored `nanoid` high" the board carried was **stale**: both
  `origin/main` and this branch already carry **`nanoid@3.3.18`, the FIXED version** (the advisory is
  `<3.3.18`), so the `pnpm-workspace.yaml` allowlist was suppressing **nothing** while standing ready
  to hide the *next* nanoid advisory — and its own comment named the exact drop condition, *"once
  vite/vitest ship a postcss with nanoid >=3.3.18"*, already met. **Entry removed; `pnpm audit`
  re-run with no allowlist at `--audit-level low`: still clean.**

### 📐 Three board claims measured and found STALE

1. **Ruling #24's toast sweep is COMPLETE — not "10 of 15".** `grep "pushToast(" src/` returns
   **zero** call sites in `src/components/settings/`, `useMcpControls.ts` and `useUiSettings.ts`. The
   single survivor, **`useSettingsSaveFailure.ts:72`**, is **deliberate and documented** (§17.3):
   `if (!settingsOpen.current && streak === 0)` — banner when Settings is open, toast only when it is
   closed, the one place a toast is the correct channel.
2. **The owed `useExternalTools` re-check is CLOSED.** P112 sub-inc 4's picker did **not** make
   `useExternalTools.ts:22/28/34` Settings-reachable — the only importers are `App.tsx:189` and
   `RepoWorkspace.tsx:1693` (repo UI); the one `src/components/settings/` reference is a **test**.
3. **Commit counts, measured 2026-09-18:** `origin/dev..HEAD` = **106** (the board carried 92, then
   94); `origin/main..HEAD` = **313**; `v1.5.0..HEAD` = **318**.

### ✅ Nothing is stranded on another branch

`git rev-list --count HEAD..<ref>` over **every** local and remote branch returns **0 for all of
them** — HEAD is a strict superset of `main`, `dev`, `origin/*` and all 17 feature branches. So
releasing from `feat/post-p91-rulings` drops nothing, and `origin/main` is an ancestor **313** commits
back, i.e. **a PR to `main` fast-forwards**.

### ✅ BLOCKS RELEASE — **EMPTY.** All four cleared (#2 on 2026-09-18, #1/#3/#4 on 2026-09-22)

1. ~~**The P112 native USER CHECKPOINT**~~ — **CONFIRMED DONE AND VERIFIED BY THE USER, 2026-09-22**,
   covering all five items (now archive **Part 76.2**). This is the **user's attestation**, the only
   thing that can clear it: the orchestrator neither ran it nor could. **P112 is done in both halves.**
2. ~~**The code has to reach GitHub.**~~ **CLEARED 2026-09-18 — the branch is pushed (branch only,
   user's choice).** What remains is recommendation 2 above, not a blocker.
3. ~~**`.tauri/updater-prod.key` still exists in exactly ONE place**~~ — **CONFIRMED BACKED UP BY THE
   USER, 2026-09-22**, which closes ruling #14. It is no longer single-copy. No agent ever read or
   copied the key; the only thing verified from here was that its **public** half matches the shipped
   `tauri.conf.json` (`B4E84ADA465319A8`). **P71 must still not touch it.**
4. ~~**GitHub secrets `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` must be
   present**~~ — **CONFIRMED PRESENT BY THE USER, 2026-09-22.** Still unverifiable from here (no
   `gh`), so this stands as the user's attestation rather than a measurement. **If a Release run ever
   yields unsigned artifacts or clients reject an update, re-check this first.**

### Verify-on-tag, and what does NOT block

- **⏳ USER ACTION — ruling #17: macOS ad-hoc signing is "PARK as blocked-on-release. Re-raise when a
  tag is next cut."** Cutting `v1.6.0` **is** that trigger — verify the sealed ad-hoc signature on the
  produced `.app`/`.dmg`. Gatekeeper will still say "unidentified developer"; that needs Developer ID
  + notarization, scaffolded in `release.yml` but not wired. Detail: `### macOS ad-hoc code signing`.
- **Not blocking:** everything under `## OPEN follow-ups`; the 36 eslint warnings (**13**
  `react-refresh/only-export-components`, **6** `react-hooks/exhaustive-deps`, **1** `no-unused-vars`,
  **1** `no-explicit-any` — the last two in test/mock scaffolding, e.g. `src/test/setup.ts:21`, not
  shipped code); `cargo doc`'s 127 pre-existing findings.
- **Zero `todo!()`, `unimplemented!()` or `FIXME` in production Rust** (`src-tauri/src`,
  `crates/*/src`). The two production TS `TODO(...)` notes are a P60 sidebar-parity polish item and a
  mock-fixture note.

---

# ✅ P112 — **DONE, BOTH HALVES** (AI gate `9fca997`; USER CHECKPOINT confirmed by the user 2026-09-22)

**Nothing in P112 remains.** The AI half went green at `9fca997` (2026-09-15: 437.6s, exit 0, all 8
steps, zero FAIL lines — nextest 2556 passed/10 skipped · vitest 2963/265 · e2e 185 passed/1
skipped). The native half was **confirmed done and verified by the user on 2026-09-22**; the
orchestrator neither ran it nor could, so that attestation is what cleared it.

**Full detail: archive Part 76** — the milestone entry, the five checkpoint items as they stood
(kept there because they are the record of *what the confirmation covered*, including item 4's open
`set_parent` sub-question), the AI-gate evidence, the still-unverifiable note and the two decisions
that were owed.

- **Sub-increments 1-4 all landed:** sub-inc 2 `a2eb091`, sub-inc 3 `d0e6cf0`, sub-inc 4 `e13ff2d` +
  `9fca997`. Contracts: `docs/contracts/P112-external-tool-detection.md`, `P112-tool-catalog.md`,
  `P112-ui.md` (all three now `done` in `docs/contracts/INDEX.md`).
- **Still Windows-only evidence.** `pnpm gate` runs Windows; `ci.yml` runs
  `[ubuntu-22.04, windows-latest, macos-latest]`. The **AMEND-8 host-bound test fix is reasoned, not
  executed** — see the release block's recommendation 1, which is now the only thing that would
  execute it.
- **Contract deltas are still owed to `architect`** on `P112-external-tool-detection.md` — the
  four of them are the last bullet of `The external-tool launch residue` under
  `## OPEN follow-ups`.
- **The one P112 decision that was owed by the user is CLOSED:** whether
  `BrowsedProgram::from_settings_field` joins the mandatory `security-auditor` path trigger. The user
  ruled it in on 2026-09-17 **scoped to the whole external-tool launch module**, exactly as
  recommended, and `3f78d50` put it in `CLAUDE.md` (`crates/bonsai-core/src/tools/*.rs`,
  `src-tauri/src/commands/external.rs`, `src-tauri/src/commands/tools.rs`). The board carried it as
  "owed" in one section and "implemented" in another for five days; `CLAUDE.md` itself settles it.
- **Its ranked follow-ups stay live immediately below** — all but the first are open.

## P113 — dev-mode log review fixes — `in-progress`

**Current step:** P113a–c **committed green** (`a7a7882`, full gate 9/9 in 502s) —
**awaiting USER CHECKPOINT**, see below. P113d contract rev 2 approved and ready to implement;
not started.

**USER CHECKPOINT owed for P113a–c.** The AI gate cannot prove the acceptance criterion that
matters — "a comparable session records zero `forgeRateLimited`" is an absence in a live session
against a real Azure DevOps account, and the tests prove the mechanism, not the field outcome.
What the user needs to confirm in `pnpm tauri dev` with the same 5 repos open for a comparable
stretch: (1) `metrics/usage.json` shows no `forgeRateLimited` under `errors`; (2)
`cmd.forgeCommitStatuses` total time falls well below 286,888 ms; (3) `listTagSync` `dup-ipc`
anomalies are gone from the log; (4) CI badges still populate normally — the known failure mode is
badges staying blank during an open suppression window, which is indistinguishable from "no CI
configured"; (5) `forgeRepoContext` records now show `"authenticated":"bool"` instead of
`<redacted:token>`.

Source: review of a real 124-min Dev-mode session (2026-09-22, 5 repos open, Azure DevOps forge).
Evidence is `%APPDATA%\com.bonsai.app\logs\bonsai-2026-09-22T04-52-03-scca8269e.jsonl` (13,667
records), `metrics/usage.json` and `settings.json`; full write-up with counts and `file:line` in
`D:\Data\Temp\claude\bonsai-devlog\dev-log-review.md`. **P91's own anomaly rules found all of
this** — the rules were validated in passing (every `dup-ipc` pair carries an identical `argsHash`,
so none were false positives).

**P113a — forge rate-limit feedback loop.** `forgeCommitStatuses` burned 286,888 ms over 51 calls
(avg 5,625 ms, max 13,225 ms; two `lvl:error` hard-cap breaches). 20 of the 51 ran to completion
`superseded`, summing 110,942 ms, spending the rate-limit budget that then produced 8
`forgeRateLimited` errors. A 429 mid-batch discards every sha already resolved
(`crates/bonsai-forge/src/rollup.rs:136`); `useForgeSignals` swallows the error and re-requests the
same set next tick; the `Retry-After` Azure sends is formatted into a message string and never
consumed (`crates/bonsai-forge/src/azure/rest.rs:163`).
AC: a 429 returns the statuses already resolved; a rate-limit suppresses forge refreshes for the
advertised interval; no `forgeRateLimited` in a comparable session.

**P113b — `listTagSync` double-fire.** 138 calls / 52,880 ms, 86 `superseded` / 36,555 ms, 12
`dup-ipc` pairs with identical `argsHash`. Volume comes from `RepoWorkspace.tsx:876-883` putting
`refetchTagSync` into every `refreshAll` whose scope carries the `tagSync` slice. The 10 s cache
window sits in `useTagSync.ts`'s `!force` branch, so `force` bypasses it.
Fixed with a 2 s in-flight floor that applies to `force` too; a failed check still resets the clock.
AC: zero `dup-ipc` anomalies for `listTagSync`; call count drops with no loss of freshness.

> **Diagnosis correction, 2026-09-22.** The review brief claimed `useTagRemoteActions.ts:31`'s
> `refetchTagSync({force:true})` was redundant because `refreshAll('refsOnly')` already pushes it.
> **Wrong** — `refreshScope.ts:93` is `refsOnly: { ...NONE, graph, branches, compare }`, with no
> `tagSync` slice, so that call is necessary and removing it would leave a tag write with no drift
> re-check. senior-dev caught it and left the call in. **The actual source of the 12 `dup-ipc` pairs
> is still unconfirmed** (hypothesis: autoFetch success → a `repo-changed` full round's non-forced
> tagSync racing `afterAutoFetch`'s forced one). The floor covers it either way, which is why this
> did not block — but the cause is open, not closed.

**P113c — `authenticated: bool` logged as `<redacted:token>`.** `is_sensitive_key` matches
`k.contains("auth")` (`src-tauri/src/obs/scrub.rs:73`) and hits `ForgeRepoContext.authenticated`,
a `bool`. `resultShape` holds type names only, so the redaction protects nothing and removes the
one field that matters when debugging forge auth. Needs a `tests_redact.rs` case.

**SEC-2026-09-22 — `security-auditor` on the redaction narrowing: CLEAN.** No CRITICAL/HIGH/MEDIUM.
Not a mandatory CLAUDE.md trigger (`tools_*.rs` and the launch surface untouched); requested because
it is the one change that can fail only by writing *more* to disk. Confirmed the shape-map value
vocabulary is closed (`shapeOf` in `src/obs/redact.ts`), `ArgShape = BTreeMap<String, String>` keeps
it typed across the `log_append` boundary, and there is a single scrub choke point
(`writer.rs:269-288`) with no mode branch — raw mode is not more permissive than strict.
`mcpToken` is a `str`, not exempt, still scrubs.

Three items owed from that audit (route with the reviewer's MUST-FIX in one pass):
- **LOW** — `tests_redact.rs:237-264` pins only `str`; add `fn`, `arr:3`, `obj:2` and an unknown
  token under sensitive keys so the *deliberate* exclusions have a tripwire. Without it, a
  contributor "completing the list" breaks nothing.
- **Hardening** — `scrub.rs:401`'s `continue` skips `scrub_string` and home masking for exempt
  values. Behaviourally identical for the four tokens, but `scrub_value(val, home); continue;`
  exempts only the *key rule* and keeps P91 §7.2.1's "runs last, on every string field" literally
  true.
- **Contract drift (architect-owned)** — `docs/contracts/P91-observability.md:1122` lists
  `is_sensitive_key` as SHIPPED with no exception; the code now has one. Needs an amendment naming
  the carve-out and its two bounding conditions.

Disclosure delta to state in the commit message (do not claim "no change"): an absent
credential-named field now reads `"null"` where it previously read `<redacted:token>`, so a reader
learns such a field was unset. Presence only, never value.

**P113d — watcher log volume (architect first).** **12,001** `watcher` records = **61.4%** of the
log; **11,809 (98.4%) are non-firing** — 8,384 with `relevant>0` and 3,425 with `relevant==0`
against just **192** actual fires. Emitted per raw notify batch before the debounce and the
relevance filter (`src-tauri/src/watcher/mod.rs:204`). **No data loss** — zero `drop` records.

The 8,384 `relevant>0` non-fired records are **already re-reported, summed, in the `fired` record**
that closes their burst (`BurstAcc`, mod.rs:232 — verified numerically), so they can simply stop
being emitted; only the 3,425 `relevant==0` batches need a counter. The ~44:1 coalescing ratio shows
the 300 ms debounce is working — **do not change `DEBOUNCE`**, this is a logging fix only.

Record shape is P91 §2.4 → `docs/contracts/P113d-watcher-log-volume.md` (**rev 2, ready**).
Design: delete the per-batch `log_watcher` at `:204` outright — the notify thread stops emitting
entirely and every watcher record comes from the debounce thread. `relevant>0` batches lose only
their count and burst extent, recovered by two scalars on the fired record (`batches`, `burstMs`);
`relevant==0` batches go to a shared `NoiseCounters` drained onto the next fired record or a
standalone summary. Additive schema, `OBS_SCHEMA_VERSION` stays 2. Targets: 12,001 → ≤1,200 watcher
records, 61.4% → ~6% of the log, firings unchanged at ~192.

Two correctness traps the contract calls out for the implementer: notify `Err(_)` events lose their
immediate record and must be carried as `errors` on the fired record that closes their burst (§4.1 —
the one place a lost signal is a correctness bug, not a diagnostic one); and the inner-loop
`Disconnected` arm must stay a bare `return` with **no** fired record — logging a fire there would
stamp `fired:true` on a burst that never dispatched `on_change`, and because
`obs/anomaly/window.rs:234` keys `watcher-storm` on a single global key rather than per repo, a
`stop_all_watchers()` across 5 open repos would raise a spurious `watcher-storm` on every app exit.

> **Evidence correction, 2026-09-22.** The first pass reported "7,778 of 7,942 carry `relevant:0`".
> Wrong: records were grouped by JSON key signature and the group generalised from three samples.
> The architect caught the arithmetic inconsistency in review. Cross-tabulating the fields gives the
> figures above and **inverts the fix** — `relevant==0` is 29% of the waste, not 98%. Lesson for
> future log passes: cross-tabulate the fields you are claiming, never infer a field's distribution
> from a key-set count.

**Review round 1 (2026-09-22): `reviewer` → request changes on ONE mechanical MUST-FIX.**
`cargo fmt --all --check` failed on 8 files, every hunk in lines this increment added (the
`CommitStatusBatch` import pushed five `use` lines past `max_width`; two new multi-line `assert!`s;
the new `ratelimit.rs`). **The "reported green" list had omitted `cargo fmt`, so the increment was
never checked against the full gate** — `scripts/gate.mjs:159` runs it as a first-class rust step
(user ruling, 2026-09-17). Fixed in place by the orchestrator (mechanical, no senior-dev round-trip
— reviewer's own call). Everything else was SHOULD-FIX/NIT and is filed below per velocity mode.

Reviewer explicitly cleared: the `AppError` wire form (no arm missed, no catch-all, additive-only
serialization), `CommitStatusBatch` "Ok iff ≥1 resolved" semantics with 404-omit unchanged across
all four providers, `rebuildCiCache` retaining un-attempted tips, the backoff clamps, and the
redaction narrowing. No `security-auditor` path trigger fires.

**Carried forward from the P113a–c implementation** (reported by senior-dev, none blocking):

- **The 100-sha serial batch is untouched.** Worst case is still 100 serial GETs inside one
  `spawn_blocking` — that is the 13.2 s max, and the reason a 429 lands mid-batch at all. Partial
  success reduces the damage, not the latency. Options: cut `MAX_STATUS_BATCH` to ~25 for Azure, or
  parallelise 4–8 at a time.
- **Back-off coverage is partial.** Only `useForgeSignals` *consults* the suppression window.
  `useBranchChecks` feeds it but still fires on user action (deliberate — the error surfaces).
  `PrPanel`'s `forgeListPrs` / `forgeGetPr` / `forgePrDiff` neither feed nor consult it. **6 of the
  8 observed rate limits were `forgeListPrs`** — if any came from the panel rather than the badge
  refresh, those paths are still unprotected.
- **New user-visible state, no design pass yet:** opening or switching a repo during an open
  suppression window shows blank CI badges until it closes. Pre-P113 it 429'd and also showed
  blank, so this is not a regression — but "suppressed" and "no CI configured" now look identical.
  Worth a `ui-designer` look.
- **Stale archived contracts** (architect-owned, not touched): `docs/contracts/archive/
  P63-forge-graph-signals.md` and `archive/P90-ci-checks.md` still document
  `forgeCommitStatuses → CommitStatus[]` "same order". In-code docs are updated.
- **AC not verifiable here:** "a comparable session records zero `forgeRateLimited`" needs a real
  session. The tests prove the mechanism, not the field outcome — carry it to the USER CHECKPOINT.

**Open in-code follow-ups from review round 1** (not spun out — small, and they live in files a
future P113 pass will touch anyway):

- **NIT 6 — a 404'd sha keeps its stale badge on a cut-short batch.** `useForgeSignals.ts:271`
  narrows `requested` to *resolved* shas, so a sha that 404'd before the stop is also retained,
  contrary to the "drop a 404'd sha" rule at `:86-88`. Not a regression (pre-P113a the whole batch
  threw and everything stayed stale). Clean fix: an `attempted` count on `CommitStatusBatch` so TS
  can use `chunk.slice(0, attempted)`.
- **NIT 8 — the mock's doc contradicts its arithmetic.** `mock/handlers/forgeRateLimit.ts:48-53`
  says "a single-sha batch resolves nothing and the handler rejects", but
  `rateLimitCutoff(1) = max(1, 0) = 1`, so a one-sha batch resolves fully *and* sets `stoppedBy`.
  Mock-only.
- **NIT 9** — typo, `ratelimit.rs:18`: "A absurd/garbage header".
- **NIT 11** — `src/ipc/mock/handlers/forge.ts` is at 492 lines, one increment from the soft limit.
  The new mock seam was correctly given its own file; keep it that way.
- **Doc drift owed to `docs-curator`/`architect`:** `docs/contracts/archive/P63-forge-graph-signals.md`
  and `archive/P90-ci-checks.md` still say `forgeCommitStatuses → CommitStatus[]` "same order";
  `docs/contracts/P91-observability.md:1122` still lists `is_sensitive_key` with no exception.
  In-code docs are current.

**Deferred to background tasks** (spun out, not blocking): the 24 non-Tags render-storms from
refresh-round fan-out, 23 `redundant-refresh` pairs, the 5/60 graph cache hit rate,
`settings.json` stale-path pruning (`D:\Repos` no longer exists; the `ham-digi-backend`
`repoForgeOverrides` entry is dead), the TS/Rust `OBS_SCHEMA_VERSION` parity test, **extending the
backoff to the Checks and PR panels** (6 of the 8 observed rate limits were `forgeListPrs`, and the
ChecksPanel currently extends a window it never consults), and **locking the `AppError` wire shape
+ the redaction-exclusion tripwire**.

**Verified clean, do not re-chase:** `mcpToken` appears in zero log records across all three files;
empty `lifetime` totals are correct inside the 90-day window; the 1.2 KB log is a sink restart on
the `includeRawNames` toggle, not a crash.

## Follow-ups, ranked, none blocking

- **✅ SPLIT 2026-09-16 (`934a280`) — both zero-slack files now have room, and the baseline is
  regenerated so the gain is locked.** `App.tsx` **590 → 559**, `useSettingsPanelAdapter.ts`
  **499 → 445**, `SettingsPanel.test.tsx` **526 → 325**. New files: `useMcpWiring.ts` (95),
  `useSettingsConsentGates.ts` (123), `settingsPanelKit.tsx` (134),
  `mcpOutcomeLifetime.test.tsx` (90). Equivalence **2978 = 2978**, with the 27 `it()` titles diffed
  against the 23 + 4 they became. `scripts/file-size-baseline.json` regenerated — `App.tsx` 590→559
  plus **14 lines three untouched files had already reclaimed** (`ai_digest_cli.rs` 525→519,
  `ai_stream_bulk_cli.rs` 532→525, `RepoWorkspace.tsx` 2265→2264). **Shrinking only REPORTS a
  reclaim** — without `pnpm lint:size -- --update-baseline` the record is not rewritten and the file
  creeps back unnoticed. That is why the regen is part of the increment.
- **🆕 STILL PACKED, deliberately: `App.tsx:480`/`:481` (and `:423`/`:426`).** Same defect class as
  the `:506` line `934a280` removed — `:481` packs six props including `onChange`. **Why they were
  NOT taken:** unpacking costs ~+7 lines, which was safe against the old 590 but now **grows a
  freshly-baselined 559 file and fails the ratchet** unless paired with another extraction. So this
  is an extraction task, not a formatting one.
- **`Combobox.tsx`'s NUL byte** is fixed in the working tree but git will class the *pair* binary
  until the commit that lands it is itself the base — so the shared control was undiffable during
  its own review.
- **`cargo doc` reports 127 pre-existing findings** crate-wide (not a gate step; doctests are, and
  are green with `-D warnings`). One lands on `PickedTool` — public doc links to a private item.
- **Narrow, reasoned-not-tested:** a stale rescan landing after a Browse confirm. The mock **cannot
  reproduce it** — it builds its payload at *resolve* time where the real command reads settings at
  *entry* — so a regression guard needs a mock change.
- `useExternalToolScan.ts`'s mount-recovery-scan rejection and the second-browse-failure token case
  are reasoned only, for the same reason.
- **StrictMode** double-runs the owed-adopt mount effect → two identical microtask `report`s (legal,
  one visible note, one extra dev `flushSize`). No e2e reaches that state.


---

## The 2026-09-14/15 queue and the verification state — CLOSED (full detail: archive Parts 72 and 77.1)

- **What landed:** P112 sub-increments 1-4 · **F6** `usage.json` 90-day window + deletable (ruling
  #3, `d46c98e` + `b53618a`) · **P77** tag-sync on the auto-fetch cycle (ruling #11, `d46c98e`) · the
  **e2e cold-timing measurement** (ruling #9, `6a6f284`: 102 s cold bundle vs 191.4 s dev, build
  included) · the **UNC `canonicalize` ship-blocker cleared** by a real UNC probe.
- **Still open from that queue:** `src/assets/generate.rs` duplicates `stub_path()` and `set_mode()`
  (~35 lines) against `ai::testutil`'s versions. **Not swapped because it changes behaviour, not just
  the shared lock:** `testutil::set_mode` additionally does `remove_var(STUB_MARKER_ENV)`. The
  residual risk is **SEQUENTIAL, not concurrent** — a marker path left by an earlier
  `set_mode_with_marker` test persists into the generate tests. Harmless today (`generate.rs`'s only
  call sites are `:120` `"success"` and `:139` `"error"`, and only `stream_slow`/`stream_hang_stdin`
  tick the marker). **It becomes a real bug the moment a `generate.rs` test uses a stream mode** —
  that is the trigger to watch for, not a date. Needs its own non-behaviour-preserving increment.
- **The `h_ai` env race is CLOSED** (`80a852e` integration half, `105131a` lib half: six mutexes over
  one process-global become one, equivalence 1102 = 1102). Two standing rules from it: **do NOT add
  `--test-threads=1` to the `scripts/gate.mjs:148` fallback** (56 of 57 `tests/ai/*_cli.rs` tests
  hold the shared lock; the one that does not asserts two consts and touches no env), and **the
  `.config/nextest.toml` `h-ai-stub` group stays** — it exists for *process* concurrency (57
  concurrent `cmd.exe` + `ping` trees stall Windows), not for an env race.
- **The `\\wsl$` and OneDrive-placeholder UNC cases remain UNTESTED.**
- **The e2e bundle default was measured, NOT flipped** (ruling #9) — the number is on the record and
  the default stays the user's call.
- **⏳ USER ACTION — identify the Dependabot moderate alert** (ruling #15 — "not now", and **no `gh`
  install authorised**, so the orchestrator cannot read the page). All but identified from the push
  itself — see the release block. The "known `nanoid` high" this entry used to name was **stale**;
  `nanoid` is at the fixed 3.3.18 on both branches.
- **Current gate state is the 11-step `--full` green in the release block.** Superseded states:
  archive Parts 58, 72 and 77.1 (`5654eaa` 450.1s · `934a280` 414.2s) and 79 (`ea6d323` 411.6s ·
  `5f015be` 374.9s · `3948478` 446.1s).
- Port **1420 must stay free** — `strictPort: true` means a held port breaks `pnpm tauri dev`.

> **Curator note, updated 2026-09-22 — the four ruling ledgers below are reproduced verbatim and
> are authoritative.** What changed around them, never inside them: the evidence blocks their
> preambles point at ("the sections that follow") were archived once every item was ruled *and*
> shipped. The FOR-USER evidence for items 0-6 is **archive Part 63**; ruling #24's scope facts are
> **Part 77.2**; the second round's evidence blocks (`Why #21`, `LOW-1's surviving rung`, `Home
> masking was FAIL-OPEN`) are **Part 77.3**; the whole 2026-09-11 ruling queue is **Part 77.4**; and
> the 2026-09-17 set's narratives are **Parts 79.1/79.3/79.4**. **Nothing inside any ledger was
> shortened, reworded or re-opened.**

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
| 14 | Back up `.tauri/updater-prod.key` | ~~**Not now** — stays on the board as a user action.~~ **✅ DONE — the user confirmed the backup on 2026-09-22.** No longer single-copy. |
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

## 🆕 THIRD ROUND OF USER RULINGS — 2026-09-14 (**three**, from the harness measurement)

Same authority as the 23 before them. All three were asked with evidence in hand, not speculatively.

*(Header corrected 2026-09-22: it said "two" while the table below has always carried **three** rows
— #24, #25 and #26. Found by `docs-curator`; the rulings themselves are untouched.)*

| # | Item | Ruling |
|---|---|---|
| 24 | Settings toasts render behind Settings' own `.dialog-overlay` — scope of the fix | **SWEEP EVERY CALL SITE** (asked as "all 17"; the true figure is **10** — see the correction below). Not the delete outcome alone (which is what I recommended, on the grounds that it was the highest-stakes one and would build the recipe cheaply). The user chose the full surface. So: every `pushToast` reachable from Settings moves to an inline note, bringing the surface into line with `ui-reference.md:2377`'s standing rule instead of leaving 16 known violations behind a fixed one. |
| 25 | The 30 unpushed commits on `feat/post-p91-rulings` | **DO NOT PUSH.** Stays local. **Do not raise this again** — it has now been asked and answered, and re-raising it is noise. |
| 26 | UNC paths for external tools — refused by **both** detection and Browse, so a share-installed tool was unusable with no workaround | **ALLOW UNC VIA BROWSE ONLY.** Detection keeps refusing it — stat-ing a share inside the 1500 ms budgeted scan can hang or go over the wire. An explicit pick through the native dialog is a deliberate one-time act naming an exact file, so it is allowed. **This overrides contract line 577**, which mandates UNC refusal on the browse path; `custom::is_absolute_for`/`is_unc` must be amended for sub-inc 2/3. `looks_absolute` (detection) keeps refusing UNC. |

> **Two dated notes on the table above, added rather than woven in — the ruling text is verbatim and
> authoritative and is never reworded.** **#25 is SUPERSEDED for this branch**: on 2026-09-18 the
> user was asked again with three options and chose "Push branch only", so the branch is on `origin`
> at `b80dd36` — see the release block's push section. The ruling's *history* stands, its
> *instruction* does not. **#24 is COMPLETE**: its "10 of 15" scope prose was archived (Part 77.2)
> once `grep "pushToast(" src/` returned zero Settings-reachable call sites — see `📐 Three board
> claims measured and found STALE` in the release block.

## 🆕 SECOND ROUND OF USER RULINGS — 2026-09-11 (four more, from the review findings)

These came out of the reviews of the first increment, not from the original 17. Same authority.

| # | Item | Ruling |
|---|---|---|
| 18 | MCP audit scope (`stage_paths` HIGH + MEDIUM + 4 LOWs) | **DO EVERYTHING IN THE AUDIT.** Not just the symlink guard. ~~**QUEUED, not started.**~~ **✅ DONE — `216ca45` "close the MCP audit — stage escape, status membership, and the hook gate", i.e. exactly the three the audit named. Verified 2026-09-22 by the orchestrator; the row is annotated, never reworded.** |
| 19 | MCP review gate | **Snapshot test + review trigger.** A test snapshotting `list_all()` tool descriptions, **and** a rule that any diff touching `crates/bonsai-mcp/src/server/tools_*.rs` requires a `security-auditor` pass regardless of the commit subject. ~~**QUEUED, not started.**~~ **✅ DONE, BOTH HALVES. Verified 2026-09-22:** the snapshot test is `crates/bonsai-mcp/src/server/model_contract_tests.rs:248`, `tool_descriptions_match_the_checked_in_snapshot()`; the review trigger is in `CLAUDE.md` (`3f78d50`). Annotated, never reworded. |
| 20 | macOS `open -a` / `.app` regression from the `{path}` removal | **Teach the ladder app bundles** — Bonsai supplies the arguments itself, so no injection surface. **SUPERSEDED IN PLACE by #21:** this moves into the *detection* logic of the removal milestone rather than into the configured-program path, which is being deleted. Recorded so the intent is not lost. |
| 21 | Security MEDIUM-2, after the auditor showed validation insufficient | **DO THE REMOVAL NOW**, as its own milestone — not the interim native-confirmation dialog. Free-text program entry is eliminated and replaced by a detected-list picker. Contract in flight. |
| 22 | Fail-open home masking + the LOW-1 doc claim | **TAKE BOTH.** |


---

## 🆕 FOURTH ROUND OF USER RULINGS — 2026-09-17 (the credential-honesty set)

Same authority as the 26 before them. **Reproduced verbatim, as relocated from the sections the
2026-09-22 pass archived** (narratives: archive Parts 79.1, 79.3 and 79.4) — nothing here was
reworded, and the curator does not resolve or re-open any of it.

### Ruling — "remove the token": what "Remove account" must mean

*Verbatim, including its original list numbering — it was item 2 of the two decisions then owed
by the user. The defect write-up it cites (`🚨 NEW 2026-09-14 — "Remove account" reports success
even when the token was NOT deleted`) is **archive Part 79.1**.*

2. **✅ RULED 2026-09-17 by the user: "remove the token."** Token deletion IS the operation — if
   the token is not gone, the removal failed. The three implemented outcomes: (a) `delete_token`
   fails → `Err`, and **nothing else changes** (record stays, so the row is visible and the user can
   retry); (b) key not present → success, the command stays idempotent and therefore re-runnable
   after (a); (c) delete succeeded but `settings::update` failed → `Err` with a *distinguishable*
   message, because the credential genuinely is gone while the list entry may persist. Routed to
   `senior-dev` 2026-09-17 with a `security-auditor` pass to follow (credential storage is a
   standing mandate trigger). Original defect write-up: `### 🚨 NEW 2026-09-14 — "Remove account"
   reports success even when the token was NOT deleted`.


### Ruling — best-effort legacy sweep (the return-type change it forced)

**USER RULING — best-effort legacy sweep.** Fail-closed could **permanently strand** a user: the
account's own token is already gone, so a retry hits `NoEntry → Ok` while the legacy key refuses
again → a listed, disconnected, **unremovable** account. That forced a **return-type change**:
`Result<ForgeRemoveOutcome, AppError>` with `leftover: Option<String>`. **My assumption that it could
ride the existing `Err` channel was wrong** — `ui-designer` blocked it: R2's `Err` is a real failure,
this one means **success**, and riding `Err` keeps the dialog open over a deleted account (no
`refetch()` on that path) and mis-counts obs/`ipc.result`/the DEV toast guard.

### ✅ ALL FOUR USER DECISIONS OF 2026-09-17 ARE IMPLEMENTED

1. **Dormant command DROPPED** — `871d16a`. The audit named 5 plumbing sites; **grep found 6 more,
   and 2 of those would have broken a test rather than merely lingering**:
   `obs/metrics_cmds.rs` declares `KNOWN_CMDS` as a **fixed-length array** (201 → 200), and
   `src/obs/rawArgPolicy.json:138` carried an entry a test asserts is a subset of the mock-IPC
   methods. The other 4 were doc claims that had quietly gone false (a broken intra-doc link to the
   deleted `clear_token`; a comment naming the helper the command layer stopped calling;
   `commands/forge.rs` claiming auth flows through a function that no longer exists).
   **The module is `#[cfg(test)]`, not module-wide `#[allow(dead_code)]`** — the implementer's
   blanket was overridden, then refined again when `cfg(test)` alone still left clippy red: exactly
   one item (`_inner`) is unreachable even from the tests, since all 12 drive `_inner_with`, so the
   allow is scoped to that **one function**. A module-wide allow would have hidden future dead code
   in a credential-deleting module. Implementation + all 12 tests kept, so a rewire is covered.
2. **Both dead helpers DELETED** — `871d16a`. No caller anywhere in the workspace but one test,
   which went with them. `clear_token_for_host` was the footgun (bundled `evict_viewer` into the
   delete). `bonsai-forge` 214 → 213.
3. **CLAUDE.md audit trigger ADDED** — `3f78d50`, scoped to the **module**
   (`crates/bonsai-core/src/tools/*.rs`, `src-tauri/src/commands/external.rs`,
   `src-tauri/src/commands/tools.rs`), not to `from_settings_field` alone, because a caller change
   can widen what gets spawned without that function appearing in the diff.
4. **rustfmt DONE** — `8ad3c72` (+ `5f015be` baseline). User overrode the recommendation to defer.

### ✅ USER RULING 2026-09-17 — "do the sibling one too": MEDIUM-1 IS AUTHORIZED WORK

`forge_clear_token_for_host_inner` (`src-tauri/src/commands/forge_accounts.rs:364-380`) gets the same
treatment as `forge_remove_account`: collect the per-key `delete_token` results, and if **any**
failed, return `Err` and mutate **nothing** — no `retain`, no `clear_token_for_host`, no
`settings::update`. The host stays listed so the sign-out is retryable, which is the same property
ruling 1 bought for the single-account case.

**Scope boundary the user did NOT authorize:** MEDIUM-2 (the two upstream orphan sources —
`forge_accounts.rs:229`/`:240` token-written-record-not, and `settings/forge_accounts.rs:170`'s
legacy re-key) stays filed. "The sibling one" is MEDIUM-1. Do not widen.

**SEQUENCED, not parallel — deliberately.** The B fix pass was still in flight when this ruling
landed, and it owns `forge_remove_account.rs`, `forge_remove_account_tests.rs`,
`forgeRemoveFailure.ts`, and `SettingsAccountsSection.remove.test.tsx`. The sibling fix needs
`forge_accounts.rs`, the mock seams in `forge.ts`, and the accounts UI section — **`forge.ts` and the
settings section overlap**. Two agents editing one crate concurrently is what produced today's
107-file `cargo fmt` near-miss; this one waits for that pass to report.

**Copy is modelled on the two approved messages, and the polish is routed, not skipped.** The
sign-out failure needs its own string; senior-dev writes it in the shape of the approved pair and it
joins the existing `ui-designer` copy item (the prefix stutter) rather than being invented twice.

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


---

## 🔄 The 2026-09-11 ruling queue — CLOSED (full detail: archive Part 77.4)

Every item that was "not in a contract" has landed or been ruled: the P112 removal (rulings #4 →
#21), F6 (#3), P77 tag-sync (#11), the e2e-bundle measurement (#9), the UNC `canonicalize` clearance,
and the 8-step gate green under happy-dom at `b53618a`. The two residues that stayed live are in
`## The 2026-09-14/15 queue and the verification state` above.

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
- **Check port 1420 after any harness-heavy pass** — `Get-NetTCPConnection -LocalPort 1420` is the
  whole check. An agent that drives `pnpm dev` by hand **orphans** it (once for **3.5 hours**,
  2026-09-14, PID 12712, 13:18:31 → 16:50:55), and `vite.config.ts` sets **`strictPort: true`**, so
  `pnpm tauri dev` then **fails outright** rather than falling back — i.e. it breaks the USER
  CHECKPOINT. Brief agents to the Playwright-managed path (`scripts/e2e-server.mjs`), never a
  hand-run dev server. `playwright.config.ts` already carries the warning. Narrative: Part 71.2.
- **❌ VOID as of 2026-09-17 (`8ad3c72`): `cargo fmt --check` IS a gate step and the tree IS clean.**
  This line used to say it was not a gate step and dirty at baseline, so never to read its output on
  a diff as a regression. All of that is now false — a fmt failure is a real regression.

### The coverage and evidence rules (earned 2026-09-14/15; narratives archive Parts 71 and 73)

- **A test that passes in the correct AND the broken state is not coverage.** Four instances in the
  P112 work alone. The negative must be proven red on the unfixed code first.
- **The limit case is a test that is never in any state.** `external_picked_tests.rs:186` is
  `#[cfg(not(debug_assertions))]`, so it compiles only under `--release`, which the gate never runs.
- **A green gate says nothing about a Rust/TS DTO change.** Every frontend tier — vitest, tsc, e2e,
  the browser harness — consumes the mock, so all of them stay green while the real app is broken.
  Only `src-tauri/src/settings_defaults_parity_tests.rs` spans the two languages, so **weakening the
  parity oracle is never routine**, and a DTO change must land its Rust and TypeScript halves in the
  **same** increment. The mock is not inventing anything in this failure mode; it is **stale**, and
  staleness is invisible to every test that consumes it.
- **A green `pnpm gate` is Windows-only evidence.** Any test that passes an explicit `TargetOs` while
  touching the real filesystem is host-bound; the absoluteness rule is what makes it so.
- **When a contract signs an error string that names a recovery, the recovery is part of the same
  contract item.** §16.4a signed `BROWSE_STALE`'s string and its report but **not its verb**, so as
  written the message named an action that did nothing.
- **Cite from the file, not from a summary.** Four citations in one brief were wrong, and one had
  already propagated a false claim into a contract — where the next reader treats it as established.
  A `file §section` pair is the dangerous shape: the section number can be right for a *different*
  file. Verify a citation **before** delegating on it.
- **Mislabelled evidence is this repository's named defect.** A test message that claims more than
  the test discriminates (`tests_tools_pick.rs:148`: "both fields in ONE update cycle", which two
  sequential `settings::update` calls would also satisfy) is a defect even when the code is right.

### 📌 TWO DURABLE CONSTRAINTS discovered in the fix pass — keep these

1. **`tracing` DOES NOT EXIST in this workspace.** My brief said "`settings.rs` has zero `tracing::`
   calls, so there is nothing to piggyback on" — that understated it: **no crate depends on
   `tracing` at all**, so there is no facade to add a call to. The project's actual non-fatal
   diagnostic facade is `eprintln!("bonsai: …")` (as in `commands::repo`, `lib.rs`,
   `commands::ui_settings`). Use that, and do not write `tracing::` into a brief again.
2. **The settings-load path CANNOT use the observability sink — a bootstrapping constraint.** The
   P91 `obs` sink is **configured from the very settings** that `load_from` is in the middle of
   reading, so it cannot be running yet when migration code executes. Any diagnostic inside
   `load_from` has to be `eprintln!`. This is a real ordering constraint, not a preference, and it
   will bite anyone who tries to route settings-layer diagnostics through `obs`.


### Rules earned 2026-09-16/17 (narratives: archive Parts 76.1, 78 and 79)

- **Commit the moment an increment is approved, even when a MUST-FIX is routed; review the fix
  against a small diff.** Earned by letting one review diff reach **41 modified + 13 new files**, of
  which the reviewer had to name **nine files it did not re-review**.
- **The fixture, not the `expect`, is where "green in both states" hides.** Three misses in one
  session were assertions with **nothing to bite on** (`changedProps` compared two empty objects;
  `settingsToastGuard` asserted on a bare `vi.fn()`; the churn fixture omitted `onReveal`). Require an
  **observed red state per case**, and on review probe a regression *other* than the induced one.
- **A claim that something is *visible* or *clickable* needs `elementFromPoint`, computed style or
  bounding boxes.** Text extraction (`innerText` / `get_page_text`) proves a string is in the DOM and
  is **blind to z-index stacking** — which is how a toast rendered under a scrim, unclickable, passed
  two code reviews and every harness pass.
- **When fixing a defect defined by a code shape, grep for the shape, not the filename.** The third
  swallowing write path (`forge_set_token`, `forge.rs:297-318`) held the **verbatim** pre-fix body and
  was found only because the auditor re-checked after the "fix".
- **A counterfactual against credential-writing code requires the DI seam FIRST.** Proving a fix red
  by running the pre-fix body wrote a test token into the user's real Windows Credential Manager.
- **`cargo fmt -p <crate>` while concurrent Rust edits are live, never `--all`.** A bare `--all`
  rewrapped two files mid-flight and reverting would have destroyed in-flight work.
- **Run `node scripts/check-file-size.mjs` before committing any pass that adds lines to a test
  file.** A 526-line test file was a **hard fail** (not baselined) and would have gone red at gate
  step 7 after a 414 s run.
- **The coverage standard (keep all four):** two counterfactuals, not one — against the **pre-fix**
  defect *and* against the **wrong fix**; compile-gated pins are **declared, not counted**; every
  "this is covered" claim needs a **mutation proof**; restored files are verified with `cmp`, not by
  eye.
- **A test that passes in the correct AND the broken state is not coverage** — of 8 new tests in one
  increment only **2** were fix coverage, and the file header overselling them was itself a finding.
- **`a_failed_connect_leaves_no_repo_override` was ALREADY red** against a production mutation that
  wrote the pin outside the injected closure, because `override_for` reads the settings **file**. The
  prescribed change produced **byte-identical** red output — execution coverage, **no new
  discriminator**. Recorded on the board's own instruction, so nobody re-derives a fix for a
  non-problem.

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
  **⚠ SUPERSEDED 2026-09-17 (`8ad3c72`, baseline `5f015be`):** the Rust tree is now rustfmt-clean
  and `cargo fmt --all --check` is **gate step 3**, so a fmt failure **is** a real regression. The
  live rule is `### ❌ VOID — the rule that fmt output on a diff is not a regression` below; the
  standing constraint that survives is *never a bare `cargo fmt --all` while concurrent Rust edits
  are live — use `-p <crate>`*.
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


### Filed 2026-09-16 — the real-log residue (full narrative: archive Part 78.1)

The real-log investigation landed in `88a4004`, `7f186b3`, `2fc03cc`, `5654eaa`: the render storm is
fixed (`MAX_TALLY_RENDERS` **1012 → 8** for one ref change), `mono` is stamped, the StrictMode
activation guard is fixed, `changedProps` absence now means "not tracked", `OBS_SCHEMA_VERSION` is 2.
**Still open:**

- **The one-commit refactor.** A *genuinely-changed* round still lands **up to 3** commits, because
  each IPC response resolves in its own microtask. Having the refetches **return** data and applying
  it once after `Promise.all` is the real next step. **The 126× win is the no-change case, not
  universal.**
- **`Sink::mono()` (`sink.rs:401/405`) is wall-clock derived** (`now_ms() - started_ms`), which
  contradicts the "jitter-free" framing on the producer path. ~4 lines (an `Instant` beside
  `started_ms`). Interacts with "0 means unset": the sink can legitimately return 0 in the first ms.
- **`each`-mode `changedProps` still cannot distinguish** tracked-unchanged from untracked (it omits
  when empty, by instruction). Only `render.tally` carries the three-state guarantee.
- **The 10 `useRenderCount(…, undefined, 'aggregate')` sites** — pass real props to make the gauge
  diagnostic. Deliberately NOT done: inventing props for 10 sites is a judgment call, not a fix.
- **Cross-tab detector keying → `architect`.** `window.rs:158-165` keys the redundant-refresh window
  on `scope` **alone** and the refresh record carries **no repoId**, so two unrelated repos refreshing
  within 1 s are indistinguishable. Latent, but the log shows two coalescer instances both at
  `round: 1`, so multiple tabs do happen. Needs a DTO field.
- **Dev-mode log VOLUME:** 10,280 watcher records = **87%** of a 2.2 MB / 6-minute log, and **7,247
  had `relevant: 0`** — a record per file change it then correctly ignores. Cost, not correctness.
- **`RemotesSection` still re-renders on a local-branch change** — it receives `data` directly; the
  `remoteFlatFiltered` dep narrowing does not stop it. 2 renders / 1 instance.
- **Contract drift (contracts are not the orchestrator's to edit):**
  `docs/contracts/P91-observability.md:261` still says "jitter-free ordering aid" and
  `:230,235,252,322` still say schema 1; the **P81** contract names `pendingTagForceRef`, **renamed**
  to `pendingUserOriginRef` (a rename, not a merge — reviewer-verified byte-identical).
- **Headroom:** `Sidebar.tsx` **491/500** (9 lines — the next sidebar prop needs a split),
  `writer.rs` **491**, `record.rs` **487**. All hard caps, not baselined.

### Filed 2026-09-16 — the nine-file second review pass and P113 (narrative + closure: archive Part 78.1)

The pass itself is **✅ CLOSED** — both MUST-FIX fixed (`2b3dfd6`), the four SHOULD-FIX resolved
(`9aa25f4`), two coverage holes and three file splits landed (`934a280`), gate green. **What it left
open:**

- **`adoptToolSelection` cites a STRUCK, REVERSED contract bullet → `architect`.**
  `useUiSettings.ts:50` and `:126` (echoed `SettingsContext.ts:139`, `useExternalToolScan.ts:106`)
  cite "P112 §16.16-5", struck and reversed at `P112-ui.md:1594-1601`; the governing §17 passage
  (`:1683-1690`) rules *"accept it as shipped; do not rework"*. **No passage in `docs/contracts/`
  specifies `adoptToolSelection` at all** — a shipped, approved **deviation cited to a dead bullet**.
- **`useSettingsSaveFailure.ts:71` decides banner-vs-toast ONCE, at failure time.** Fail with Settings
  open → banner, no toast; the user closes Settings → the condition persists with **no surface at
  all**, backoff having stopped after 3 attempts. Against `P113-settings-inline-notes.md:1133-1137`
  ("persists exactly as long as the condition does"). Fix: raise the toast lazily on the open→closed
  transition while `settingsSaveFailed` is still true. **Its own increment** — a behaviour change.
- **The narrow pre-existing late-`report` race:** Add → stop → clear → `report` writes with rows
  unmounted → re-enable takes the `enabled === true` early return. Reachable only without a Settings
  close.
- **Selector-constant risk:** `useOutcomeScrollCorrection.ts:61,64` hardcodes `.settings-pane` /
  `.settings-row` and **so does the test fixture**, so a rename in `SettingsShell.tsx:206` /
  `SettingsRow.tsx:109` breaks **production** while test and module keep agreeing. Only an exported
  constant *consumed by production* closes it.
- **`useOutcomeScrollCorrection.ts:65-66`'s comment is STALE and `:67` is unreachable here.** It
  blames jsdom; this repo runs **happy-dom** (`vite.config.ts:45`), which implements `scrollIntoView`
  as a no-op. The module is inert in other suites because of **zero geometry**, not the bailout. The
  stale comment actively invited a wrong diagnosis once already.
- **Four NITs.** `useSettingsOpenSignal.ts:20` cites `App.tsx:90`, actual wiring `:83` · §10.3's
  `while (deficit > 0)` shipped as one pass + a height guard, so **a note taller than the scrollport
  gets no correction and AC2b is unachievable for it by construction** (one clause in §10.3 would
  close it) · `useToastQueue.ts:63` reads an effect-synced ref, so a handler that opens Settings and
  pushes in the same commit escapes the DEV guard · `useMcpControls.ts:71-74` swallows a failed
  `getMcpStatus` and the section then renders "Stopped.", indistinguishable from a genuinely stopped
  server.
- **Citation drift the `934a280` split created — none is a defect, all four will mislead:**
  `P113-settings-inline-notes.md:1088` cites `App.tsx:208` for the `useMcpControls` wiring, but App
  no longer calls the hook at all (`src/hooks/useMcpWiring.ts:67`; `App.tsx:201` is the
  `useMcpWiring` call) — **contract-owner's fix** · the `hydrateUiSettings` sole caller is
  `App.tsx:259`, not `:272` · `mcpOutcomeNotes.test.tsx:41` cites the adapter at `:413` → **`:358`** ·
  `useMcpControls.ts`'s header still says it "is wired from `App.tsx`".
- **P113 residuals, each independently deferrable:** **`P113-F1-accounts-error-copy`** (own
  increment) — rows 6-8 append **raw `errorMessage(e)`** to a mapped lead, so raw backend text is now
  **permanent instead of transient**; mapping it needs the backend error-kind inventory, so P113
  renders verbatim with `overflow-wrap: anywhere` · **recorded, not fixable by lint:** a toast raised
  by a **background** event while Settings is open is equally invisible, and no lint rule can catch
  it — deliberately changing no z-index · **NIT, deliberately not done:** `DevToast` /
  `deleteResultToast` / `devDeleteToastRows.test.ts` are now misnomers; renaming churns two test
  files for no behaviour change.

### ✅ `watcher::tests::git_internals_filtered` — SETTLED. Stop re-characterising it

- **It is ambient-load sensitivity in a wall-clock negative assertion.** `tests.rs:127-153` declares
  quiet after a 1 s residual sweep, then asserts **nothing arrives for 1500 ms of wall clock**; under
  enough ambient load a stale init event delayed past the sweep lands inside that negative window.
- **Three earlier characterisations are history, not competing claims:** the "nextest isolates
  processes / `cargo test` uses threads" mechanism (refuted — `serialize_watcher_test()` is a
  process-wide mutex, so nextest is the *less* protected configuration), the orphaned-vite-dev-server
  contributor (archive Part 71.2), and the "did not reproduce" note (531 passed / 0 failed). **The
  canonical reading is this one.** Full narrative: archive Part 78.1.
- **Deterministic fix direction (follow-up, not urgent):** `watcher::classify::is_relevant` already
  pins `objects/aa/bb ⇒ false` as a **pure unit test** (`classify.rs:118`), so the negative-window
  half is **redundant coverage carrying all of the timing risk**. The `.git/HEAD` positive
  (`tests.rs:149-152`) is the half that earns its keep.
- **Two gate facts worth keeping:** the `cargo test --workspace` fallback is a **fresh-contributor
  path, not a pipeline path** (`hasNextest` gates it at `gate.mjs:86`; CI installs nextest explicitly,
  `ci.yml:92`) — real, but never exercised by our own gates; and **the local gate is STRICTER than CI
  on flakes** — CI runs `--profile ci` with `retries = 1` (`.config/nextest.toml`), `gate.mjs` uses
  `default` with **no retries**.


### ❓ OPEN USER DECISION from the real-log run — the `render-storm` threshold (verbatim)

**❓ USER DECISION OWED — the `render-storm` threshold.** `anomaly.rs:136` is
`renders > 3 * instances`, and the docstring claiming `aggregate` mode "collapses away" the StrictMode
doubling was **false** (it collapses *records*, not *renders*) — so the threshold was set against a
belief that never held, and effectively trips at **1.5× real renders**. That is why it fired **423
times in 6 minutes**. Options: raise the threshold · have the tally divide the doubling out for
aggregate mode · leave it noisy-but-sensitive. **My recommendation: divide it out** — a detector that
fires on essentially every refresh round trains the user to ignore it. **Not actioned; it is a
detector-tuning decision and it is the user's.**

### Open from P112 sub-increment 2 — the observability-capture privacy decision (verbatim; narrative: archive Part 78.1)

- **STILL OPEN — the observability-capture privacy decision.** `setUiSettings`/`getUiSettings` are
  both already in the captured-command list (`obs/metrics_cmds.rs:130,209`) and `obs/record.rs:207`
  has an optional raw-payload capture. Now that `pick_external_tool` returns a browsed path and
  `tool_scan` returns `DetectedTool.detail`, a **filesystem path can be written to a log file on disk
  under dev mode**. Decide whether those two commands stay in the capture set — same class as P91's
  home-masking work.
- **Trust boundary, stated once so nobody later reads it as a gap:** a **hand-edited `settings.json`**
  carrying `customEditorPath` **is honoured**, deliberately and in scope. The property is scoped to
  *a **renderer-written** program path is unrepresentable*; someone who can edit `settings.json` can
  equally replace the binary it names.
### 🚨 A4, FINDING 6 — my approval was ambiguous and the implementation lost the raw error

The implementer read "Settings closed → the toast, unchanged" as **the channel** unchanged, and ships
**one string in both channels**. **I approve that half** — a single string cannot drift, and the new
copy is true in both contexts.

**But I checked what happened to the underlying error and it is gone.** `useSettingsWriteQueue.ts:128`
calls `noteFailure(streak)` with **only the streak**; there is no `errorMessage(e)`, no `console`, no
log call anywhere on the new path. The raw OS error previously rode in the toast text
(`Could not save settings: ${errorMessage(e)}`) and **now nothing captures it at all.**

That is a **diagnostic regression**, and a pointed one: the approved copy tells the user to *"check
that Bonsai can write to its config folder"*, which is a **guess** — disk-full, permission-denied,
path-too-long and a locked file all produce the same sentence, and the raw error is exactly what would
distinguish them. P91's whole observability programme exists so failures are diagnosable; this change
quietly removed the only record of a real one. **Routing: keep the user-facing string, and preserve
the raw error on the diagnostic path (dev-mode log).** Not a copy change — the string stays as
approved.


### Filed 2026-09-17 — the credential arc's open residue (full narrative: archive Parts 79.1 and 79.4)

The arc itself landed: `105131a`, `e583f11`, `3948478`, `871d16a`, `61af79b`, `ea6d323`. **What it
left open, including three things only the user can settle:**

- **⏳ AWAITING USER — `auth::global()` has no test-mode guard.** All four credential write paths now
  have a DI seam and the seamless body is gone, but `crates/bonsai-forge/src/auth.rs:145` builds the
  real keychain unconditionally, so the hazard is **one greppable choke point, NOT structurally
  closed**. Recommended: a guard that panics unless `BONSAI_ALLOW_REAL_KEYCHAIN=1`.
- **⏳ AWAITING USER — persist the leftover-credential disclosure.** The `leftover` disclosure is
  **TERMINAL for outcome R4**: the record is deleted, so `last_on_host` can never be re-derived and
  the sweep can never re-run. Its sole disclosure is a section note that `begin(SECTION_SLOT)` clears
  on the next operation and that dies with the Settings view — so **a live orphaned PAT is disclosed
  exactly once, transiently.** Fix would be persisting a `leftoverCredentials` note in settings.
- **🔎 A LIVE ORPHAN IN THE USER'S OWN KEYCHAIN — found while verifying, deliberately NOT touched.**
  `cmdkey /list` shows `github.com.com.bonsai.app` and `azuredevops:dev.azure.com.com.bonsai.app`,
  while the user's `settings.json` holds **one** account (`azureDevOps:dev.azure.com`) and **no record
  naming `github.com`** — a genuine unreferenced orphan, the live specimen of the bug. Removing a
  credential from the user's keychain is not the orchestrator's call, and it may be a PAT they use
  elsewhere. Workaround if wanted: add an account on `github.com` then remove it, which triggers the
  sweep. (The `azuredevops:` / `azureDevOps:` casing difference is expected — `TokenStore`
  lowercases internally.)
- **⚠ The orphan class splits in two, and one half has no mechanism at all.** With
  `clear_token_for_host` deleted and `forge_clear_host` dormant, `forge_remove_account.rs:225` is the
  **only bare-host sweep in the product**, gated on removing the last account on a host **that still
  has a record**. So: **permanent-but-disclosed** for the record-bearing shape, and
  **permanent-and-undisclosed** for the record-less shape — which is exactly the user's real
  `github.com` entry. Structural fix would be reviving `forge_clear_host`'s outcome 1'.
- **The add-path superseded sweep swallows its failure** with zero disclosure
  (`forge_add_account.rs:287`). The same `leftover`-on-success channel would close it.
- **Genuine dead code, filed not deleted:** `forge_set_token.rs:69`'s empty-host early return is
  **unreachable** (`detect_provider` returns `None` on an empty host, `resolve_target` pairs that with
  `ForgeKind::Unknown`, and `require_supported()` is the first statement of `viewer()`). True at HEAD
  too, so preserving it verbatim was right.
- **Residual gaps in BOTH credential modules, stated not hidden:** the `include_str!` guard checks
  that prefix and suffix *appear*, **not that they belong to the same literal**; and because the
  command is UI-unreachable the `?forgeClearHostFail=` seams **cannot be browser-verified**, so the
  Rust tests are the only live coverage of that path until the UI wires it.
- **NITs left unfixed, deliberately:** `forgeOffline.ts:11`'s `FORGE_OFF` export has no external
  consumer · the retry ledger inside `forgeClearHostFailure.ts:74` mutates on every call including
  `seam === null`, where the sibling keeps it in `forge.ts` and passes `attempt` in (inconsistency,
  not a defect) · `KEYCHAIN_FAIL_NO_ACCOUNT_SUFFIX` is one long unwrapped line.
- **P114 §A.5 carries one stale option-2 line** ("then *reject* under the new kind"), written for the
  shape that was not chosen — `ui-designer`-owned. Also: duplicated `scratch_dir` helpers in three
  test modules; `ChecksPanel.connect.test.tsx` rides a 300 ms real-timer debounce (precedent-
  following, but the one place a slow CI runner could bite).
- **`health::tests_sections::perf_ceiling_on_20k_fixture` headroom is thin, not drifting.** A 2067 ms
  reading against the 2000 ms budget was **host load**; best-of-3 on the same host is **1542 ms, 23%
  under**. `branches` alone swung **1155 → 2273 ms across three passes in one run**. A single timing
  sample on a loaded machine is not evidence of drift.

### Filed 2026-09-17 — what the contract-hygiene pass surfaced (narrative: archive Part 79.2)

- **`P87b-FU1-FU4-git-dock-ui.md` F-F(a) is OPEN and its precondition has FLIPPED.**
  `repoState.ts:89` already has `RepoKind = 'default' | 'detached' | 'unborn'`, so the "fix when a
  fixture exists" condition is satisfied and `Commit main` on unborn is **live** (`status.ts:145`,
  `stash.ts:148`, `merge.ts:83` pass `'main'` unconditionally). **Trap, verified at source:**
  `repoState.ts:335-336` returns `branchName: 'main', unborn: true`, so **`?? null` does NOT fix it**
  — the mock needs the same explicit `unborn || detached → null` guard as the Rust resolver.
- **`forgeAccountStore.ts:51`'s `FORGE_LONG_HOST_CASE` is dead** — `:58`'s
  `urlParam('forgeRemoveFail') !== null` subsumes `=== 'long'`. (A reviewer judged it merely
  redundant; the architect judged it dead. Check its other uses before deleting.)
- **Guard asymmetry:** the remove seams have a **fourth** pin enumerating every value
  (`SettingsAccountsSection.remove.test.tsx:115-137`); the clear-host seams have **no vitest
  equivalent**.
- **`refactorer` batches 2 and 3 are still queued** — batch 2 is `bonsai-core` `src` (4 files, incl.
  1 app file), batch 3 is `src-tauri` (3 files). Batch 1 landed as `fbf81d0`.
- **✅ CLOSED 2026-09-22 (curator-verified) — the U+200B at `P107-F2-copy-chip-ui.md:249` is gone.**
  A codepoint grep for U+200B / U+202E / U+200E over `docs/contracts/*.md` returns **zero hits**;
  `2233cc0` touched that file. **Three more remain under `docs/contracts/archive/`** — out of scope
  as history.
- **`git blame` noise:** use `--ignore-rev 8ad3c72`.

### 🆕 The external-tool launch residue — filed, not routed (audits: archive Parts 74.2 and 79.4)

- **LOW-1, a robustness regression the `.cmd` fix introduced.** `code.CMD` **spawns successfully
  whenever `cmd.exe` exists** — even if `Code.exe` has been deleted — because the failure then happens
  *inside* the batch file, **after `spawn()` returned `Ok`**. With `hide_console = true` the user sees
  nothing. Previously the extension-less shim's `os error 193` made rung 1 fail and the ladder
  advanced to `code-insiders`. **Net: a broken primary install now yields a silent no-op instead of
  falling through.** No cheap fix (`wait_for_exit` would block on the editor's lifetime).
- **INFO-2, pre-existing: a bearer token transits a `cmd.exe` command line.** `ai/mod.rs:399` passes
  `format!("Authorization: Bearer {token}")` as an **argv element** to `claude`, which on Windows
  resolves to `claude.cmd`. Bonsai-generated so not attacker-controlled, and std's `%` handling
  protects the value — but it is a secret **visible in process listings**.
- **A UNC `PATH` entry sends `is_file()` to the network, inside `resolve_in`.** It passes
  `is_absolute()` and is inconsistent with the house `is_unc` rule applied everywhere else. **This is
  the real hang source** — detection's UNC refusal is *post-hoc* for the `OnPath` rung, so preventing
  the I/O needs a guard **inside `procutil::resolve_in`**, which changes app-wide resolution and is
  its own change.
- **`procutil.rs:41-43`'s separator shortcut bypasses both guards**, returning `PathBuf::from(program)`
  verbatim. Unreachable in production — `external_cmd::validate_command_setting` accepts only a bare
  allow-listed name or an absolute existing file — but it is a **structural dependency on upstream
  validation**, worth knowing before anyone adds a caller.
- **Truncation is silent** — no ellipsis, so a 512-char-truncated subtitle can read as a complete
  path. Grapheme clusters can also split (base kept, combining mark dropped); no attacker under the
  stated trust model. Routed as a cheap fix.
- **MEASURED, LEFT UNFIXED — a cold first scan can spend the registry budget before using it.**
  `SCAN_REG_BUDGET`'s clock starts at `HostToolEnv::new()`, **before any filesystem work**, and the
  scan is dominated by the PATH walk: **55 directories × 11 `PATHEXT` entries ≈ 4400 stats, measured
  at 2.1 s cold / 0.45 s warm against a 1500 ms budget.** The safety argument holds (all three
  `AppPaths` rows have `WinFolder` siblings) **with a sharp limit: `WinFolder` covers DEFAULT install
  locations only, so an exhausted budget degrades precisely the case `AppPaths` uniquely exists to
  serve** — a tool installed somewhere non-standard is silently not offered. Browse is the workaround
  (and per ruling #26 now accepts UNC). Fix is its own change: start the deadline at the first
  registry call.
- **`BrowsedProgram` is a PARTIAL control — recorded so nobody reads it as stronger.** A newtype
  constructible only inside the settings module **is not expressible here** (`settings` lives in the
  `bonsai` crate, `tools` in `bonsai-core`, and Rust has no cross-crate module privacy), so
  `BrowsedProgram::from_settings_field` is `pub`, with **no `From` / `FromStr` / `Deserialize`** and
  that prohibition documented on the type. **Delivered property: a request-body `&str` no longer
  type-checks into `tool_scan` / `picked`. It does NOT prove origin.**
- **🆕 Contract deltas owed to the architect** on `P112-external-tool-detection.md`: the §2 signatures
  now take `BrowsedProgram`; the §4 example becomes
  `tools::picked(&s.terminal_tool, ToolKind::Terminal, BrowsedProgram::from_settings_field(&s.custom_terminal_path))`;
  AMEND-5 items 1, 2 and 7 are now reflected in code; and the `.cmd`-hit consequence needs stating.

### ❌ VOID — the rule that fmt output on a diff is not a regression

That rule (formerly at `### cargo fmt has never been run on this repo`) is **dead as of `8ad3c72`**.
The tree is now rustfmt-clean and `cargo fmt --all --check` is **gate step 3**, so a fmt failure from
here **is** a real regression. Every earlier board line telling you to discount fmt output is
history, not instruction.

**Superseded measurements, all three kept with their dates:** 1773 hunks / 221 files (original),
2290 (2026-09-14), and **2496 hunks across 484 of 608 Rust files (2026-09-17, the one acted on)**.
Each was true when measured; the tree simply grew. Config is stock rustfmt with only
`edition = "2021"` — the measured choice, since `use_small_heuristics = "Max"` benchmarked **worse**
(2065 vs 1773).

**One rustfmt quirk worth keeping:** rustfmt **reports** a leading blank line in
`crates/bonsai-core/src/git/pr_diff_tests.rs` but does **not** remove it, so `cargo fmt --all` left
the tree one hunk short of clean and the new gate step would have failed on a freshly formatted tree.
Removed by hand. If a future `cargo fmt --all` leaves `--check` red, look for this shape first.

**The superseded section this block replaces — `cargo fmt has never been run on this repo`, named
above — is no longer on the board: it is archive Part 81.2, void in full.**

### 🆕 WORK QUEUE — 20 files the reformat pushed over the 500-line limit

Baseline absorbed them (`5f015be`): **18 → 38** files over limit, **3405 → 4591** excess lines.
**Bookkeeping, not absolution** — rustfmt wraps lines, so these crossed the limit without gaining
any complexity, and they are now genuine split candidates. Presented split per the standing rule
(already-clean vs has-violations, with counts, user chooses):

**Genuinely oversized — 12 files, 510-616 lines:** `tests/worktree_submodule/submodule_wedge_cli.rs`
**616** · `src-tauri/src/commands/tests_diff_search_history.rs` **602** ·
`src/git/cred_cache/tests.rs` **588** · `src-tauri/src/graph_cache/tests.rs` **568** ·
`tests/rebase_merge/essentials_autostash_cli.rs` **567** · `tests/status_stage/branches_cli.rs`
**553** · `src/git/hooks/tests.rs` **543** · `tests/remote/signing_cli.rs` **530** ·
`src-tauri/src/obs/tests_metrics.rs` **522** · `src/tools/scan_tests.rs` **520** ·
`src/git/ai_operation_preview.rs` **519** · `tests/remote/force_push_cli.rs` **516**.

**Marginal — 8 files within 11 lines, will fall back under on any real cleanup:**
`ai_resolve_cli.rs` 513 · `graph_cache.rs` 511 · `bisect/tests.rs` 510 · `detect_tests.rs` 508 ·
`tests_writer.rs` 507 · `hooks_commit_cli.rs` 504 · `gitbin.rs` 504 · `record.rs` 503.

**Only 2 of the 20 are application code** (`ai_operation_preview.rs`, `graph_cache.rs`); the other 18
are test files, where `refactorer` can prove equivalence by identical before/after test counts.
**NOT queued — awaiting the user.**


**STATUS 2026-09-22 — 5 of 20 done, verified with `git show --stat fbf81d0`** ("split five files the
reformat pushed over the limit"): `submodule_wedge_cli.rs` (616), `essentials_autostash_cli.rs`
(567), `branches_cli.rs` (553), `signing_cli.rs` (530) and `force_push_cli.rs` (516) were split into
eleven files, and `scripts/file-size-baseline.json` shrank by 5 entries. **The remaining seven
genuinely-oversized files and all eight marginal ones stand as listed above.** The two application
files (`ai_operation_preview.rs`, `graph_cache.rs`) are both still on the list.

### 🆕 NEW 2026-09-14 — follow-ups both P112 reviews produced, NOT routed

Ranked. None is a MUST-FIX; `reviewer` and `security-auditor` both passed the increment.

1. **The house bidi/control predicate is incomplete, in three identical copies** (LOW-4, and
   **pre-existing — the new module faithfully reused it**): `tools/custom.rs:44-48`,
   `external_cmd.rs:142-144`, `ai/stream.rs:383-385`. The set is `char::is_control()` (Cc only) plus
   U+200E/200F, U+202A-202E, U+2066-2069. **Omitted:** U+061C ARABIC LETTER MARK (a genuine bidi
   control of the same family); U+200B-200D, U+2060, U+FEFF, U+00AD, U+180E (invisible ⇒ look-alike
   paths); and **U+2028/U+2029, which are Zl/Zp — NOT `is_control()` — and render as line breaks in
   a DOM label**, defeating the "a newline cannot appear in a program name" intent outright. Fix once
   as a category predicate (reject Cc + Cf + Zl + Zp) applied at all three sites so they cannot
   drift. Homoglyphs (Cyrillic `с` for `c`) are not strippable by any filter — accepted residual,
   worth one contract sentence. ANSI escapes are already adequate (ESC is Cc, leaving inert `[31m`).
2. **`crates/bonsai-core/src/gitbin.rs` is at EXACTLY 500 lines** — zero headroom; one added line
   trips the ratchet, which also means **comment-only fixes there are blocked**. `refactorer`.
3. **A second Delete click reports deleting a `usage.json` that is already gone.**
   `src/ipc/mock/obsLogFixture.ts:194-199` returns `deletedMetrics: 1` unconditionally and the
   fixture's `logFiles` is never consumed by the delete, so click #2 re-renders a success against an
   emptied fixture. **Pre-existing, not a regression** (the old constants behaved identically), but it
   is exactly the class that file's own header condemns, and §6.4 dropped the `hasLogs` gate so the
   button stays enabled. **If fixed, it must ship with a reset export wired into
   `obsDeleteCounts.test.ts`'s `beforeEach` beside `ringClear()`**, or the test after a
   successful-delete test inherits the flag and goes flaky.
4. **`procutil`/`gitbin` duplication for `refactorer`:** `custom.rs:155` `is_mac_bundle` duplicates
   `HostToolEnv::is_bundle` (`detect.rs:110-112`), and the `mode() & 0o111` check now exists **three**
   times (`custom.rs:160-163`, `detect.rs:120-123`, `gitbin.rs:199-202`).
5. **Mock seam polish** (all NIT, `src/ipc/mock/obsLogFixture.ts`): flag precedence undocumented and
   `?obsMetricsFresh=1&obsDeleteFail=1` self-contradicts (reports a held-open file that `fresh`
   asserts is absent); `DeleteFailMode`'s `'metrics'` member is unreachable by any query value;
   `countFlag`'s doc overstates strictness (`parseInt` makes `3abc` ⇒ 3); three exports
   (`MOCK_USAGE_BYTES`, `MOCK_ROLLED_PART_BYTES`, `MOCK_EXPORT_ZIP_BYTES`) are module-local.
6. **One signed string has no test:** `Deleted 1 export.` (T1 with `logParts === 0, exports === 1`).
   The invariant loop's fixture has `logParts 3`, so it exercises ` and 1 export` instead.
7. **Picker subtitles will show uppercase extensions** — the host scan returns `detail` values like
   `...\wt.EXE` and `...\pwsh.EXE`, because `PATHEXT` entries are uppercase. `ui-designer` polish for
   sub-inc 4, not a backend bug.

**Two `ui-designer` contract amendments owed on `P91-privacy-copy-ui.md`** — the code is right and
the signed text is wrong, so these are text fixes, not code fixes:
- **§6.11.3's T2 template omits `{rolled?}`, but shipped T2 already carried the clause.** Omitting it
  would regress R13c's own complaint. Add it, and add the row the harness now reaches
  (`?obsLogFiles=0`, Dev ON): `Usage counts cleared. 4.0 KiB freed. Still recording — Bonsai started
  a new log file.` — the table's claim to cover "every state the harness and the tests reach" is
  false by exactly that row.
- **AC5 is unsatisfiable as worded.** It says a user with `totalFiles === 0` can produce no string
  containing `log file` and explicitly includes the Dev-ON case — but Dev ON ⇒ `rolled` ⇒ `Bonsai
  started a new log file.`, which R13c requires. The two clauses contradict each other. Reword to
  "no **counted** log file (`{N} log file`)", which is what the implementation asserts.

### 🆕 NEW 2026-09-14 — per-category failure counts need a `purge_scope` split, not just a struct field

§6.11.4's precise failure copy is **conditional** on `failedLogs` / `failedExports` existing (two
fields, not the architect's three — see the F6 entry above). The non-obvious part:
`src-tauri/src/obs/sink.rs:60-67`'s `PurgeCounts`/`PurgeReply` carry **one** `failed_files` across
log parts *and* export zips, so the attribution has to be split **inside `writer::purge_scope`** —
the IPC struct is the last place it surfaces, not where the information is lost. Until it ships,
§6.10 R10's three-row table stays the live spec. Architect's call on the `purge_scope` signature.

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

- **P87b contract-hygiene residue — VERIFIED 2026-09-17 by `architect`, mostly closed** (narrative:
  archive Part 79.2). Closed: §3 ranges (`2aa1e06`) · §4's unborn-HEAD rationale, which now correctly
  says the `if head.unborn || head.detached` guard is **LOAD-BEARING** · §8's seams, for scope · §8's
  `MOCK_LONG_TARGET` · **§8/§9's hostile characters** (`f00fad3`; a codepoint scan of the whole active
  contracts dir found **zero** hits in either P87b file). **Still drifted: §1's counts, 2 of 4** —
  `activity_tests.rs` **285 → 299**, `activity_target_tests.rs` **267 → 273** (`activity.rs` 437 and
  `activity_target.rs` 108 are exact). **Part of that drift is not code growth** — `8ad3c72` wrapped
  lines across 484 files, so *any* line-count claim written before that commit is suspect on
  formatting alone. `P87b-FU1-FU4-git-dock-ui.md` F-F(a) is **still open** — see the hygiene-pass
  filings above for its flipped precondition and the `?? null` trap.
- **STANDING WARNING — do NOT ungate `''` from `hasExecExt` in `whichAll`**
  (`scripts/lib/spawn-tool.mjs`). npm/corepack install **extensionless POSIX shell scripts** beside
  every shim and `resolveTool` picks `hits.find(p => !isBatch(p))`, so an unconditional `''` selects
  an unrunnable script. 3 regression tests guard it.
- **STANDING WARNING — the toast auto-dismiss timer fix is deliberately REVERTED.**
  `React.StrictMode` makes cancel-on-unmount strand a toast on screen permanently;
  `useToastQueue.test.tsx` carries the finding. Reasoning: archive Part 52.4.
- **Deliberately not taken:** `src/obs/types.ts:68` still cites `A26 §D`, the dead lettered scheme
  retired inside `raw_args.rs`. A repo-wide letter→section migration is a decision, not a drive-by.

### P91 — open items (branch merged 2026-09-11; **the owed AI-gate item is CLOSED 2026-09-16**)

Milestone detail: Part 54.6 · security arc 42 · audit F1–F9 43 · build diary 44 · SHOULD-FIX full
text 45 · pre-condensation board text 69.1. User decisions + architectural rulings: see
`## Accepted decisions` above.

- **✅ The owed real `logs/*.jsonl` parse is CLOSED (ruling #16), and so is the `mono` defect it
  found.** The parse (11,848 records, **zero rejections**, zero dropped fields across a re-serialized
  key-set diff, privacy invariants verified on real output for the first time) and the `mono`-is-0
  defect it exposed — **fixed in `88a4004`**, with `OBS_SCHEMA_VERSION` 1 → 2 — are **archive Part
  81.1**.
- **📊 PRODUCT SIGNAL from the same run, not a schema issue — the anomaly detector fired 431 times
  in ~6 minutes:** `render-storm` **423**, `redundant-refresh` **8**, all `severity: warn`. And
  `watcher` records are **87% of the whole log** (10 280). The board already carries the rule that
  the watcher fires event storms and must be debounced (~300 ms); this is the first real measurement
  of what that looks like in a live session, and **423 render-storms is the app complaining about
  itself.** Worth a look before Polish — it is exactly what P91 was built to surface.
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

- **`SEC-2026-09-17` — the forge credential audits (two passes, both REMEDIATED).** Board narratives:
  **archive Part 79.4** (the sibling / MEDIUM-1 audit and B's audit). **Both carry a verified-CLEAN
  register — READ THEM BEFORE RE-AUDITING THIS GROUND.** Between them they establish: fail-closed
  totality is total on the clear-host path; error-string disclosure is clean at source (keyring
  3.6.3's `Display` never interpolates a secret; `ipcProxy.ts:92-96` logs only `err.kind`); the DI
  seams cannot be subverted (`pub(crate)`, the `#[tauri::command]` constructs `Deps::default()`
  unconditionally); record removal is strictly gated on `Ok` from `delete_token`; and
  `migrate_forge_hosts_to_accounts` cannot resurrect a removed account.
  **Their open residue is live under `### Filed 2026-09-17 — the credential arc's open residue`**:
  the `auth::global()` guard, the terminal leftover disclosure, and the record-less orphan half.
  **Two acknowledged gaps, stated rather than glossed:** `Ambiguous` Debug-prints matched credential
  structs, and the auditor did **not** read the platform credential `Debug` impl to confirm it
  excludes the password (unverified, low risk); and on the sibling pass the 9 tests plus the
  `include_str!` mirror were **read, not run**.
- **INFO residue from those audits, both pre-existing:** outcome 3's message carries an absolute home
  path into dialog copy and **reaches no masking sink** — checked rather than assumed
  (`src/obs/ipcProxy.ts:92-97` logs `errCode` only, never `message`; the rejection uses a
  `.then(ok, err)` pair so there is no `unhandledrejection`; no command-layer wrapper logs `AppError`
  Display on this path), so it is local-user-visible text, **not a disclosure** · and `keychain_key`
  derives from a **remote-controlled `login`** (`forge_accounts.rs:228` → `account_id(...)` →
  `keyring::Entry::new`), so a hostile forge API response controls half the keyring target name;
  `map_keyring_err` does not echo the key.
### The external-tool launch surface — SEC-2026-09-14/15 (three audits, all CLEAN at HIGH and above)

- **The three audits:** sub-inc 2 ("unrepresentable, not rejected" **holds**), sub-inc 3 (*"a net
  reduction in attack surface"* — it **deletes** the free-text program capability rather than putting
  a validator in front of it), and the `.cmd` launch change. Board narratives: archive Parts 71.3,
  73.1 and 74.2.
- **THE LAUNCH SURFACE NOW RUNS A BATCH FILE.** With `PATHEXT`-first resolution (`fd93616`),
  `vscode`'s provenance flips **`Registry` → `Path`** and its program becomes `…\bin\code.CMD`
  instead of `Code.exe` — **so Bonsai launches a batch file where it previously launched a PE**,
  through std's case-insensitive batch detection, i.e. the **mitigated CVE-2024-24576 /
  "BatBadBut"** path. Audited CLEAN. **Deliberately not tightened to `.exe`-only:** the Windows
  `idea` row has **only** a `Rung::OnPath` and JetBrains ships `idea.cmd`, so tightening would delete
  a catalog row. The only attacker-influenced argv source is **a crafted filename inside a cloned
  repository**, not `PATH`. MSRV is sufficient and deliberate — `rust-toolchain.toml` pins
  `channel = "1.97"` and the mitigation landed in 1.77.2.
- **THE SINGLE FACT A FUTURE REFACTOR MUST NOT BREAK:** *no code outside `bonsai-core` constructs a
  `PickedTool` literal.* Everything AC6 claims rests on it, and it is enforced by **convention, not
  by the compiler**: `PickedTool` (`tools/mod.rs:119-131`) is `pub` with five `pub` fields and no
  `#[non_exhaustive]`, while `terminal_ladder` (`:357`), `editor_ladder` (`:389`),
  `open_in_terminal` (`:427`) and `open_in_editor` (`:447`) are **all `pub` and all take
  `Option<&PickedTool>`**. Verified: `grep 'PickedTool' src-tauri/src/` returns **exactly one hit**,
  a return type (`commands/external.rs:140`), with **zero field reads**. Fix at the right layer
  (zero-caller): make the five fields `pub(crate)`, or add `#[non_exhaustive]`.
- **`src-tauri/capabilities/default.json` grants NO `fs:` permission** — only `core:default`,
  `dialog:allow-open`, `updater:default`, `process:default`. **Adding any `fs:` write permission
  scoped to the app config dir would defeat P112 entirely without touching a single line of Rust**,
  and it would not show up in any Rust review. Cheapest way to lose the property.
- **`.exe`-only is sufficient and not a heuristic.** A batch file **renamed** to `.exe` is handed to
  `CreateProcess`, which validates the **image header** and fails with **error 193** — it never
  reaches `cmd.exe`. The dialog filter is cosmetic (`custom.rs:262-267`); the gate is
  `custom.rs:268-275`, and `tests_tools_pick.rs:78-109` writes **real** `payload.cmd`/`.bat`/`.ps1`
  files and asserts all three are refused.
- **"No path is ever an argument" holds only for the two NEW commands.** `openInTerminal`,
  `openInEditor` and `revealInFileManager` carry `["path"]` in `rawArgPolicy.json`, so paths **do**
  reach raw-mode logs on the neighbouring external surface (pre-existing). **Pin the dependency this
  rests on:** the pipeline logs **arguments and never result values** — a future change that logged
  result values in raw mode would break the property **without touching either command or the policy
  file**.
- **INFO, on record:** `browsed_tool_row` validates the real `&Path` but stores `to_string_lossy()`.
  For a non-UTF-8 path the stored string differs from the validated one. It **fails closed**
  (re-validated on every launch; `is_file()` false, auto ladder runs), so there is no security
  consequence — but "validated one value, stored another" is a shape worth having on record.

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


> **⚠ Two lines in this area are superseded and are kept for their reasoning, not their status.**
> (1) The `watcher::tests::git_internals_filtered` entry above calls its flake status
> *"undetermined"*; the canonical reading is now `### ✅ watcher::tests::git_internals_filtered —
> SETTLED` earlier in this section (ambient-load sensitivity in a wall-clock negative assertion).
> (2) The `h_ai` bullet below ends *"until then run `h_ai` with `--test-threads=1`"*. **The env race
> it describes is CLOSED** (`80a852e` + `105131a`: six mutexes over one process-global become one).
> What survives is the **process**-concurrency half — 57 concurrent `cmd.exe` + `ping` trees stall
> Windows — and that is handled by the `.config/nextest.toml` `h-ai-stub` group, which stays. **Do
> NOT add `--test-threads=1` to the `gate.mjs:148` fallback.**

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
| `docs/history/todo-archive-2026-09.md` | **Parts 76-82 (moved 2026-09-22, after the user confirmed P112's native checkpoint on 2026-09-22 — `ba4b9d3`):** P112 in full, incl. the five checkpoint items as confirmed and item 4's `set_parent` sub-question (76) · the completed 2026-09-14/15 queue, the superseded `5654eaa`/`934a280` gate states, ruling #24's scope facts, the second-round rulings' evidence blocks and the whole 2026-09-11 ruling queue (77) · the 2026-09-16 session — the real-log investigation, the nine-file second review pass and its closure, the Settings-scrim finding, **P113 phases 1+2** (78) · the 2026-09-17 session — the orphaned-credential arc, **P114**, contract hygiene, the rustfmt pass, three security audits with their two CLEAN registers, and the superseded `ea6d323`/`5f015be`/`3948478` greens (79) · the release block's pre-tightening text (80) · P91's closed `logs/*.jsonl` parse + the `mono` defect it found + the superseded `cargo fmt` section (81) · superseded curator bookkeeping (82). **Parts 71-75 (moved 2026-09-16):** the P112 sub-inc 3/4 build + review transcript incl. the `P112-ui.md` §17 rulings, the four bad citations, the coalescing lesson and the sub-inc-3 audit (71) · superseded gate states (`d0e6cf0`, `dcff54b`, the 427.4s confirming run, the `e9ed93d` Rust tier) and the completed 2026-09-14 queue — F6, P77, the e2e cold-timing measurement, the UNC clearance (72) · the P112 sub-inc 2 + P113 phase-1 review transcript (73) · the two items CLOSED 2026-09-16 with their evidence — "Open in editor" (fixed `fd93616`, with its `os error 193` measurement table) and the false General subtitle — plus the `.cmd` launch-path audit (74) · superseded curator bookkeeping and the duplicated `cargo fmt` measurement (75). **Parts 62-70 (moved 2026-09-14, after the user ruled all 22 FOR-USER items on 2026-09-11):** the stale 2026-09-10 resume block + FU-1 residue (62) · the FOR-USER evidence blocks for items 0-6 (63) · the `IN FLIGHT` queue, the 2026-09-11 orchestrator closures, the unreviewed-MCP-merge warning (64) · **`SEC-2026-09-11`**, the MCP tool-contract audit, with its verified-CLEAN register (65) · **`SEC-2026-09-11b`**, the review of that implementation, with its verified-CLEAN register (66) · P108 `AC11`, closed by ruling #13 (67) · the happy-dom load-flake narrative (68) · the open follow-ups as they stood pre-condensation (69.1 P91 · 69.2 SEC-2026-09-03 through the 2026-09-01 hoisted items · 69.3 P69 Settings) · superseded curator bookkeeping (70). **Parts 54-61 (moved 2026-09-10):** the whole USER-CHECKPOINT block — P102+P105, P106, P107, P108, P91 (54) · P110 + P109 (55) · the 2026-09-03 closures + SEC-2026-09-03 remediation (56) · `Queued housekeeping` incl. the `e149382` CSS-split proof (57) · the superseded `c218258` and earlier gate states (58) · the 2026-09-10 session: P111, FU-1, six reviewer-follow-up closures (59) · the board's record of the confirmation (60) · superseded curator bookkeeping (61). **Parts 51-53 (2026-09-03):** the `5c2dcd2` + `c6cd7dd` gate states and the e2e-contention mis-diagnosis · the full narratives of everything closed 2026-09-03 · the durable-lessons stories and worked numbers. **Parts 36-50 (2026-09-03):** the file-size refactor pass · P102+P105, P106, P107, P108 and the P91 security arc + audit + build diary · superseded pre-ship filings · the 2026-09-03 velocity pass · P99, P100, P101, P98, P95, P96, P97 · built-bundle e2e + P103 + P104 · the DX/velocity stubs · the pre-condensation open-follow-up text. **Parts 33-35 (2026-09-01):** the P84 record gap · macOS ad-hoc signing · the two 2026-08-22 design reviews. **Parts 22-32 (2026-09-01):** P94 · P93+P92 · DEP REFRESH · P90+P89 · P88 · the P85-P87 batch · P82+P83 · divergence reconcile + Release 1.1.0 · the DX dev-loop text · the confirmed-checkpoints block · the 2026-08-21 resolved follow-ups. |
| `docs/history/todo-archive-2026-08.md` | Parts 1-9: P65 to P28 build detail, the Phase 1-4 banners, resolved FOR-USER decisions, P69(1.0.0)/P67/P68 detail. Parts 10-16: the P62-P74 checkpoint waiver + P71-P74, the P69 Settings redesign, the Audit #2 fix batch. Parts 17-18: P70 and P77. Part 19: the follow-ups resolved 2026-08-21, verbatim. Part 20: P78/P79/P80. Part 21: P80b/P81/P82. |
| `docs/history/todo-archive.md` | P27 to P2, M0-M6 |
| `docs/history/milestones-mvp.md` | the M0-M6 AI-gate vs USER CHECKPOINT split |
| `docs/history/context-pollution-audit.md` | the context/token-cost audit |
| `docs/history/velocity-2026-09-01.md` | gate wall-clock, test-suite hotspots, inner-loop rebuild cost, ceremony-vs-machine-time split (2026-09-01) |
| `docs/contracts/INDEX.md` | one line per contract file — milestone, scope, status |

Move a milestone's section into the current dated archive file only once **both** halves of its gate
have passed (or the native half is explicitly waived). A milestone with a pending USER CHECKPOINT
stays on this board. **An owed AI-gate item also keeps its entry here** — both live examples are now
closed (P91's `logs/*.jsonl` parse on 2026-09-16, P108's `AC11` by user ruling on 2026-09-11), which
is why P91 and P112 could finally leave.

### Why this board is ~1810 lines, not ~300 (curator note, 2026-09-22)

**3361 → 1810, a 46% cut.** **2425 lines were extracted verbatim into archive Parts 76-82 across 17
ranges, and every range was diffed byte-identical against `git show HEAD:TODO.md` before removal and
then re-verified in place inside `todo-archive-2026-09.md` after the append — 17 of 17, twice.** A
final whole-file check confirmed that **every non-blank, non-`---` line of `git show HEAD:TODO.md`
still exists** either on this board or in the archive: zero lines unaccounted for. Nothing was
summarized away and **no status was upgraded by the curator.**

**The release block was tightened, not archived: 269 → 229 lines**, with the verbatim pre-tightening
text kept as **archive Part 80**. Every number, SHA, path and caveat survived — which is why the cut
is only 40 lines; that block is almost entirely evidence.

**What was closed this pass, each verified against the tree first, never from a commit subject
alone:** P112 (the user's own 2026-09-22 attestation — the only thing that could) · the
`CLAUDE.md` audit-trigger decision (`3f78d50`, and `CLAUDE.md` carries the text today) · the two
INFO user decisions of 2026-09-17 (`871d16a`) · the `ui-designer` copy pass (`61af79b` = P114) · the
P113 contract debt (`2233cc0`) · audit MEDIUM-2 (`ea6d323`) · the `h_ai` env race (`80a852e` +
`105131a`) · the U+200B in `P107-F2-copy-chip-ui.md` (codepoint grep, zero hits).

**What was REFUSED, and why.** The live release block and its **two** remaining USER ACTIONS (macOS
ad-hoc signature verify-on-tag — ruling #17 — and the Dependabot moderate, ruling #15) ·
**A4 finding 6**, a diagnostic regression with no
commit that closes it · the three AWAITING-USER credential items · the open `render-storm` threshold
decision · the Dependabot user action · **the 20-file split queue, which the task listed as possibly
complete and is not** — `fbf81d0` did 5 of 20 · every other open follow-up · all four ruling ledgers
· the durable rules · the accepted decisions · the cross-platform-gap finding.

**Composition of what is left, measured with `awk` over the finished file:** header + conventions +
navigation **78** · the release block **229** · P112's closure record + its live follow-ups **63** ·
the queue/verification summary **41** · **the four ruling ledgers, verbatim and authoritative, 187**
(the 17, the two of 2026-09-14, the four of 2026-09-11, the 2026-09-17 credential set, plus the
security auditor's do-not-re-audit register) · durable rules incl. the ten earned 2026-09-16/17
**214** · accepted decisions **120** · **open follow-ups 803** · the ruling-queue closure line **7** ·
archive table + this note **68**.

**What sets the floor is still one number: 803, the open-follow-up backlog.** It is below the 845 the
2026-09-16 note measured despite two sessions filing ~60 new items, because this pass condensed those
findings to one line each with their `file:line` citations intact. Every line of it is an unresolved
item, and **working the backlog down is the only thing that moves the number; curating cannot.** The
four must-survive blocks — ledgers **187**, rules **214**, accepted decisions **120** and the release
evidence **229** — are **750** lines on their own, so ~300 is unreachable by curation at all.

**The one structural option, unchanged from the last two notes and still not a curator's call:** give
the durable rules their own file under `docs/` and leave a pointer here. That trades 214 board lines
for one more hop on the session's most load-bearing content. **User/orchestrator decision.**
