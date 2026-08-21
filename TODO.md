# Bonsai — Milestone TODO

> Single source of truth for session resume. Keep the "Current step:" line of the
> in-progress milestone updated at every workflow transition.

Environment: Rust 1.97.1 stable-msvc, VS Build Tools 2022 17.14, pnpm 11.17.0, Node 24, WebView2.
Cargo not on default PATH — `$HOME/.cargo/bin`. Browser harness: `pnpm dev:mock` (port 1420).
Avoid tauri "test" feature on this machine (STATUS_ENTRYPOINT_NOT_FOUND); use runtime-free
inner functions for command tests.
**USER MANDATE (2026-07-28, updated 2026-08-04 for cross-platform support): on Windows, never use
C: for temp/scratch/mock repos — C: is critically full. Use `D:\Temp\bonsai-scratch`; when running
cargo tests set TMP/TEMP to `D:\Temp` (tempfile honors them). On macOS/Linux, `scratch_dir()` now
falls back to the OS temp dir (`std::env::temp_dir()/bonsai-scratch`) automatically — no special
handling needed there. Include the Windows-specific guidance in every subagent prompt that runs
tests or creates repos only when running on a Windows machine.**

## Board conventions

Status vocabulary: `pending` · `in-progress` · `done` · `awaiting USER CHECKPOINT` · `deferred`
(deferred always carries a one-line reason). A milestone is `done` only when the AI gate **and** the
native USER CHECKPOINT have both passed — the orchestrator never self-declares the second half.

**History is archived, not deleted.** Archiving is a **move**, never a delete: condense on the board,
keep the full text in the archive, and leave a pointer. Archive files are listed at the bottom;
contract files are indexed in `docs/contracts/INDEX.md`.

---

## 🚢 Release 1.1.0 — cut 2026-08-20

Version files bumped to 1.1.0; `CHANGELOG.md` `[1.1.0]` finalized 2026-08-20 (Settings redesign,
audit-2 fixes, P70–P74). **Final tag → `e3cd2ea`** (the first `v1.1.0` tag failed macOS+Linux CI —
`gitbin::parse_reg_query` was dead code off Windows; `e3cd2ea` gates it `#[cfg(windows)]`, tag moved
onto it).

**P62–P74 native USER CHECKPOINTs were WAIVED and marked `done` 2026-08-20** (user decision): P62–P65,
P67, P68, P69 Settings (P69a–P69l), P71–P74. **P70 was NOT waived — its checkpoints were run and
confirmed (item 1 2026-08-20, items 2–8 2026-08-21); P70 fully `done`, archived → Part 17.** Full
build detail: `docs/history/todo-archive-2026-08.md` Parts 2, 4, 5, 7, 8, 11–15. Open follow-ups
spun out of those milestones are on this board below (NOT closed by the waiver).

---

## 🌿 P80b — commit-panel UX overhaul + next-file bug — AI-gate GREEN, native checkpoint pending

> Numbering note: this shipped from a **concurrent worktree session** that also used the label
> "P80" (contracts `P80-commit-panel-ux-ui.md`, commits `7ebe7fd`…`03a6453`). The main-session
> "P80 — multi-account forge" (below, `done`) is a different milestone. Tracked here as **P80b** to
> disambiguate; the on-disk contract/commit labels are left untouched.

**Branch:** `worktree-commit-panel-ux` (worktree `.claude/worktrees/commit-panel-ux`; forked from
origin/main to avoid the concurrent main session). Not yet merged to main.

**Current step:** all four commits landed & reviewed; AI gate green (the only nextest red is the
pre-existing `prop_status` porcelain proptest, unrelated). **Native USER CHECKPOINT CONFIRMED by the
user 2026-08-21** (amend-focus + visual pass). Ready to merge to main.

Origin: user asked for a designer pass over the right-panel Working tab (make staging/committing
easier, maximize list space) + a bug where staging opened a "random" next file. Contract:
`docs/contracts/P80-commit-panel-ux-ui.md`.

- **Bug fix** (`7ebe7fd`): auto-advance after staging now uses the *rendered* order (tree order via
  `buildPathTree`/`flattenTreeLeaves` in tree view, flat otherwise) instead of flat backend order.
  `listView` threaded into `useCommitActions` via a ref. +3 tests.
