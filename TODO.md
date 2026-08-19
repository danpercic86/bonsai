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

**History is archived, not deleted.** Anything no longer on this board is in
`docs/history/todo-archive-2026-08.md` (P65 → P28 + the Phase 1–4 banners moved 2026-08-18; Parts
5–9 — P69/P67/P68 build detail, the 2026-08-17 batch mapping + spike facts, resolved spun-out
items — moved 2026-08-19), `docs/history/todo-archive.md` (P27 → P2, M0–M6) and
`docs/history/milestones-mvp.md` (the M0–M6 AI-gate vs USER CHECKPOINT split). Contract files are
indexed in `docs/contracts/INDEX.md`.

## 🐛 P73 — submodule init/update: reconnect an orphaned `.git/modules` gitdir — awaiting USER CHECKPOINT

**Current step:** ✅ **P73 CODE-COMPLETE — AI gate GREEN, awaiting native USER CHECKPOINT**
(`docs/contracts/P73-user-checklist.md`). Commits: `e3c4ad1` contracts · `df9274d` contract
amendments · `b632347` implementation + tests.

**AI gate (sequential):** `cargo test --workspace --no-fail-fast` **1866 passed / 0 failed / 6
ignored** · `cargo clippy --workspace --all-targets -D warnings` clean · `pnpm tsc --noEmit` clean ·
`pnpm vitest run` **1829 passed / 152 files** · `pnpm exec playwright test` **118 passed / 1
skipped** · `pnpm lint` 0 errors / 30 pre-existing warnings · `pnpm lint:size` OK (baseline
ratcheted: `submodule.rs` 677→453 dropped out entirely, `Sidebar.tsx` 918→892).

