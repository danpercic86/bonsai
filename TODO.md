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

**2026-09-28 pass — Parts 88-91. The user attested on 2026-09-28 that every pending native USER
CHECKPOINT was confirmed ("Verified by me, incl. P115"; not observed by the orchestrator).**
Archived in full: P113a–c, P118/P118b, P115 (**88**) · P117 (**89**) · P119, P120, P121, P122,
P123, P124 (**90**) · superseded release and curator bookkeeping (**91**). Release blockers V1–V4
cleared. Short form: `## ✅ DONE 2026-09-28` below.

**Not archived, deliberately:** **P113d** (`pending` — contract rev 2 approved, not implemented) ·
the live release block and its **non-native** steps (push, installer rebuild, release notes,
changelog date) · the **two** remaining USER ACTIONS — the macOS ad-hoc signature, verify-on-tag
(ruling #17) and the Dependabot moderate (ruling #15) · **every open follow-up**, incl. the
residue hoisted out of Parts 88-90 · all four ruling ledgers · the accepted decisions · the
cross-platform-gap finding · both security CLEAN registers' pointers · **A4 finding 6** (only the
obs-sink mirror remains). Pre-pass text of this paragraph: archive Part 91.1.

**Order of operations, mandatory since `c5b3ea5`.** That pass cut 1950 lines from this file and wrote
them **nowhere**; `67e2ce6` had to restore them wholesale. **Extract → diff byte-identical against
`git show HEAD:TODO.md` → only then remove → leave a Part pointer.** 2026-09-28: **14 ranges, all
14 verified byte-identical in `todo-archive-2026-09.md` after the append** (10 milestone ranges
against `git show HEAD:TODO.md`, 4 bookkeeping ranges against the working tree). 2026-09-23: **15 ranges, 936
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

## 🚀 RELEASE v1.6.0 — PREPARED 2026-09-18, RE-SCOPED 2026-09-24 TO THE BRANCH HEAD, **NOT PUBLISHED**

**Current step (2026-09-28): v1.6.0 — native blockers V1–V4 CLEARED by the user's 2026-09-28
attestation; still NOT published — the non-native steps under `### 🔲 BLOCKS RELEASE` (push,
installer rebuild, release notes, changelog date) and verify-on-tag remain open.**
Pre-pass text of this paragraph and its bullets: archive Part 91.2.

**As of 2026-09-24:** v1.6.0 was BLOCKED — the release now targets the branch head, and the
code committed 2026-09-22/23 still owed native verification. The 2026-09-18 "zero blockers"
verdict was a claim about the tree on `origin` (`3c7f1a8`, which differs from the verified
2026-09-18 tree only in docs plus the 11-line `pnpm-workspace.yaml` nanoid removal). Twelve commits
since then (`a7a7882`..`41cb916`; 101 files, +6493/−420 excluding markdown) carry P113a–c, P115,
P116, P117 and P118/P118b, and are **local only**. The release was re-scoped to that head, so:

- **✅ AI gate re-run at `41cb916`, 2026-09-24 — `pnpm gate --full`, all 11 steps green** (pasted
  below). Nextest **2698 run / 2698 passed / 11 skipped**, vitest **3060 / 280 files**, e2e **185**,
  eslint **38 warnings** (ceiling 50; was 36), cargo-deny + pnpm audit clean, 0 LEAK lines.
  Log: `D:/Data/Temp/claude/bonsai-gate/gate-v160-recheck-41cb916.log`.
- **✅ `CHANGELOG.md` `[1.6.0]` extended 2026-09-24** with the post-verdict work (three Fixed
  entries: forge rate limit, stale settings paths, diagnostics accuracy; two Performance entries:
  graph cache, sidebar renders). The heading date is still 2026-09-18 and must be set to the tag
  date. `D:/Data/Temp/claude/bonsai-gate/release-notes-v1.6.0.md` is **stale** — re-extract it from
  `CHANGELOG.md` before pasting it into the draft body.
- **✅ VERIFY NATIVELY — release blockers CLEARED 2026-09-28** (user attestation): P113a–c, P117,
  P118/P118b, and P115's Linux/macOS run. See `### 🔲 BLOCKS RELEASE` below.
- **🔲 Push the local commits** — `release.yml` tags `context.sha` on GitHub, so unpushed code is
  not in the release. *(2026-09-28: 22 unpushed, not 12 — see below.)*
- **🔲 Rebuild the Windows installer for this head** — the NSIS proof below is for the 2026-09-18
  tree only.
- **Not in 1.6.0:** P119 (activity dock, `in-progress` in a concurrent session, uncommitted as of
  2026-09-24). Keep it off the release commit. *(2026-09-28: P119 is committed (`105ffc2`) and
  `done`; whether the release head moves past `41cb916` is the open question under
  `### 🔲 BLOCKS RELEASE`.)*

```
════ gate summary — full (≈ CI) — 489.1s total ════
  ✓   184.9s  [rust] cargo nextest
  ✓     3.6s  [rust] cargo test --doc
  ✓     2.1s  [rust] cargo fmt --check
  ✓     1.2s  [rust] cargo clippy
  ✓    14.8s  [frontend] eslint
  ✓    773ms  [frontend] file-size ratchet
  ✓    75.9s  [frontend] vitest (coverage)
  ✓    12.6s  [frontend] tsc + vite build
  ✓   187.9s  [e2e] playwright e2e
  ✓     4.2s  [audit] cargo-deny
  ✓     1.1s  [audit] pnpm audit

✓ all 11 steps passed
```

**Superseded, kept for the record:** *"v1.6.0 IS CLEAR TO PUBLISH — zero blockers"* (2026-09-22) —
true of `3c7f1a8`, not of the head. The user's 2026-09-22 confirmations (P112 checkpoint,
`updater-prod.key` backup, signing secrets) **still stand**.

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
  branch only"**. `feat/post-p91-rulings` was pushed at **`b80dd36`**, tracking set. **As of
  2026-09-24 `origin` is at `3c7f1a8`** (four docs commits later) and the local head is **12
  commits ahead**, unpushed.
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

### 🔲 BLOCKS RELEASE — native blockers V1–V4 CLEARED 2026-09-28; the non-native steps are still open

**✅ V1–V4 cleared by the user's 2026-09-28 attestation** ("Verified by me, incl. P115"; the
orchestrator did not observe the runs). V1 = P113a–c · V2 = P117 · V3 = P118/P118b · V4 = P115's
Linux/macOS run. The checkpoint text each one was confirmed against: archive **Part 91.3**.

**Still required, not native checks — all OPEN:**
- 🔲 Push the local commits. *(Measured 2026-09-28: `41cb916` is on `origin`; **22** commits in
  `origin/feat/post-p91-rulings..HEAD` are unpushed — the "12" this line used to say is stale.)*
