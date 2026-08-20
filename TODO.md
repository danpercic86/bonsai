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

Version files bumped to 1.1.0. `CHANGELOG.md` `[1.1.0]` finalized 2026-08-20 (Settings redesign,
audit-2 fixes, P70–P74). **Tag is gated on P70's native checkpoint (item 1) below.**

**P62–P74 native USER CHECKPOINTs were WAIVED and the milestones marked `done` 2026-08-20** for the
1.1.0 release (user decision): P62, P63, P64, P65, P67, P68, the P69 Settings redesign (P69a–P69l),
P71, P72, P73, P74. Full build detail: `docs/history/todo-archive-2026-08.md` Parts 2, 4, 5, 7, 8
(P62–P68) and Parts 11–15 (P71, P72, P73, P74, P69 Settings). Open follow-ups spun out of those
milestones are on this board below (they were NOT closed by the waiver).

---

## ⏳ P70 — git-executable resolution + honest "git not found" diagnostics — awaiting USER CHECKPOINT

⏳ **Native verification in progress for 1.1.0 (2026-08-20) — item 1 (SSH-agent auth survives the
banner) must pass before release.**

**Current step:** AI gate GREEN, code committed (`f0e9aee`). All increments implemented, two reviewer
rounds closed, tester's acceptance gaps (#13/#14/#23) filled. Awaiting the native checkpoint.

**AI gate (tester, sequential, all first-pass):** clippy `-D warnings` clean · `cargo test --workspace
--no-fail-fast` 1788 passed / 0 failed / 6 ignored · tsc clean · build ok · vitest 1701 / 140 files ·
lint 0 errors / 30 pre-existing warnings · `lint:size` OK · e2e 117 passed / 1 pre-existing skip.

**Trigger (field report, 2026-08-19):** user auto-updated to v1.0.0 via the MSI updater.
`msiexec.exe` relaunched `bonsai.exe` as its child, so the app inherited the installer's environment.
Their Git is a per-user install (`%LOCALAPPDATA%\Programs\Git\cmd\git.exe`) whose PATH entry lives
only in the **User** PATH — so `Command::new("git")` could not resolve `git`. Two symptoms, one cause:
"program not found" from `git/search.rs:129`; and 3× "authentication failed … no cached credentials"
because `git/remote.rs:180` `credential_fill` did `cmd.spawn().ok()?`, swallowing NotFound into `None`,
read as "helper had nothing" → wrong message at `git/remote.rs:326`. (P71 fixes the upstream cause.)

**Scope shipped:** D1 single cached git-binary resolver (PATH → Git-for-Windows registry key →
well-known dirs → bare name; `BONSAI_GIT_BIN` override as the test seam) used by every production
`Command::new("git")` site · D2 `credential_fill` distinguishes spawn-failure from empty-helper · D3
startup preflight + UI banner. `RwLock` cache (Re-check works without restart) · HKCU probed before
HKLM · child-PATH augmentation in `git_command()` · `gitNotFound` suppresses the error toast, banner
is the single surface.

**Two design defects caught during contract review (do NOT reintroduce):**
- **SSH regression.** The original design short-circuited the whole credential ladder when git was
  unresolvable. SSH remotes with a running ssh-agent authenticate inside libgit2 and never need
  `git.exe`. Narrowed to the credential-**helper** rung: the ladder still tries SshAgent/Default,
  `GitNotFound` only at exhaustion. Guards: #16 (SSH-only exhaustion ⇒ `AuthFailed`) + #18 (Helper
  rung performs zero spawns when git is missing).
- **Toolbar disabling.** Disabling Fetch/Pull/Push while git is missing would break those same SSH
  users, and the transport isn't knowable at the toolbar. Buttons stay enabled; blanket toast
  suppression narrows to background/scheduler failures; a user-pressed remote op gets one coalesced
  toast.

**USER CHECKPOINT (needs the native window / a human eye):**
1. 🔴 **SSH-agent auth survives the banner (BLOCKING).** Point `BONSAI_GIT_BIN` at a nonexistent
   path, relaunch, confirm the banner shows, then fetch/push an **SSH remote with a loaded ssh-agent**
   → must SUCCEED, no `gitNotFound` toast. A red or unrun result means the §3.3/§5.4 copy is wrong and
   must change before release — do not ship on it.