**Proof on the REPORTER'S OWN DATA (the evidence that matters).** A faithful replica of the wedged
state was built without touching the user's repo — `git clone --local` of `D:\Repos\ham-digi-backend`
plus a copy of its real `.git/modules/src/Hamilton.Voyager.Protocol/protocol` gitdir, with a sentinel
file planted inside it. BEFORE: `Uninitialized`, 0 workdir files, no gitlink, `git submodule status`
`-e96ae50d…`. AFTER `update_submodule`: `UpToDate`, `wt_oid == index_oid == e96ae50d…`, 6 real files
restored (`.gitignore`, `README.md`, `envelope.proto`, `robotics/`, `storage/`), gitlink =
`gitdir: ../../../.git/modules/src/Hamilton.Voyager.Protocol/protocol` (relative, no `\?\`),
`rev-parse --absolute-git-dir` resolving inside `.git/modules`, and the **sentinel intact** — proving
the cached gitdir was REUSED, not re-cloned. **No credentials configured in that process and no
network touched**, which is exactly why it fixes the Azure DevOps case.

Contracts: `docs/contracts/P73-submodule-reconnect.md`, `docs/contracts/P73-submodule-reconnect-ui.md`,
design review `docs/contracts/design-review-2026-08-19-p73-submodules.md`, user checklist
`docs/contracts/P73-user-checklist.md`.

**Review record.** Reviewer round 1 CHANGES REQUESTED on a **reproduced data-loss path the diff
itself introduced** (see below); round 2 APPROVE. ui-designer: approve with one SHOULD-FIX (pill
heights stepped because only verdict pills had a border) — taken. Tester added 8 wedged-state
integration tests + the Tauri-wrapper leg and found **no implementation defect**; it did catch two
stale acceptance criteria in the contract, both since corrected.

**What shipped:** 10-step fail-closed reattach of an orphaned `.git/modules` gitdir (traversal +
containment guards, empty-workdir requirement, origin-URL match, hand-written relative atomic
gitlink, `recreate_missing` on the salvage path only — never `force`); rollback for a failed fresh
clone so Bonsai can no longer create the wedge; a backstop that converts libgit2's raw
`attempt to reinitialize` into an actionable sentence; Init = init + checkout, mutually exclusive
with Update; badge `not checked out`; row-local `checking out…` pill; and the `remove_cached_git_dir`
`repo.path()` → `repo.commondir()` fix (the cleanup silently no-oped inside a linked worktree).

**Harness trap worth remembering:** the first full e2e run reported **118 failed**. Cause was a
STALE `pnpm dev:mock` server left listening on port 1420 from an earlier harness session, whose Vite
module graph still pointed at `src/components/settings/SettingsContext.tsx` after another session
renamed it to `.ts` — a single 404 that the e2e fixture (rightly) treats as a console error, so every
spec failed. `playwright.config.ts` has `reuseExistingServer: !CI`, so it adopted the broken server.
Killing the stale vite process and re-running gave 118 passed. **Never diagnose an all-specs-failed
e2e run without first checking what is actually listening on 1420.**

Two user-reported defects on the P19 submodule surface (found 2026-08-19 on a real Azure DevOps
superproject, `D:\Repos\ham-digi-backend`, submodule `src/Hamilton.Voyager.Protocol/protocol`).

**Bug 1 — Init's success toast disagrees with the badge.** `init_submodule` (`sm.init(false)`) only
writes `submodule.<name>.*` into `.git/config`; the worktree stays empty, so `list_submodules` still
classifies the row `uninitialized` (`git submodule status` agrees: leading `-`). The toast says
"Initialized <name>". Backend is git-faithful — the UI lies. Never caught because the mock handler
(`src/ipc/mock/handlers/submodules.ts:27`) flips the row to `upToDate` on init.
**Decision (user, 2026-08-19): make Init do init + checkout** — the menu action invokes
`updateSubmodule` (which is `sm.update(init=true, …)`), so the toast and the badge always agree.

**Bug 2 (blocking) — Update is wedged: `attempt to reinitialize '<...>/.git/modules/<path>'`.** The
submodule workdir exists but is EMPTY (no `.git` gitlink) while `.git/modules/<path>` is a complete
gitdir (right `core.worktree`, right `remote.origin.url`, pinned commit already local). Confirmed in
vendored libgit2 1.9.6: `git_submodule_update` branches on `WD_UNINITIALIZED` alone
(`submodule.c:1443`), and that bit is set purely from "does `<workdir>/<path>/.git` exist"
(`submodule.c:2222`, `:2443`) → it takes the CLONE path → `submodule_repo_create` passes `NO_REINIT`
(`submodule.c:1329`) → `git_repository_init_ext` errors (`repository.c:2886`). No
`SubmoduleUpdateOptions` setting steers around it. Upstream `git submodule update` instead REUSES the
module gitdir and rewrites the worktree gitlink — libgit2 has no such path, so Bonsai must add one.
Bonsai can also CREATE this state: `update_submodule` has no rollback for a half-finished clone
(unlike `add_submodule`'s `rollback_partial_add`).

Two libgit2 subtleties that shape the fix (do not skip):
- `Repository::set_workdir(abs, true)` is a NO-OP here (early-returns when the resolved workdir
  already matches, `repository.c:3271`) and writes an ABSOLUTE gitlink when it does fire
  (`repository.c:3284`) → write the gitlink ourselves, relative.
- A plain SAFE checkout will NOT repopulate the empty workdir (missing files classify
  `GIT_DELTA_UNMODIFIED`; `RECREATE_MISSING` is only auto-added under FORCE — `checkout.c:302`,
  `:2447`) → update would report `upToDate` over an empty dir. Set
  `CheckoutBuilder::recreate_missing(true)` on the SALVAGE PATH ONLY (not `force`, so the invariant
  in `crates/bonsai-core/tests/submodule_cli_2.rs:113` holds).

Increments: (1) core reconnect/salvage in `update_submodule` + `remove_cached_git_dir` commondir fix ·
(2) rollback for a failed fresh clone · (3) UI Init = init + checkout · (4) tests (wedged-state
fixture, offline reattach, non-empty/URL-mismatch refusals, rollback, traversal).

Plan: `~/.claude/plans/i-opened-hamiltondigitalizationbackend-compiled-biscuit.md`

**Reviewer's reproduced MUST-FIX (being fixed).** The new `rollback_partial_update` deleted user
data in a case the `uninitialized` guard does not cover: a submodule registered but never cloned, no
`.git/modules/<key>`, and the user has uncommitted files sitting in the submodule folder — libgit2's
SAFE checkout correctly refuses, rollback then ran its contents-only branch and wiped the files. Pre-P73
there was no rollback at all, so the diff INTRODUCED the loss. Fix: snapshot `workdir_was_empty` and
require it alongside `uninitialized` before touching the workdir. Also missing: 5 of the contract's 8
unit tests, including the only coverage of acceptance criteria 9 (traversal) and 11 (commondir).

**SPUN OUT of P73 (pre-existing, not caused by this diff) — both from the ui-designer review:**
- **Toast-tone contrast.** `.toast-error` / `.toast-success` / `.toast-info` measure 3.34–4.07:1 in both
  themes — below AA. Deferred deliberately (it is a global toast change, not a submodule one), but it now
  carries P73's entire prose payload: the two new refusals are long sentences rendered in a sticky error
  toast. Worth its own small milestone.
- **Sub-24px hit targets in the sidebar**: the Submodules `+` button is 20×20 and the section toggle
  183×16 (`Sidebar.tsx:814-835`).


## 📝 P69 follow-ups + full-gate record (inserted by a concurrent session 2026-08-19)

> These notes were written into the middle of the P73 section by another session working in
> this repo at the same time; moved here verbatim so both milestones stay readable. Two
> attributions in them are wrong: P73 added **no** IPC commands, and it did not touch
> `src/App.tsx` or its file-size baseline.

**Open follow-ups created by P69c/P69e (tracked so they are not lost):**
- **Rust half of defaults parity — DEFERRED, not done.** `src/settings/uiSettingsDefaults.json` is
  currently pinned from the **TS side only**, so nothing machine-checks TS against Rust; the reviewer
  re-derived all 30 keys + 12 nested `graph` keys by hand and they match, but that is a one-off. The
  assert needs one `cargo test`, forbidden while P73 shares the tree. Two facts it will need:
  **there is no `UiSettings::default()`** (the contract's snippet does not compile) — serialise
  `ui_settings_of(&settings::Settings::default())`; and `ui_settings_of` is a private `fn` in
  `commands/ui_settings.rs`, so it needs `pub(crate)`. Goes in `settings_ui_tests.rs`
  (`settings.rs` is exactly at its 663 baseline and cannot take even a `mod` line).
- **`ai::Limit` label collision — allowlisted with an expiry.** Rows #48 and #50 both render a
  control labelled `Limit` in AI → Runs → Limits: two `spinbutton`s, one accessible name, one group.
  This is the SAME a11y defect P69d fixed for the two `Interval` rows. `KNOWN_LABEL_COLLISIONS`
  carries `TODO(P69j): relabel to 'Time limit' / 'Spend limit'`. The mechanism is proven — the
  catalog already leads the UI on labels — so the fix is two string edits + deleting the entry.
- **The anti-drift DOM guard needs an architect ruling BEFORE P69g writes it.** It cannot hold as
  specified for two categories: `identities` renders one card PER PROFILE (duplicate
  `data-setting-id`s), and `git-config`'s Behaviour/Custom-keys blocks are dynamic so no single
  control carries the row's accessible name. Reviewer's recommendation: add `repeats: 'perProfile'`
  to the entry type + a distinguishing `data-profile-id`, and dedupe ONLY for flagged entries
  (asserting the duplicate count equals `profiles.length`) rather than exempting a whole category;
  and add a distinct `'group'` control kind instead of overloading `'readonly'`, so the
  accessible-name check applies to the block rather than being silently skipped.
- **The P69d contract acceptance line is WRONG and needs amending.** It says "the profiles pill
  lights up in the default harness state". It does not and should not: `fixtures/config.ts` seeds
  global `Mock Fixture User` / `fixture@bonsai.dev` while the seeded profiles are
  `work@bonsai.dev` and `me@personal.dev` — nothing matches. P69d pinned the honest no-match state
  rather than fudging the fixture. **Decision needed** (fixture/UI, not code): to exercise the
  "matches a profile" state in the harness, either align one seeded profile's email with the fixture
  identity or add a dedicated `?fixture=` state. Recommend the latter — changing the default would
  hide the more common real-world no-match case.
- **`user.signingkey` is not in `CURATED_KEYS`**, so a single `getConfig(repo,'local')` sees only a
  LOCAL signing key; a global-only one reads `null`. No consumer yet, so deferred — but the identity
  menu in P69i must not claim to show a signing key it cannot see.
- **Spun out of P69b:** the teardown flush is dispatch-only, so a hard OS kill can still drop a
  pending settings write (needs a synchronous save on the Rust side); and the save-failure toast
  auto-dismisses after 5 s, so a user who looked away never learns — making it sticky was declined
  because `App.tsx` has one line of ratchet headroom.

✅ **FULL GATE RUN 2026-08-19, ALL GREEN** — taken the moment P73 committed (`b632347`) and released
`src/styles.css`, with the tree otherwise clean, so these numbers cover every committed P69
increment (b/c/d/e/f) plus P73's landed work:
- `pnpm test` — **152 files / 1829 tests passed, 0 failed** (session baseline was 128 / 1580)
- `pnpm test:e2e` — **118 passed / 1 skipped / 0 failed** (baseline 104 / 1 skipped)
- `pnpm exec tsc --noEmit` — clean · `pnpm lint:ci` — **30 warnings, 0 errors** (budget 40)
- `pnpm lint:size` — exit 0, no baselined file grew (`App.tsx` 1167 ≤ 1168)
- `cargo clippy --workspace --all-targets -- -D warnings` — clean
- IPC commands **162** = 160 + 2 from P73's submodule work; **P69 contributed +0**, as designed.

This closes the gap flagged below. The e2e run matters most: it is where this repo has historically
caught what vitest passed clean (a StrictMode latch with 1440 tests green), and P69b hit that exact
class again this session.

Superseded warning (kept for the record): ⚠️ **The milestone gate was NOT satisfiable while P73
shared the tree.** No `cargo`, no full
`pnpm test`, and no `pnpm test:e2e` have been run for P69 — e2e in particular has historically
caught defects in this repo that vitest passed clean on (including a StrictMode latch with 1440
tests green, and P69b hit exactly that class again). Every increment is individually verified with
scoped runs; **a full gate run is owed before P69 can be called green.**

## 🐛 P72 — forge connect fixes: Azure DevOps 401 + dead external links — in-progress

**Current step:** P72 — **AI GATE GREEN. Both increments implemented, reviewed, security-audited and
committed. Only the native USER CHECKPOINT remains.**

Commits: `285828b` contracts - `c1455c7` Increment A (Azure 401) - `4888315` resume note -
`3391286` Increment B (openUrl). Contracts: `docs/contracts/P72-forge-connect-fixes.md`,
`docs/contracts/P72-ui.md`.

**Increment A — Azure validate-then-identify (DONE).** `viewer()` validates on
`GET _apis/git/repositories/{repo}` (covered by `vso.code`, inherited by `vso.code_write`) instead of
the profile endpoint (which needs `vso.profile`), so a PAT scoped only Code (Read & Write) connects.
Identity is one best-effort profile call; every error is swallowed to an empty login and can never
fail a connect. Adds the missing 203 arm (Azure's HTML sign-in page for an expired PAT, previously
surfacing as `ForgeApi("malformed response")`), and a 404 that names org/project/repo — appended to
the status message, not substituted, so a 5xx outage keeps its own text. Reviewer APPROVED, no
MUST-FIX; both SHOULD-FIX applied. `azure/mod.rs` 513 -> 245 lines via `mod_tests.rs` +
`viewer_tests.rs` + `testkit.rs`, proven behaviour-preserving at 157 tests before new cases.

**Increment B — `openUrl` IPC (DONE).** `bonsai-core::external::{validate_web_url, url_ladder,
open_url}` on the P49 machinery; `open_url` Tauri command skipping the `path.exists()` precheck;
`openUrl` on the IPC surface + mock; both anchors routed through it keeping `href`/link semantics,
with modified and middle clicks passing through. No opener plugin, no shell, no capability change
(upholds P49 D1). Reviewer APPROVED, no MUST-FIX; its SHOULD-FIX applied (the rejection test pinned
only the error variant, so collapsing the three category messages into one would have kept it green).

**Security audit of the URL-launch surface: nothing at Critical/High/Medium.** All four Low findings
were closed rather than accepted, because `PrDetailView`'s URL comes from a forge API response:
reject userinfo (`https://github.com@evil.example/` previously validated while the browser would
navigate to `evil.example`); reject whitespace/control characters anywhere (inert on Windows/macOS,
but `xdg-open`'s `$BROWSER`-with-`%s` branch word-splits unquoted); cap length at 2048;
stop exporting `url_ladder` (its `rundll32 url.dll,FileProtocolHandler` rung is a general
ShellExecute dispatcher guarded only by a doc comment); flatten+truncate the destination tooltip.
Auditor's verdict on the P49 D1 question: hand-rolling is *safer* here than
`tauri-plugin-opener` would have been, because the input space is narrowed to http(s) in Rust before
any spawn and the OS-dispatch surface is four fixed argv vectors.

**AI gate (sequential):** `cargo test --workspace --no-fail-fast` **1845 passed / 0 failed / 6
ignored** - `cargo clippy --workspace --all-targets -D warnings` clean - `pnpm tsc --noEmit` clean -
`pnpm vitest run` **1711 passed / 143 files** - `pnpm lint` 0 errors / 30 pre-existing warnings -
`pnpm lint:size` OK - `pnpm exec playwright test` **118 passed / 1 skipped**.

**Harness verification (browser pane, `pnpm dev:mock`):** open repo -> Pull requests -> Connect
panel. `.forge-connect-link` has `rel="noreferrer noopener"` and `cursor: pointer`; a plain left
click is intercepted (`defaultPrevented: true`) and resolves through the mock to
`window.open('https://github.com/settings/tokens')` with no navigation away; ctrl-click and
middle-click are NOT intercepted. Screenshot unavailable (the pane is not displayed, so the page
does not composite) — the DOM assertions above are the evidence instead.

**KNOWN CAVEAT (unresolved).** The FIRST `pnpm vitest run` after the security-hardening edits
reported `1 failed | 1710 passed`, and the failing test name was not captured. Three subsequent full
runs were clean (1711/1711), and the committed tree is what those clean runs exercised. So this is
**unreproduced, not explained** — if a flaky frontend test surfaces later, start here.

### P72 USER CHECKPOINT (batched — neither half is AI-verifiable)
Run `pnpm tauri dev`:
- **Azure (Increment A, ready to test now):** connect the EXISTING Code (Read & Write) PAT — must
  succeed. Then try a deliberately wrong/expired PAT — the error must be a clear auth rejection,
  never "malformed response".
- **Links (Increment B, after it lands):** click "Create a token" in the Connect panel and
  "Open in browser ↗" on an authenticated PR — the system browser must open.

### P72 process finding (worth keeping)
Both bugs lived exactly where the test doubles were. The Azure `FakeTransport` returned 200 from the
profile URL, so no test ever asked what a Code-scoped PAT could actually reach; and the browser
harness is the ONE environment where `target="_blank"` works, so the AI gate structurally could not
see the dead link. Neither was a coverage-count problem — both suites were green throughout.

Two user-reported defects on the P62/P64 forge connect surface (found 2026-08-19 by the user, on a
real Azure DevOps org — neither was catchable by the existing suites, see "why this shipped" below).

**Bug 1 (blocking) — Azure connect rejects a valid Code-scoped PAT with 401.** The Basic auth header
is CORRECT (`Basic base64(":"+PAT)`, empty username, `api-version=7.1` everywhere) and the remote
parses fine. The fault is the *validation endpoint*: `set_token` → `validate_token` →
`AzureDevOpsProvider::viewer()` probes `app.vssps.visualstudio.com/_apis/profile/profiles/me`
(`azure/rest.rs:28`), which is gated on the **User Profile (Read)** (`vso.profile`) scope. The
Connect panel tells the user to create a **Code (Read & Write)** PAT (`ForgeConnect.tsx:54`), which
carries no profile scope → Azure 401 → surfaced as "rejected the credentials", implying a bad token.
The contract encodes the same mismatch (`P64-forge-providers-ai-pr.md:158` vs
`P64-user-checklist.md:64-67`).

**Bug 2 — "Create a token" / "Open in browser ↗" do nothing in the native app.** Both are plain
`<a href target="_blank">` with no handler (`ForgeConnect.tsx:98`, `PrDetailView.tsx:46`). There is
no opener/shell plugin, no `opener:*` capability, and no new-window handler, so the webview drops
the request. Already predicted and deferred at `P62-user-checklist.md:39-43`; this is that follow-up.

**Why this shipped (process finding).** Both bugs live exactly where the test doubles are: the Azure
`FakeTransport` returns 200 from the profile URL, so no test asks what a Code-scoped PAT can reach;
and the browser harness is the one environment where `target="_blank"` works, so the AI gate could
not see the dead link. Neither is a coverage-count problem.

**Decisions (user-confirmed 2026-08-19).**
- Azure: **both** — repo-endpoint validation (the scope the app actually uses) **and** a better
  401/203 message. The "clearer scope copy" half turned out to be a no-op: the existing Azure
  hint ("Code (Read & Write)") becomes TRUE once the backend stops probing a profile endpoint,
  and advertising User Profile (Read) would promise an account name nothing renders.
- Links: fix **both** sites; **hand-rolled per-OS spawn, no new plugin and no new capability grant**
  (upholds P49 D1, which explicitly rejected `tauri-plugin-opener`).
- Sequencing: one combined batch, committed as two increments (A = Azure, B = openUrl).

**Increment A — Azure validate-then-identify.** `viewer()` validates with
`GET .../_apis/git/repositories/{repo}?api-version=7.1` (must succeed; 404 ⇒ message naming
org/project/repo), then identity is *best-effort*: `_apis/connectionData` →
`authenticatedUser.providerDisplayName`, else the profile endpoint, else `ForgeViewer` with an empty
login (UI shows a plain "Connected"). Also adds the missing **203** arm to `map_status` (Azure's HTML
sign-in response for an expired PAT currently surfaces as `ForgeApi("malformed response")` — closes
the known gap at `P64-user-checklist.md:69-74`).

**Increment B — `openUrl` IPC.** New `bonsai-core::external::{validate_web_url, url_ladder, open_url}`
on the existing `LaunchSpec`/`launch_first` machinery (Windows `explorer` → `rundll32
url.dll,FileProtocolHandler`; macOS `open` with `wait_for_exit`; Linux `xdg-open`; never a shell),
a `open_url` Tauri command that skips the P49 `path.exists()` precheck, `openUrl` on the IPC
surface + mock, and both anchors routed through it with the standard error-toast pattern.
`validate_web_url` is load-bearing: `PrDetailView`'s URL comes from the forge API response.

**Out of scope (separate, still open):** the `dev.azure.com/{org}/_git/{repo}` shorthand (repo ==
project) returns `None` from `detect_azure` (`detect.rs:123`) — surfaces as *unsupported*, not 401;
documented at `P64-user-checklist.md:59-63`.

**Acceptance criteria.**
1. An Azure PAT with only Code (Read & Write) connects successfully; nothing is stored on failure.
2. An invalid/expired Azure PAT yields a clear auth error (never "malformed response").
3. Identity lookup is best-effort and can NEVER fail a connect: a PAT that cannot read the
   profile endpoint still connects, yielding `ForgeViewer.login == ""`. Verified at the data
   layer only — `login` has no render site anywhere in the frontend, and P72 deliberately adds
   none (see `docs/contracts/P72-ui.md` §1). Originally worded as a UI criterion; corrected
   2026-08-19 after ui-designer established there are zero `viewer` render sites.
4. Clicking "Create a token" / "Open in browser ↗" opens the system browser in the native app;
   a launch failure raises an error toast.
5. `validate_web_url` rejects non-http(s) schemes, hostless URLs, and leading `-`.
6. Full gate green: clippy `-D warnings`, `cargo test --workspace`, tsc, vitest, lint, e2e.
7. Contracts/doc drift corrected (`P64-*`, `phase4-forge-overview.md:67-68`, `bonsai-forge/src/lib.rs:11-12`,
   `http.rs:49` — all four still claim `Bearer` only, stale for Azure/GitLab).

**USER CHECKPOINT (not AI-verifiable — no real Azure org, no native webview here):** connect the
existing Code-scoped PAT in `pnpm tauri dev`; try a deliberately bad PAT; click both links.

Plan file: `~/.claude/plans/in-the-connect-to-clever-lantern.md`

## 🐛 P70 — git-executable resolution + honest "git not found" diagnostics — in-progress

**Current step:** P70 — **AI gate GREEN, awaiting native USER CHECKPOINT.** All increments implemented,
two reviewer rounds closed, tester's acceptance gaps (#13/#14/#23) filled.

**AI gate (tester, sequential, all first-pass):** clippy `-D warnings` clean · `cargo test --workspace
--no-fail-fast` **1788 passed / 0 failed / 6 ignored** (4 pre-existing perf gates + 2 new out-of-process
children) · tsc clean · build ok · vitest **1701 passed / 140 files** · lint 0 errors / 30 pre-existing
warnings · `lint:size` OK · e2e **117 passed / 1 pre-existing skip**.

**USER CHECKPOINT — item 1 is BLOCKING:**
1. 🔴 **SSH-agent auth survives the banner.** Point `BONSAI_GIT_BIN` at a nonexistent path, relaunch,
   confirm the banner shows, then fetch/push an **SSH remote with a loaded ssh-agent** → must SUCCEED,
   no `gitNotFound` toast. A red or unrun result means the §3.3/§5.4 copy is wrong and must change
   before release — do not ship on it. (#16 and #18 pin the internal halves; this is the only e2e proof.)
2. HTTPS-with-helper fails honestly — exactly one surface (the banner), no toast, and nowhere the words
   "no cached credentials" or "authentication failed".
3. Re-check recovery `false → true` without restart (not harness-verifiable — `?git=` is fixed at module init).
4. The original bug: MSI-installed build / Machine-only-PATH parent on a per-user Git install → resolves
   via HKCU, no banner, commit search works, HTTPS auth works through GCM.
5. First paint not delayed by the probe; no flash or jump of the notice bar on a healthy launch.
6. Screen-reader pass (NVDA/VoiceOver) **specifically under the Variant B / bad-`BONSAI_GIT_BIN` repro**.
7. Both themes on the real webview + visible focus ring on Re-check.
8. macOS/Linux: normal launch resolves via PATH, no banner.

**Two defects the browser harness caught that jsdom could not:** a hard-coded screen-reader announcement
(Variant A copy spoken for Variant B — root cause was the *UI contract*, which only said "a live region
exists"; now derived from the same `bannerCopy()` the banner renders, so drift is structurally
impossible), and 7 pre-existing e2e failures in `06-merge-conflicts`/`07-rebase` whose `getByRole('status')`
banner locator collided with P70's always-mounted announcer. **One orchestrator finding did NOT hold:**
the 400 ms Re-check window reported as never rendering was an artifact of the hidden Browser pane
(throttled timers); Playwright shows it at 20–380 ms. The underlying test gap was real though — the old
test asserted the returned promise's elapsed time, never that `checking` reached the DOM — and is now
closed by a rendered-label assertion + e2e spec 19, both mutation-verified.

**Two design defects caught during contract review (both corrected before any code was written):**
- **SSH regression (architect §3.1).** The original design short-circuited the *whole* credential
  ladder when git was unresolvable. SSH remotes with a running ssh-agent authenticate entirely inside
  libgit2 and never need `git.exe` — that version would have broken a working flow for every SSH user.
  Narrowed to the credential-**helper** rung: the ladder still tries SshAgent/Default, and
  `GitNotFound` is chosen only at exhaustion. Guards: test #16 (SSH-only exhaustion ⇒ `AuthFailed`,
  not `GitNotFound`) + #18 (Helper rung performs zero spawns when git is missing) + a native
  checkpoint (SSH fetch succeeds while the banner shows). No injectable ssh-agent seam — rejected as
  indirection in the credential hot path for marginal coverage.
- **Toolbar disabling (my decision 2, reversed by ui-designer, ratified).** Disabling Fetch/Pull/Push
  while git is missing would have broken those same SSH users, and the transport isn't knowable at the
  toolbar (it depends on the branch's resolved upstream). Buttons stay enabled; blanket toast
  suppression narrows to background/scheduler failures; a user-pressed remote op gets one coalesced
  toast. The reported 3-toast symptom still dies — those were auto-fetch retries, which stay silent.

**Orchestrator decisions on contract §9 (all approved as recommended, 2026-08-19):** `RwLock` cache
(not `OnceLock`) so the banner's Re-check works without a restart · HKCU probed before HKLM (matches
the field case) · child-PATH augmentation in `git_command()` included (repairs hooks/helpers that
shell out to `git` themselves) · `gitNotFound` suppresses the error toast entirely, banner is the
single surface (this is what kills the 3-repeated-toasts symptom) · `fixture.rs::have_git()` migrated.

**Trigger (field report, 2026-08-19):** user auto-updated to v1.0.0 via the MSI updater. `msiexec.exe`
relaunched `C:\Program Files\Bonsai\bonsai.exe` as its child, so the app inherited the *installer's*
environment. Their Git is a per-user install (`%LOCALAPPDATA%\Programs\Git\cmd\git.exe`) whose PATH
entry lives only in the **User** PATH — the Machine PATH has no Git. Inside the app,
`Command::new("git")` therefore could not resolve `git`.

Two symptoms, one cause:
- "failed to run `git log`: program not found" — `git/search.rs:129` (`SpawnGitRunner`).
- 3× "authentication failed for 'origin': the configured credential helper has no cached
  credentials…" — `git/remote.rs:180` `credential_fill` does `cmd.spawn().ok()?`, swallowing the
  NotFound into `None`; `acquire_cred` reads that as "helper had nothing" → exhausts → the wrong
  message at `git/remote.rs:326`. GCM had the credentials all along. Repeats via auto-fetch backoff.

**Scope (3 deliverables):** D1 single cached git-binary resolver (PATH → Git-for-Windows registry key
→ well-known install dirs → bare name; `BONSAI_GIT_BIN`-style override as the test seam; no new
`bonsai-core` dependency) used by every production `Command::new("git")` site · D2 `credential_fill`
distinguishes spawn-failure from empty-helper so the error names the real problem · D3 startup
preflight command + UI banner (ui-designer pass required — user-visible).

**Queued follow-up (user-requested 2026-08-19, starts once P70 commits):** `refactorer` split of the
credential subsystem out of `git/remote.rs` into `git/cred.rs` (`FillOutcome`, `CRED_EXHAUSTED_MSG`/
`GIT_MISSING_MSG`, `CredAttempts`, `next_cred_method`, `credential_fill`, `acquire_cred*`,
`exhausted_error`, `map_remote_err` + tests). Strictly behavior-preserving; equivalence proven by
identical before/after test counts, so the **baseline must be captured after P70 commits**, not now —
the in-flight fix batch is still changing remote.rs's test layout. Guard tests #16 and #18 must still
run and pass wherever they land.

**Acceptance:** git resolves when the process PATH lacks it but Git is installed per-user; a genuinely
missing git produces a "git not found" message, never an auth message; preflight banner appears in the
mock harness via a query-param seam; cargo + clippy + vitest + e2e gates stay green.

**USER CHECKPOINT (native):** install/auto-update, launch the app from a parent WITHOUT the user PATH,
confirm fetch/pull/push + commit search work and no spurious auth toast appears.

## 🐞 Spun out — `read_status` vs `git status --porcelain` discrepancy (found 2026-08-19)

Found incidentally by `status_matches_porcelain` (`crates/bonsai-core/tests/prop_status.rs:133`) during
the P70 credential-split verification run — **not** caused by that refactor (`status.rs` untouched).
`read_status` reported `("unstaged", "e/pwn", None, "modified")` which the porcelain oracle did not.
Random-seed failure; passed on re-run, so it is a **latent correctness bug, not a flake**.

The refactorer deleted the `proptest-regressions` artifact rather than commit it (checking it in would
have turned a random failure into a deterministic one *inside a behavior-preserving refactor* — not its
call to make). Seed preserved here instead:

```
cc 7092b6a8ad052d40b3a382fbaf1450dde7bd1d77ac401b9e7af9a23db3965a5a
initial = [("js", 830085073), ("y/hdq", 750409876), ("zsl/ozrm", 2913031937), ("io", 3268388894), ("e/pwn", 3853189158), ("ap", 2793669611)]
ops = [(1, 4217376688305137722, 1942386613, "gnry"), (4, 3842487655791072095, 1278675183, "bp"), (1, 10402710393222198989, 1229272639, "mf/r"), (0, 13825244896185113770, 2802026316, "sgno"), (3, 5327597818003222344, 1785525724, "yeqjj/sd"), (5, 3279438120187805109, 3677744552, "gso/vvtix"), (1, 6214002843715606778, 2793541552, "lggl")]
```

Reproduce by writing the `cc` line into `crates/bonsai-core/tests/prop_status.proptest-regressions`.

Also observed: `ai::session_tests::watchdog_does_not_fire_while_awaiting_input` failed once under load
("the sentinel should have blocked the run (session already finished: true)") and passed on immediate
re-run — timing-sensitive, add to the known load-flake list.

## 🔧 P71 — auto-update relaunch inherits the installer's environment — in-progress (research)

**Velocity mode for the P71 remainder (user decision, 2026-08-19) — all three levers ON:**
(1) **targeted gates on intermediate rounds** — subagents run only the suites their change can break;
the orchestrator runs ONE full gate before commit. (2) **MUST-FIX only routed back** — SHOULD-FIX/NIT
become filed follow-ups rather than another implement+gate cycle. (3) **independent agents run in
parallel**, accepting occasional spurious cargo failures from the shared-target-dir race (disambiguate
by isolated re-run, never by assuming). Rationale: each full gate is serial (cargo and clippy cannot
overlap) and each review round costs a complete implement+gate cycle; P70 took 3 rounds. Correctness
bar is unchanged — the levers cut redundant verification, not verification.

**Current step:** P71 — both increments implemented; security audit done (no critical/high); reviewer
**APPROVED** with zero MUST-FIX. Awaiting the orchestrator's full gate, then commit. Native USER
CHECKPOINT (C-1…C-5) still required — a real signed update round-trip cannot be machine-verified.

**Append reversal (orchestrator, 2026-08-19) — do not re-flip.** I originally chose *prepend* and
justified it as "restoring what a Start-menu launch would have resolved". That was factually wrong:
Windows composes **system Path first, user Path appended after**, so prepending put recovered *user*
entries ahead of *system* ones — in the msiexec case, user-writable
`%LOCALAPPDATA%\Microsoft\WindowsApps` ahead of `C:\Windows\System32` for every child process.
Decisive argument: **R2 only ever adds entries that are absent, so a missing directory cannot lose a
race it is not in** — append rescues just as well and creates no shadowing question. Guarded by
`recovered_entries_never_precede_inherited_ones_append_reversal_p71` (byte-for-byte prefix + positional
tail + concrete System32-before-WindowsApps), plus three exact-equality ordering tests. The audit found
**no** privilege-boundary crossing either way (`RunAsUser` duplicates explorer's token, so the relaunch
has strictly less privilege than the installer and no more than a Start-menu launch).

**Efficacy bug the audit caught that the reviewer did not:** R2 expanded `%VAR%` against the very
environment block it exists because it distrusts. Under msiexec that block is SYSTEM-context, so
`%LOCALAPPDATA%`/`%APPDATA%` resolve under `C:\Windows\system32\config\systemprofile\…` — the entries
R2 exists to rescue would be rehydrated pointing at the **wrong directory**, and C-2 could report
`applied: true` while the rescue silently failed. Now resolved from `HKCU\Volatile Environment` via one
un-filtered `reg query` behind a `OnceCell`; the test fake records which process vars were read, so
"the systemprofile value was never consulted" is asserted, not assumed.

**Measured cost — FOR USER:** rehydration adds **~197 ms pre-first-paint** on this machine (3 `reg.exe`
spawns, ~100 ms each on an AV-heavy corporate box; pessimistic), hard-bounded at 1.5 s shared. Paid on
every launch including the common case where nothing is missing. Mitigation deliberately not taken in
the fix pass: issue the three reads concurrently (→ one spawn's latency). Filed as a follow-up —
**needs a user call on whether 200 ms is an acceptable price for in-place repair of MSI-installed
clients.**

**P71 follow-ups (filed, non-blocking — reviewer APPROVED without them):**
1. **`lookup_var` passes registry-sourced text to `std::env::var` as a key** (`winenv_merge.rs:94`).
   `%%` yields an empty name and `%A=B%` a name containing `=`; `std::env::var`'s documented *Panics*
   clause permits a panic for both. No std impl panics today, so not blocking — but it is the same
   class as M1 (registry-controlled data reaching a may-panic std call before first paint). One-line
   fix: return `None` when `name.is_empty() || name.contains(['=', '\0'])`. The existing test only
   proves the *fake* returns `None` for `""`.
2. **`WinEnv::set_path` documents no precondition** (`winenv.rs:176`). The NUL/length precondition is
   documented on `rehydrate_path`, not on the method that actually panics, and `is_applicable` is
   `pub(crate)` and unexported — a second call site could reintroduce M1. Better: fold the check into
   `HostWinEnv::set_path` so it is unbypassable.
3. **Contract §5.3 is stale** — still shows a two-method `WinEnv` trait (implementation has three;
   `set_path` is the seam that makes the `applied: true` branch assertable) and still says
   "`winenv.rs` (~110 lines logic + ~110 tests)" though the module became five files.
4. **`parse_reg_values` mis-slices when the *data* contains a type token** (`winenv_merge.rs:45`).
   Contrived, no escalation. Add an "index must be preceded by whitespace" check.
5. **`OnceCell` one-shot semantics untested** — `FakeWinEnv` models no spawn counts, so "read the block
   exactly once" is inspected, not asserted. A counting fake would fix that.
6. **Pre-first-paint `eprintln!`** (`src-tauri/src/lib.rs:30`) panics on write error, and a release
   `windows_subsystem="windows"` build launched from Explorer has no stderr handle. Matches house
   practice elsewhere and current std returns success for a detached handle — but this is the one call
   running *before* the window exists, in a module whose premise is "never panic before first paint".
   `let _ = writeln!(std::io::stderr(), …)` closes it.
7. **`merge_path` with a whitespace-only `process_path`** emits a whitespace component. Unreachable.
8. **Startup latency** — issue the three `reg.exe` reads concurrently (~197 ms → one spawn's latency).
   Needs the user's call on whether 200 ms pre-paint is acceptable at all.
Deferred earlier, still open: **LOW-4** (`GetSystemDirectoryW` instead of reading `%SystemRoot%` from
the env — shared owner with P70's `gitbin.rs`, needs a dependency decision) · **N5** (stale P42 docs
still describing `"targets": "all"` and an MSI artifact — docs-curator scope).

**Root cause found — the MSI was never a deliberate choice.** `tauri-action`'s `updaterJsonPreferNsis`
defaults to `false` "for legacy reasons" and release.yml never overrides it, so `latest.json` points at
the `.msi` by accident. The WiX relaunch is broken *by construction*: a `LaunchApplication` custom
action run by msiexec's own process inherits msiexec's env block (`Impersonate="yes"` fixes the token,
not the environment). NSIS is correct by construction: `RunAsUser` duplicates **explorer's** token and
calls `CreateProcessWithTokenW` with `lpEnvironment = NULL`, which per MSDN builds the environment from
the user profile — Start-menu-equivalent, guaranteed by API not by luck.

**R1 (chosen):** drop MSI from `bundle.targets`, set `updaterJsonPreferNsis: true`. Zero Rust/TS/IPC/UI.
**R2 (increment 2):** startup PATH rehydration from HKCU/HKLM. Approved on the argument that **R1 does
nothing for clients already on an MSI install** — including the reporting user — so R2 is in-place
repair, not defence-in-depth. Prepend missing entries only, never reorder/dedupe/drop, no persistence,
malformed registry → silent no-op. It does NOT restore `USERPROFILE`/`HOME`, `SSH_AUTH_SOCK`, proxy
vars or `TEMP` — R2 never makes R1 optional.
**Rejected:** stop-auto-relaunching is *impossible*, not just worse — the updater plugin calls
`std::process::exit(0)` right after launching the installer, so the app is dead before install begins
(an in-app "relaunch now" prompt has no process to live in). Also rejected: forked WiX template,
launcher shim, R2-alone.

**C-1 acceptance probe (elegant, reuse it):** after an update, P70's `GitAvailability.source` must read
**`path`**. If it reads `registry`/`wellKnown`, the environment is still foreign and P70 is merely
masking it — every non-git surface in the blast radius stays exposed. Fastest self-diagnosis with no
debugger.

**Do not delete:** `readyToRestart`/Restart UI is unreachable on Windows but is the *sole* relaunch path
on macOS/Linux (mirrored as a doc comment on `UpdateController.restart()` so a future cleanup grep
lands on it).

**FOR USER (§10):** the reporting user's install came from the MSI, so R1 won't reach them until they
reinstall. Recommended: one-time manual uninstall + reinstall from the NSIS `-setup.exe` once P71 ships,
rather than betting a working install on an untested passive-mode WiX→NSIS migration.

**Why:** this is the *upstream* cause of P70. The MSI updater relaunched `bonsai.exe` as a child of
`msiexec.exe`, so the app inherited the installer's environment instead of the user's. P70's resolver
ladder rescues **git only** — every other environment-dependent behaviour has the same exposure
(proxy vars, `SSH_AUTH_SOCK`, credential-helper config, the P49 editor/terminal/file-manager
integrations, the AI CLI resolution in `ai/mod.rs`). Bar: after an auto-update the running app must
have the same environment it would have if launched from the Start menu.

**Open questions the architect must answer with evidence:** which artifact the updater actually picks
when both NSIS and MSI updater artifacts are published (and whether `.github/workflows/` makes that
incidental) · whether the NSIS relaunch path shares the defect · whether the right answer is to stop
auto-relaunching and prompt instead. Requires a `security-auditor` pass (updater trust/install path)
and a native USER CHECKPOINT (real signed update round-trip). **Must not touch
`.tauri/updater-prod.key`** — see P69 FOR-USER item 1.

## 🎯 P69 — 1.0.0 release readiness — ✅ **SHIPPED 2026-08-18** (tag `v1.0.0`)

**Current step:** none — `v1.0.0` tagged and pushed at `bd52483`; the release workflow
builds/publishes from the tag. Reviewer verdict was APPROVE WITH NITS (no MUST-FIX); its two
SHOULD-FIXes were landed (changelog `v0.3.1` compare link + note; CONTRIBUTING apt-list claim)
except the branch-protection check-name question, which needs repo-settings access — **FOR USER:
if branch protection requires the status check `Frontend — vitest + build`, the CI matrix renamed
it to `… (ubuntu-22.04)` / `… (macos-latest)` and the old name will never report again.**

**FOR USER — two open items I could not close:**
1. **Back up `.tauri/updater-prod.key`.** It is correctly gitignored and untracked, so it exists
   in exactly ONE place: this working copy. Losing it permanently breaks auto-update for every
   installed client. (The committed `tauri.conf.json` pubkey was verified to match it.)
2. **GitHub reported 2 Dependabot alerts (1 high, 1 moderate)** on push. The high is the known
   `nanoid` GHSA-2v37-7h3g-55p8 — build/test tooling only (vite/vitest → postcss → nanoid), never
   in the shipped app, deliberately ignored in `pnpm-workspace.yaml`. **The moderate is
   unidentified** — `gh` is not installed here, and both project gates are green
   (`cargo deny --all-features check` = advisories/bans/licenses/sources all ok; `pnpm audit`
   shows only the one ignored high). Check the Dependabot page.

**User decisions taken 2026-08-18** (full text: archive Part 5): scope = macOS defects +
contributor docs only · ship 1.0.0 **unsigned** (per `docs/code-signing.md`; its "decision needed"
is ANSWERED for 1.0: defer) · tag WITHOUT the six outstanding native USER CHECKPOINTs
(P62–P65, P67, P68), forge/PR shipped **flagged beta**.

**v1.0.0 AI gate at `bd52483`:** cargo `--workspace --no-fail-fast` exit 0, 0 failed / 4 ignored
(perf gates) · clippy `-D warnings` exit 0 · vitest 1596 / 130 files · e2e 104 passed, 1 skipped
(the permanent `08-stash` one) · eslint 0 errors, 29 warnings (budget 40) · `lint:size` OK ·
tsc+vite clean. Increment detail (P69a–P69e) + a known Playwright-CI limitation
(the `msedge`→`win32` fix is never exercised in CI): archive Part 5.

### Still open after P69 (explicitly NOT in scope)
`cargo fmt` adoption (1773 hunks / 221 files, no `rustfmt.toml`), SECURITY.md +
CODE_OF_CONDUCT.md, issue/PR templates, refetch-storm coalescing (now a spun-out item below),
`CommandPalette` highlight, persisted-settings write path, `NumberSlider` mid-typing clamp, and
the six native USER CHECKPOINTs themselves.

## 🔍 Audit #2 (`docs/audit-2026-08-18.md`) + fix batch — all confirmed bugs & SHOULD-FIXes **fixed** 2026-08-18/19

Audit baseline at `3a0a153`: cargo 1727/1/4-ignored (the 1 = the §2.1 watcher flake) · vitest 1580
· e2e 104/1-skip · harness clean. Every finding above NIT is now closed:
- **§2.2** CommitPanel mid-stream crash (MUST-FIX) — `ffa80d0` (P69c) · **§3.1** worktree-copy
  symlink write-through — `55acb98` (P69c). Both shipped in 1.0.0.
- **§3.8** streamAssembler throw containment · **§3.9** BulkAiConfirmDialog in `dialogOpen` ·
  **§3.10** AI runs cancelled on workspace unmount — `84cedb7`.
- **§3.2** F-T5-4 corrupt-object hang — `7edd23e`: `run_with_git_timeout` (`git/timeout.rs`,
  30 s inactivity deadline, `BONSAI_GIT_TIMEOUT_MS` override) wraps `get_status`/`get_graph`/
  `stream_graph`/history-index build; C1 now pins Err-not-Hung for read surfaces;
  `create_commit` deliberately UNWRAPPED (a false timeout on a mutation could race a late
  commit — rationale in `corrupt_repo_cli.rs` C1). · **§3.3** hook spawn failures →
  `HookRunInfo::warning`/`hookWarning`; indexer skips → `skippedCommits` + toast — same commit.
- **§3.4** dedupe canonicalize moved off the repos lock · **§3.5** registration filter now skips
  `tests_*` · **§3.6** forge HTTP `redirect(Policy::none())` + bounded body read · **§3.7** AI
  pid zeroed in `reap()`/`complete()` — `67539fd`.
- **§2.1** watcher-test sentinel-file positive sync (5× green solo AND in full workspace) + CI
  `cargo test --no-fail-fast` — `29e72a7`.
- **§4.1** `e2e/11-forge.spec.ts` written (9 tests, +`?forge=unsupported` seam) · **§4.2**
  `usePartialStaging.test.tsx` (24 tests) — `83a9b2f`.

**Gate at fix-batch HEAD `83a9b2f`:** cargo workspace 1754/0/4-ignored · clippy `-D` clean ·
vitest 1629 / 134 files · e2e 114 passed / 1 skipped · lint:ci 0 errors · lint:size OK.

**Still open from the audit:** §4.3–§4.8 test gaps (CommandPalette/NumberSlider pins once fixed,
streaming-graph e2e, 08-stash conflicted-apply fixture, Linux case-sensitivity assertions,
low-value untested units, missing journeys: updater/AI-PR-description/clone-init/worktrees etc.) ·
§7's 13 NITs (recorded in the audit, no action required) · §5.6 perf/visual ACs stay USER
CHECKPOINT (headless harness cannot observe rAF/compositing).

## ✅ Confirmed checkpoints and accepted decisions (condensed — full text in the archive)

- **All native USER CHECKPOINTs for P2 → P61 are CONFIRMED.** Batches: 2026-07-30 (P4, P3a–P3f, P7,
  P7e, P7f, P8, P9), 2026-08-03 (P18–P23, P24, P25, P26, P27), **2026-08-08** ("mark everything as
  checked" — P28 through P61 inclusive: P32, P37–P46, the credential-cache and UX-fix batches,
  Phase 1 P49–P52, Phase 2 P53–P57, Phase 3 P58–P61). P5/P6 were confirmed earlier still.
- **The entire approved roadmap P49–P65 is code-complete** (2026-08-10). P49–P61 are fully done;
  P62–P65 are still `awaiting USER CHECKPOINT` (below).
- **Accepted defaults (2026-08-08, "ACCEPTED AS-IS"; changeable any time):** P55 `undoLastMerge` =
  reset-to-first-parent (Mixed, rewrites history, confirm-gated preview) · P57 retriever = BM25
  lexical, no embeddings · P61 image-diff base64 = hand-rolled, no new crate.
- **OD1 (confirmed):** AI stays **local-`claude`-CLI-only**; model tiers deferred.
- **Forge defaults (2026-08-08, autonomous, accepted):** new Rust deps `reqwest{blocking,json,
  rustls-tls}` + `keyring` · auth = **PAT-only** v1 (OAuth device-flow deferred) · provider order
  GitLab → Bitbucket → Azure DevOps.
- Full text of every banner and decision: `docs/history/todo-archive-2026-08.md` Part 1.

## 🚀 PHASE 4 — forge/PR + paged loading (P62–P65) — code-complete, **awaiting USER CHECKPOINT**

Build detail for all four milestones (per-increment bullets, contract amendments, gate numbers) was
moved verbatim to `docs/history/todo-archive-2026-08.md` **Part 2**. Contracts:
`docs/contracts/phase4-forge-overview.md` + `P62`/`P63`/`P64`/`P65-*.md`. Command count reached
**157** here (P62 +7, P63 +1, P64 +1, P65 +1). Shipped in 1.0.0 **flagged beta** (forge/PR).

- **P62 — forge foundation (GitHub first)** — **awaiting USER CHECKPOINT**
  (`docs/contracts/P62-user-checklist.md`). AI gate green: new `crates/bonsai-forge/` (60/0) +
  7 commands (cmd 154) + right-pane PR panel; harness verified connect→list→detail→create.
  Native half = a real GitHub PAT against real PRs.
- **P63 — forge signals on the graph** — **awaiting USER CHECKPOINT**
  (`docs/contracts/P63-user-checklist.md`). AI gate green: batch `forge_commit_statuses` (cmd 155),
  `forgeBadges.ts` + `useForgeSignals`, PR/CI badges on branch-tip pills, Settings toggles.
  Native half = the canvas badge pixels and canvas click→PR.
- **P64 — GitLab + Bitbucket + Azure DevOps + AI PR descriptions** — **awaiting USER CHECKPOINT**
  (`docs/contracts/P64-user-checklist.md`). AI gate green: `ai_generate_pr_description` (cmd 156),
  three more providers on the same trait (`bonsai-forge` 153/0), per-provider connect hints.
  Native half = live AI generation + real tokens per provider; two reviewer NITs are parked in that
  checklist §D (Azure bad-PAT HTTP-203 message; `dev.azure.com/{org}/_git/{repo}` shorthand).
- **P65 — paged/streaming graph loading** — **awaiting USER CHECKPOINT**
  (`docs/contracts/P65-user-checklist.md`). AI gate green: shared `LaneWalker` + `stream_graph`
  channel (cmd **157**), `compute_graph` output byte-for-byte unchanged, frontend stream assembler,
  120k-commit correctness test. **Shipped with an honest first-paint reframe** — see the finding
  below. Native half = scroll feel + progressive load on a real large repo.
  - **FINDING (kept on the live board because it constrains future work):** libgit2's
    `Sort::TOPOLOGICAL` runs an eager `prepare_walk` before yielding row 0, so first paint is
    **O(total commits)** (release/warm: 40k ≈ 0.73 s, 120k ≈ 1.37 s, 200k ≈ 2.3 s), **not**
    O(first 512). Streaming still gives lane-stable progressive render, scroll-ahead and no giant
    IPC, but "instant first paint on 1M repos" is **not** met by the topo walk. `stream_perf.rs`
    deliberately does not assert a `<150 ms` target.

- **P66 — lazy generation-number topo order** — **deferred** (user decision 2026-08-10: the proper
  fix for the P65c finding is a Large build; not taking it on now). Feasibility spike:
  `docs/contracts/P65a-lazy-topo-spike.md` (VERDICT: TRACTABLE via path (c), effort **L**).
  Shape: reimplement git's lazy `--topo-order` (Stolee generation numbers) in Rust as a shared order
  stage replacing `seeded_revwalk`, sourcing generation numbers from the P52 commit-graph file via
  `gix-commitgraph` or an own parser (git2 0.21 / libgit2-sys 0.18 expose none — grepped, 0 hits).
  Committed P65a/P65b stay as-is (IPC / `GraphChunk` / `LaneWalker` unchanged); only the internal
  walk order changes. Costs: regenerate ALL graph fixtures (lazy order differs from libgit2 in
  commit-date **tie-breaks** only), guarded by a `get_graph ≡ stream_graph` equivalence test + a
  differential test vs `git rev-list --topo-order`. Pre-build: re-verify newest git2 still lacks the
  generation API (F5); confirm `gix-commitgraph` reads P52 split/chain graphs (F1). Architect advises
  **against** shelling out to `git log --topo-order` (would make the git binary a hard runtime dep of
  the core read path). Sets the deferred `stream_perf.rs` first-batch threshold.

## 🐛 USER-REPORTED BATCH (2026-08-17) — P67 UX polish + P68 AI conflict resolution

7 issues from real use (real repo, live merge conflict); user-approved plan:
`~/.claude/plans/1-the-dotted-line-cozy-llama.md`. Item→milestone mapping + `claude` CLI spike
facts: archive Part 6.

**LOCKED USER DECISIONS (asked + answered 2026-08-17; do not re-litigate):**
- AI timeout = **no hard timeout + Cancel button**; idle-output watchdog ~300s; optional hard cap
  configurable, default 0 (unbounded).
- AI visibility = **live log stream + interactive prompts** (user can answer Claude mid-run).
- Log panel location = **bottom dock**, collapsible, full width.
- Conflict-resolve repo access = **read-only allowlist** `--tools "Read,Grep,Glob"` (no
  write/edit/bash; Bonsai still writes nothing — staging stays an explicit post-review call).
- Bulk resolve = **ONE AI run for all conflicts**, with per-file attribution.
- Right-panel density = **tighter default AND** a Cozy/Compact toggle.
- "Stash all" = **demote to a `⋯` overflow menu** (keeps all 3 scopes; sidebar keeps 1-click stash).
- Panel density is **independent** of graph Compact rows (cross-reference hint in Settings only).
- Dashed HEAD line = **always visible while scrolling**.

### P67 — HEAD guideline + right-panel density — ✅ **AI GATE PASSED** (awaiting native USER CHECKPOINT)
Contract: `docs/contracts/P67-ux-polish-batch.md` (+ `P67-user-checklist.md`). **+0 Tauri commands.**

**Current step:** ✅ **P67 CODE-COMPLETE — AI gate PASSED, awaiting native USER CHECKPOINT**
(`docs/contracts/P67-user-checklist.md`). All five sub-increments committed and reviewer-approved:
**P67a** `0ec69f9` · **P67b** `e607d2c` · **P67c** `5e68db5` · **P67e** `d50361a` · **P67d** `f3ca6e5`.
Nothing here is user-confirmed: **the dashed guideline has never been seen by anyone** (headless
harness — no compositing, `rAF` paused, no canvas pixel is ever produced; geometry proven
arithmetically + via the `window.__bonsai.p7` seam). Line visibility while scrolling, dash crawl,
halo termination, marker direction, and compact readability are all native-only. Build detail,
measured reclaim (+115.5 px ≈ 4.8 rows cozy, +30 px compact), contract amendments A5/A6.1–A6.3
(binding, canonical in the contract), the Chromium-only `field-sizing: content` guard, and the
harness gate results: archive **Part 7**.

### P68 — streaming/interactive/bulk AI conflict resolution — ✅ **AI GATE PASSED** (awaiting native USER CHECKPOINT)

**Current step:** ✅ **P68 CODE-COMPLETE — awaiting native USER CHECKPOINT**
(`docs/contracts/P68-user-checklist.md`; the four runs that matter are listed at the end of it).
Every sub-increment committed and reviewer-approved (commit list: archive **Part 8**). **Nothing
about appearance or real-CLI behaviour has been verified by anyone** — no live log, no real tool
use, no real cost and no pixel has been seen.

**Gate history:** P68-final AI gate measured at `44067af` (2026-08-18): tsc 0 · vitest 1580/128 ·
e2e 104/1-skip · cargo core 764/0/1 + bonsai 238/0 · clippy clean · IPC commands **160**.
Superseded by the v1.0.0 gate at `bd52483` and then the audit-#2 fix-batch gate at `83a9b2f`
(cargo 1754/0/4 · vitest 1629 · e2e 114/1-skip — see the audit section above).

Contracts: `docs/contracts/P68-ai-conflict-streaming.md` (invariants D1–D16; canonical for the
durable warnings — D16 stdin/reader ordering, the `RunOpts::default()` non-migration,
`StreamLogItem.assistant_text`; do NOT "fix" those back) · `P68e-ai-activity-dock.md` ·
`P68g-ui.md` · `P68-security-audit.md` · `P68-user-checklist.md`.
**Commands 157 → 160** (`ai_resolve_conflict_stream` = Channel, `ai_cancel_run`, `ai_reply_run`).

**ADDITIONAL LOCKED DECISIONS (asked + answered 2026-08-17):**
- **Spend cap = NONE by default**, configurable (`ai_max_budget_usd` default `0.0` = no cap).
  `--max-budget-usd` is only passed when > 0; live cost shows in the dock so a runaway is visible.
- **Bulk + `autoResolve` = STAGE the marker-free files** from the run; any file still carrying
  conflict markers falls back to review (`hasUnresolvedMarkers` gate). First place Bonsai stages
  several files from one AI call — nothing is committed, everything stays visible and revertible.

**Security follow-ups still OPEN** (audit items 7–11, each with its rationale in
`docs/contracts/P68-security-audit.md` — that file is canonical, do not duplicate them here): the
novel-content gate (the structural defeat for H1), proposals shown as a diff, bulk path-count cap +
per-batch reads + batch count in the dialog, process-group kill off Windows + zeroing `ctl.pid`
after reap (the *zeroing* half landed in `67539fd`; the process-group half is still open), and a
symlink-safe `resolve_conflict_text` write.

Orchestrator-settled OQs, sub-increment commits, durable warnings, accepted limitations:
archive **Part 8**. Original (drifted) board text: archive Part 4.

## 🐞 SPUN-OUT ITEMS — open follow-ups (resolved ones move to archive Part 9)

### CommandPalette highlight resets on `actions` array identity — **OPEN**
`src/components/CommandPalette.tsx:103→107→118` re-lands the highlight on the first enabled row whenever
the `actions` **array identity** changes, and `filterActions` always returns a fresh array. Any producer
whose memo deps churn therefore steals the user's keyboard selection mid-typing:
- **P65** bumps `graph` identity per streamed batch → the highlight jumps while a large graph streams
  (this is the real root cause of the `e2e/09-search-palette` flake, which was worked around by
  settling the stream first rather than fixed).
- **P68e** made it fire ~once a **second** during any live AI run (inline-arrow palette thunks +
  `focusDock` closing over `orderedRuns`). The P68e-side churn was fixed; the component was not.
**Correct fix (reviewer's recommendation):** reset on the filtered **ids**, not array identity — that
immunises every future `actions` producer instead of requiring each one to stay identity-stable forever.
Do NOT keep patching producers.

### Refetch storm: every mutation double-fetches per open tab — **OPEN** (audit #1 §3.10, 2026-08-07)
Every mutation runs `refreshAll` (~9 parallel fetches) and the watcher-debounced `repo-changed` for
the same writes re-runs the identical 9 ~300 ms later, per open tab (`RepoWorkspace.tsx`, was
:871-905/:1008-1046 at audit time). Fix = self-event suppression/coalescing; structural. Was tracked
only in `docs/audit-2026-08-07.md:163-166`; moved onto this board 2026-08-19 (audit #2 §5.3).

### Stash `expectedOid` UI wiring — **OPEN, now unblocked** (campaign deferral)
The Rust side is oid-verified (F-A6-B/F-A7-6) but the UI does not yet pass `expectedOid`. Was parked
on the `ipc/types.ts` freeze (`docs/testing-campaign-2026-08/SUMMARY.md`); the freeze lifted when
forge landed. Moved onto this board 2026-08-19 (audit #2 §5.4).

### Submodule dirty-deinit force flag (F-A7-7) — **OPEN, now unblocked** (campaign deferral)
Same parking condition, now lifted; details in `docs/testing-campaign-2026-08/FINDINGS.md` F-A7-7.
Moved onto this board 2026-08-19 (audit #2 §5.4).

### Persisted-settings write path has three latent defects — ✅ **FIXED 2026-08-19 (P69b)**
Found while extracting `useUiSettings`; none introduced by it (each verified against `git show HEAD`).
1. **Four writers bypass the debounced merge.** `handleSettingsChange` coalesces into one 300 ms
   `setUiSettings`, but `closeOnboarding` (`{onboardingSeen}`), `toggleTheme`, `toggleListView` and
   `commitPaneWidths` each fire their own independent write. Benign **only** because the key sets happen
   to be disjoint today — the ordering is unguaranteed, so any future overlap silently loses a field.
2. **A failed write is dropped and never retried.** `pendingSettingsPatchRef` is emptied *before* the
   `ipc.setUiSettings` call, so on rejection the merged patch is gone: the user gets a toast while the UI
   keeps showing values the disk does not have. Pinned as current behaviour by a test, not fixed.
3. **No unmount flush** for `settingsSaveTimerRef`. A pending patch still fires after unmount (so it is
   not lost in-session), but it is lost if the JS context dies inside the 300 ms window — app quit or
   window close right after a knob change — and a late write can outlive the unmount and race a read.


**Resolved by P69b** (reviewer APPROVED after two fix rounds). 1 = all four writers now route through
one coalescing window via a new persist-only `queueSettingsWrite`. 2 = a rejected patch is merged
back newest-wins AND retried with bounded backoff (300/600/1200 ms, one toast per user action);
correctness required serialising flushes behind an in-flight guard, because `{...merged, ...pending}`
alone could resurrect a stale value when two writes overlapped. 3 = flush on unmount **plus**
`pagehide`/`beforeunload`, since `App` is the root and never unmounts in production — the unmount
hook alone was inert there.

⚠️ **Two traps found during P69b — do NOT "fix" these back:**
- `disposedRef` must be **cleared at the start of the effect body**, not only set in cleanup.
  `main.tsx` wraps the app in `React.StrictMode`, whose dev double-mount runs cleanup once on the
  same hook instance; a write-once flag would permanently dispose the hook at boot and **no setting
  would ever persist in dev**.
- `commitPaneWidths` reading `paneWidthsRef.current` synchronously broke keyboard pane resize:
  `PaneDivider` calls `onResize` + `onResizeEnd` in ONE keydown handler, and the ref only refreshed
  during render, so every Arrow nudge persisted the PRE-keypress width. Fixed by making the ref
  authoritative at call time (`applyPaneWidths`); the render-time assignment is deliberately gone.
  Its test forces `window.innerWidth = 1600` — at jsdom's 1024 the clamp collapses to `SIDEBAR_MIN`
  and the buggy value is indistinguishable, so the test would be vacuous.

**Spun out, not done here:** the teardown flush is dispatch-only, so a hard OS kill can still drop a
pending write (needs a synchronous save on the Rust side); and the save-failure toast auto-dismisses
after 5 s, so a user who looked away never learns — making it sticky was declined because `App.tsx`
has one line of ratchet headroom.

### `docs/contracts/P68e-ai-activity-dock.md` is 1064 lines and now under-describes shipped code — **OPEN**
Twice the ~500-line house limit. Also stale: P68g-1 (audit M3) added two elements to the ask block — an
untrusted-model-output attribution line and a fixed "Bonsai never asks for passwords or tokens" guard —
and made `aria-describedby` a two-id list, none of which §4.1/§4.2 describe. `ui-designer` produced
splice-ready replacement blocks in `docs/contracts/P68g-ui.md` §3.1–§3.5 rather than rewriting a
1064-line canonical contract wholesale with no line-level edit tool (correct call — a truncated write
would have destroyed it). **Needs: apply the splice, then split the file.**

### `docs/contracts/P68-ai-conflict-streaming.md:304` is one module level stale — **OPEN**
Says `session_drain_tests.rs` is `#[path]`-included "as a child of `session`". After the `session.rs`
split it is a child of `session::session_drain` — still a descendant, so the privacy claim holds, but the
wording is out of date.

### `NumberSlider` clamps mid-typing, so a field's own minimum is hard to type — ✅ **FIXED 2026-08-19 (P69c)**
`src/components/NumberSlider.tsx` commits on every `change` while the input is controlled, so with
`min = 60` a user typing `6` then `0` lands on **600**, not 60 (the field snaps to 60 after the first
keystroke, then the next digit appends). Verified. Pre-existing and shared by **every** settings slider,
so it is not P68-specific — but P68g's new limits fields (idle `min 60`, default 300) are where it is
most likely to bite. The USD field in `SettingsAiLimits` already dodges it with a local draft string,
because `Number('12.')` → `12` makes `12.50` otherwise unenterable. **Fix = move NumberSlider to
draft-string + commit-on-blur/Enter**, which changes commit semantics for every settings slider and so
needs its own increment with its own review — deliberately NOT bolted onto a security milestone.
Related and already fixed in P68g-2: clearing the field used to snap the setting to `min`
(`Number('') === 0`), contradicting the component's own doc comment.


**Resolved by P69c** (reviewer APPROVED). Fixed with a draft DISPLAY + the unchanged clamped commit
per keystroke — NOT the commit-on-blur/Enter shape originally planned, which would have killed live
preview for the graph geometry sliders and rewritten three suites for no user-visible gain. The
draft drops on blur, on Enter, and when the incoming `value` differs from the value this control
last committed; that comparison uses the CLAMPED committed value, which is what stops the fix
defeating itself on keystroke 1. Acceptance was that `SettingsSections.test.tsx`,
`SettingsPanel.test.tsx` and `SettingsAiRunSection.test.tsx` pass completely UNEDITED — they do.

⚠️ **Two test-design facts worth keeping (both found by trying to break the tests, not pass them):**
- The bug is **invisible to a naive single `fireEvent.change('30')`** — it only reproduces with a
  controlled parent where the second keystroke appends to what the field actually DISPLAYS. Hence
  the suite's explicit `typeFirst` / `typeMore` helpers.
- The obvious "range stays authoritative" assertion **cannot kill its mutant**: a range input
  sanitizes an out-of-range draft to exactly the clamp the setting already holds, so `value="3"`
  with `min=24` renders as 24 either way. The discriminating case is a **blank** draft, so that pin
  lives in the cleared-field test.

**Carried forward, not done here:** out-of-range drafts now persist with no visual affordance
(typing `128` into Row height leaves the field reading 128 while the setting sits at max, where the
old snap-back made the divergence obvious) — routed to `ui-designer` for the P69g styling pass. And
`SettingsAiLimits.tsx`'s USD field still hand-rolls its own draft with blur-only resync and NO
external-value rule; rule (c) must come with it when it folds onto the shared control.

### `STDERR_GRACE_TOTAL` is not the absolute cap its doc comment claims — **OPEN**
`drain_stderr` checks `Instant::now() < deadline` *before* each `recv_timeout(STDERR_GRACE)`, so the
drain can run up to `STDERR_GRACE_TOTAL + STDERR_GRACE` (~1150 ms vs the documented 1000 ms). Visible
only as ≤150 ms extra shutdown latency; the existing test's 500 ms slack passes either way.

### `cargo fmt` has never been run on this repo — **OPEN**
No `rustfmt.toml` anywhere, no fmt check in any hook or CI. `cargo fmt --all --check` reports **1773
hunks across 221 files**; `--config use_small_heuristics=Max` is *worse* (2065), so no single config
matches the existing hand style. Right shape: its own commit — pick a config, add `rustfmt.toml`,
one-shot reformat, then add `cargo fmt --check` to the gate. **Do it between milestones, never inside
one** (it would bury a review in a mechanical diff).

---

---

### P69 — Settings redesign (two-pane shell + extracted identity menu) — **in-progress**

Plan: `~/.claude/plans/make-the-designer-subagent-compiled-robin.md` (user-approved 2026-08-19).
Frontend-only, **+0 Tauri commands** (160 unchanged). Started from HEAD `e3c4ad1`.

**Goal.** Settings has accreted into a 560px single-column modal with **11 flat sections** / ~45
controls, no nav and no search. Replace it with a two-pane overlay (category rail + content pane +
search), promote identity out of Settings into a header identity menu, unify the control vocabulary,
make global-vs-repo scope explicit, and close the two OPEN defects that live in this surface.

**Locked user decisions (2026-08-19 — do not re-litigate):**
- Shell = **two-pane modal (~880px)** with a left category rail + settings search. NOT a full-window
  page, NOT the current single column.
- Extract **all four**: identity profiles -> header identity menu · Git config -> clearly repo-scoped
  surface · getting-started tour + Updates -> About/Help category · the three AI sections -> one AI
  category.
- Scope = IA restructure **+** control-level polish (toggle switches, row anatomy, help text,
  reset-to-default, keyboard/a11y, both themes) **+** fix both known OPEN defects.

**Binding constraints (verified 2026-08-19, not guesses):**
- `check-file-size.mjs` is a **ratchet**: `src/App.tsx` is baselined at **1168** and may not grow (P73 raised it from 1114 mid-session), so
  the identity menu cannot be markup bolted into App. P69e's prop collapse buys the headroom.
- Toggle switches must be **CSS over a native `<input type="checkbox">`**, not `role="switch"` divs —
  otherwise ~30 existing `getByRole('checkbox', …)` assertions break for no real gain.
- The header menu must read the **effective** identity (local overrides global). Today's badge reads
  `local` only, and the mock seeds identity at **global** with `local` empty, so the default harness
  state is exactly the case the current logic cannot express.
- The `configMissing` deep link (`App.tsx:102-105` -> `SettingsGitConfigSection.tsx:139-144`) must
  select the owning category BEFORE the focus effect runs, or it silently no-ops.
- Contract surface that must not be renamed casually: `#settings-graph-row`, and the accessible names
  `Row height`, `Switch to light theme` / `Switch to dark theme`.

**Sub-increments:** P69a contracts (ui-designer, then architect) · P69b write-path hardening ·
P69c primitives + NumberSlider commit semantics · P69d migrate sections onto primitives ·
P69e props->context (refactorer, identical test counts) · P69f the two-pane shell + search + Ctrl/Cmd+, ·
P69g identity extraction · P69h About + copy + reset · P69i docs.

⚠️ **CONCURRENCY (2026-08-19):** a second session is building **P73 (submodule reconnect)** in the
SAME working tree. Overlap was measured: only **`src/styles.css`** is contested (P73 has it dirty,
+46/-8). P69a (docs only) and P69b (`useUiSettings.ts`, `App.tsx`) touch nothing P73 touches.
**P69c onward is BLOCKED until P73's tree is clean.** Protocol while both run: commit with explicit
paths only (never `git add -A`), scope vitest runs to the files under change, and run no cargo
commands (P69 is frontend-only; concurrent cargo races the shared target dir).

**Current step:** ⏸️ **PAUSED AT THE CSS GATE — all six CSS-free increments are committed.**
Done: **P69a** (both contracts) · **P69b** write path · **P69c** slider minimum · **P69d** a11y
labels + effective identity + two splits · **P69e** defaults + catalog · **P69f** props->context.
Every one reviewer-APPROVED; commits `5a10704` `24fc732` `36b08b5` `45bee34` `44bd029` `4f98d1e`
`d015c7e` `605606d` `21ec9af` `be9975c` `4fe3341`.

**NOTHING THE USER SEES HAS CHANGED YET** beyond two accessible-name fixes and the identity pill.
The two-pane shell, the rail, search, the header identity menu and the control re-skin are all
still to build — they are specified in full but not implemented.

**NEXT: P69g, the first CSS increment — BLOCKED** on the concurrent P73 session holding
`src/styles.css` (held for this entire session; P73 is still active). Remaining after it: P69h
git-config scope · P69i identity extraction · P69j graph/AI re-skin · P69k search · P69l docs.

**Before P69g starts, two things must happen:**
1. The architect must rule on the anti-drift DOM guard for `identities` / `git-config` (see the
   follow-ups above) — P69g is where that guard gets written.
2. `ui-designer` needs to spec the out-of-range draft affordance P69c introduced (typing `128` into
   Row height leaves the field reading 128 while the setting sits at max; the old snap-back made
   the divergence obvious).

**P69h inherits a measured fact:** `getConfig(repoId,'local')` fires THREE times at mount — from
`SettingsGitConfigSection` (own view), `SettingsHooksToggle` (nested inside it), and
`useEffectiveIdentity` (via the profiles section). `CuratedConfigEntry` already carries
`effectiveValue`/`effectiveLevel`, so all three collapse to one. Deliberately untouched by P69f
because fixing it in a refactor pass would have invalidated the equivalence proof.

Superseded detail: P69a DONE — both halves (`docs/contracts/P69-settings-ui.md` +
`ui-reference.md` §12 + `docs/contracts/P69-settings-shell.md`). P69b code-complete, reviewer
returned CHANGES REQUESTED (2 MUST-FIX), round 2 in flight.

**Increment plan re-cut by the architect (supersedes the plan's P69c-P69i):** ten increments
P69c->P69l. **P69c-P69f are CSS-FREE** (NumberSlider fix -> standalone behaviour/a11y + two required
splits -> data layer -> refactorer props->context behind the OLD layout) and can run while P73 holds
`src/styles.css`. **P69g is the CSS gate** (shell + primitives + reset), then git-config, identity,
graph/AI re-skin, search, docs.

**Ratchet ceilings re-measured 2026-08-19 (earlier figures were stale/misread):**
`src/App.tsx` 1168 (now 1161) · `src/ipc/types.ts` **2701 = exactly at baseline, ZERO slack** ·
`src-tauri/src/settings.rs` **663 = exactly at baseline** (NOT 1778) · `SettingsPanel.tsx` and
`src/ipc/mock/persistence.ts` are unbaselined, so a hard 500 ceiling. Consequence: new TS types for
P69 must live in NEW modules, never appended to `types.ts`; `settings.rs` cannot take even a `mod`
line, so the Rust half of the defaults-parity test goes in `settings_ui_tests.rs`.

**Orchestrator-settled OQs:**
- **OQ-1 `NumberSlider` = draft-DISPLAY + clamped commit per keystroke** (architect's
  recommendation), NOT the plan's commit-on-blur/Enter. It fixes the real defect (the display
  snapping to `min` is what made the next digit append, e.g. min 24: typing `3` snapped to 24 so
  `30` was unreachable) while KEEPING live preview for the graph geometry sliders and leaving all
  three pinning suites green and unedited. Commit-on-blur would have killed live preview and
  rewritten `SettingsSections.test.tsx:56-64`, `SettingsPanel.test.tsx:129-140`,
  `SettingsAiRunSection.test.tsx:195-226` for no user-visible gain.
- **OQ-2** defaults-parity: land the TS half now; the Rust `assert_eq!` half is a short follow-up
  because it needs `cargo test`, which the P73 no-cargo protocol forbids.
- **OQ-3** prop count goes 41->44; collapsing further needs `useUiSettings` ownership to move — a
  separate milestone, not P69.
- **Focus trap DEFERRED** (architect §8): no shared trap hook exists and no dialog has one; a
  Settings-only trap is an inconsistency plus risk to ~30 role queries. Ship focus RESTORE only.
- **Search deferred to the LAST increment**: a search box that can only find 3 of 7 categories'
  rows is a control that lies, and a disabled one is a dead control.


## Archive

| File | Covers |
|---|---|
| `docs/history/todo-archive-2026-08.md` | P65 → P28 build detail, the Phase 1–4 banners, resolved FOR-USER decisions (Parts 1–4, moved 2026-08-18); P69/P67/P68 build detail, the 2026-08-17 batch mapping + spike facts, resolved spun-out items (Parts 5–9, moved 2026-08-19) |
| `docs/history/todo-archive.md` | P27 → P2, M0–M6 |
| `docs/history/milestones-mvp.md` | the M0–M6 AI-gate vs USER CHECKPOINT split |
| `docs/history/context-pollution-audit.md` | the context/token-cost audit |
| `docs/contracts/INDEX.md` | one line per contract file — milestone, scope, status |

Move a milestone's section into `todo-archive-2026-08.md` only once **both** halves of its gate have
passed. A milestone with a pending USER CHECKPOINT stays on this board. Archiving is a **move**, never
a delete — condense on the board, keep the full text in the archive, and leave a pointer.