- 🔲 Rebuild the installer for the head.
- 🔲 Re-extract the release notes.
- 🔲 Set the changelog date and delete its "PREPARED" note.
- ❓ **Open scope question (not a curator's call):** the release was re-scoped to the head at
  `41cb916`; HEAD is now 26 commits further on and carries P119–P124, while the line above says
  "Not in 1.6.0: P119". Which head is v1.6.0?

**Cleared earlier, still standing:**

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
- **Not blocking:** everything under `## OPEN follow-ups`; the eslint warnings — **38 as of
  2026-09-24** (ceiling 50; one new one is an unused `eslint-disable` directive for
  `react-hooks/exhaustive-deps`); the 2026-09-18 breakdown of 36 was (**13**
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

## ✅ DONE 2026-09-28 — P113a–c, P115, P117, P118/P118b, P119–P124 → archive Parts 88-90

**User attestation, 2026-09-28** (in chat to the orchestrator): *"mark all items awaiting native
checkpoint as done and move them to archive"*, recorded at the user's choice as **"Verified by me,
incl. P115"**. The orchestrator did not observe the runs; the attestation is what cleared them
(same form as P112's 2026-09-22 confirmation). Sections verbatim in
`docs/history/todo-archive-2026-09.md`. Open residue: `### Hoisted 2026-09-28` under
`## OPEN follow-ups`.

| Milestone | AI gate (as recorded) | Archive |
|---|---|---|
| P113a–c — forge rate limit, `listTagSync` floor, `authenticated` redaction (V1) | `a7a7882` 9/9 502s; `--full` at `41cb916` | Part 88.1 |
| P118 / P118b — sidebar render storm (V3) | `8790b62` + `c6ae304`; `--full` at `41cb916` | Part 88.2 |
| P115 — `settings.json` stale-path prune + forge-override migration (V4, incl. the Linux/macOS run) | `bbd8993` 9/9 574.5s | Part 88.3 |
| P117 — graph-cache wipe + per-repo anomaly rules (V2) | `d63a571` + `83bbdf3` 9/9 561.2s | Part 89.1 |
| P119 — every repo-changing action in the activity dock | `105ffc2`, re-run 9/9 468.9s | Part 90.1 |
| P120 — perf & render-stability measurement loop | `1e2b538` … `4e7f584` 11/11 828.5s | Part 90.2 |
| P121 — first `/perf-review` fixes | `8bd346f` 11/11 493.3s | Part 90.3 |
| P122 — the `s229d042a` leftovers | `37bc1d7` 11/11 502.3s | Part 90.4 |
| P123 — activity dock per repo, unsubscribe leak | `a4a5371` + `92f80bb` 11/11 650.0s | Part 90.5 |
| P124 — second `/perf-review` fixes | `a508f41` + `841566f` + `835a63a` 11/11 450.5s | Part 90.6 |

- Native half of every row: confirmed by the user 2026-09-28 (attestation, not observed).
- P121's own record said its checkpoint was "NOT declared passed" from the `s1d3f226f` log; it is
  closed by the 2026-09-28 attestation, not by that log.
- P119 was still headed `in-progress` on the board, though its body read "AI gate done — awaiting
  USER CHECKPOINT".

## P113d — watcher log volume — `awaiting USER CHECKPOINT`

**Current step:** P113d — code committed `b5d66ec` (contract rev 3.1, `6ccef9e`; P91 §3.2 rule 6
reader rule added). Defaults taken on the contract flags: 9.7 no schema bump (stays 3), 9.2 repo id
deferred, 9.6 watcher-storm quirk left as-is. Reviewer APPROVE, no MUST-FIX.
**AI gate: bare `pnpm gate` 2026-09-28 over `b5d66ec` — all 11 steps passed, 806.0s** (nextest
2816/2816, 11 skipped; vitest 3368/3368 in 319 files; e2e 185 passed; perf 9 passed; perf ratchet OK).
**USER CHECKPOINT (criteria 10–15, need a real Dev session):** `pnpm tauri dev` with the usual repos
for a stretch comparable to 2026-09-22, then in the log: `watcher` records ≤ ~1,200 (was 12,001);
fired records still ~192; `watcher-storm` / stability anomalies unchanged in kind.
Review NITs, not blocking: `noise.rs:21` + `record.rs` `skipped_paths` doc should say "except ≤ one
batch's paths at teardown"; `watcher/mod.rs:13-14` "no timers idle" is stale (60 s outer wake);
`tests_noise.rs:236` absorbs race the first 50 ms flush (absorb before `Loop::spawn`);
`tests_noise.rs:65` `mine()` errors-marker needs a comment; `tests_noise.rs` is at 495 lines.
- Goal: stop the per-raw-batch `watcher` records — 12,001 = 61.4% of the 2026-09-22 log, 11,809
  of them non-firing. Target ≤1,200 records (~6%), firings unchanged at ~192.
- Logging fix only: do **not** change `DEBOUNCE` (300 ms).
- Two correctness traps (contract §4.1): notify `Err(_)` → `errors` on the closing fired record;
  the `Disconnected` arm stays a bare `return` with no fired record.
- **Re-check before implementing (curator, 2026-09-28):** the contract says `OBS_SCHEMA_VERSION`
  "stays `2`" (§3, §7 item 7), but code is at **3** (`src-tauri/src/obs/record.rs:54`, P117 inc 2).
  P124b (`841566f`) has since changed the same watcher path (ignored-only suppression + 5-min
  heartbeat). The per-batch `log_watcher` call now sits at `watcher/mod.rs:185`, not `:204`.
  Rev 2 predates both → route to `architect` first.
- Full design text and evidence: archive Part 88.1 (the P113d paragraphs).

## P125 — dependency update tracker — `in-progress`

**Current step:** P125a–g + P125h follow-ups committed through `14402d1`; AI gate green 2026-09-29
at `14402d1` — **awaiting USER CHECKPOINT** (`docs/contracts/P125-smoke-checklist.md` §C steps
1–18). Not declared passed. `P125-followups.md` §8 doc amendments applied to
`P125-dependency-tracker.md` (§2.1, §2.3, §5.3, §7.5) and `P125-ecosystems.md` (E3, E5, E6).

History: P125 — `architect` contract written 2026-09-28
(`docs/contracts/P125-dependency-tracker.md`, 768 lines, sub-increments P125a–g); **awaiting user
rulings on contract §11 Q1–Q6** → **RULED 2026-09-28: all six recommendations accepted**
(recorded in contract §11). `ui-designer` done → `docs/contracts/P125-ui.md` (610 lines;
placement = header-toolbar "Dependencies" icon → 880px dialog; ui-reference §1/§11 updated).
`security-auditor` pre-audit done → `docs/audit-2026-09-28-P125-deps-contract.md`: **amend first**
(0 CRIT / 2 HIGH / 4 MED / 4 LOW). Now: `architect` amending the contract (audit findings + UI
§11 flags) → **rev 2 done** (971 lines + `P125-ecosystems.md`); new Q7/Q8 **RULED 2026-09-28:
defaults accepted** (Q7 Cargo/NuGet config walk to fs root; Q8 neutral OSV copy). **Rev 3 done**
(888 lines; `P125-ecosystems.md` 186 lines holds E3–E6). `security-auditor` re-check: **OK to
implement** after NEW-1..NEW-3 + notes, applied by orchestrator; design docs committed `c66e2ad`.
**P125a** implemented by `senior-dev` (new `crates/bonsai-deps`, uncommitted); orchestrator re-ran:
nextest 65/65, doctests 13/13, clippy `--all-targets -D warnings` clean, fmt clean. `dep-tree.txt`
now path-stripped via `sed` (contract §7.4 updated). `reviewer`: **approve**, no MUST-FIX →
committed **`a4cd1d4`**. Reviewer SHOULD-FIX 1–3 (scan hardening, `package =` rename guard,
loader fail-open paths) folded into P125b. **P125b** implemented (uncommitted); orchestrator re-ran:
nextest 149 passed / 1 skipped, doctests 13/13, clippy + fmt clean, `cargo deny` ok, dep-tree
identical. (`a4cd1d4` had omitted `bonsai-deps` from `deny.toml` → `cargo deny` red until P125b.)
`reviewer` on P125b: **REQUEST CHANGES** — 2 MUST-FIX (BOM/CR-only lines drop `@scope:registry`
→ fail-open; `YNode` derives Debug), orchestrator-verified in code. Now: `senior-dev` round 2
(MUST-FIX + 5 cheap SHOULD-FIX). Deferred items written into contract §10 (P125c1 npm/pnpm
above-workdir walk + pnpm global `config.yaml`; P125d builtin npmrc, `pnpm_config__auth`
presence-only, catch_unwind/MAX_PARSE_BYTES; P125e precondition). Symlink tests: unix CI only.
Round 2 re-run: nextest 155 passed / 1 skipped, doctests 13/13, clippy/fmt/deny clean →
`reviewer` **approve** → committed **`615593b`**. Now: **P125c1** (Cargo + Go + deferred npm/pnpm
above-workdir walk + pnpm global `config.yaml` + TOML depth pre-scan) with `senior-dev`.
P125c1 implemented; re-run: nextest 201 passed / 1 skipped, doctests 13/13, clippy/fmt/deny
clean, dep-tree identical (+`toml` 1.1.4). `reviewer`: **request changes** — MUST-FIX Go
corporate `GOPROXY` fail-open (orchestrator-verified). **Q9 RULED 2026-09-28 (user):** follow
symlinked config files above the workdir (regular-file-after-open, capped, key-filtered, no UNC);
recorded in contract §11. Now: `senior-dev` round 2 (GOPROXY/GOSUMDB/GOINSECURE, pnpm
`auth.ini`, Q9, sparse-switch exemption, prod `read_nofollow_under`, junction fixture), then
`reviewer` + `security-auditor` on round 2. Round 2 re-run: nextest 213 passed / 1 skipped,
doctests 13/13, clippy/fmt/deny clean. `reviewer`: new MUST-FIX (Cargo source replacement
first-seen → fail-open; verified `cargo_cfg.rs:102,112,118`). `security-auditor` Q9 re-pass:
OK-as-wip, 1 MED (Unix automounter via followed symlink) + 4 LOW. **Q9 NARROWED — RULED
2026-09-28 (user):** follow only fixed user/system configs + `$HOME`/`$CARGO_HOME` ancestors.
Round 3 re-run: nextest 221 passed / 1 skipped, doctests 13/13, clippy/fmt/deny clean (13
symlink tests skip on this Windows host → Linux run is a P125e precondition) → `reviewer`
**approve** → committed **`eeb85c5`**. **P125c2** (PyPI + NuGet) implemented (uncommitted);
re-run: nextest 282 passed / 1 skipped (16 symlink tests skip on Windows), doctests 13/13,
clippy/fmt/deny clean, dep-tree identical (+`quick-xml` 0.41). User `%APPDATA%\NuGet\NuGet.Config`
confirmed untouched by the implementer's `dotnet nuget` checks. `reviewer`: **request changes**
— 2 MUST-FIX (includer's `-i` not applied to included files; PDM/Rye indexes unread),
orchestrator-verified. Now: `senior-dev` round 2 (+ mixed-`<clear/>` level closed, bare archive
lines Local, no default `read_user_files`, local/UNC/`%VAR%` feeds + `fallbackPackageFolders` as
hostless private sources, in-repo venv `pip.conf`, `Directory.Build.props` in the Q7 walk).
Round 2 re-run: nextest 293 passed / 1 skipped (17 skip on Windows), doctests 13/13,
clippy/fmt/deny/workspace clean → `reviewer` **approve** → committed **`5d1dc96`**. All 5 v1
ecosystems done. Now: **P125d** (remote deps.dev/OSV, cvss, check pipeline, cache, full
sentinels) with `senior-dev`, then `reviewer` + **`security-auditor`**. P125d implemented
(uncommitted); re-run: nextest 337 passed / 2 skipped, doctests 13/13, clippy/fmt/deny clean,
dep-tree identical (reqwest 0.13 blocking + rustls ring; no compression crates). Live probes:
yanked = `isDeprecated` (Q1 holds), OSV batch 1000 VERIFIED, Go `v` prefix irrelevant. Now:
`reviewer` + `security-auditor` in parallel. `reviewer`: **request changes** — MUST-FIX
transitive copies dropped by name-only filter (verified `check/rows.rs:80-97`).
`security-auditor`: **OK to commit**, MEDIUM-D1 NuGet case-sensitive OSV ⇒ silent "no vulns"
(fix before P125g), LOW-D2 malformed advisory id aborts check. Reviewer/auditor split on
`npm_config__auth` → orchestrator sided with auditor (no routing change ⇒ not checked).
Now: `senior-dev` round 2 (MUST-FIX + D1 + D2 + cache FIFO, TLS test, pagination test, docs).
Follow-ups: ui-designer consent copy — disclose org-private packages hosted on npmjs.org are
sent; per-package TooLarge instead of whole-check abort; live pagination probe; package whose
only advisories had malformed ids reads clean (→ `vulns_checked=false`).
Round 2 re-run: nextest 342 passed / 2 skipped, doctests 13/13, clippy/fmt/deny clean →
`reviewer` **approve**, `security-auditor` **OK** (D1/D2 resolved) → committed **`7619cba`**.
(Implementer's `dotnet restore` probe contacted api.nuget.org for one public package — outside
the stated host list; reported to user.) **P125e Linux precondition MET 2026-09-28:** podman
`rust:1.97-bookworm` (user-approved download), uid 1000, `cargo test -p bonsai-deps --locked` at
`7619cba` → 357 passed / 0 failed / 2 ignored (incl. 13 doctests), 0 `skipping:` lines, 17
symlink tests `ok` (log: `D:\Data\Temp\P125e-linux\run2.log`).
**PAUSED 2026-09-28 (user left) — resume here:** P125f `senior-dev` was STOPPED mid-work (last
step: wiring into `src/ipc/mock.ts` + `mock/handlers/session.ts`). Its partial, UNREVIEWED,
UNVERIFIED changes are uncommitted in the working tree (src/ipc/{types,tauri,mock,fixtures}/*
depTracker*, src/hooks/useDepTracker*, src/settings/*, src/obs/rawArgPolicy*). On resume: re-spawn
`senior-dev` for P125f with the same brief, telling it to inspect and finish (not restart) the
partial work; then tsc + targeted vitest, `reviewer`, commit `wip(P125f)`; then P125e.
**Resumed 2026-09-29:** same `senior-dev` resumed to finish P125f in place. P125f TS done;
orchestrator re-run: tsc clean, vitest 5 files / 57 tests pass, lint:size OK — but Rust
`rust_default_ui_settings_match_the_shared_oracle` FAILS (TS oracle has `depTracker`, Rust
doesn't). Pulled P125e's **setting** item (Rust `dep_tracker` field + `clamp_dep_tracker`
1..=168) forward into P125f so no commit is red. Added (`settings/{prefs,clamp}.rs`,
`settings.rs` at exactly 500 lines, `commands/ui_settings.rs`, new `settings_dep_tracker_tests.rs`);
re-run: `cargo nextest -p bonsai settings` 151/151 incl. parity, clippy `--tests` + fmt clean, tsc
clean. No mandatory security-trigger path touched. `reviewer`: **approve**, no MUST-FIX;
orchestrator fixed 2 fixture `declared` values (path string → null) + a stale comment; re-run
tsc clean, vitest 57/57. P125e obligations added to contract §10.
P125 follow-ups (architect): E5 asks for a transitive+dev row Rust can't emit (carry scope onto
transitive rows, or drop it from E5); §2.3 says derived states are "computed in the hook" →
name `depsView.ts` (P125g); `settings.rs` at 500 lines — split before the next field.
Committed **`c2d877c`** `wip(P125f)`. Now: **P125e** (src-tauri: state, planner, exec, scheduler,
GatedTransport, deadline, commands + event, real HostEnv, disable/startup wipe) with `senior-dev`,
then `reviewer` + **`security-auditor`**.
P125e implemented (uncommitted); re-run: nextest `deps::` 22/22, `settings` 153/153,
`registration` 4/4, `bonsai-deps` 343 passed / 2 skipped; clippy/fmt --all/deny/tsc clean; no
trigger path; bonsai-deps dep without `test-support`. `reviewer`: **approve** (no MUST-FIX;
SHOULD-FIX: refresh joins a cancelled run; cancelled-then-deadline reported as timeout).
`security-auditor`: **OK to commit**, 0 MED+, 3 LOW. Now: `senior-dev` round 2 (both
SHOULD-FIX + LOW-E2/E3 + 2 NITs). Known cost: cache save holds `repos` lock (contract-mandated).
Follow-ups: LOW-E1 skip Windows `DRIVE_REMOTE` PATH roots; P125g copy for UNC-redirected
profile ⇒ all "private — not checked" (ui-designer); re-check frontend link wiring at P125g.
Round 2 re-run: nextest `deps::` 25/25, `settings` 153/153, `registration` 4/4, `bonsai-deps`
343 / 2 skipped; clippy (both crates) + fmt --all + deny clean; dep-tree identical → committed
**`7e87fba`** `wip(P125e)` (excluded another session's uncommitted `src/components/*` +
`src/styles/*` edits). Now: `ui-designer` amends `P125-ui.md` (3 copy items + stated gaps),
then **P125g** UI with `senior-dev`, then `reviewer` + `ui-designer` review + browser harness.
`ui-designer` amended `P125-ui.md` (687 lines): consent ¶5b (org-private npmjs packages are
sent), §8.4 private-reason table, report-wide network-share-profile lead, `not verified` pill for
`vulnsChecked:false`, `deriveState` in `depsView.ts`. Model gaps (§11.5, follow-ups): no
local-feed reason variant; no report-level network-profile flag (UI infers it — keep copy
hedged); no field for why `vulnsChecked` is false. Now: **P125g** with `senior-dev`.
P125g implemented (uncommitted; `3cc491c` from another session landed meanwhile). Re-run: tsc
clean, lint:size OK, vitest 84 files / 965 tests. **Browser harness (orchestrator, this session):**
all 10 `?deps=` modes render, 0 console errors; populated dialog 880px, toolbar
"Dependencies, 6 vulnerable"; large = 200 rows + "Show all 1,240 packages"; screenshot taken.
Finding: report-wide "your user settings folder" lead is inferred from ≥2 `configUnreadable`
ecosystems — could blame the wrong folder → sent to ui-designer. Now: `reviewer` + `ui-designer`.
`reviewer`: **approve** (external-tools move behaviour-neutral: yes; settingsCatalog.test 67
asserts before/after). `ui-designer`: **changes required** — MUST-FIX report-wide lead copy
(amended P125-ui.md §8.3). Now: `senior-dev` round 2 (lead copy + `.deps-link` + clipboard note +
null-ecosystem lead + SR double + `depsOpen` reset + dialog state reset on repo change).
Follow-ups: footer should scroll to the Dependency checks group; USER CHECKPOINT items: card
geometry/focus ring in native window, NVDA/VoiceOver table reading.
Round 2 re-run: tsc clean, lint:size OK (App.tsx 559), vitest 99 files / 1,106 tests; harness
`?deps=mirror` lead verified verbatim vs amended §8.3, 0 console errors (ui-designer's MUST-FIX
verified by orchestrator, not re-reviewed by ui-designer) → committed **`b7ee24b`** `wip(P125g)`.
All P125a–g committed. Now: milestone step 6 — `tester` (smoke checklist + gaps), then step 7
full `pnpm gate` + USER CHECKPOINT.
`tester`: AC→test map complete except **AC10 (responsiveness) — no automated proof** (reviewer
inspection + harness only); added AC7 restart + AC8 interval Rust tests, rendered-row UI tests,
`#[ignore]` offline smoke, `docs/contracts/P125-smoke-checklist.md`. Smoke: 7 groups, private
names absent from all 31 requests. Committed **`4fea478`**. Cosmetic finding: truncated JSON
manifest reports "col 0" (`adapter.rs:111`). Now: full `pnpm gate` running (log
`D:\Data\Temp\P125-gate\gate.log`).
**`pnpm gate` at `4fea478`: RED** (781s; 10/11 steps ✓: nextest, doctest, fmt, clippy, eslint,
size, vitest, tsc+build, e2e 9 passed, perf counts) — ✗ perf ratchet: `cold-open: ipcCalls
baseline 40 -> new 45 (+5) [msedge]`. Cause (harness `ipcByCmd`): `useDepTracker` does
getDepTracker ×1 + onDepsChanged ×2 + onWindowFocus ×2 even with the tracker OFF. Fix (not a
baseline raise): hook does zero IPC while `depTracker.enabled` is false → committed **`22b96a0`**
(harness cold-open back to 40; vitest 90/90). Follow-up: right after enabling, the first fetch
can still read "off" until the backend `started` event refetches (transient).
**AI GATE GREEN 2026-09-29 at `22b96a0`:** `pnpm gate` — ✓ all 11 steps passed, 651.2s (nextest
3192 passed / 14 skipped (1 leaky); doctests 13; fmt; clippy; eslint; size; vitest 327 files /
3456 tests; tsc + build; e2e 185 passed; perf counts 9 passed; perf ratchet 48 counts OK). Log
`D:\Data\Temp\P125-gate\gate2.log`. **Now: awaiting USER CHECKPOINT** — native steps in
`docs/contracts/P125-smoke-checklist.md` §C. Not declared passed.
P125 follow-ups (not blocking): ui-designer copy for a "local feed" private reason; hostless
NuGet `RestoreSources` → hostless-private model (today Closed); document symlinked `.venv` ⇒
PyPI fail-closed; Windows junction fixture; relax `CARGO_REGISTRY_DEFAULT`; unlisted-target
`O_NOFOLLOW` = 0 → `compile_error!`/fail-closed.
Follow-ups (not blocking): Windows junction fixture (needs windows-sys/unsafe or a process);
relax `CARGO_REGISTRY_DEFAULT` fail-closed (publish-only per cargo docs).
**P125h follow-ups (2026-09-29):** `refactorer` split `settings.rs` 500→326 (+`settings/io.rs`,
`settings/recent.rs`; settings 153/153 before/after, re-run by orchestrator) → committed
**`ad86b01`**. `architect` wrote `docs/contracts/P125-followups.md` (malformed-only advisories ⇒
`vulnsUnverified`, per-package TooLarge, NuGet `localFeed`, network-profile flag, R1–R3);
orchestrator applied 3 architect-flagged corrections. **RULED by user:** R1 relax
`CARGO_REGISTRY_DEFAULT`; R2 show all-malformed transitive as `not verified`; R3 network-profile
flag only; LOW-E1 UNC-only, mapped drives deferred. Small fixes (O_NOFOLLOW `compile_error!` on
unlisted unix targets, no "col 0", no "off" flash after enabling, footer deep-links to Dependency
checks) implemented by `senior-dev` → `reviewer` **approve** (0 MUST-FIX; orchestrator re-ran
vitest 38 files / 442, tsc clean) → committed **`8b09665`**. Fold into h4: off-retry only while view
is null/loading (else ~2 s "loading" + up to 9 IPC per focus when backend genuinely off);
`HeaderToolbar.tsx:45` focus prop → `SettingsFocusTarget | null`; stale comments
`SettingsContext.ts:103`, `SettingsShell.tsx` ~150; ui-designer ack for switch focus.
**h1** `reviewer` approve (0 MUST/SHOULD) → orchestrator re-ran bonsai-deps 359 passed / 3 skipped
→ committed **`3d59656`**. **h2+h3** (+ `profile_on_network` narrowed to user-scoped bases by
orchestrator decision) → `reviewer` approve + `security-auditor` OK to commit (0 MED+; LOW-H1
`%VAR%@`/`$(…)@api.nuget.org` judged public — fixed; label bug Mapped-private vs LocalFeed — fixed)
→ orchestrator re-ran bonsai-deps 371 passed / 3 skipped, `deps::` 30/30, clippy/fmt clean →
committed **`06ef9a9`** (incl. ui-designer's `P125-ui.md` amendments 694→842). Now: **h4** (TS
mirror, fixtures, UI, `?deps=unverified|netprofile`) with `senior-dev`, then `reviewer` +
`ui-designer` + harness, tester, full gate.
**h4** `reviewer` approve (0 MUST/SHOULD; 978 vitest) + `ui-designer` approve after MUST-FIX
(expanded-row hueless pills `--text-2` 4.06:1 dark → `--text-1`, applied by orchestrator, verified
computed colour in harness); SR ruling: pill only, no suffix (P125-ui.md amended). Harness
(orchestrator): `?deps=unverified` 8 rows + titles as §9, `?deps=netprofile` lead verbatim, 0
console errors. Re-ran vitest 81 files / 978, tsc clean → committed **`3ad900b`**. `tester`: every
P125h item mapped to a test, 3 gaps filled, offline smoke `repo-h` (18 requests; NuGet folder-feed
sentinel absent; crates.io sent with `registry.default`), checklist §C steps 13–18 → committed
**`14402d1`**. O_NOFOLLOW `compile_error!` UNVERIFIED on non-Windows targets (no cross `cc`).
**AI GATE GREEN 2026-09-29 at `14402d1`:** `pnpm gate` — ✓ all 11 steps passed, 782.4s (nextest
3224 passed (1 leaky) / 15 skipped; doctests 13; fmt; clippy; eslint; size; vitest 328 files /
3481; tsc + build; e2e 185 passed; perf counts 9 passed; perf ratchet OK). Log
`D:\Data\Temp\P125-followups\gate\gate.log`. **Now: awaiting USER CHECKPOINT** at `14402d1`
(`docs/contracts/P125-smoke-checklist.md` §C steps 1–18). Not declared passed. Follow-ups: calm line "All checked packages are up to date…" shows when zero
packages were checked (netprofile/mirror) — hide when no public rows; not-verified *cause* is
hover-only on non-expandable rows (keyboard/SR can't reach it); combined cell id ~60px. Follow-ups: extend `localFeed` to Cargo
`directory`/`local-registry` and pip `--find-links` (architect); `scope_of` doesn't catch a reused
index below N; mapped drive letters (LOW-E1). Old: **h1** (Rust §0/§1/§2/§4c) with
`senior-dev`. Then h2 (NuGet localFeed + R1), h3 (network profile), ui-designer copy, h4 (TS/UI),
`security-auditor` over h1–h3, tester, full gate. USER CHECKPOINT will run at the final P125h
commit, not `22b96a0`.
- Spec (what/why, 13 acceptance criteria, decisions D1–D9): `docs/specs/009-dependency-tracker/spec.md`.
- Goal: for the open repo, show direct deps with available updates (patch/minor/major) and
  direct + transitive deps with known vulnerabilities, for npm/pnpm/yarn, Cargo, PyPI, Go, NuGet;
  unsupported ecosystems listed as "not tracked".
- Hard lines: static manifest/lockfile parsing only, never run repo tooling (AC11); opt-in, one
  global switch, off by default (D1); private/path/git/workspace dep names never sent (D7);
  opt-out deletes cache (D6); 12 h default interval, configurable (D4); manifest change = mark
  stale only (D5).
- Research leaning (not binding on `architect`): deps.dev + OSV.dev (keyless) with
  native-registry fallback. Later phases: Maven/Gradle, Composer, Pub, Renovate-if-installed,
  ignore/snooze.
- Milestone-sized: new outbound network capability (CSP/capabilities) → `security-auditor`
  review of the contract before implementation, not after.

## P116 — `OBS_SCHEMA_VERSION` TS/Rust drift + parity test — `done` → **archive Part 83**

**ARCHIVED 2026-09-23.** `done`, `reviewer` approved (no MUST-FIX), committed **`e49cf20`**, and its
own text states **"No USER CHECKPOINT applies"** — nothing user-visible changed, so there was no
native half to clear. That made it the only cleanly archivable milestone on this board; P113, P115,
P117 and P118/P118b all stayed. Full text: `docs/history/todo-archive-2026-09.md` **Part 83**.

**The two things it left live:** the guard it added (`src-tauri/src/obs/tests_schema_parity.rs`) is
what P117 inc 2's 2 → 3 bump had to move — see the P117 section (archive Part 89.1 since
2026-09-28); and its reviewer follow-up about
`docs/contracts/P91-observability.md` is folded into the one canonical contract-drift entry under
`### Filed 2026-09-16 — the real-log residue`, re-measured 2026-09-23.

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


### Filed 2026-09-28 — residue of the three rulings (`1e7223f`, `4ba7ffe`)

- **SHOULD-FIX → `architect`:** `docs/contracts/P91-observability.md:558` (§5 `render-storm` row) and
  `:1843` still state `renders > 3 × instances`; §3 `RenderTallyPayload` lacks `strictModeDoubling`.
- NIT: `anomaly.rs:181` `real = renders / factor` floors — 13 raw / 2 instances fires but reads
  "6 renders vs 2 instances" (exactly 3×). Show the fraction or lead with the raw count.
- NIT: no vitest guards layer 1 of the UI-settings masking (`setUiSettings` absent from
  `src/obs/rawArgPolicy.json` ⇒ `buildRawArgs` sends no values); only the writer layers are tested.
- NIT: stale comments `src-tauri/src/commands/tests_repo_graph_cache_rearm.rs:40,:118` point at
  `graph_cache/tests.rs::…`; those tests now live in `graph_cache/tests/classify.rs`.
- Fragility: `obs_tree_reaches_no_http_dependency` self-skips by the filename `tests_metrics.rs` and
  now also scans `obs/tests_metrics/*.rs` — a future "reqwest" literal there would break it.
- Out of scope, noted: `graph_cache/tests_filter.rs` / `tests_fold.rs` keep private fixture copies.
- Stale prose quoting the old rule: `src/components/sidebar/sidebarBusyContext.ts:12`,
  `src/components/Sidebar.busyChurn.test.tsx:14`.

### Hoisted 2026-09-28 — open residue of the milestones archived as Parts 88-90

One line each; full context in the Part named. **Not re-verified by the curator unless marked
"(re-measured 2026-09-28)".** None blocks anything.

**Owed to the user**
- **`CLAUDE.md`'s gate rule is factually wrong** — it says bare `pnpm gate` "already runs the `rust`,
  `frontend` and `audit` groups"; `scripts/gate.mjs:79` gates audit behind `--full`. Fix: say
  `--full`, or pair the bare gate with `cargo deny` + `pnpm audit`. Canonical write-up: Part 88.3.
  (re-measured 2026-09-28: `CLAUDE.md:210` still carries the sentence; `gate.mjs:79` unchanged.)
- **Does the 2026-09-28 Linux/macOS run also cover P117's Linux-only gap?**
  `commands/tests_repo_graph_cache_rearm.rs:228` is `#[cfg(any(windows, target_os = "macos"))]`,
  so on Linux that leg vanishes silently. The attestation named P115, not this leg. Part 89.1.
- **P124a2 (Azure `commitsbatch` batching) — `deferred`:** revisit only if a checkpoint session
  still shows `forgeCommitStatuses` as the top cost; needs a live PAT probe by the user. Part 90.6.

**Forge / CI (P113a–c, P121, P124)**
- 100-sha status batch is still serial GETs in one `spawn_blocking` (13.2 s max); options: cut
  `MAX_STATUS_BATCH` to ~25 for Azure, or parallelise 4–8. Parts 88.1, 90.3.
- Back-off coverage is partial: `PrPanel`'s `forgeListPrs` / `forgeGetPr` / `forgePrDiff` neither
  feed nor consult the suppression window (6 of 8 observed rate limits were `forgeListPrs`). Part 88.1.
- UI: blank CI badges during a suppression window look identical to "no CI configured" →
  `ui-designer`. Part 88.1.
- NIT 6: a 404'd sha keeps its stale badge on a cut-short batch (`useForgeSignals.ts:271`); fix =
  `attempted` count on `CommitStatusBatch`. Part 88.1.
- NIT 8: `mock/handlers/forgeRateLimit.ts:48-53` doc contradicts `rateLimitCutoff(1) = 1`. Part 88.1.
- NIT 9: typo "A absurd" (re-measured 2026-09-28: still at `crates/bonsai-forge/src/ratelimit.rs:18`).
- NIT 11: `src/ipc/mock/handlers/forge.ts` (re-measured 2026-09-28: 492 lines) — keep new seams in
  their own files.
- P113b: the root cause of the 12 `listTagSync` `dup-ipc` pairs was never confirmed (the 2 s floor
  covers it). Part 88.1.

**Redaction / obs (P113c, P117, P122)**
- SEC-2026-09-22 LOW: `tests_redact.rs:237-264` pins only `str`; add `fn`, `arr:3`, `obj:2` and
  an unknown token under sensitive keys as a tripwire (re-measured 2026-09-28: no `arr:3` / `obj:2`
  in that file). Part 88.1.
- Hardening: `scrub.rs:401`'s `continue` skips `scrub_string` + home masking for exempt values;
  use `scrub_value(val, home); continue;`. Part 88.1.
- Lock the `AppError` wire shape (deferred background task from P113). Part 88.1.
- `buildRawArgs` in `src/obs/rawArgPolicy.ts` needs the same `Array.isArray(row)` guard that fixed
  `repoIdArg('constructor')` (re-measured 2026-09-28: no `Array.isArray` in that file). Part 89.1.
- Audit INFO (i): `strict.rs`'s `repo` field rule is top-level only; make `walk` key-aware. Part 89.1.
- Audit INFO (ii): the `redact_names` whitespace gap leaks `project` from `D:\Repos\my project` in
  any other string field — file against whichever increment first gives `LogPayload::Error` a
  producer. Part 89.1.
- Accepted residual (P117): strict-mode `logAppend` IPC batches carry raw paths; re-open if a
  "dump detector state" affordance is ever added. Part 89.1.
- `effect-no-change` (P91 §5, `src/obs/react.ts:113`) may never have been wired, like
  `superseded-result` was. Part 90.4.
- The Rust-side `event` record always reports `listeners: 0`. Part 90.4.
- `frame` record `ts` is the flush time, so `jank-trace` can mis-attribute `worstMs` up to 120
  paints later. Part 90.4.
- P122d's 17 vs 300 `outcome: superseded` counts were never reconciled. Part 90.4.

**Graph cache / perf counters (P117, P120)**
- `seed_fingerprint` is refs-only (`crates/bonsai-core/src/graph/seed.rs:43-55`): `.git/shallow`,
  `info/grafts`, `refs/replace/*` changes can serve a stale layout as `HitVerbatim`. Fix shape:
  fold `is_shallow()` / `.git/shallow` mtime in. Part 89.1.
- `perf.repo_opens` = 564/day — re-measure now that the cache carry-over landed. Part 89.1.
- Open question, do not fix blind: day-wide `perf.graph_walks` over-reports by exactly the hit
  count. Part 89.1.
- P117 AC1-7's green rested on senior-dev's run when reviewed; the later 561.2s full gate ran over
  both commits — confirm and close. Part 89.1.
- P120 MUST-FIX 2: cold-open `layoutShiftsUnprompted` is pending a structural-blindness proof under
  the paused clock (currently not ratcheted). Part 90.2.
- P120 AC 8 (count tracks clock) — `deferred`: needs a **wide** fixture (hundreds of lanes); the
  reverted `lane.rs:149-158` `pending.remove(&oid)` change is the ready candidate. Part 90.2.
- Perf ratchet reach: baseline seeded only for `msedge`; seed a `chromium` channel and add perf to
  `ci.yml`. Part 90.2.
- Mock harness boots log 6× `dup-ipc` warn anomalies, clean boots included — triage. Part 90.2.
- Intermittent extra sidebar `layout.shift` in 1 of 3 `?perfInjectFocusSteal=1` runs. Part 90.2.
- Checker NIT: a counts file with no channels passes with 0 counts checked. Part 90.2.
- `background-refresh` zero `recalcStyle`/`layout` ceilings are exact — watch for a 1-rep flake.
  Part 90.4.
- The `details` field shift (0.0011) from `s229d042a` never reproduced in the lab. Part 90.4.
- P120 review NITs: `readiness.ts` tab stuck in `boot` after a failed `openRepo`; `devSink.ts` boot
  bucket held up to 30 s; `obsAnomaly.ts:219` pass `emit`; `obsAnomalyStability.ts:85` `toFixed`
  throw; `lane.rs:153`; unused `Deserialize` at `counters.rs:14`; duplicated preamble
  `graph.rs:166-177`. Part 90.2.

**Sidebar render stability (P118/P118b, P121)**
- The deferred container state-model pass: `data.remote` and `remotes` commit in separate
  continuations, so a round changing both still costs 2 real renders. Part 88.2.
- BranchesSection data-prop pass: it takes `data={data}` whole; `localTree`'s `: []` fallback
  belongs there (`Sidebar.tsx:204-210` comment). Part 88.2.
- Generalise `useStableRemoteRefs` → `useStructurallyStable<T>` in that pass. Part 88.2.
- `flushRenderTally` nulls `timer` without `clearTimeout` (re-measured 2026-09-28:
  `src/obs/renderTally.ts:80-82`); `Sidebar.busyChurn.test.tsx` now uses fake timers (P119), but
  `talliesByComponent` should throw on a duplicate key. Part 88.2.
- `Sidebar.test.tsx:80-90` leaves the configured-remote-but-no-tracking-refs arm untested. Part 88.2.
- Migrate `Sidebar.churn.test.tsx` / `Sidebar.busyChurn.test.tsx` setup to `src/test/renderTallyKit.ts`.
  Part 90.3.

**Settings prune (P115)**
- `pinnedBy: "user" | "migration"` marker for the accepted same-host basename residual —
  `ui-designer` first; contract §10. Part 88.3.
- Surface kept-dead forge overrides (ambiguous basename) — UI increment; contract §10. Part 88.3.
- `src-tauri/src/settings.rs` is at exactly 500 lines (re-measured 2026-09-28: 500); split
  `record_recent` + hooks-ack helpers into `settings/recents.rs` (`refactorer`). Part 88.3.
- `hooks_ack_repos`, `repo_forge_overrides`, `open_repos` are uncapped and re-probed every launch.
  Part 88.3.
- `recents-changed` event: first `getRecentRepos` can show the unpruned list once. Part 88.3.

**Activity dock (P119, P123)**
- P119-4 NITs: `GitDockApi.activeRun` unread outside the hook; defaulted `targetCount` at
  `gitActivityState.ts:76`; `useMinimumBusy` sticks if `minMs` changes mid-hold; merge the two
  `.git-dock-progress` blocks in `git-dock.css`; single-item `deleteBranches` label. Part 90.1.
- P119-2/3: coverage tests missing for submodules, compose, interactive rebase/continue/skip,
  checkoutRemote, deleteRemoteTracking, network tag ops, bisect skip, command-level HookRejected.
  Part 90.1.
- P119-2/3 NITs: mock FF merge can move HEAD to a phantom oid; stray trailing blank lines in mock
  `stash.ts` / `status.ts` / `tagSync.ts` / `rebase.ts`; record that `path_leaf` refuses `repo@2`.
  Part 90.1.
- Test-kit bug: `resetObsConfigForTests` (`src/obs/enabled.ts:89-92`) clears the IPC proxy's
  listener, so later `vi.spyOn(mockIpc, X)` spies go unseen (workaround at
  `TagsSection.recheck.test.tsx:146-149`). Part 90.5.
- A rejected `gitActivitySubscribe` is swallowed with no retry (`useGitActivitySubscription.ts:35`).
  Part 90.5.
- `lib.rs:1` `pub mod activity_hub;` grouping; `subscriber_count` visibility. Part 90.5.
- The tab strip has no busy indicator for background repos (`ui-designer` suggestion). Part 90.5.
- `toolbarReadoutShift.spec`: add a `scrollWidth <= clientWidth` fit check (ui-designer SHOULD-FIX).
  Part 90.4.

**Watcher / tag sync (P121, P124)**
- P124 NITs: rename `useForgeSignals.test.ts` → `forgeCiCache.test.ts`; `burst.rs` doc overstates
  the `.git`-noise proxy; tidy `#[cfg(test)]` re-imports in `watcher/mod.rs`; drop the `count()`
  helper in `ignore.rs`; `IgnoreStats.worktree_paths` naming. Part 90.6.
- `ignore.rs`: doc note that a deleted dir under a `dir/`-only rule may undercount. Part 90.3.
- `useTagSync.ts` refetch comment: the 10 s cache now also covers in-flight re-checks. Part 90.3.

**Contract drift → `architect` (contracts are not the curator's to edit)**
- `P91-observability.md:1122` lists `is_sensitive_key` with no exception; code has the P113c
  carve-out. Part 88.1.
- `archive/P63-forge-graph-signals.md` and `archive/P90-ci-checks.md` still say
  `forgeCommitStatuses → CommitStatus[]` "same order". Part 88.1.
- `P117-perf-signal-fixes.md`: §1.2 calls `info.path` canonical (it is the raw caller string);
  AC1-4 is unimplementable as written; §1.2 omits `same_repo_path`'s `eq_ignore_ascii_case`
  fallback (`repo.rs:408`). Part 89.1.
- P86 B1's "reset to `None` on `open_repo` re-arm" contradicts P117's preserve-on-re-arm; P91
  §2.5 + §5.1 need `repoId` on `RefreshPayload` and `(repoId, scope)` keying. Part 89.1.
- `P119-activity-for-all-actions.md` §6 T-R2 `r.git` line is stale (now `None`). Part 90.1.
- Tag-sync contract §2.2 "badge-less while checking" is stale for re-checks (P121 Q1 / P123
  ruling 2). Part 90.3.
- `P113d-watcher-log-volume.md` says `OBS_SCHEMA_VERSION` stays 2; code is 3 — see `## P113d`.

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
  *(2026-09-28: P117 is now `done` by user attestation, archive Part 89.1; the contract delta is
  under `### Hoisted 2026-09-28` → Contract drift.)*
  Pre-sweep text: archive Part 86.1.
- **Dev-mode log VOLUME:** 10,280 watcher records = **87%** of a 2.2 MB / 6-minute log, and **7,247
  had `relevant: 0`** — a record per file change it then correctly ignores. Cost, not correctness.
  → the live fix is **`## P113d`** (`pending`).
- **✅ CLOSED 2026-09-23 (curator-verified in the tree) — `RemotesSection` re-rendering on a
  local-branch change.** Closed by **P118b, `c6ae304`**. Verified at source, not from the subject:
  `src/components/sidebar/RemotesSection.tsx:13-17` no longer takes `data: BranchesSnapshot` at all
  — it takes `hasRemoteRefs: boolean`, with the reason in a doc comment — and the component is
  `memo`-wrapped at `:156`. The board's own P118b block records the measured floor (the assertion
  moved from `{renders: 2, instances: 1}` to `toBeUndefined()`) and the third change that was
  actually needed, `sidebar/useStableRemoteRefs.ts`. **P118/P118b themselves are NOT done** — they
  read `reviewer approved`; only this follow-up closes. *(2026-09-28: P118/P118b are now `done` by
  user attestation, archive Part 88.2.)*
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

**✅ RULED 2026-09-28 (user): divide the doubling out.** The tally corrects for the StrictMode
double-render so the `3×` factor means 3× *real* renders; fix the false `aggregate`-mode docstring.
(The check has drifted to `anomaly.rs:173`.) **Done `4ba7ffe`** — optional `strictModeDoubling`
flag on `render.tally`, Rust doubles the budget when set; reviewer approve, no MUST-FIX.

### Open from P112 sub-increment 2 — the observability-capture privacy decision (verbatim; narrative: archive Part 78.1)

- **STILL OPEN — the observability-capture privacy decision.** `setUiSettings`/`getUiSettings` are
  both already in the captured-command list (`obs/metrics_cmds.rs:130,209`) and `obs/record.rs:207`
  has an optional raw-payload capture. Now that `pick_external_tool` returns a browsed path and
  `tool_scan` returns `DetectedTool.detail`, a **filesystem path can be written to a log file on disk
  under dev mode**. Decide whether those two commands stay in the capture set — same class as P91's
  home-masking work.
  **✅ RULED 2026-09-28 (user): keep both in the capture set, home-mask their path fields** (P91
  `scrub_home.rs` masking), with a regression test proving a browsed path reaches disk as
  `<home>…`. **Done `4ba7ffe`** — the pipeline already masked them (rawArgPolicy deny →
  `raw_args::enforce` → writer `scrub_value` home-mask); only regression tests were added.
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

**✅ LARGELY CLOSED — re-verified in the tree 2026-09-24; the "STILL OPEN" line above is stale.**
`useSettingsWriteQueue.ts:138-141` now logs the raw error on every failed attempt, before
`noteFailure(streak)` at `:151`: `console.warn(\`bonsai: settings write failed (attempt N of M):\`,
errorMessage(e))`. Its comment says why: the `ipcProxy` sink records only the `AppError.kind`
(`'io'`), and only with Dev capture on. **What is left:** the raw error goes to the **webview
console**, not the dev-mode JSONL log the routing asked for, so it is lost unless devtools is open
at the time. Downgraded from a diagnostic regression to a **follow-up: mirror the raw message into
the obs sink**. Not a release blocker.


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
~~**NOT queued — awaiting the user.**~~ **✅ RULED 2026-09-28 (user): split the 7 genuinely
oversized files** (602/588/568/543/522/520/519, incl. `ai_operation_preview.rs`); the 8 marginal
ones stay baselined and fall back under on normal cleanup. **Done `1e7223f`** — largest resulting
file 327 lines; per-module test counts identical; security-auditor CLEAN on the `tools/` split.

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
| `docs/history/todo-archive-2026-09.md` | **Parts 88-91 (moved 2026-09-28, after the user attested every pending native checkpoint confirmed — "Verified by me, incl. P115"):** P113a–c (with P113d's design text, P113d itself still live) · P118/P118b · P115 (88.1-88.3) · P117 (89.1) · P119 · P120 · P121 · P122 · P123 · P124 (90.1-90.6) · superseded release + curator bookkeeping — the `Not archived` paragraph, the release `Current step` + bullets, the V1–V4 BLOCKS RELEASE list, the 2026-09-23 curator note (91.1-91.4). **Parts 83-87 (moved 2026-09-23):** **P116 in full** — `done`, `e49cf20`, its own text states "No USER CHECKPOINT applies", the only cleanly archivable milestone on the board (83) · **P117's pre-tightening text** — NOT an archived milestone, P117 is live and `awaiting USER CHECKPOINT`; 380 → 352 (84) · **P115's pre-tightening text** — NOT an archived milestone, P115 is live and `in-progress` with a Linux/macOS run owed; 134 → 134, rebalanced not shrunk (85) · **the open follow-up entries rewritten by the staleness sweep** (86.1 the real-log residue · 86.2 A4 finding 6's citation · 86.3 the U+200B closure · 86.4 the 20-file split queue · 86.5 the `gitbin.rs` line count · 86.6 the gate `DEP0190` bullet · 86.7 the velocity follow-ups · 86.8 the struck `no_proxy_client` · 86.9 P80 forge follow-up (a)) · superseded curator bookkeeping (87). **Parts 76-82 (moved 2026-09-22, after the user confirmed P112's native checkpoint on 2026-09-22 — `ba4b9d3`):** P112 in full, incl. the five checkpoint items as confirmed and item 4's `set_parent` sub-question (76) · the completed 2026-09-14/15 queue, the superseded `5654eaa`/`934a280` gate states, ruling #24's scope facts, the second-round rulings' evidence blocks and the whole 2026-09-11 ruling queue (77) · the 2026-09-16 session — the real-log investigation, the nine-file second review pass and its closure, the Settings-scrim finding, **P113 phases 1+2** (78) · the 2026-09-17 session — the orphaned-credential arc, **P114**, contract hygiene, the rustfmt pass, three security audits with their two CLEAN registers, and the superseded `ea6d323`/`5f015be`/`3948478` greens (79) · the release block's pre-tightening text (80) · P91's closed `logs/*.jsonl` parse + the `mono` defect it found + the superseded `cargo fmt` section (81) · superseded curator bookkeeping (82). **Parts 71-75 (moved 2026-09-16):** the P112 sub-inc 3/4 build + review transcript incl. the `P112-ui.md` §17 rulings, the four bad citations, the coalescing lesson and the sub-inc-3 audit (71) · superseded gate states (`d0e6cf0`, `dcff54b`, the 427.4s confirming run, the `e9ed93d` Rust tier) and the completed 2026-09-14 queue — F6, P77, the e2e cold-timing measurement, the UNC clearance (72) · the P112 sub-inc 2 + P113 phase-1 review transcript (73) · the two items CLOSED 2026-09-16 with their evidence — "Open in editor" (fixed `fd93616`, with its `os error 193` measurement table) and the false General subtitle — plus the `.cmd` launch-path audit (74) · superseded curator bookkeeping and the duplicated `cargo fmt` measurement (75). **Parts 62-70 (moved 2026-09-14, after the user ruled all 22 FOR-USER items on 2026-09-11):** the stale 2026-09-10 resume block + FU-1 residue (62) · the FOR-USER evidence blocks for items 0-6 (63) · the `IN FLIGHT` queue, the 2026-09-11 orchestrator closures, the unreviewed-MCP-merge warning (64) · **`SEC-2026-09-11`**, the MCP tool-contract audit, with its verified-CLEAN register (65) · **`SEC-2026-09-11b`**, the review of that implementation, with its verified-CLEAN register (66) · P108 `AC11`, closed by ruling #13 (67) · the happy-dom load-flake narrative (68) · the open follow-ups as they stood pre-condensation (69.1 P91 · 69.2 SEC-2026-09-03 through the 2026-09-01 hoisted items · 69.3 P69 Settings) · superseded curator bookkeeping (70). **Parts 54-61 (moved 2026-09-10):** the whole USER-CHECKPOINT block — P102+P105, P106, P107, P108, P91 (54) · P110 + P109 (55) · the 2026-09-03 closures + SEC-2026-09-03 remediation (56) · `Queued housekeeping` incl. the `e149382` CSS-split proof (57) · the superseded `c218258` and earlier gate states (58) · the 2026-09-10 session: P111, FU-1, six reviewer-follow-up closures (59) · the board's record of the confirmation (60) · superseded curator bookkeeping (61). **Parts 51-53 (2026-09-03):** the `5c2dcd2` + `c6cd7dd` gate states and the e2e-contention mis-diagnosis · the full narratives of everything closed 2026-09-03 · the durable-lessons stories and worked numbers. **Parts 36-50 (2026-09-03):** the file-size refactor pass · P102+P105, P106, P107, P108 and the P91 security arc + audit + build diary · superseded pre-ship filings · the 2026-09-03 velocity pass · P99, P100, P101, P98, P95, P96, P97 · built-bundle e2e + P103 + P104 · the DX/velocity stubs · the pre-condensation open-follow-up text. **Parts 33-35 (2026-09-01):** the P84 record gap · macOS ad-hoc signing · the two 2026-08-22 design reviews. **Parts 22-32 (2026-09-01):** P94 · P93+P92 · DEP REFRESH · P90+P89 · P88 · the P85-P87 batch · P82+P83 · divergence reconcile + Release 1.1.0 · the DX dev-loop text · the confirmed-checkpoints block · the 2026-08-21 resolved follow-ups. |
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

### Why this board is ~1960 lines, not ~300 (curator note, 2026-09-28)

- **3115 → 1961.** 1270 lines of milestone sections left the board (P113a–c, P115, P117,
  P118/P118b, P119–P124), cleared by the user's 2026-09-28 attestation. The predecessor note is
  archive Part 91.4.
- **Protocol:** 14 ranges appended to `todo-archive-2026-09.md` as Parts 88-91 and each checked
  byte-identical in place (`cmp`) **before** its removal from this file. Removals were done with
  `Edit` only, bottom-up, with `wc -l` after each; every step shrank by exactly the range moved.
- **No status was upgraded by the curator.** Every `done` here is the user's attestation.
- **What came back on:** a 10-row `## ✅ DONE 2026-09-28` table, a short live `## P113d` entry, and
  `### Hoisted 2026-09-28` — the open residue of Parts 88-90, one line per item, ~145 lines.
- **What sets the floor now:** `## OPEN follow-ups` (incl. the hoisted residue) is 1017 lines, ~52%
  of the board. Curating cannot move it; working it down can.