2. HTTPS-with-helper fails honestly — exactly one surface (the banner), no toast, and nowhere the
   words "no cached credentials" or "authentication failed".
3. Re-check recovery `false → true` without restart (not harness-verifiable — `?git=` fixed at init).
4. The original bug: MSI-installed / Machine-only-PATH parent on a per-user Git install → resolves via
   HKCU, no banner, commit search + HTTPS auth work through GCM.
5. First paint not delayed by the probe; no flash/jump of the notice bar on a healthy launch.
6. Screen-reader pass (NVDA/VoiceOver) under the bad-`BONSAI_GIT_BIN` repro.
7. Both themes on the real webview + visible focus ring on Re-check.
8. macOS/Linux: normal launch resolves via PATH, no banner.

**Queued follow-up (user-requested 2026-08-19, starts once the checkpoint clears):** `refactorer`
split of the credential subsystem out of `git/remote.rs` into `git/cred.rs` (`FillOutcome`,
`CRED_EXHAUSTED_MSG`/`GIT_MISSING_MSG`, `CredAttempts`, `next_cred_method`, `credential_fill`,
`acquire_cred*`, `exhausted_error`, `map_remote_err` + tests). Strictly behavior-preserving; baseline
captured after P70's tree is finalized. Guard tests #16 and #18 must still run and pass.

---

## 🛠️ DX — dev-loop acceleration — in-progress

**Goal:** act on the full-workflow velocity analysis (2026-08-20) — 68 GB `target/`, no build
acceleration, serial clippy/test, the 4-file IPC lockstep, a ~12-milestone deferred native-checkpoint
backlog. Ten improvements.

**Current step:** 8 of 10 landed & verified (`3ada322`, `2019e71`, `8e55be8`). P75 (IPC codegen)
queued to start **after P74's tree is committed**; P76 (native-checkpoint automation) **held as
contract-only** per user.

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

**Designed — contracts on disk, implementation gated on a decision:**
- **P75** — generate the IPC boundary with tauri-specta v2 (kills the types.ts / tauri.ts / mock-layer
  lockstep; keeps the `IpcApi` facade; typecheck-enforced anti-drift). 9–11 increments.
  `docs/contracts/P75-ipc-codegen.md`. **DECISION: implement AFTER P74's tree is committed** — the
  rewrite touches the IPC layer. tauri-specta is a release candidate → pin it, commit the generated
  bindings, add a `git diff --exit-code` staleness gate. (types.ts is deliberately NOT hand-split;
  P75 regenerates it.)
- **P76** — automate the native USER CHECKPOINT backlog with tauri-driver + WebdriverIO (~60–70%
  automatable; macOS has no WebDriver so its checkpoints stay human).
  `docs/contracts/P76-native-checkpoint-automation.md`. **DECISION: HELD (contract-only)** per user.

**Deferred cleanups (noted, not done):** lock the file-size baseline reclaim for App.tsx (P74) and
RepoWorkspace.tsx once those land; the duplicated private `open_repo_at` helper across many `git/`
modules (a real refactor with call-graph impact, not a leaf move).

---

## ✅ Confirmed checkpoints and accepted decisions (condensed — full text in the archive)

- **P62–P74 native USER CHECKPOINTs waived and marked done 2026-08-20 for the 1.1.0 release (user
  decision).** (P70 excepted — still awaiting its native checkpoint above.)
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

### `read_status` vs `git status --porcelain` discrepancy — **OPEN** (latent bug, found 2026-08-19)
Found incidentally by `status_matches_porcelain` (`crates/bonsai-core/tests/prop_status.rs:133`)
during the P70 credential-split verification run — **not** caused by that refactor (`status.rs`
untouched). `read_status` reported `("unstaged", "e/pwn", None, "modified")` which the porcelain
oracle did not. Random-seed failure; passed on re-run → a **latent correctness bug, not a flake**. The
regression seed (reproduce by writing the `cc` line into
`crates/bonsai-core/tests/prop_status.proptest-regressions`):

