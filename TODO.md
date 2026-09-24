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

**⚠️ ONE LIVE REFERENCE IS NO LONGER ON THIS BOARD: `docs/durable-rules.md`.** The "durable lessons"
block moved there on **2026-09-23** by explicit user decision — 211 lines, byte-identical. It is
**not** an archive file: read it before asserting that anything is tested, measured, covered,
closed or green. Pointer section is still in place further down.

**2026-09-23 pass — Parts 83-87, plus the durable-rules move.** Archived: **P116 in full** (`done`,
`e49cf20`, and its own text says "No USER CHECKPOINT applies" — the only cleanly archivable
milestone on the board) (**83**) · P117's pre-tightening text (**84**) · P115's pre-tightening text
(**85**) · the open follow-up entries rewritten by a staleness sweep (**86**) · superseded curator
bookkeeping (**87**). **Three follow-ups were CLOSED, each verified against the current tree and
never from a commit subject:** the cross-repo detector keying (P117 inc 2, `83bbdf3`) · the
`RemotesSection` local-branch re-render (P118b, `c6ae304`) · the gate's `DEP0190` / `shell: true`
argv concatenation (zero live uses in `scripts/`). Six more were re-measured and **left open**
because the tree says they are still live.

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

**Not archived, deliberately:** the live release block · the **two** remaining USER ACTIONS — the
macOS ad-hoc signature, verify-on-tag (ruling #17) and the Dependabot moderate (ruling #15, which
the release block argues a merge to `main` should clear) · **P113** (`in-progress`, USER CHECKPOINT
owed) · **P115** (`in-progress`, a Linux/macOS run owed) · **P117** (`awaiting USER CHECKPOINT`) ·
**P118/P118b** (`reviewer approved` — which is not `done`, and the orchestrator does not
self-declare a checkpoint half) · **every open follow-up** · all four ruling ledgers · the accepted
decisions · the cross-platform-gap finding · both security CLEAN registers' pointers · and
**A4 finding 6** — a diagnostic regression with no commit that closes it.

**Order of operations, mandatory since `c5b3ea5`.** That pass cut 1950 lines from this file and wrote
them **nowhere**; `67e2ce6` had to restore them wholesale. **Extract → diff byte-identical against
`git show HEAD:TODO.md` → only then remove → leave a Part pointer.** 2026-09-23: **15 ranges, 936
lines, all 15 verified byte-identical in the destination after the append** (14 in
`todo-archive-2026-09.md`, 1 in `docs/durable-rules.md`), plus the whole-file check. 2026-09-22:
**17 ranges, 2425 lines, all 17 verified**, plus a whole-file check that every non-blank line of
`git show HEAD:TODO.md` still exists in the board or the archive. 2026-09-16: 20 ranges, 935 lines,
all 20 verified.

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

## P118 — sidebar render-storm from refresh-round fan-out — `reviewer approved`

**Current step:** P118 committed `8790b62`. **P118b (the RemotesSection residual) — `reviewer`
approve, no MUST-FIX; committed `c6ae304`.** See the P118b block below for the measured floor.

> **Renumbered P114 → P118 before commit.** The orchestrator's brief invented "P114", which was
> already taken by the shipped forge credential-failure copy work
> (`docs/contracts/P114-forge-failure-copy-ui.md`, commit `61af79b`, `TODO.md:2130`). The reviewer
> caught it with ~15 code citations already pointing at the ambiguous ID. P115–P117 are held by the
> three concurrent background increments, so this is P118.

Target: the 24 non-Tags `render-storm` anomalies from the 2026-09-22 Dev session — RemotesSection 9,
BranchesSection 8, BranchRow 7 (tallies read `100 renders vs 25 instances`, a 4x multiplier). The
other 37 were TagsSection and were handled by P113b. Almost all correlate with
`refresh:{remoteMeta,stash,worktree,full}` at origin **`mutation`**.

**Mechanism — settled on the THIRD diagnosis. The first two were wrong; do not resurrect them.**

1. ~~The task brief guessed `refreshAll` awaits its slices sequentially.~~ It does not —
   `runRefreshRound` (`RepoWorkspace.tsx:843-887`) has exactly one `await Promise.all`.
2. ~~The orchestrator then modelled it as "1 loading commit + 3 completion commits = 4".~~ **Also
   wrong.** The 4x is **StrictMode double-rendering** — `obs/react.ts` documents that `renders`
   doubles while `instances` does not, so `100 renders / 25 instances` is **2 real renders per
   row**, not 4. The commit model matched the number by coincidence. The log histogram settles it:
   BranchRow tallies are 50/25, 100/25, 140/70, 280/70 — a clean 2x (fine) / 4x (storm) split.
3. **Actual cause: the `busy` / `actionsDisabled` boolean prop.** A mutation runs
   `setMutating(true)` → git call → `await refreshAll(...)` → `setMutating(false)` in a `finally` —
   **two flips inside one 500 ms tally window**, reaching both memoized sections and every
   `BranchRow`. On a row `busy` has *zero* visual effect (it only gates the checkout gesture), so
   every one of those renders was pure waste. Exactly 2 real renders per instance, matching every
   observed storm.

Ruled out with evidence, not assumption: `rowPropsEqual` bails correctly, and
`useSidebarCallbacks` / `contextMenuOpeners` / `handleReveal` are all ref-latched and stable — a
churning callback would have produced 8x, not 4x.

**Fix.** Split the flag in two: `sidebarBusyContext.ts` provides `SidebarBusyRefContext` (a stable
`useRef` box for event-time readers — consuming it re-renders nothing) and `SidebarBusyContext`
(the boolean, for controls that actually render `disabled`). `BranchRow` drops its `busy` prop and
reads the ref at event time; `BranchesSection`/`RemotesSection` drop `actionsDisabled`. Three
components extracted (`SidebarActionButton`, `BranchCreateRow`, and `StashesSection` purely to
satisfy the size ratchet — `Sidebar.tsx` 507 → 469).

**The fan-out (a) is real but does not cause these storms.** It produces extra commits of
`RepoWorkspace`/`Sidebar`, which are `each`-mode and not subject to the `render-storm` rule. It only
reaches a tallied component when that component's own IPC props change identity — and `status`,
`branches`, `opState`, `stashes`, `submodules`, `worktrees` and `remotes` all already store through
`keepIfUnchanged`. **The 11-callback inversion of `RepoWorkspace.tsx` is therefore NOT worth doing
for this acceptance criterion.**

**Residual — RemotesSection is not fully covered, but it is NOT a design-pass problem.** The
implementer framed this as needing the container state-model pass; the reviewer showed that
**two of the three couplings are one-liners**, and the design-pass framing only applies after
those:

- `RemotesSection.tsx:14` takes the whole `data: BranchesSnapshot` but reads it exactly once —
  `data.remote.length === 0` at `:143`. A `hasRemoteRefs: boolean` prop removes the coupling.