- **2a staging affordances** (`5707f9a`): persistent stage `+`/`−` toggle (was hover-only); resting
  danger treatment on "Discard all"; empty-Staged/Changes placeholder hints; section labels + bulk
  buttons off sub-AA hues onto `--text-2`.
- **2b space-saving footer** (`7edff3d`): footer chrome ~150→~87px (**~63px reclaimed to the
  lists**, live-measured commit box 137px). All modifiers folded into one `⋯` `CommitOptionsMenu`
  (Amend/Sign/Skip/Stash/Compose/Review; arrow-key roving, focus-first, Escape/outside close).
  Amend moved into CommitBox with an internal reseed effect (dropped the amend remount key; no
  longer destroys an in-progress draft). Deleted `RightPanelActionsRow` + `CommitOptionsRow`.
  E1 consolidated the four AI entry points to one context-scoped Review + toolbar Generate.
  **D1: new "Primary commit action" setting** (General → Committing, default **Commit** — user chose
  "make it a setting"), full-stack Rust+TS+mock; CommitBox swaps which button is `.btn-primary`.
- **2c a11y + microcopy** (`03a6453`): single `.commit-note` line with `⚠`/`✓` glyphs (hue no longer
  sole carrier); plain-language signing copy (dropped the `user.signingkey` leak into a title);
  section `aria-labelledby`; empty-state `--text-3`→`--text-2`; 24px targets verified.

**AI gate:** vitest 1981 ✓ · e2e 156 ✓ · settings Rust lib 52/0 ✓ · clippy/check/doctests ✓ ·
tsc+build ✓ · eslint ✓ · file-size ratchet ✓. Live DOM harness confirmed the new footer, toolbar,
consolidated `⋯` menu, and Commit-primary default. Full-workspace `cargo nextest` could not complete
— **environmental: D: drive 100% full (os error 112 writing the target cache)**, not a code failure.