```
cc 7092b6a8ad052d40b3a382fbaf1450dde7bd1d77ac401b9e7af9a23db3965a5a
initial = [("js", 830085073), ("y/hdq", 750409876), ("zsl/ozrm", 2913031937), ("io", 3268388894), ("e/pwn", 3853189158), ("ap", 2793669611)]
ops = [(1, 4217376688305137722, 1942386613, "gnry"), (4, 3842487655791072095, 1278675183, "bp"), (1, 10402710393222198989, 1229272639, "mf/r"), (0, 13825244896185113770, 2802026316, "sgno"), (3, 5327597818003222344, 1785525724, "yeqjj/sd"), (5, 3279438120187805109, 3677744552, "gso/vvtix"), (1, 6214002843715606778, 2793541552, "lggl")]
```

Also observed (add to the known load-flake list):
`ai::session_tests::watchdog_does_not_fire_while_awaiting_input` failed once under load and passed on
immediate re-run — timing-sensitive.

### CommandPalette highlight resets on `actions` array identity — **OPEN**
`src/components/CommandPalette.tsx:103→107→118` re-lands the highlight on the first enabled row
whenever the `actions` **array identity** changes, and `filterActions` always returns a fresh array.
Any producer whose memo deps churn steals the user's keyboard selection mid-typing (P65 per streamed
batch — the real root cause of the `e2e/09-search-palette` flake; P68e ~once a second during a live AI
run, since fixed producer-side). **Correct fix (reviewer's): reset on the filtered ids, not array
identity** — immunises every future `actions` producer. Do NOT keep patching producers.

### Refetch storm: every mutation double-fetches per open tab — **OPEN** (audit #1 §3.10, 2026-08-07)
Every mutation runs `refreshAll` (~9 parallel fetches) and the watcher-debounced `repo-changed` for
the same writes re-runs the identical 9 ~300 ms later, per open tab (`RepoWorkspace.tsx`). Fix =
self-event suppression/coalescing; structural. Moved onto this board 2026-08-19 (audit #2 §5.3).

### Stash `expectedOid` UI wiring — **OPEN, now unblocked**
The Rust side is oid-verified (F-A6-B/F-A7-6) but the UI does not yet pass `expectedOid`. Was parked
on the `ipc/types.ts` freeze; the freeze lifted when forge landed. Moved onto this board 2026-08-19
(audit #2 §5.4).

### Submodule dirty-deinit force flag (F-A7-7) — **OPEN, now unblocked**
Same parking condition, now lifted; details in `docs/testing-campaign-2026-08/FINDINGS.md` F-A7-7.
Moved onto this board 2026-08-19 (audit #2 §5.4).

### `STDERR_GRACE_TOTAL` is not the absolute cap its doc comment claims — **OPEN**
`drain_stderr` checks `Instant::now() < deadline` *before* each `recv_timeout(STDERR_GRACE)`, so the
drain can run up to `STDERR_GRACE_TOTAL + STDERR_GRACE` (~1150 ms vs the documented 1000 ms). Visible
only as ≤150 ms extra shutdown latency; the existing test's 500 ms slack passes either way.

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

---

## Archive

| File | Covers |
|---|---|
| `docs/history/todo-archive-2026-08.md` | Parts 1–9: P65 → P28 build detail, the Phase 1–4 banners, resolved FOR-USER decisions, P69(1.0.0)/P67/P68 detail, the 2026-08-17 batch mapping, resolved spun-out items. **Parts 10–15 (moved 2026-08-20): the P62–P74 checkpoint waiver + P71, P72, P73, P74 and the P69 Settings redesign, condensed.** |
| `docs/history/todo-archive.md` | P27 → P2, M0–M6 |
| `docs/history/milestones-mvp.md` | the M0–M6 AI-gate vs USER CHECKPOINT split |
| `docs/history/context-pollution-audit.md` | the context/token-cost audit |
| `docs/contracts/INDEX.md` | one line per contract file — milestone, scope, status |

Move a milestone's section into `todo-archive-2026-08.md` only once **both** halves of its gate have
passed (or the native half is explicitly waived). A milestone with a pending USER CHECKPOINT stays on
this board.