- `Sidebar.tsx:216-219` — `remoteTree`'s deps are `[treeMode, data]`, **not** `[treeMode,
  data?.remote]`. Compare `remoteFlatFiltered` at `:252-255`, which narrows correctly. So any
  *local*-branch change remints `remoteTree` → `remoteTreeFiltered` → re-renders RemotesSection.
  Flat mode is worse: the `: []` branch mints a fresh array literal every recompute.
- **The new test bakes this in:** its round changes only a local branch's ahead count, yet
  `Sidebar.busyChurn.test.tsx:227` asserts `RemotesSection {renders: 2}` — one avoidable real
  render, asserted as correct.

Only after both narrowings is the remainder the genuine `data.remote`-vs-`remotes`
two-continuation fan-out. The 9-vs-8 anomaly asymmetry explanation survives either way.

### P118b — the RemotesSection residual, closed — `reviewer approved`

Both narrowings above landed as specified, **and they were correct but insufficient** — measured,
not assumed. The floor after them alone is still `RemotesSection {renders: 2, instances: 1}`,
i.e. one real render, unchanged from before.

**Why the narrowings alone cannot work.** `refetchBranches` stores through
`keepIfUnchanged(setBranches, snapshot)` at **whole-snapshot** granularity
(`RepoWorkspace.tsx:797`), so any LOCAL-branch change replaces the snapshot and hands `data.remote`
a **fresh array identity even though `origin/*` is byte-identical**. Narrowing the dep to
`data?.remote` narrows *which* changes propagate, but that identity churns every round anyway —
and `filterItems`/`filterTree` return their input BY IDENTITY on a blank query
(`repoWorkspace/listFilter.ts`), so the churn passes straight through `remoteFlatFiltered` to the
section. The brief assumed `data.remote` is identity-stable when only local branches change; it is
not.

**Third change, the one that actually reaches zero:** `sidebar/useStableRemoteRefs.ts` — a
`useRef` + `structuralEqual` identity cache over `data?.remote`, with all five consumers
(`remoteTree`, `remoteFlatFiltered`, `showRemoteFilter`'s count, `remoteNoMatch` transitively, and
the new `hasRemoteRefs` prop) rewired to it. Pure identity cache: a discarded StrictMode/concurrent
render can only churn identity, never change the value a committed render observes.

**This is NOT the deferred container state-model pass, in either direction.** That one is about
`data.remote` and `remotes` being two IPC-derived families that commit in separate continuations,
so a round genuinely changing both (a fetch moving tips *and* ahead/behind) still costs 2 real
renders. The identity cache addresses the complementary case — `data.remote` NOT changing but
churning identity. `RepoWorkspace.tsx` is untouched; the deferred item stands.

**Measured, this session.** `Sidebar.busyChurn.test.tsx:241` tightened from
`toEqual({renders: 2, instances: 1})` to `toBeUndefined()` — an ABSENT `render.tally` record IS the
zero-render signal, because `obs/renderTally.ts` only creates a bucket when a component renders.
Mutation-checked by the orchestrator: reverting `Sidebar.tsx:228` to raw `data?.remote ?? []` fails
with `expected { renders: 2, instances: 1 } to be undefined`, so the assertion has teeth and the
stage-1 floor is confirmed independently. Not vacuous: `containerRenders === 6` and
`BranchesSection {renders: 2}` above it prove the round happened.
`Sidebar.churn.test.tsx`'s independent 500-ref fixture went 8 renders/4 tallies → **6 renders/3
tallies**, so `MAX_TALLY_RENDERS` was lowered 8 → 6 per that file's own only-lower rule (its
`2 * MAX` negative control was failing at 12-vs-16) and `RemotesSection` joined `MUST_NOT_REPORT`.
Checks: vitest 28/28 across the three sidebar files, `tsc --noEmit` exit 0, eslint clean,
`lint:size` OK (`Sidebar.tsx` 482).

**`localTree`'s identical `: []` was deliberately left alone** — `BranchesSection` still takes
`data={data}` whole, so it re-renders on any snapshot identity change regardless; the fallback is
not the binding identity there and fixing it alone buys zero renders. It belongs with the
BranchesSection data-prop pass. A comment at `Sidebar.tsx:204-210` says so in place.

**Follow-ups filed, not blocking (reviewer SHOULD-FIX / NIT):**

- `Sidebar.busyChurn.test.tsx:174-184` — **pre-existing flake, confirmed mechanism.**
  `flushRenderTally` (`obs/renderTally.ts:69-71`) nulls `timer` without `clearTimeout`, and the
  500 ms window is a real timer, so a slow cold run can flush mid-round; `talliesByComponent` then
  `Map.set`s and the later record **overwrites** the earlier instead of summing. Exposes
  `BranchesSection :229`, `BranchRow :244`, and the CONTROL's `instances === 25` at `:269` — NOT
  the new `toBeUndefined()`, which is strictly more flake-robust than what it replaced (a split
  flush cannot invent a record for a component that rendered zero times). Cheapest robust fix:
  have `talliesByComponent` throw on a duplicate component key so a split window fails loudly.
- `useStableRemoteRefs` is really `useStructurallyStable<T>(value, fallback)`; `data.local` will
  want the identical hook in the BranchesSection pass. Generalise there, don't copy-paste.
- `Sidebar.test.tsx:80-90` covers only one arm of its own title — the configured-remote-but-no-
  tracking-refs arm (⇒ **no** "No remotes") is untested.

Also checked and unrelated: `refetchGraph`'s unconditional `setGraphLoading(true)` (`:765`) reaches
only `WorkspaceToolbar`, never the sidebar.

## P115 — stale repo paths in settings.json: prune + forge-override migration — `in-progress`

**Current step:** P115 — **code committed `bbd8993`** (contract now rev 2.1, 722 lines).
Both a code re-review and a security re-audit returned **approve / zero MUST-FIX**, the audit
confirming **F1, F3, F4 closed** and ack-safety not regressed by the phase-split rewrite.
**Full gate GREEN over a tree containing this commit** (9/9, 574.5s, exit 0 — pasted below), plus
`cargo deny` and `pnpm audit` run separately because the bare gate does not cover them.
**Still owed before `done`: a Linux/macOS run** — gate gap 2 below.
No USER CHECKPOINT is owed beyond the orchestrator's own run: nothing user-visible changes, and
the effect (five stale recents gone, the pin moved) is verifiable from `settings.json` itself.

**Verified in-session before committing** (not from a subagent's summary, per the new `CLAUDE.md`
rule): `cargo test -p bonsai --lib settings::prune` → **43 passed, 0 failed**;
`cargo clippy -p bonsai --lib --all-targets -- -D warnings` clean; `cargo fmt --check` clean;
`check-file-size` OK.

**✅ GATE GAP 1 — CLOSED.** The P117 session committed `83bbdf3`, the tree went static, and the
full gate ran clean over a tree containing `bbd8993`. Pasted, not paraphrased:

```
════ gate summary — pre-commit — 574.5s total ════
  ✓   200.9s  [rust] cargo nextest
  ✓     8.6s  [rust] cargo test --doc
  ✓     4.4s  [rust] cargo fmt --check
  ✓     2.7s  [rust] cargo clippy
  ✓    17.7s  [frontend] eslint
  ✓   972ms   [frontend] file-size ratchet
  ✓    78.4s  [frontend] vitest
  ✓    22.7s  [frontend] tsc + vite build
  ✓   238.1s  [e2e] playwright e2e
✓ all 9 steps passed                                         [exited with code 0]
```

**Advisories run SEPARATELY, and deliberately so — bare `pnpm gate` does NOT cover them.**
`cargo deny --all-features check` → `advisories ok, bans ok, licenses ok, sources ok` (two benign
`license-not-encountered` warnings for unmatched `deny.toml` allowances);
`pnpm audit --audit-level high` → `No known vulnerabilities found`.

> **⚠️ OWED TO THE USER — `CLAUDE.md`'s gate rule is factually wrong. THIS IS THE CANONICAL COPY**
> (P117 raised it independently; its version is archive Part 84).
> The rule states that `pnpm gate` "already runs the `rust`, `frontend` and `audit` groups". It does
> not: `scripts/gate.mjs:79` is `const wantAudit = full && !rustOnly && !frontOnly`, `:15`
> documents bare `pnpm gate` as "rust + frontend + e2e", `:17` documents `--full` as "gate +
> supply-chain audit + coverage", `:215` gates the audit group behind `wantAudit`, and `:206`/`:207`
> are the `cargo deny` / `pnpm audit` steps bare `pnpm gate` cannot reach. The gate summary above is
> self-confirming — nine steps, three groups, no audit group. Since the same `CLAUDE.md` block
> calls advisories a **red gate**, anyone following it literally would run the bare gate and believe
> advisories were covered. Fix: say `--full`, or pair the bare gate with the two commands above.
> **Neither session edited `CLAUDE.md`** — a peer flagging a defect is not authorization to change
> an instruction file. **Re-confirmed at source by `docs-curator` 2026-09-23** (`gate.mjs:79`
> unchanged; `CLAUDE.md` still carries the wrong sentence).

**⚠️ GATE GAP 2 — STILL OWED.**
2. **The `#[cfg(unix)]` legs have never compiled or run on this machine.** They cover F4's
   case-fold fix and the `\`-is-a-filename-character leg — i.e. exactly the platform where that
   bug lives. Verified by construction + a simulated-Unix negative control (below); **execution
   proof is owed to Linux/macOS CI.**

**Negative control, run not asserted.** The macOS rule-(4) bug cannot manifest on Windows
(`norm` already folds case there), so `senior-dev` simulated a Unix build — made `norm`
case-sensitive unconditionally, reverted the fallback to `==` — and ran the new test alone:
`rule_four_blocks_a_case_variant_of_the_candidates_path` **FAILED**, `overrides_migrated` left 1
right 0 (the dead pin landing on an already-pinned repo). Files restored and independently
re-verified by the orchestrator (`prune.rs:281`, `prune_gate.rs:159`, no stray backups).
**Process note, kept as a rule:** that experiment temporarily mutated a shared working tree and a
peer's `cargo` run during the window sees phantom breakage. Disclosed after the fact. The
instruction gap: "don't touch files outside your surface" does not forbid temporarily mutating
files *inside* it.

**Rev 2 — what the audit changed (all in §4, none in §1's policy verdicts).**
- **F1 (MEDIUM, blocking) — rule (6), positive forge identity.** Basename-only migration could
  destroy the binding P115 exists to preserve. A dead pin on `…\work\api` (a github.com account)
  migrating onto a live `…\personal\api` whose origin is gitlab.com yields a pin that (a) never
  resolves — `forge_accounts.rs:33-40` host-filters *before* the `account_id` find, so it falls
  through; (b) is **unremovable**, because both clearing affordances
  (`ForgeAccountSwitcher.tsx:100-104`, `:124-131`) are gated on `accountSource === 'override'`,
  which it can never produce, and no Settings pane lists overrides; (c) is **permanent**, because
  the override now names a Live path so rule (1) blocks it forever. Rule (6) requires the live
  candidate's own forge host to equal the pinned account's host. Empty host (unparseable origin,
  `bonsai-forge/src/lib.rs:85-92`) blocks, never wildcards. Resolver runs in lock-free phase 1,
  lazy-gated so a clean launch opens zero repos (AC22).
- **F4 (blocking) — `norm`'s case fold is now `#[cfg(windows)]`.** Rev 1 lowercased
  unconditionally; on a case-sensitive FS `/x/api` and `/x/Api` collapse to one key, and since
  `collect_paths` keeps the first-seen string a dead twin's verdict lands on a **live** repo.
- **F3 — comparison split: dead side = `norm`, live side = canonical.** Rev 1's claim that every
  comparison has a dead side is false for rule (4); `read_repo_info` stores the raw opened path
  (`git/repo.rs:43,64`), so two strings for one directory are routine.
- **INFO-2** — `classify_all` probes distinct roots first and short-circuits under a dead root,
  collapsing N dead-UNC entries to one slow probe.

**Accepted residual, on the record:** two same-host repos sharing a basename can still
mis-migrate. Post-rule-6 that pin *does* resolve, so it shows "Pinned to this repo" and one unpin
clears it — but nothing distinguishes a machine-made pin from a user-set one. Closer is a
`pinnedBy: "user" | "migration"` marker, logged as a UI follow-up in §10, not specified.

**Audit returned CLEAN on:** ack safety (no code path can migrate or add an ack), TOCTOU, log
redaction (counts only, never paths), and malformed-`settings.json` robustness.

**Evidence (a real user's `%APPDATA%\com.bonsai.app\settings.json`, 2026-09-22).** The directory
`D:\Repos` no longer exists — the repos moved to `D:\Data\Repos` — yet the file still carries
5 `recentRepos` entries under `D:\Repos\*`, a `repoForgeOverrides` entry for
`D:\Repos\ham-digi-backend` (DEAD: the path is gone, and the repo's live path
`D:\Data\Repos\ham-digi-backend` has **no** override — so the user silently lost their per-repo
forge account binding), and `hooksAckRepos` carrying both the old and the new path for two repos.

**NOT a bug, do not "fix":** the one forward-slash recents entry (`D:/Repos/.worktrees/...`).
`record_recent` dedups through `same_repo_path` (`settings.rs:449`), a canonicalizing compare —
the slash difference is cosmetic and already dedups correctly.

**Goal.** Prune `recentRepos` entries whose path is gone, and decide + implement what happens to
`repoForgeOverrides` / `hooksAckRepos` entries pointing at a vanished path. Two asymmetric risks
drive the policy: dropping a `hooksAckRepos` entry re-prompts a security disclosure, and silently
dropping a forge override is exactly what produced the invisible breakage above. A path can also be
*temporarily* absent (unplugged drive, unmounted share), so the prune must gate on something
stronger than one failed `exists()`.

**P115 follow-ups — none blocking, all filed rather than silently dropped.**
- **`pinnedBy: "user" | "migration"`** — the closer for the accepted F2 residual (two same-host
  repos sharing a basename can still mis-migrate). Post-rule-6 such a pin *resolves*, so it shows
  "Pinned to this repo" and one unpin clears it, but nothing distinguishes a machine-made pin from
  a user-set one. **UI increment — `ui-designer` first.** Contract §10.
- **Surfacing kept-dead overrides** — a dead pin that is kept but not migrated (ambiguous
  basename) is the one case where the user still learns nothing. It is retention, not loss.
  Also a UI increment; contract §10.
- **`settings.rs` is at EXACTLY 500 lines** — the ratchet is `> 500`, so the next line added there
  by anyone trips it. Fix is splitting `record_recent` + the hooks-ack helpers into
  `settings/recents.rs` (`refactorer`). Deliberately not done under three-way tree concurrency.
- **Uncapped lists are re-probed every launch** — `hooks_ack_repos`, `repo_forge_overrides` and
  `open_repos` have no cap, and every distinct path is probed at each launch. Roots-first
  collapses N dead entries under one dead share to a single timeout, but the unbounded growth
  itself is untouched. Contract §10.
- **`recents-changed` event** — the frontend's first `getRecentRepos` can race the pass and show
  the unpruned list once, for one launch. Taking it would make P115 an IPC change; deliberately
  deferred. Contract §10.

## P116 — `OBS_SCHEMA_VERSION` TS/Rust drift + parity test — `done` → **archive Part 83**

**ARCHIVED 2026-09-23.** `done`, `reviewer` approved (no MUST-FIX), committed **`e49cf20`**, and its
own text states **"No USER CHECKPOINT applies"** — nothing user-visible changed, so there was no
native half to clear. That made it the only cleanly archivable milestone on this board; P113, P115,
P117 and P118/P118b all stayed. Full text: `docs/history/todo-archive-2026-09.md` **Part 83**.

**The two things it left live:** the guard it added (`src-tauri/src/obs/tests_schema_parity.rs`) is
what P117 inc 2's 2 → 3 bump had to move — see the P117 section; and its reviewer follow-up about
`docs/contracts/P91-observability.md` is folded into the one canonical contract-drift entry under
`### Filed 2026-09-16 — the real-log residue`, re-measured 2026-09-23.

## P117 — two 2026-09-22 perf signals: graph-cache wipe + repo-blind anomaly rules — `awaiting USER CHECKPOINT`

**Current step:** P117 — contract signed (`docs/contracts/P117-perf-signal-fixes.md`, **617 lines**
as re-measured 2026-09-23; the 583 figure predates `91d9377`, which corrected four wrong statements
in it — two standalone increments). **Both increments implemented, reviewed, audited, gated and committed —
`d63a571` (inc 1) and `83bbdf3` (inc 2). AI gate PASSED. Status is `awaiting USER CHECKPOINT`,
which the orchestrator does not self-declare.**

**AI gate — the actual printed output, run in this session (CLAUDE.md's paste-don't-paraphrase
rule):**

```
════ gate summary — pre-commit — 561.2s total ════
  ✓   257.9s  [rust] cargo nextest
  ✓     4.7s  [rust] cargo test --doc
  ✓     2.1s  [rust] cargo fmt --check
  ✓    18.6s  [rust] cargo clippy
  ✓    13.4s  [frontend] eslint
  ✓   759ms   [frontend] file-size ratchet
  ✓    66.3s  [frontend] vitest
  ✓    12.4s  [frontend] tsc + vite build
  ✓   185.0s  [e2e] playwright e2e
✓ all 9 steps passed
```

Plus the **audit** group, run separately (see the CLAUDE.md correction below):
`cargo deny --all-features check` → `advisories ok, bans ok, licenses ok, sources ok` (two benign
`license-not-encountered` warnings for unmatched `deny.toml` allowances, not failures);
`pnpm audit --audit-level high` → `No known vulnerabilities found`.

**⚠ CORRECTION OWED TO `CLAUDE.md` ITSELF — raised independently by this session and by P115;
neither edited the file. The canonical write-up is in the P115 section above** (it carries the more
precise citation, `gate.mjs:79`). Two independent nine-step gate runs over a tree containing **both**
`bbd8993` and `83bbdf3` printed three groups and no audit group, so the claim is disproved twice by
the tool's own output. **Worth keeping from this session's version:** a peer flagging a defect is
not authorization to edit an instruction file, even when both agents agree on the facts — and the
rule *worked while being wrong about the mechanism*, because its intent is what made both sessions
run `cargo deny` separately instead of assuming coverage. A wrong sentence inside a right rule.
Pre-tightening text: archive Part 84.

**⚠️ OWED, and stated as owed: a platform-gated test leg in inc 1 has never compiled on Linux.**
`commands/tests_repo_graph_cache_rearm.rs:228` is `#[cfg(any(windows, target_os = "macos"))]` and
covers the case-insensitive-FS complement of `distinct_canonical_path_gets_a_fresh_slot` — per the
inc-1 review **the one place** where "cache carried while `entry.path` is overwritten with a
different path string" is checked end to end. **On Linux that leg vanishes and the test still
passes** (its fresh-slot half uses a second distinct repo), so the coverage loss is silent. Owed to
CI, exactly as P115 owes its `#[cfg(unix)]` case-fold legs; both bear on the v1 "runs on Windows,
macOS, and Linux" definition of done. Inc 2 has **no** platform-gated tests (grepped).

**USER CHECKPOINT (pending — both items need the native app and real repos; neither is
AI-verifiable):**
1. **Inc 1 — the cache actually retains across real refreshes.** Run `pnpm tauri dev`, open the
   same ~5 repos, work normally for a while (let `full` rounds fire via focus/activation), then
   read `%APPDATA%\com.bonsai.app\metrics\usage.json` for the day. **The unconditional signal is
   `perf.graph_cache_hits` rising sharply against the `cmd.streamGraph` count** (was 6 of 81 in the
   session, 6 of 103 across the day).

   **CORRECTED — an earlier version of this item said a still-zero `perf.graph_redecorates` "would
   mean the fix did not take". That is false, and would have sent the checkpoint chasing a phantom
   regression.** `HitRedecorate` requires a ref to move onto an **already-walked** oid — a branch
   created at an existing commit, or a checkout between existing branches (`classify`:
   `c.tips.is_subset(tips)` AND `tips ⊆ c.node_oids`). A session of only new commits and fetches
   produces **new** oids, which are legitimate Misses, so redecorates can correctly stay 0 with the
   fix working perfectly. Make it the **conditional** check instead: *if* you switched branches or
   created a branch at an existing commit at least once and redecorates is still 0, then the fix
   did not take.
2. **Inc 2 — the rules stop firing cross-repo.** Same session, then grep the new
   `logs/*.jsonl` for `"rule":"redundant-refresh"` and `"rule":"cache-collapse"`. Expect the
   count to fall substantially (the estimate was ~17 of 26 firings were cross-repo), and expect
   every surviving firing's records to share one `repo` ordinal. Note the log is **v3** now, so
   records carry `"repo":"repo#N"` in strict mode — if a raw path appears there instead, that is a
   redaction regression and is the one thing worth stopping for.

Why these cannot be AI-verified: the defect was only visible in a real multi-repo session with the
real `notify` watcher and real focus events, none of which the mock harness or the e2e suite
reproduces. That is exactly how it was found. (`TODO.md` and `docs/contracts/INDEX.md` were
deliberately left out of `d63a571` — they carried in-flight P114/P115 hunks from concurrent
sessions.)

**Increment 2 (obs repo dimension) — `reviewer` APPROVED, no MUST-FIX; `security-auditor` CLEAN;
committed `83bbdf3`.** Reviewer independently re-ran `cargo test --lib obs::` **243 passed / 0
failed / 1 ignored**, `tsc` clean, `vitest src/obs src/components/repoWorkspace` **678/678 in 64
files**, ratchet OK, and verified every freeze by diff (`git diff d63a571 -- src-tauri/src/watcher`
**empty**; `DEBOUNCE` still 300 ms; the pinned `redactionNote` at `record.rs:74-91` untouched;
`dup-ipc` only destructures the tuple). Both size-forced splits confirmed behaviour-preserving —
`tests_anomaly_slow.rs` is **byte-unchanged** and still drives the relocated `cache-collapse`
through the `None` bucket.

**Two SHOULD-FIX items were accepted rather than deferred** (deliberate deviation from velocity
mode: ~10 lines, and one is a test asserting on an input that cannot occur). Both shipped as fixes
(1) and (2) of the follow-up pass below. Keep the reasoning: **(a)** the private name map in
`repoArg.ts` was chosen **over** `rawArgPolicy.json` rows because rows would also start emitting
`args:{repoId}` for fetch/pull/push in raw mode — a real, if benign, privacy widening; **(b)**
`"clone_repo"` **cannot occur** — the wire `cmd` is `cloneRepo` and never matches `is_mutation_cmd`,
so the fixture was fictional. Pre-tightening text: archive Part 84.

**Inc-2 follow-up pass (5 fixes) — landed, targeted gates green.** `cargo test --lib obs::`
**248 passed / 0 failed / 1 ignored**; `vitest src/obs src/components/repoWorkspace` **684 passed /
64 files**; check/clippy/fmt/eslint/tsc/ratchet all clean. (1) `REPO_PARAM_FALLBACK` in
`src/obs/repoArg.ts` — a private name map for the 10 unattributed mutations, **not**
`rawArgPolicy.json` rows, so raw-mode `args` exposure is not widened to buy a signal fix; all 10
verified `repoId`-at-position-0 against `ipc-api.ts`. (2) the fictional `"clone_repo"` fixture →
`"fetch"`. (3) `is_allowed_scalar` now gates `repo` in `raw_args::enforce`, placed **before** the
`contains_key(ARGS_KEY)` early return — span and refresh records carry no `args` and would
otherwise have skipped the gate entirely. (4) second-consumer note + two guards. (5) the UI-sourced
strict assertion now reuses the same fragment loop as the Rust-sourced one, plus an
`ipc.call`-through-the-writer case.

**REAL BUG found in passing and fixed — `repoIdArg('constructor', …)` threw a `TypeError`.**
`POLICY_LOOKUP` is a plain object, so prototype keys (`constructor`, `toString`, …) resolved to
**functions** and `row.indexOf` blew up — inside the proxy that wraps **every** user-facing IPC
call. Introduced by inc 2, caught only because the follow-up pass wrote a negative control for
commands deliberately absent from the fallback map. Fixed with `Array.isArray(row)` + a test.
**Follow-up:** `buildRawArgs` in `rawArgPolicy.ts` has the identical shape but degrades safely
(`row[i]` → `undefined` → skipped) — worth the same guard, since "degrades safely today" is how
this one started.

**Third instance of a spec being too literal to implement (a pattern now, not bad luck).** After
AC1-4 and §2.4's `clone`/`init` example, the orchestrator's own wording for fix 4 — "every `IpcApi`
method that declares a `repoId` parameter has a non-null policy entry" — is unimplementable:
**157** methods declare `repoId` at position 0 and **57 have no policy row**, 47 of them
non-mutations (`getStatus`, `streamGraph`, `forge*`, `list*`) unattributed **by design**. The
narrower shipped guards are the correct reading: (a) every recognised mutation attributes
*positionally*, (b) no existing row nulls a declared `repoId` slot, (c) a synthetic self-check
proving (b) bites, so no tracked file had to be broken to show the guard red.

**Consequence of fix 1 worth recording:** all 29 recognised mutations now attribute, so the `None`
bucket is reachable only via a `schema: 2` line replayed from an older log (§2.6/AC2-10), a lift
that failed its non-empty-string check, or a future producer. There is no command that "truly
yields no repo".

**Two stale comments corrected by the orchestrator directly (post-review, comment-only, disclosed):**
`obs/anomaly.rs`'s `mutation_attributed_to` doc repeated §2.4's factually wrong `clone`/`init`
example (wrong twice over — those never match `is_mutation_cmd`, *and* fix 1 removed the
"no `repoId` argument" route), and `obs/raw_args.rs:37` still said the schema "now reads 2".

**`security-auditor` (SEC-2026-09-22, P117 inc 2 redaction surface): CLEAN at HIGH and above.** No
repo path, and no fragment of one, could be got onto a strict-mode line or into any other sink
through the new `repo` field. What earned it, recorded so a future session need not re-derive it:
`LogWriter::append_record` (`writer.rs:247-282`) is the **only** place a `LogRecord` becomes bytes
(grepped — no second `serde_json::to_string` of a record in `src-tauri`; the session header and
`truncate` records deliberately re-enter through it), and `strict::enforce` (`strict.rs:187-190`)
matches the **key name** before `walk` runs, so UNC, non-ASCII, space-bearing, under-home and
drive-rooted paths all collapse identically to `repo#N`. **The contract's whitespace rationale is
verified real, not theoretical:** `is_run_char` excludes space and `is_path_shaped` requires a
separator, so without the field rule `D:\Repos\my project` yields `path#N project` — which makes
the bare-word `"project"` assertion at `tests_repo_redaction.rs:103` load-bearing. No UI/Rust
asymmetry: UI records take the same `logAppend → log_append → sink.enqueue → append_record` path,
and there is no `console.*`/`localStorage`/`tauri-plugin-log`/devtools sink anywhere in
`src/obs/*` or `src/ipc/tauri/`. Anomaly records carry `repo: None` and the composite key
`"{repo}\0{scope}"` lives only in `Sliding.events[].key`, never serialised. Raw mode is
**double-gated** (`dev.enabled` AND `include_raw_names`, both false by default,
`settings/prefs.rs:299`).

**ACCEPTED RESIDUAL RISK, stated rather than dropped.** In the shipping **strict** configuration
the `logAppend` IPC batch now carries **raw paths** where previously it carried none. Bonsai adds
no sink that would capture them — verified — but whether the WebView2/Tauri transport itself
buffers or diagnostically logs invoke payloads is **outside what can be read from here**. Same for
Windows crash-dump/WER capture of the in-memory raw paths in `anomaly/cache.rs`, `mutations` and
the `Redactor` ordinal map (the last already held `Kind::Path` values, so not new). Judged
acceptable: it is no worse than `AppState::repos` itself, and nothing today dumps detector state.
Re-open if a "dump detector state" affordance is ever added.

**Audit follow-ups (INFO, deliberately NOT fixed in this increment):** (i) `strict.rs`'s field rule
is **top-level-only by construction** — `walk`, with its whitespace gap, is the fallback for any
deeper `repo` key. Nothing nests one today (checked against every `LogPayload` variant) and the
typed round-trip drops unknown keys, but making `walk` key-aware at any depth would make the
guarantee depth- and ordering-independent. (ii) The `redact_names` whitespace gap remains the
default for every *other* string field — an `error.message` embedding `D:\Repos\my project` would
put `project` on a strict line. **Latent only because `LogPayload::Error` has no producer at all**
in Rust or TS; file this against whichever increment first wires one, not against P117.

**Orchestrator-verified first-hand for inc 2 (not quoted from a subagent):** both
`OBS_SCHEMA_VERSION` literals read **3** (`record.rs:47`, `src/ipc/types/obs.ts:22`) and
`cargo test --lib -- obs::tests_schema_parity` → **1 passed, 0 failed, 678 filtered out**.
AC2-9's *mutation* direction (revert the TS literal ⇒ red) stays **senior-dev-reported**,
deliberately: proving it means temporarily editing a tracked file in a tree other sessions build
in, which is the hazard this session twice asked peers not to create. Not exempting itself from
its own rule. The non-mutating half — both literals at 3, guard green, `include_str!` so a moved
TS file becomes a build error — is first-hand.

**✅ Owed housekeeping — DONE 2026-09-23 by `docs-curator`.** `docs/contracts/INDEX.md` had **no row
for `P117-perf-signal-fixes.md`** (`bbd8993` added P115's row and carried no foreign hunks; P117's
was simply never written). The row is now present with status `awaiting USER CHECKPOINT` and a
pointer to `docs/audit-2026-09-22-perf-signals.md`, worded so it does **not** restate the original
17/9 split as exact — that audit carries a CORRECTION block.

**Two framing corrections the reviewer made to the orchestrator's own brief, worth keeping:**
`forcePush` is not part of gap (a) at all — it never matches `is_mutation_cmd` (`forcePush` vs
snake-case `force_push`), so a policy row would change nothing for it; and the exposure is
**click-rate-bounded, not periodic**, because scheduled auto-fetch runs entirely in Rust
(`scheduler/exec.rs:151`) and `LogPayload::IpcCall` has no Rust producer.

**Inc-2 shape, for the record** (this paragraph was spliced into the middle of the one above at
`473d9fa`; untangled here): 24 modified files + 7 new; `OBS_SCHEMA_VERSION` 2 → 3; the repo
dimension sits on `LogRecordBase` as one optional field. Two behaviour-preserving splits were forced
by the size ratchet *inside* a behavioural increment (`LogPayload::kind()` → `obs/record_kind.rs` to
hold `record.rs` at its 503 ceiling; `cache-collapse` → `obs/anomaly/cache.rs` because `slow.rs` hit
508) — flagged to the reviewer precisely because that is when a split goes wrong.
`security-auditor` was invoked because the new field carries an **absolute filesystem path** into
every record and the privacy property rests entirely on one writer-side enforcement point
(`strict.rs`), in the **default** mode. Not a mandatory path trigger — a judgement call.

**Browser-harness AI-gate item (AC2-12), verified first-hand by the orchestrator 2026-09-22.**
`pnpm dev:mock`, port 1420, 1440×900, localStorage seeded per the harness traps at the top of this
file: app boots, canvas renders ref pills (`stash@{0..2}`, `main`), sidebar populated, **four real
`full`-scope rounds through the modified `useCoalescedRefresh` with zero console errors**
(`read_console_messages` onlyErrors empty, checked three times), and two rapid clicks coalesced into
ONE round (`__bonsaiRefreshScopes` = `{full:1}`). **NOT verified here: the `repo` value on the
wire** — obs logging is gated off in mock/dev mode, so the record is not observable from the
harness; that assertion rests on `useCoalescedRefresh.repo.test.tsx`, which spies `logAppend` with a
space-bearing path-shaped repoId.

**Provenance caveat on `d63a571`'s message (CLAUDE.md gate-output rule).** Its claim that reverting
the carry-over "fails 4 of the 6 new tests" is **senior-dev-reported, not orchestrator-verified** —
and it will stay that way deliberately: reproducing it means temporarily mutating `repo.rs` in a
working tree two other sessions are building in, which is exactly the hazard this session asked a
peer not to create. The forward direction (6/6 green) is now first-hand. Read the negative-control
figure as a subagent claim.

**Concurrency note (2026-09-22), kept as a rule not a state.** Three sessions shared this working
tree, so P117's reviews were **path-scoped** rather than "the whole working-tree diff" — step 4's
usual framing would have handed the reviewer P115's code (`settings/prune*.rs`, `settings.rs`,
`settings/forge_accounts.rs`, `lib.rs`, `tests_repo_session_misc.rs`). P116's `e49cf20` was
convenient rather than conflicting: it pinned the TS `OBS_SCHEMA_VERSION` to Rust's 2 and added
`tests_schema_parity.rs` — exactly the guard inc 2's 2 → 3 bump had to move.

**RESOLVED — the step-7 gate was blocked by three FOREIGN defects in turn, never by P117**, and the
561.2s green above is the proof it cleared. The three, in order: inc 2's own `obs/anomaly.rs`
declaring `mod tests_anomaly_repo;` before the file existed · P115's `mod prune_forge_tests;` doing
the same · P115's `prune_tests.rs:290-291` writing `assert_eq!(norm("D:\x\Api"), …)` in a **non-raw**
literal, where `\x` opens a hex escape (`invalid character in numeric character escape`), fixed
with `r"…"`. All three red-lined the one shared lib test binary. Pre-tightening text: archive
Part 84.

**Lesson for the shared-tree situation (worth keeping):** with three sessions in one working tree
the shared `cargo` test binary is a single point of failure — any one session declaring a `mod`
before writing its file, or landing a syntax error, red-lines everyone's verification. Path-scoped
staging protects the *commits*; nothing protects the *build*. Verify and commit an increment as
soon as it is green rather than batching, because the window in which the tree compiles is not
under your control.

**Orchestrator rulings on the architect's four flags (all accepted):** (1) §2.4's scope widening to
`ipc.call` is necessary, not optional — `IpcCall.args` is raw-mode-only, so attributing the
`mutations` timeline in the default strict mode requires `repo` on the record base. (2)
`OBS_SCHEMA_VERSION` **does** bump 2 → 3, despite `record.rs:18-19`'s additive-no-bump rule: the
justification is the changed *meaning* of existing `anomaly` records (a v3 `redundant-refresh`
means one repo; a v2 one means any set), which is exactly the bar that comment sets. Staying at 2
would leave a reader unable to tell the two apart — rejected. (3) The dimension lives on
`LogRecordBase` as one optional field, not three per-payload fields. (4) The writer-side
field-name redaction rule stands and must not be relaxed to "`strict::enforce` already redacts
paths" — `is_run_char` splits on whitespace, so `D:\Repos\my project` would leak `project`.

Investigated from a real 124-min Dev-mode session with **5 repos open**
(`logs/bonsai-2026-09-22T04-52-03-scca8269e.jsonl`, 19,547 records + `metrics/usage.json` day
`2026-09-22`). Both signals arrived undiagnosed; the report is the diagnosis. Full evidence,
numbers and the corrected framings live in the audit file — only the verdicts are duplicated here.

**Signal 1 — graph cache hit rate 8% ⇒ REAL DEFECT (low severity, design-level).**
The cache is **per-repo** (`RepoEntry.graph_cache`), so the single-slot hypothesis is FALSE and
`graph_cache.rs` is correct. The defect is a composition: `refreshScope.ts`'s `full` slice sets
`openRepo: true` **and** `graph: true`, and `open_repo_inner` has **no early return** for an
already-open repo — its dedupe scan only reuses the key, then unconditionally inserts a fresh
`RepoEntry` with `graph_cache: None` (`src-tauri/src/commands/repo.rs:293`). So every `full` round
wipes the cache it is about to read. **62 of 81 graph requests (77%) were structurally guaranteed
misses**; 62 full rounds ↔ 63 `openRepo` calls; repos whose node count never changed
(`items` 1041, 5376, 5374, 1009, 409) re-walked on nearly every request; `HitRedecorate` fired
**0** times all day for the same reason. Excluding the forced misses the cache ran at **32%**
(6 of 19), and the 13 remaining misses are explained by 3 `stash` rounds, ~6 new commits in
`bonsai` itself and 7 fetches. Recoverable: **~9.0 s of blocking-pool time per 152-min session**.
The wipe's stated justification ("topology may have changed while closed") does not apply to a
repo that was never closed, and is redundant regardless — `classify` is exact-set on
`(tips, head, hide)` from a freshly probed seed.

**Signal 2 — redundant-refresh ×23 ⇒ MOSTLY CORRECT BEHAVIOUR + a monitoring defect.**
`detect_redundant_refresh` (`src-tauri/src/obs/anomaly/window.rs:152`) keys on `scope` **and
nothing else**, because `RefreshPayload` carries **no repo id** (`src/obs/types.ts:108`) — the
coalescer receives `repoId` but never emits it. With 5 repos and 5 watchers, two *different* repos
each doing one legitimate refresh inside 1000 ms is flagged as one repo refreshing twice.
`round` is per-repo (a `useRef` per coalescer), which separates the populations: **~17 of 26 are
cross-repo false positives, including all five `full`-scope pairs** — so the reported
"541 ms + 684 ms full pair" is round 9 of one repo and round 28 of another, not a redundancy.
The **9 genuine** same-repo pairs are all `worktree`, Δt 381–935 ms, and cost **805 ms total over
152 minutes**; every Δt exceeds the 300 ms trailing-edge quiet period, i.e. the watcher really did
see new external writes and the debounce behaved exactly as specified.
⇒ **`DEBOUNCE` stays at 300 ms and no post-fire quiet window is added.** The architecture invariant
needs no contract change; the fix belongs in the obs rule. The same repo-blindness also explains
both `cache-collapse` firings (five spans, five *different* repos, 217 ms apart — not a user
tab-switching over 100 s as first read).

**Increment 1 — DONE, `reviewer` approved with no MUST-FIX (2026-09-22).** Fix: `open_repo_inner`
carries the existing `Arc<GraphCache>` over when an entry is already present under the same key,
read **under the same lock acquisition as the insert** (which also closes the dedupe scan's TOCTOU
for free — an entry closed in between makes `get` return `None`, the correct cold start). Watcher
still rebuilt and installed on every re-arm; `bump_repo_generation` untouched. All three durable
doc-comment copies updated (`repo.rs`, `state.rs`, `graph_cache.rs` — the last net-zero-lines
because the ratchet pins it at 511, `scripts/file-size-baseline.json:26`). New
`commands/tests_repo_graph_cache_rearm.rs` (343 lines, own module because
`tests_repo_session_misc.rs` is 560). **The evidence is the negative control, not the green run:**
reverting only the carry-over fails 3 of 5 new tests, while the other 2 pass in both states because
they are boundary guards. Reviewer independently re-ran 45 tests green and verified the soundness,
TOCTOU, `path`-overwrite and concurrency claims one by one.

**P117 inc-1 follow-ups from the review (none blocking, ranked):**

1. **`seed_fingerprint` is refs-only, and P117 lengthens the window where that matters.**
   `crates/bonsai-core/src/graph/seed.rs:43-55` seeds from refs alone, so `classify` assumes a tip
   oid pins its whole ancestry. That holds except when parentage is overridden *externally*:
   `.git/shallow` (a CLI `fetch --unshallow`/deepen leaves tips, HEAD and hide identical while the
   DAG grows), `info/grafts`, `refs/replace/*`. Pre-P117 the `full`-round wipe masked this within a
   session by accident; now a truncated layout can be served as `HitVerbatim` until a tip moves or
   the tab closes. **Bonsai is a Git client, so external CLI use in an open repo is the normal
   case, not an edge case** — that is why this is worth carrying even though nothing in our own
   code creates shallow clones (`grep -rn "shallow|grafts|refs/replace" --include=*.rs` → empty).
   Fix shape: fold `repo.is_shallow()` / `.git/shallow` mtime into `seed_fingerprint`. Hedge on
   libgit2's exact revwalk behaviour for replace refs rather than assuming it.
2. **Contract-record correction for `architect`: §1.2 mis-describes `info.path` as canonical.**
   `read_repo_info` returns `path.to_string_lossy()` verbatim
   (`crates/bonsai-core/src/git/repo.rs:43`, `:64`) — it never calls `canonicalize` and never
   consults `repo.workdir()`, and nothing canonicalizes before `open_repo_inner`. So `info.path` is
   the **raw caller string** and the map key is the raw string of whichever open came first. The
   pre-existing comment at `repo.rs:225` ("repoId == canonical workdir path string") carries the
   same inaccuracy. §1.2's *comparison* is what the code does; only its narrative is wrong. Does
   not affect the fix's safety.
3. **Contract-record correction for `architect`: AC1-4 is unimplementable as written.** The
   `to_uppercase()` construction at `tests_repo_isolation.rs:127` can only yield a
   canonicalize-equal path (Windows/macOS ⇒ `same_repo_path` matches ⇒ a *same-path* re-arm, the
   opposite of what the AC asks) or a nonexistent directory (case-sensitive FS ⇒ `is_dir()` rejects
   it first). Junctions/symlinks canonicalize back too. A second distinct repo is the only
   construction that yields "canonical id differs from every existing key". Reword the AC; the
   implementation is the right reading of §1.4(d).
4. §1.2 should also name `same_repo_path`'s fallback branch (`repo.rs:408`): when either side fails
   to canonicalize it degrades to `eq_ignore_ascii_case`, so "same path" can mean "same string
   modulo ASCII case" for a vanished directory. Safe under §1.4(b), but unstated.
5. **AC1-7 is only partially verified in this tree** — `tests_repo_session_misc.rs` is modified by
   the concurrent P115 session, so the reviewer deliberately did not run it. Its green status rests
   on senior-dev's 61-test run, not an independent one. Re-verify once P115 lands.

Dropped deliberately: the reviewer's NITs (comment duplication between `repo.rs` and
`RepoEntry::graph_cache`; `drop(dir_a)` needing a comment) — not worth a round trip.

**Two contract changes for `architect` (both routed, neither implemented):**
1. `graph_cache` preserve-on-re-arm — contradicts the P86 B1 contract line *"reset to `None` on
   `open_repo` re-arm"*.
2. `repoId` on `RefreshPayload` + the `graph.get` span record, and key
   `redundant-refresh` / `cache-collapse` / the `mutations` list on `(repoId, scope)` — P91 §2.5 + §5.1.

**Follow-up, not in scope:** `perf.repo_opens` = 564/day is the same root cause
(`bump_repo_generation` on every `full` round evicts the pool handle cache) — re-measure after
fix 1 lands. **Open question, do not fix blind:** day-wide `perf.graph_walks` (103) over-reports
by exactly the hit count (6) versus the 97 spans that actually ran a `revwalk`; the offset predates
the observed session and could not be attributed from static reading.

## P119 — every repo-changing action is logged in the git activity dock — `in-progress`

**Current step:** P119 — contracts written (`docs/contracts/P119-activity-for-all-actions.md`,
`docs/contracts/P119-ui.md`); architect contract at rev 2 (610 lines); P119-ui.md
aligned (593 lines). **P119-1 (core model) implemented, awaiting reviewer round 1.**
**Second rulings (2026-09-24):** stage/unstage + conflict-resolution writes are **NOT** logged;
a run that pauses on conflicts ends in a distinct **`! Conflicts`** outcome; fast-forward = one
**Merge** row whose finished detail says fast-forwarded vs merged; clone/init logged only when a
repo is already open (empty-screen clone keeps the dialog's own bar).

User request 2026-09-24: (1) the toolbar `header-progress` bar duplicates the dock's progress
bar — remove it, keep only the dock's; (2) checkout / merge / fast-forward / rebase / etc. give
no feedback — every action must appear in the bottom activity log with a loading bar.
**User rulings (2026-09-24):** log **every repo-changing action** (instant ones included, a short
row is fine); **Refresh** is NOT logged — its icon spins while refreshing instead.

Acceptance criteria:
- No `header-progress` element under the toolbar in any state; `ToolbarPhaseReadout` text stays.
- The determinate fetch/pull fraction (formerly only on the toolbar bar) is shown on the dock's
  progress bar.
- Every mutating command (checkout branch/commit/remote, merge/abort, fast-forward, rebase*,
  cherry-pick*, revert*, reset, stash ops, branch create/delete/rename, tag ops, submodule ops,
  worktree ops, discard, clone, undo…) emits `started`/`finished` on the git-activity stream
  with a category + target; the dock shows a row, running bar, and success/failure.
- Refresh icon spins while refreshing; submodule ops (formerly `netBusy`) show in the dock.
- Mock IPC emits the same events so the browser harness shows the rows.
- `@keyframes header-progress-sweep` is kept (the AI panel reuses it).

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

## Durable lessons — the rules → **`docs/durable-rules.md`**

**MOVED 2026-09-23 on an explicit user decision. The rules are not archived and not deleted — they
are a LIVE REFERENCE at `docs/durable-rules.md`** (237 lines), moved byte-identical from this file
at `473d9fa`. They stayed on the board for four curator passes precisely because they are
load-bearing; the move trades one hop for 211 lines that every session paid for whether or not it
was about to make a claim.

**Read `docs/durable-rules.md` before you assert that something is tested, measured, covered,
closed or green.** It holds, in this order: the six failed app-wide claims and the enumerate-bucket-
verdict rule · the aliasing rule (scope by resolved VALUE, not token name) · the grep-counting rules
· the BASE rule for contrast figures · the three P91 testing rules · "evidence lost in
transcription" + the specificity trap · the measurement rules · **the gate-running rules** (serialize
cargo; a queued cargo is indistinguishable from a hang; read the `gate summary` block not the exit
status; redirect the log to a file; check port 1420; `--workspace` not `-p`) · the coverage and
evidence rules (incl. "a green `pnpm gate` is Windows-only evidence" and "cite from the file") ·
**the two durable constraints** (`tracing` does not exist in this workspace; the settings-load path
cannot use the `obs` sink) · the rules earned 2026-09-16/17.

The stories and worked numbers behind them remain archive Part 53 (plus 46, 51, 58, 71, 73, 76.1,
78, 79). A new rule goes in `docs/durable-rules.md`, not here.

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
- **✅ CLOSED 2026-09-23 (curator-verified in the tree) — cross-tab/cross-repo detector keying.**
  Delivered by **P117 increment 2, `83bbdf3`**, which is exactly what this item asked for. Verified
  at source, not from the commit subject: `src-tauri/src/obs/anomaly/window.rs:160-178` documents
  *"keyed on `(repo, scope)`, not `scope` alone"* and builds `format!("{}\u{0}{scope}", repo…)`,
  and the DTO field exists — `src/obs/types.ts:266-269` declares `repo?: string` on
  `LogRecordBase`. `repo: None` gets its own `""` bucket, never merged into a repo's.
  **NOTE the split:** the *code* is done; **the contract delta routed to `architect` is still
  open** and lives in the live P117 section ("`repoId` on `RefreshPayload` … P91 §2.5 + §5.1").
  P117 itself is `awaiting USER CHECKPOINT` — this bullet closing does not close P117.
  Pre-sweep text: archive Part 86.1.
- **Dev-mode log VOLUME:** 10,280 watcher records = **87%** of a 2.2 MB / 6-minute log, and **7,247
  had `relevant: 0`** — a record per file change it then correctly ignores. Cost, not correctness.
- **✅ CLOSED 2026-09-23 (curator-verified in the tree) — `RemotesSection` re-rendering on a
  local-branch change.** Closed by **P118b, `c6ae304`**. Verified at source, not from the subject:
  `src/components/sidebar/RemotesSection.tsx:13-17` no longer takes `data: BranchesSnapshot` at all
  — it takes `hasRemoteRefs: boolean`, with the reason in a doc comment — and the component is
  `memo`-wrapped at `:156`. The board's own P118b block records the measured floor (the assertion
  moved from `{renders: 2, instances: 1}` to `toBeUndefined()`) and the third change that was
  actually needed, `sidebar/useStableRemoteRefs.ts`. **P118/P118b themselves are NOT done** — they
  read `reviewer approved`; only this follow-up closes.
- **Contract drift (contracts are not the orchestrator's nor the curator's to edit) — RE-MEASURED
  2026-09-23, and it got WORSE, not better.** `OBS_SCHEMA_VERSION` is now **3** in code
  (`src-tauri/src/obs/record.rs:47`, `src/ipc/types/obs.ts:22`), while
  `docs/contracts/P91-observability.md` still declares **1** at **`:235`, `:252`, `:322`, `:648`,
  `:1384` and `:1769`** — six sites, two versions behind — and `:261` still says "jitter-free
  ordering aid". **This is the same item P116's reviewer filed "for `docs-curator`" (`:252`,
  `:648`); it is contract substance and belongs to `architect`. This is its one canonical home.**
  Separately: the **P81** contract names `pendingTagForceRef`, **renamed** to `pendingUserOriginRef`
  (a rename, not a merge — reviewer-verified byte-identical).
- **Headroom — RE-MEASURED 2026-09-23; all three numbers were stale.** `Sidebar.tsx` is **482**
  (not 491 — P118 split three components out of it), `writer.rs` **497** (not 491, 3 lines of
  headroom), and `record.rs` is **503** (not 487): it crossed the limit and is now *baselined* at
  503, so it is on the split queue rather than a hard cap. `Sidebar.tsx` and `writer.rs` are still
  hard caps.

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

**But I checked what happened to the underlying error and it is gone.** `useSettingsWriteQueue.ts`
calls `noteFailure(streak)` with **only the streak**; there is no `errorMessage(e)`, no `console`, no
log call anywhere on the new path. The raw OS error previously rode in the toast text
(`Could not save settings: ${errorMessage(e)}`) and **now nothing captures it at all.**
**STILL OPEN, re-verified in the tree 2026-09-23** — only the citation moved: the call is at
**`:151`**, not `:128` (pre-sweep text: archive Part 86.2).

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
- **✅ CLOSED 2026-09-22 — the U+200B in `P107-F2-copy-chip-ui.md`** (`2233cc0`; zero codepoint hits
  across active contracts; three remain under `docs/contracts/archive/`, out of scope). Full text:
  archive Part 86.3.
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

**The 15 still over the limit, paths as resolved and counts as re-measured 2026-09-23** (7
genuinely oversized, 8 marginal; `~` marks the 8 that will fall back under on any real cleanup):
`src-tauri/src/commands/tests_diff_search_history.rs` **602** ·
`crates/bonsai-core/src/git/cred_cache/tests.rs` **588** · `src-tauri/src/graph_cache/tests.rs`
**568** · `crates/bonsai-core/src/git/hooks/tests.rs` **543** · `src-tauri/src/obs/tests_metrics.rs`
**522** · `crates/bonsai-core/src/tools/scan_tests.rs` **520** ·
`crates/bonsai-core/src/git/ai_operation_preview.rs` **519** ·
~`crates/bonsai-core/tests/ai/ai_resolve_cli.rs` 513 · ~`src-tauri/src/graph_cache.rs` 511 ·
~`crates/bonsai-core/src/git/bisect/tests.rs` 510 · ~`crates/bonsai-core/src/tools/detect_tests.rs`
508 · ~`src-tauri/src/obs/tests_writer.rs` 507 ·
~`crates/bonsai-core/tests/status_stage/hooks_commit_cli.rs` 504 ·
~`crates/bonsai-core/src/gitbin.rs` 504 · ~`src-tauri/src/obs/record.rs` 503.

**Only 2 of the 20 are application code** (`ai_operation_preview.rs`, `graph_cache.rs`); the other 18
are test files, where `refactorer` can prove equivalence by identical before/after test counts.
**NOT queued — awaiting the user.**

**Trap when searching for these:** `.claude/worktrees/` holds stale copies of several of these
paths at different line counts. Measure the repo path, not a `find` hit.


**STATUS 2026-09-23 — 5 of 20 done, 15 open. Every one of the 20 was re-measured with `wc -l`
against the working tree this pass** (not taken from `fbf81d0`'s subject, and not from the previous
board line). Pre-sweep text: archive Part 86.4.

**The 5 that `fbf81d0` split, confirmed under the limit today:** `submodule_wedge_cli.rs`
616 → **349** · `essentials_autostash_cli.rs` 567 → **471** · `branches_cli.rs` 553 → **343** ·
`signing_cli.rs` 530 → **187** · `force_push_cli.rs` 516 → **436**.
`scripts/file-size-baseline.json` went **38 → 33 entries** (counted at `473d9fa`), matching the
five removals.

**The 15 still open are all unchanged at their listed line counts** — no drift in either direction.
The two application files (`ai_operation_preview.rs` 519, `graph_cache.rs` 511) are both still on
the list.

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
2. **`crates/bonsai-core/src/gitbin.rs` — re-measured 2026-09-23: it is 504, not "exactly 500".**
   It crossed the limit in the `8ad3c72` reformat and `scripts/file-size-baseline.json` now pins it
   at **504**, so comment-only fixes are no longer blocked — but any *growth* still trips the
   ratchet, and it is one of the 15 files still on the split queue below. `refactorer`.
   Pre-sweep text: archive Part 86.5.
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
- **✅ CLOSED 2026-09-23 (curator-verified in the tree, not from `833f2f9`'s subject) — the gate's
  Node `DEP0190` / `shell: true` argv concatenation.** `grep -rn "shell: true" scripts/` returns
  **zero live uses** — the only three hits are prose in the comment headers of
  `scripts/lib/spawn-tool.mjs` and `scripts/lib/spawn-tool.test.mjs` explaining why it was removed.
  `scripts/lib/spawn-tool.mjs` is a shell-free launcher (resolves pnpm's JS entry and runs it under
  the current node), `scripts/gate.mjs:40` documents "so no step needs a shell", and
  `spawn-tool.test.mjs` guards it. The 2026-09-10 contradiction is resolved in favour of the commit
  message. Pre-sweep text: archive Part 86.6.

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
  #8: the `src/ipc/mock/repoState.ts` query helper.** Re-measured 2026-09-23 (pre-sweep text:
  archive Part 86.7): `query()` is now at **`repoState.ts:159-165`** and carries a
  **Node-environment guard** (`if (typeof window === 'undefined') return null;`) whose own comment
  states the module-init problem. So the crash is fixed; **the item is NOT closed** — whether the
  19 otherwise-DOM-free files actually left the DOM vitest project is **unverified**, and that
  (~70 s of CPU) was the point. Verify by reading the project globs, not by re-reading this line.
- **`pnpm gate --quick` is 305s and only drops e2e** — not a fast tier; `cargo nextest --workspace`
  alone is 181s of it. Add a genuinely narrow tier or lean on `--rust` / `--frontend`.

### Hoisted off milestones archived 2026-09-01

- **keyring 3 → 4** needs a dedicated increment: 4.x moves onto `keyring-core`, renames every
  per-backend feature, drops `crypto-rust`, requires explicit credential-store registration — real
  changes to `crates/bonsai-forge/src/auth.rs`. (DEP REFRESH, archive Part 24.)
- ~~`no_proxy_client()` `.expect(...)`~~ — **CLOSED 2026-09-03**, `#[cfg(test)]`-only so it never
  reaches a shipped binary. Detail: archive Parts 52.5 and 86.8.
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
  **Still open; citation re-measured 2026-09-23** (pre-sweep text: archive Part 86.9). The code
  moved: `src-tauri/src/commands/forge_set_token.rs:56` is the fn, `:65` calls
  `validate_repo_token`, `:69` is the `host.is_empty()` guard. The old `commands/forge.rs:290/:291`
  citation is dead — that range is `forge_pr_diff` today.
  **⚠ CONTRADICTION with a live board entry, unresolved.** `### Filed 2026-09-17 — the credential
  arc's open residue` records `forge_set_token.rs:69`'s empty-host early return as **unreachable
  dead code** ("genuine dead code, filed not deleted"). If it is unreachable, "guard host first"
  reorders a branch that can never be taken. Settle which reading holds before acting on either.
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

**⚠️ The first row is NOT history.** `docs/durable-rules.md` is live reference content that used to
sit on this board; it is listed here only because this is where a session looks for "where did that
block go". Everything below it is archive.

| File | Covers |
|---|---|
| **`docs/durable-rules.md`** — **LIVE REFERENCE, not an archive** | **The durable rules**, moved verbatim off this board **2026-09-23** by user decision (211 lines, from `TODO.md:1474-1684` at `473d9fa`). The six failed app-wide claims · the aliasing rule · the grep-counting rules · the BASE rule · the three P91 testing rules · evidence-lost-in-transcription + the specificity trap · the measurement rules · **the gate-running rules** · the coverage and evidence rules · **the two durable constraints** (no `tracing` in this workspace; the settings-load path cannot use the `obs` sink) · the rules earned 2026-09-16/17. **Read it before claiming anything is tested, measured, covered, closed or green.** The stories behind the rules stay in archive Part 53. |
| `docs/history/README.md` | **The archive index** — which file/part holds which milestone. |
| `docs/history/todo-archive-2026-09.md` | **Parts 83-87 (moved 2026-09-23):** **P116 in full** — `done`, `e49cf20`, its own text states "No USER CHECKPOINT applies", the only cleanly archivable milestone on the board (83) · **P117's pre-tightening text** — NOT an archived milestone, P117 is live and `awaiting USER CHECKPOINT`; 380 → 352 (84) · **P115's pre-tightening text** — NOT an archived milestone, P115 is live and `in-progress` with a Linux/macOS run owed; 134 → 134, rebalanced not shrunk (85) · **the open follow-up entries rewritten by the staleness sweep** (86.1 the real-log residue · 86.2 A4 finding 6's citation · 86.3 the U+200B closure · 86.4 the 20-file split queue · 86.5 the `gitbin.rs` line count · 86.6 the gate `DEP0190` bullet · 86.7 the velocity follow-ups · 86.8 the struck `no_proxy_client` · 86.9 P80 forge follow-up (a)) · superseded curator bookkeeping (87). **Parts 76-82 (moved 2026-09-22, after the user confirmed P112's native checkpoint on 2026-09-22 — `ba4b9d3`):** P112 in full, incl. the five checkpoint items as confirmed and item 4's `set_parent` sub-question (76) · the completed 2026-09-14/15 queue, the superseded `5654eaa`/`934a280` gate states, ruling #24's scope facts, the second-round rulings' evidence blocks and the whole 2026-09-11 ruling queue (77) · the 2026-09-16 session — the real-log investigation, the nine-file second review pass and its closure, the Settings-scrim finding, **P113 phases 1+2** (78) · the 2026-09-17 session — the orphaned-credential arc, **P114**, contract hygiene, the rustfmt pass, three security audits with their two CLEAN registers, and the superseded `ea6d323`/`5f015be`/`3948478` greens (79) · the release block's pre-tightening text (80) · P91's closed `logs/*.jsonl` parse + the `mono` defect it found + the superseded `cargo fmt` section (81) · superseded curator bookkeeping (82). **Parts 71-75 (moved 2026-09-16):** the P112 sub-inc 3/4 build + review transcript incl. the `P112-ui.md` §17 rulings, the four bad citations, the coalescing lesson and the sub-inc-3 audit (71) · superseded gate states (`d0e6cf0`, `dcff54b`, the 427.4s confirming run, the `e9ed93d` Rust tier) and the completed 2026-09-14 queue — F6, P77, the e2e cold-timing measurement, the UNC clearance (72) · the P112 sub-inc 2 + P113 phase-1 review transcript (73) · the two items CLOSED 2026-09-16 with their evidence — "Open in editor" (fixed `fd93616`, with its `os error 193` measurement table) and the false General subtitle — plus the `.cmd` launch-path audit (74) · superseded curator bookkeeping and the duplicated `cargo fmt` measurement (75). **Parts 62-70 (moved 2026-09-14, after the user ruled all 22 FOR-USER items on 2026-09-11):** the stale 2026-09-10 resume block + FU-1 residue (62) · the FOR-USER evidence blocks for items 0-6 (63) · the `IN FLIGHT` queue, the 2026-09-11 orchestrator closures, the unreviewed-MCP-merge warning (64) · **`SEC-2026-09-11`**, the MCP tool-contract audit, with its verified-CLEAN register (65) · **`SEC-2026-09-11b`**, the review of that implementation, with its verified-CLEAN register (66) · P108 `AC11`, closed by ruling #13 (67) · the happy-dom load-flake narrative (68) · the open follow-ups as they stood pre-condensation (69.1 P91 · 69.2 SEC-2026-09-03 through the 2026-09-01 hoisted items · 69.3 P69 Settings) · superseded curator bookkeeping (70). **Parts 54-61 (moved 2026-09-10):** the whole USER-CHECKPOINT block — P102+P105, P106, P107, P108, P91 (54) · P110 + P109 (55) · the 2026-09-03 closures + SEC-2026-09-03 remediation (56) · `Queued housekeeping` incl. the `e149382` CSS-split proof (57) · the superseded `c218258` and earlier gate states (58) · the 2026-09-10 session: P111, FU-1, six reviewer-follow-up closures (59) · the board's record of the confirmation (60) · superseded curator bookkeeping (61). **Parts 51-53 (2026-09-03):** the `5c2dcd2` + `c6cd7dd` gate states and the e2e-contention mis-diagnosis · the full narratives of everything closed 2026-09-03 · the durable-lessons stories and worked numbers. **Parts 36-50 (2026-09-03):** the file-size refactor pass · P102+P105, P106, P107, P108 and the P91 security arc + audit + build diary · superseded pre-ship filings · the 2026-09-03 velocity pass · P99, P100, P101, P98, P95, P96, P97 · built-bundle e2e + P103 + P104 · the DX/velocity stubs · the pre-condensation open-follow-up text. **Parts 33-35 (2026-09-01):** the P84 record gap · macOS ad-hoc signing · the two 2026-08-22 design reviews. **Parts 22-32 (2026-09-01):** P94 · P93+P92 · DEP REFRESH · P90+P89 · P88 · the P85-P87 batch · P82+P83 · divergence reconcile + Release 1.1.0 · the DX dev-loop text · the confirmed-checkpoints block · the 2026-08-21 resolved follow-ups. |
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

### Why this board is ~2545 lines, not ~300 (curator note, 2026-09-23)

**2678 → 2545, a 5% cut** — and the honest headline is that **the board grew 868 lines between
2026-09-22 and this pass and only 174 came back off.** Five live milestone sections (P113, P115,
P116, P117, P118/P118b) landed in one day; four of them are still open, so four of them stayed.

**Protocol.** **956 lines were extracted verbatim across 15 ranges**, each `sed`-extracted directly
from `git show HEAD:TODO.md` (the working tree was confirmed byte-identical to `473d9fa` first) and
each re-verified in place after the append — **15 of 15, twice**: 14 inside
`todo-archive-2026-09.md` as Parts 83-87, and 1 inside `docs/durable-rules.md`. A final whole-file
check confirmed **every non-blank, non-`---` line of `git show HEAD:TODO.md` still exists** on this
board, in `docs/history/`, or in `docs/durable-rules.md`: **zero lines unaccounted for.** Nothing
was summarized away and **no status was upgraded by the curator.**

**The structural change, decided by the user on 2026-09-23 after two notes escalated it: the
durable rules now live at `docs/durable-rules.md`.** 211 lines off the board, byte-identical, as a
live reference under `docs/` and deliberately **not** in `docs/history/`. A pointer section stays
in place and the Archive table carries it as a clearly-marked live-reference row. **That move is
essentially the whole cut.**

**Tightened, not archived** (verbatim pre-tightening text kept, as with the release block on
2026-09-22): **P117 380 → 352** and **P115 134 → 134** (nine lines out, nine back in — the
`CLAUDE.md` gate-rule correction was promoted to P115 as the single canonical copy and absorbed
P117's citations). Every number, SHA, path, caveat and provenance qualifier survived, including
both "senior-dev-reported, not orchestrator-verified" hedges. These blocks are evidence, and
evidence does not compress.

**Archived: exactly one milestone — P116** (`done`, `e49cf20`, its own text says "No USER CHECKPOINT
applies"). Part 83.

**CLOSED in the staleness sweep — three, each verified against the current tree, never from a
commit subject:**
- **cross-repo detector keying** — `obs/anomaly/window.rs:160-178` keys on `(repo, scope)` and
  `src/obs/types.ts:266-269` declares `repo?: string` on `LogRecordBase`. Delivered by P117 inc 2,
  `83bbdf3`. The *contract* delta routed to `architect` stays open; P117 itself stays open.
- **`RemotesSection` re-rendering on a local-branch change** — `RemotesSection.tsx:13-17` no longer
  takes `data: BranchesSnapshot` at all, takes `hasRemoteRefs: boolean`, and is `memo`'d at `:156`.
  P118b, `c6ae304`. P118/P118b themselves stay open.
- **the gate's Node `DEP0190` / `shell: true` argv concatenation** — `grep -rn "shell: true"
  scripts/` returns zero live uses (three prose hits in comment headers); `scripts/lib/spawn-tool.mjs`
  is the shell-free launcher and `spawn-tool.test.mjs` guards it. Resolves a contradiction the board
  had carried unresolved since 2026-09-10.

**RE-MEASURED and deliberately LEFT OPEN** (the tree says they are live; closing them would have
been closing on inference): the **20-file split queue** — the board's own named trap — re-measured
file by file, **5 of 20 done, 15 still over the limit at unchanged counts**, baseline 38 → 33
entries · the **`P91-observability.md` schema drift**, which got *worse*: code is at **3**, the
contract still says **1** at six sites · the **headroom** numbers, all three stale
(`Sidebar.tsx` 491 → **482**, `writer.rs` 491 → **497**, `record.rs` 487 → **503** and now
baselined) · **`gitbin.rs`**, not "exactly 500" but **504** · **A4 finding 6**, still discarding the
raw error, citation `:128` → **`:151`** · **`repoState.ts`'s query helper**, which gained a
Node-environment guard but whose actual payoff (19 files leaving the DOM project) is still
unverified · **P80 forge follow-up (a)**, whose citation moved to `forge_set_token.rs:65/:69` and
which now **contradicts** the credential-arc entry calling `:69` unreachable dead code.

**What was REFUSED, and why.** **P113** (`in-progress`, USER CHECKPOINT owed) · **P115**
(`in-progress`, a Linux/macOS run owed — gate gap 2) · **P117** (`awaiting USER CHECKPOINT`, both
items need the native app and real repos) · **P118/P118b** (`reviewer approved`, which is not
`done`) · the live release block and its **two** remaining USER ACTIONS (macOS ad-hoc signature
verify-on-tag, ruling #17; the Dependabot moderate, ruling #15) · **A4 finding 6** · the three
AWAITING-USER credential items · the open `render-storm` threshold decision · every other open
follow-up · all four ruling ledgers · the accepted decisions · the cross-platform-gap finding.
**Also refused: editing `docs/contracts/P91-observability.md`**, which P116's reviewer addressed to
`docs-curator` — that is contract substance and belongs to `architect`.

**Composition of what is left, measured with `awk` over the finished file:** header + conventions +
navigation **98** · the release block **229** · P112's closure record **30** · **the five live
milestone sections 810** (P113 **178**, P118/P118b **132**, P115 **135**, P116's pointer **12**,
P117 **353**) · their ranked follow-ups **33** · the queue/verification summary **41** · **the four
ruling ledgers, verbatim and authoritative, 190** · the ruling-queue closure line **7** · the
durable-rules pointer **24** · accepted decisions **120** · **open follow-ups 850** · archive table
+ this note **113** (this note alone is **86** — a one-off cost; the next pass replaces it, and its
predecessor is archive Part 87.2).

**Two numbers now set the floor, not one.** The open-follow-up backlog is **850** — it *grew* from
803, because this sweep added the measured evidence that closing three items and correcting seven
required. And the live milestone sections are **810**, of which 810 minus P116's 12-line pointer is
work the curator may not touch until the user clears four checkpoints. Together that is **1660**
lines no amount of curating can move. Add the ledgers **190**, accepted decisions **120** and the
release evidence **229** and the irreducible core is **2199**. **~300 is unreachable by curation;
it is reachable only by the user clearing checkpoints and by someone working the backlog down.**

**The structural option from the last three notes is now SPENT** — the durable rules have moved.
The next-largest single lever, if one is wanted, is the **open-follow-up backlog at 850 lines**: it
is 34% of the board and contains at least seven entries whose line-number citations drifted between
passes. A dedicated follow-up-triage session — not a curation pass — is what would move it.