**USER CHECKPOINT (native `pnpm tauri dev`):** (1) amend-toggle keyboard-focus retention in the real
webview (the reseed refactor dropped the remount; jsdom can't prove focus); (2) visual pass on the
compact footer + reclaimed list space + the `⋯` menu in both themes (this harness is headless — no
screenshots). Then re-run the full gate on a machine with disk headroom and merge to main.

---

## 🔁 P81 — refetch coalescing + watcher self-echo suppression — AI-gate GREEN, native checkpoint pending

Resolves the audit-#1 §3.10 refetch storm: every mutation ran `refreshAll` (~9 parallel fetches) and
the watcher-debounced `repo-changed` for the same writes re-ran the identical 9 ~300 ms later, per
open tab. Fix (`be01422`): a refresh coalescer + per-repoId watcher-echo suppression
(`ECHO_TTL_MS=600`). Contract `docs/contracts/P81-refetch-coalescing.md`.

**AI gate:** GREEN (vitest 2070). **USER CHECKPOINT (pending):** user-visible only as fewer redundant
fetches; a native smoke pass (`pnpm tauri dev`) is still advisable to confirm no missed refresh.

---

## 📦 P82 — submodule dirty-deinit requires explicit force (F-A7-7) — AI-gate GREEN, native checkpoint pending

deinit/remove now require an explicit force opt-in for a dirty submodule (outcome enum
`DirtyNeedsForce`, zero mutation on refuse; Flow-A danger dialog). Frontend threads the choice through
the confirm flow. Fix (`ede7674`). Contracts `docs/contracts/P82-submodule-force.md` +
`P82-submodule-force-ui.md`.

**AI gate:** GREEN (bonsai-core 876, vitest 2086). **USER CHECKPOINT (pending):** destructive
force-deinit danger dialog needs a native visual + confirm pass in both themes.

---

## 🛠️ DX — dev-loop acceleration — in-progress

**Goal:** act on the full-workflow velocity analysis (2026-08-20) — 68 GB `target/`, no build
acceleration, serial clippy/test, the 4-file IPC lockstep, a ~12-milestone deferred native-checkpoint
backlog. Ten improvements.

**Current step:** 8 of 10 landed & verified (`3ada322`, `2019e71`, `8e55be8`). **P75 (IPC codegen)
HALTED 2026-08-21 (user decision)** — a Phase 6.1 spike found that linking `tauri-specta` breaks app
launch on Windows 10 (`kernel32!WaitOnAddress` not exported → `STATUS_ENTRYPOINT_NOT_FOUND`); it's a
dev-velocity refactor with no user value, on RC crates, and the Win10 regression is unavoidable
because completing it requires linking tauri-specta into the app. 6.1 changes reverted; findings kept
in `docs/contracts/P75-ipc-codegen.md`. P76 (native-checkpoint automation) **held as contract-only**
per user.

**Landed & verified:**
- Build loop (`3ada322`): `[profile.dev] debug = "line-tables-only"` + `.cargo/config.toml` rust-lld
  linker (windows-msvc; Linux/macOS left opt-in). Verified.
- `cargo-nextest`: `.config/nextest.toml`, `pnpm test:rust`, `cargo nt` (bonsai-core 1417 / 6 skipped
  under it, ~184 s).
- One-command gate `scripts/gate.mjs` → `pnpm gate [--quick|--full|--rust|--frontend]`; clippy runs in
  its OWN target dir (`target/clippy`), so the test⟂clippy shared-target race is structurally
  impossible.
- Process (CLAUDE.md): step 4 runs code + design reviews concurrently; step 5 makes velocity mode
  (MUST-FIX-only, SHOULD-FIX/NIT filed as follow-ups, targeted intermediate gates) the default;
  senior-dev gains a pre-handoff self-review checklist.
- God-file splits (`2019e71`, `8e55be8`): branches.rs 2284→114 and stash.rs 2197→121 into focused
  submodules (public paths preserved, 1417 tests identical); RepoWorkspace.tsx overlay cluster →
  WorkspaceOverlays.tsx (382 tests identical). **Finding:** RepoWorkspace.tsx is a *legitimate*
  container — only a modest 85-line trim was safe.

**In-progress / designed:**
- **P75 — HALTED 2026-08-21 (user decision).** Would generate the IPC boundary with tauri-specta v2
  (kill the types.ts / tauri.ts / mock-layer lockstep). Phase 6.1 spike outcome: RC crates build &
  pin (`specta rc.22`/`tauri-specta rc.21`/`specta-typescript 0.0.9`), `AppError` manual `specta::Type`
  works, `bonsai-core` 881 green — BUT linking `tauri-specta` forces `tauri/specta`, whose binary
  statically imports `kernel32!WaitOnAddress`/`WakeByAddress*`; Windows 10 (this dev box, 19045) does
  not export those from `kernel32.dll` (KernelBase/api-set only) → `STATUS_ENTRYPOINT_NOT_FOUND` on
  load, so the app itself won't launch on Win10. Since P75 is dev-velocity only (no user value), on
  RC crates, and can't be completed without linking tauri-specta into the app (Phase 6.5), the Win10
  regression is unavoidable — halted. 6.1 code/deps reverted; the pinned trio, the bigint tradeoff
  (0.0.12 drops `BigIntExportBehavior::Number`), and the full blocker note are preserved in
  `docs/contracts/P75-ipc-codegen.md`. **Revisit only if** validated on Windows 11 or with a
  link-order fix forcing the `api-ms-win-core-synch` import lib ahead of `kernel32.lib`.
- **P76 — designed (HELD as contract-only per user).** Automate the native USER CHECKPOINT backlog
  with tauri-driver + WebdriverIO (~60–70% automatable; macOS has no WebDriver so its checkpoints
  stay human). `docs/contracts/P76-native-checkpoint-automation.md`.

**Deferred cleanups (noted, not done):** lock the file-size baseline reclaim for App.tsx (P74) and
RepoWorkspace.tsx once those land; the duplicated private `open_repo_at` helper across many `git/`
modules (a real refactor with call-graph impact, not a leaf move).

---

## ✅ Confirmed checkpoints and accepted decisions (condensed — full text in the archive)

- **P70 — git-executable resolution.** USER CHECKPOINT verified by user (item 1 confirmed 2026-08-20;
  items 2–8 verified 2026-08-21); shipped in 1.1.0 (`f0e9aee`). Archived → `todo-archive-2026-08.md`
  Part 17. (Refactorer follow-up RESOLVED 2026-08-21 — already split, see resolved-this-session note.)
- **P77 — tag sync management.** USER CHECKPOINT (items 1–6) verified by user 2026-08-21; AI gate
  GREEN. Commits `721349d`/`67c42b4`/`d2695bd`/`97ae417`/`e76b20b`. Archived →
  `todo-archive-2026-08.md` Part 18. (Deferred follow-ups carried to OPEN follow-ups below.)
- **P78 / P79 / P80 — forge fine-grained-token guidance, account management, and multi-account
  (host default + per-repo override).** All three `done` — AI gate GREEN + USER CHECKPOINT CONFIRMED
  (user 2026-08-21). Commits: P78 `d50cd42`; P79 `74cdfe0`+`813d305` (+settings.rs split `3386c3d`);
  P80 `01bb97e`+`323f8c5`. Resolution order (P80): repo override → owner-match (login==owner,
  lowercased, exactly one) → host default → single → first+nudge. Full condensed detail →
  `docs/history/todo-archive-2026-08.md` Part 20. Genuinely-open P80 SHOULD-FIX/NIT follow-ups are in
  the OPEN follow-ups section below.
- **All native USER CHECKPOINTs for P2 → P61 are CONFIRMED.** Batches: 2026-07-30 (P4, P3a–P3f, P7,
  P7e, P7f, P8, P9), 2026-08-03 (P18–P27), **2026-08-08** ("mark everything as checked" — P28 through
  P61 inclusive: P32, P37–P46, the credential-cache and UX-fix batches, Phase 1 P49–P52, Phase 2
  P53–P57, Phase 3 P58–P61). P5/P6 were confirmed earlier still.
- **Accepted defaults (2026-08-08, "ACCEPTED AS-IS"; changeable any time):** P55 `undoLastMerge` =
  reset-to-first-parent (Mixed, rewrites history, confirm-gated) · P57 retriever = BM25 lexical, no
  embeddings · P61 image-diff base64 = hand-rolled, no new crate.
- **OD1 (confirmed):** AI stays **local-`claude`-CLI-only**; model tiers deferred.
- **Forge defaults (2026-08-08, accepted):** new Rust deps `reqwest{blocking,json,rustls-tls}` +
  `keyring` · auth = **PAT-only** v1 (OAuth device-flow deferred) · provider order GitLab → Bitbucket
  → Azure DevOps.
- **v1.0.0 shipped** 2026-08-18 (tag `bd52483`), unsigned; forge/PR flagged beta. Full text of every
  banner and decision: `docs/history/todo-archive-2026-08.md` Part 1 + Part 10.

**FOR USER — two open items from the 1.0.0 release I could not close (carry forward):**
1. **Back up `.tauri/updater-prod.key`.** Correctly gitignored and untracked, so it exists in exactly
   ONE place: this working copy. Losing it permanently breaks auto-update for every installed client.
   (The committed `tauri.conf.json` pubkey was verified to match it.) **Also: P71 must not touch it.**
2. **GitHub reported 2 Dependabot alerts (1 high, 1 moderate)** on push. The high is the known
   `nanoid` GHSA-2v37-7h3g-55p8 — build/test tooling only, deliberately ignored in
   `pnpm-workspace.yaml`. **The moderate is unidentified** — `gh` is not installed here; both project
   gates are green (`cargo deny` all ok; `pnpm audit` shows only the one ignored high). Check the
   Dependabot page.

---

## 🐞 OPEN follow-ups (spun out — genuine unresolved items, not checkpoints)

### ✅ Resolved this session (2026-08-21) — full text archived → `todo-archive-2026-08.md` Part 19
- **`read_status` vs `git status --porcelain` discrepancy — RESOLVED** (`f0eea9e`). Windows racy-git
  `WT_MODIFIED` phantom suppressed on Windows only (`#[cfg(windows)]`, git's `ie_match_stat`
  racy-clean rule); non-Windows unchanged. Regression seed appended to
  `crates/bonsai-core/tests/prop_status.proptest-regressions`.
- **CommandPalette highlight resets on `actions` array identity — RESOLVED** (`0798c55`). Reset now
  keys on the ordered visible row-id set, not array identity. vitest 14/14.
- **Refetch storm (audit #1 §3.10) — RESOLVED** (`be01422`, now milestone P81 above — native
  checkpoint pending).
- **Stash `expectedOid` UI wiring — RESOLVED** (`f36683e`). UI threads the rendered `StashEntry.oid`
  through the F-A6-B wrong-target guard. vitest 2079.
- **Submodule dirty-deinit force flag (F-A7-7) — RESOLVED** (`ede7674`, now milestone P82 above —
  native checkpoint pending).
- **`STDERR_GRACE_TOTAL` absolute cap — RESOLVED** (`95b7632`). `drain_stderr` now clamps each
  per-recv wait to the remaining time, so total ≤ `STDERR_GRACE_TOTAL`.
- **P70 credential-subsystem split (refactorer) — RESOLVED** (no action needed; item was stale).
  `crates/bonsai-core/src/git/cred.rs` (462 lines) already holds the full subsystem
  (`next_cred_method`, `credential_fill`, `acquire_cred*`, `map_remote_err`, `exhausted_error`,
  `evict_fresh_on_auth_fail`, `FillOutcome`, `CredAttempts`); `remote.rs` imports it — landed with
  P70's finalized tree.

### Known load-flake (still open) — timing-sensitive, not a correctness bug
`ai::session_tests::watchdog_does_not_fire_while_awaiting_input` failed once under load and passed on
immediate re-run.

### P80 forge follow-ups — **OPEN** (SHOULD-FIX/NIT, non-blocking; spun off the archived P80 milestone)
- (a) `forge_set_token_inner` validates before the `host.is_empty()` guard — guard host first to skip
  a wasted round-trip on unparseable origin.
- (b) keychain-write-then-settings ordering: a failed `settings::update` leaves an orphaned keychain
  token (currently `let _ =`) — surface the error.
- (c) re-connecting a migrated legacy `login:None` host creates a 2nd three-part account + orphans the
  bare-host keychain entry (contract §1.2 rekey, optional) — cleanup ticket.
- (e) `ContextMenu` has no separator concept, so the switcher's account/command rows run contiguous
  (same gap as the P69i identity menu) — add a separator item.
- (f) Settings Accounts group ordering is alphabetical only (no repoId in scope for "current host
  first").
- (g) disabled Default radio's `aria-describedby` points at a `hidden` span — switch to a
  visually-hidden class.
- (h) switcher trigger has no busy affordance during a pin/reset write (menu shows aria-busy; trigger
  doesn't) — consider `opacity:0.6`. (i) §1.1 wireframe middot between host and caption omitted
  (cosmetic).

### `cargo fmt` has never been run on this repo — **OPEN**
No `rustfmt.toml` anywhere, no fmt check in any hook or CI. `cargo fmt --all --check` reports **1773
hunks across 221 files**; `--config use_small_heuristics=Max` is *worse* (2065). Right shape: its own
commit — pick a config, add `rustfmt.toml`, one-shot reformat, then add `cargo fmt --check` to the
gate. **Do it between milestones, never inside one.**

### Audit #2 remainder — **OPEN** (all confirmed bugs & SHOULD-FIXes fixed 2026-08-18/19)
Full audit `docs/audit-2026-08-18.md`; the resolved fix-batch mapping is in archive Part 16. Still
open: **§4.3–§4.8 test gaps** (CommandPalette/NumberSlider pins once fixed, streaming-graph e2e,
08-stash conflicted-apply fixture, Linux case-sensitivity assertions, low-value untested units,
missing journeys: updater / AI-PR-description / clone-init / worktrees) · **§7's 13 NITs** (recorded
in the audit, no action required) · **§5.6** perf/visual ACs stay USER CHECKPOINT (the headless
harness cannot observe rAF/compositing).

### P68 contract debt — **OPEN** (P68 is done, but its contracts are stale/oversized)
- `docs/contracts/P68e-ai-activity-dock.md` is **1064 lines** (twice the ~500 house limit) and now
  under-describes shipped code: P68g-1 added two elements to the ask block (an untrusted-model-output
  attribution line + a fixed "Bonsai never asks for passwords or tokens" guard) and made
  `aria-describedby` a two-id list, none of which §4.1/§4.2 describe. `ui-designer` produced
  splice-ready replacement blocks in `docs/contracts/P68g-ui.md` §3.1–§3.5. **Needs: apply the splice,
  then split the file.**
- `docs/contracts/P68-ai-conflict-streaming.md:304` is one module level stale — says
  `session_drain_tests.rs` is `#[path]`-included "as a child of `session`"; after the split it is a
  child of `session::session_drain` (still a descendant, so the privacy claim holds; the wording is
  out of date). P68 invariants D1–D16 remain canonical in that contract — do NOT "fix" them back.
- P68 security follow-ups (audit items 7–11) still OPEN; rationale in
  `docs/contracts/P68-security-audit.md` (canonical): the novel-content gate (structural defeat for
  H1), proposals shown as a diff, bulk path-count cap + per-batch reads + batch count in the dialog,
  process-group kill off Windows (the pid-zeroing half landed in `67539fd`), and a symlink-safe
  `resolve_conflict_text` write.

### P69 Settings follow-ups awaiting a user decision — **OPEN** (nothing is blocked on them)
- **A8 — bundle the two specced-but-unimplemented items into one increment** (both `ui-designer` and
  the orchestrator recommend bundling): (a) the help-text highlight fallback,
  `docs/contracts/P69-settings-ui.md` §3.2.1, `[NOT IMPLEMENTED]` — the flagship query `graph` returns
  5 hits and highlights **nothing** (every hit matched via `keywords`/`help` while the labels read
  "Row height" / "Lane width" / "Compact rows"); and (b) the half-landed draft-hint feature, §13. Note:
  the draft-hint CSS is genuinely **dead** but costs no visible layout today — the case for A8 is the
  missing feature, not a rendering bug.
- **A9 — a scoped a11y sweep of `color: var(--accent)` on text.** Fine on `--bg-0/1/2`; a latent AA
  failure anywhere accent text lands on a `--selection` fill (measured 3.51–3.74:1). ~30 call sites,
  unaudited. Now **prohibited** in `docs/contracts/ui-reference.md` §2 so new code cannot add to the
  backlog. The one deviation P69k shipped: the rail hit-count is `--text-1`, not the `--accent`
  ui-designer ruled for (accent as 11px text measures 3.74:1 / 3.51:1 on a selected item's
  `--selection` fill); the exact declaration to flip is marked in `settings-shell.css`.
- **A3 — the frozen AI gate-note copy is still unsigned.** §5.4's replacement for
  `Turn on "Enable AI features" above to change these.`; ui-designer prefers
  `These take effect once AI features are on.` The current string ships until the user rules.

### P77 tag-sync deferred follow-ups — **OPEN** (carried off the archived P77 milestone, 2026-08-21)
- **Collapsed-rollup needs first expand (FOR-USER decision):** §1.2 wants "see a problem without
  expanding", but the ls-remote check only fires on the first Tags expand per session (to avoid an
  eager network call on every repo open). So the `⚠ N` rollup can't appear until the user expands Tags
  once. Decide whether a cheap unprompted first check on repo-open is worth the network cost.
- NIT: rollup aria-label lacks singular/plural ("1 tags"); `useTagSync` re-hits network on rapid
  collapse→expand while `unavailable` (no cache stamp on the error path); confirm dialogs close
  optimistically so `busy` never paints (matches existing house pattern); tag-filter box gate counts
  local tags only (a repo with only remote-only tags shows no filter); item-7 "Delete tag on origin…"
  also shows on remote-only ghost rows (coherent — only place the tag exists).
- Backend NIT: `delete_remote_tag` doesn't `evict_fresh_on_auth_fail` (matches existing `push_tag`);
  `validate_tag_name` duplicated from `tags.rs` (module-private) — promote to shared if a 3rd caller.
  Full P77 detail: `docs/history/todo-archive-2026-08.md` Part 18.

---

## Archive

| File | Covers |
|---|---|
| `docs/history/todo-archive-2026-08.md` | Parts 1–9: P65 → P28 build detail, the Phase 1–4 banners, resolved FOR-USER decisions, P69(1.0.0)/P67/P68 detail, the 2026-08-17 batch mapping, resolved spun-out items. **Parts 10–16 (moved 2026-08-20): the P62–P74 checkpoint waiver + P71, P72, P73, P74, the P69 Settings redesign, and the Audit #2 fix batch, condensed. Parts 17–18 (moved 2026-08-21): P70 and P77, both checkpoints verified. Part 19 (moved 2026-08-21): the OPEN follow-ups resolved in the 2026-08-21 fix batch (read_status/palette/refetch/stash/submodule/STDERR/cred-split), verbatim. Part 20 (moved 2026-08-21): P78/P79/P80 forge milestones, condensed.** |
| `docs/history/todo-archive.md` | P27 → P2, M0–M6 |
| `docs/history/milestones-mvp.md` | the M0–M6 AI-gate vs USER CHECKPOINT split |
| `docs/history/context-pollution-audit.md` | the context/token-cost audit |
| `docs/contracts/INDEX.md` | one line per contract file — milestone, scope, status |

Move a milestone's section into `todo-archive-2026-08.md` only once **both** halves of its gate have
passed (or the native half is explicitly waived). A milestone with a pending USER CHECKPOINT stays on
this board.
