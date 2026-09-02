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

## 🧹 File-size refactor pass — 2026-09-02 — DONE

Behavior-preserving split of the 9 largest files back toward the ~500-line limit.
Ratchet baseline: **27 offenders / 6241 excess → 20 / 3528**. Full `pnpm gate` green
(8/8, 603s) on a quiet tree. 17 files modified, 60 new files, 454 insertions / 9636 deletions.

| File | Before | After |
|---|---|---|
| `src/components/RepoWorkspace.tsx` | 2760 | 2309 (77 → 34 `useState`; 9 hooks extracted) |
| `crates/bonsai-core/src/assets/bundle.rs` | 1366 | → `bundle/`, 9 modules, max 355 |
| `crates/bonsai-core/src/git/stash/tests.rs` | 1330 | 349 (+4 modules) |
| `crates/bonsai-core/src/assets/profiles.rs` | 1205 | → `profiles/`, 7 modules, max 353 |
| `crates/bonsai-core/tests/diff/diff_cli.rs` | 1026 | 307 (+3) |
| `crates/bonsai-core/tests/rebase_merge/rebase_interactive_cli.rs` | 1021 | 279 (+3) |
| `crates/bonsai-core/src/graph/tests.rs` | 1012 | 93 (+4) |
| `src/App.tsx` | 977 | 602 (+6 hooks, 2 dialogs) |
| `src/graph/GraphCanvas.tsx` | 973 | 784 (+4) |
| `tests/rebase_merge/{rebase,merge,conflict}_cli.rs` | 923/763/640 | 239/222/147 |
| `tests/diff/{stage,discard}_partial_cli.rs` | 760/504 | 342/355 |

Equivalence proof per increment: identical before/after test counts (`bonsai-core` 1554,
`h_diff` 80, `h_rebase_merge` 102, vitest 229 files / 2644 tests, graph e2e 25), plus
line-multiset diffs showing zero logic-line changes on the Rust splits.

**Three files deliberately stopped short of 500** — each remaining cut would have produced a
file forwarding 15–100 values to exactly one consumer (an unreadable pair, not a smaller module):
`RepoWorkspace.tsx` 2309 (render body already fully extracted; rest is state+effects+handlers),
`GraphCanvas.tsx` 784 (per-frame paint path + a 29-value handler closure — hot path, 20k-commit
target), `App.tsx` 602 (launch effect needs 6 of App's own setters threaded in).

### Follow-ups spun out of this pass (all filed as tasks, none blocking)

- **Reflog overlay not torn down** when a repo goes unusable — `runRefreshRound` clears blame +
  history + compare + opState + tagSync, but not reflog. Real bug.
- **Fold-pill cursor is dead** in `GraphCanvas.handleMouseMove` — P92 §1.4's overflow-cursor write
  unconditionally clobbers spec-004 §1/§2's `foldCursorFor`, and `computeHoverTarget` returns null
  on exactly those rows. Real regression; no vitest mounts `GraphCanvas`, so e2e is the only net.
- **Shortcuts stay live during confirm dialogs** — `pendingForcePush`, `pendingCommitPush`,
  `pendingBisectBad` are absent from `dialogOpen` (`abortConfirmOpen` is handled separately).
  Force-push is destructive, so this one matters most.
- **`ai::session*` is load-flaky** — wall-clock watchdog margins (2s idle timeout vs a ~3s stub)
  fail under CPU contention, pass in isolation. Hit independently by 3 agents; makes the crate
  suite unreliable as a gate under load. Needs a clock seam, not wider sleeps.
- **Contract divergences** the tests document as bugs-in-the-contract (both say "reported to the
  orchestrator"): rebase §3.1.5/§9.7 unstaged-changes precondition, and the libgit2-vs-CLI
  rename/delete conflict index-entry count. Plus a near-tautological `expected_presence` oracle.
- **Duplicated external-tool launchers** — `App.tsx`'s trio is statement-for-statement identical to
  `repoWorkspace/useExternalTools.ts`; hoist to `src/hooks/`. Also two timers with no unmount
  cleanup (`sessionSaveTimer`, toast auto-dismiss).
- **Duplicated helpers left visible, not merged** (behavior risk, not a move): atomic-write helpers
  across `assets/bundle/write.rs` + `assets/profiles/store.rs`; test helper families across
  `tests/diff/` and the four `tests/rebase_merge/*_support.rs`.
- **`image_diff_cli_2.rs`** numbered split remains — renaming changes nextest IDs, so it needs its
  own increment where that IS the expected diff.
- **`cargo fmt --check` is not clean at HEAD** repo-wide and is not gated. New files inherit the
  existing drift deliberately (reformatting would have destroyed the proof-by-diff). Repo-wide
  `cargo fmt` is a separate decision.

---

## 🎨 P102 + P105 — hue audit — AI GATE GREEN, ⏳ AWAITING USER CHECKPOINT (AC18/19/20)

**Current step:** ✅ **AI GATE GREEN — MILESTONE COMPLETE bar the USER CHECKPOINT.**
contract `7c623d8` → impl `0e5dcab` → both reviews APPROVE after fixes → fixes `185c352` →
**FULL gate green**: `--quick` all 7 steps (255.9s) **and e2e 181 passed / 1 skipped (2.7m)**.
e2e was deliberately run *after* the ui-designer released harness port 1420 — running two things
against one dev server is how e2e specs flake.

**⏳ AWAITING USER CHECKPOINT: AC18 / AC19 / AC20.** Not self-declared; the user's 2026-09-02
checkpoint authority was scoped to P100 + P101 and explicitly does **not** reach work created that
session. Evidence gathered to make the check fast: **AC18** — the updater panel is Tauri-only, but
its `.btn-danger` shares the exact rule measured at 4.80/4.93 rest and 5.18/5.49 hover, so the
question is "does it look right", not "is it legible". **AC19** — `--accent-strong` holds hue within
**0.7°** of `--accent` in both themes (219.0° vs 219.7°); the open question is purely whether the
dark `#7fabff` reads washed-out. **AC20** — no `filter` remains on any of the four hover targets, so
there is no compositing pass left to flicker and layout cannot move (background-only swap); what is
being judged is fill-change feel alone.

**Gate `--quick` @ `185c352`:** cargo nextest 139.0s ✓ · doctests 4.0s ✓ · clippy 22.4s ✓ ·
eslint 16.2s ✓ · **file-size ratchet 1.0s ✓ (the blocker is cleared)** · vitest 59.6s ✓ ·
tsc+build 13.7s ✓.

**The predicted grep residue matched reality on every line** — this is the milestone's central
claim, and it held: `color: var(--accent)` 30→**7**, `--accent-strong` 0→**18**, hex ink 3→**0**,
`filter: brightness` 4→**1**, `background: var(--danger)` 4→**4**, `--danger-text` 0→**4**,
`--success-text` 0→**1**. (Raw greps return 4 hex / 5 brightness; both extras are *comment* lines at
`dialogs-forms.css:164` and `controls.css:96` — declaration counts match.) Predicting the residue
*before* implementing is what makes "closed" checkable instead of asserted, and it is the practice
P95/P98/P74 skipped.

**Two implementer deviations, both flagged rather than buried (verdicts pending review):**
1. **Six `.asset-chip-*` modifiers fixed, not four.** `.asset-chip-sync` (4.02 dark / 3.61 light)
   and `.asset-chip-drifted` (4.86 / 3.50) are the same hue-over-own-tint defect inside the same
   rule block and render *beside* the fixed chips in `ProfileActivateDialog` /
   `StaleBranchesDialog`; fixing 4 of 6 would leave exactly the split the contract's own C3b
   reasoning forbids.
2. **One rule added that the contract missed:** `.settings-toggle-btn.is-active:hover:not(:disabled)`.
   `.btn-secondary:hover:not(:disabled)` is specificity (0,3,0) and out-specifies C5's (0,2,0), so
   the selected `--selection` fill vanished on hover. If the specificity claim holds this is a
   contract gap, not scope creep — **a contrast fix that is out-specified by an existing rule is a
   no-op that still passes a grep**, which is worth remembering as a review heuristic.

### Design review — **APPROVE** with one MUST-FIX, and the designer named it as its OWN defect

`ui-designer` verified the grep invariants independently (7 / 18 / 0 / 1, tokens in both blocks,
all five ratios recomputed to ±0.01) and did the visual half the implementing agent could not:
**both themes, 1440×900, computed styles with tints flattened to their composited backdrop.** Every
measured pair clears its bar — pills 4.80-6.24 dark / 4.93-5.08 light, `.right-pane-tab.active`
9.36/13.29, the six `.asset-chip` modifiers 8.99-9.67 / 11.29-11.85. **AC12 confirmed by
measurement:** all seven chip variants *and* the bare base measure exactly **19.9375 px**, so the
transparent base border really does hold outer height.

**MUST-FIX — `.forge-connect-link` (`forge-pr.css:592`) lost its RESTING distinction.** It is an
inline `<a>` in a `--text-2` `<p>` with `text-decoration: none` at rest. Because `--accent-strong` is
tuned to sit *near* `--text-2`, the swap drove link-vs-prose luminance **1.43 → 1.02** dark and
**1.72 → 1.28** light; WCAG G183 wants ≥3:1 when colour is the only resting carrier. **The designer
attributed this to its own contract** — B6 claimed "the underline is the second carrier and stays",
which was wrong about the resting state, while the implementer's code comment ("hover underline") was
accurate. Fix is `text-decoration: underline` at rest; routed to the in-flight senior-dev.

**Both deviations ACCEPTED, and both traced to contract gaps rather than implementer licence.**
(1) The six-chip expansion was **mandatory**: `ProfileActivateDialog.tsx:225-229` renders "new file"
beside "changed" and "unchanged" *in one row*, so fixing 4 of 6 would have shipped two label
treatments inside a single line of a single dialog. The miss was the contract keying its enumeration
on `color: var(--accent)`, so C3b was hand-added rather than swept. (2) The added hover rule closed a
real gap — §3.6 claimed per-call-site hover coverage but never walked the base component's cascade.
Both are now generalised into `ui-reference.md` §2 as **the specificity trap**, sibling to the
child-rule trap.

**The methodological correction behind the wrong count is the durable part.** §2's "6 live instances"
is replaced by a **16-row table carrying `file:line`, selector, hue and tint percentage**, plus a
note on `.checks-rollup-pill--pending` / `.graph-filter-chip-stale`: **descendant-selector cases
where the ink and the tint live in different rules — which a same-rule-block grep can never find.**
That is why the count was wrong, and it is the search that P107 must actually run. The failed-claim
tally in §2 is now **FIVE**, the fifth being that file's own figure, and the "no AA colour shortfall
remains" line is replaced with an explicit prohibition on ever writing it again.

### Review round 1 — both reviewers say **request changes**, one MUST-FIX each

**Code review MUST-FIX — the D4 fix was DEAD CSS, and this is the finding that justifies the whole
method.** `.diff-float-discard` is a `<button>` inside `.diff-stage-float`, so the generic descendant
rule `.diff-stage-float button` **(0,1,1)** out-specifies the modifier **(0,1,0)**: `--accent` beat
`--danger`, and `--accent-text` beat the new `--danger-text`. The hover was *newly* dead — the old
`filter` did not compete (different property), whereas `background` does. **The destructive discard
button was rendering in accent blue with no danger hue at all**, while passing every AC grep. The
contract had recorded D4 as "no visual change — ink on a `--danger` fill," which is not what rendered.
**Heuristic worth keeping: a contrast fix that is out-specified by an existing rule is a no-op that
still passes a grep.** Routed to senior-dev with the fix (`.diff-stage-float button.diff-float-discard`)
plus a requirement to prove it in the harness, in both themes.

**Design-review findings routed with it:** the "last colour literal" claim is **false** —
`src/graph/forgeBadges.ts:22` still holds `PR_MERGED_COLOR = '#8957e5'`, and light theme now visibly
**diverges** (CSS pill `#8250df` vs canvas badge `#8957e5`, same concept, same screen). And the C5
comment claims "matching specificity" when the selector is (0,4,0), not (0,3,0) — it works *only*
because it is strictly higher, so the comment as written invites a silent revert.

**Open question sent back for evidence, not guessed at:** `.settings-toggle-btn` may never render
with `is-active` at all (all 29 call sites look static). If so, C5's recipe change **and** the added
hover rule are unreachable — and the more interesting possibility is that the settings toggles never
indicate their active state to the user, which would be a **product bug, not a CSS one**. Asked for a
verdict with evidence before anything is changed.

**Confirmed good by independent recomputation** (worth recording, since the claims were the point):
all five tokens exist in **both** themes — no silent dark-value inheritance; all seven residue counts
reproduce exactly; the 7 `--accent` survivors are genuinely glyphs with a named non-colour carrier;
`--accent-strong` worst case **4.93** dark / **4.87** light really is surface-independent. Deviation 1
(6 chips not 4) **upheld** — `.asset-chip-drifted` is the *most* widely rendered chip in the app, so
fixing 4 of 6 would have left the worst-exposed one broken. Deviation 2's specificity claim is real.

**AC16 partial / AC17 half-open, stated plainly rather than rounded up:** AC16's stylelint clause is
**unsatisfiable — no stylelint exists in this repo**; tsc, build, eslint and 235 targeted tests pass.
AC17's fixtures are added and verified, but the implementing agent had **no browser tooling**, so the
visual half went to the ui-designer review pass. Branch: `feat/p91-observability`
(USER decision 2026-09-02 — everything this session lands on that one branch; no push, local
commits only).

**The enumeration overturned the filing rather than confirming it — this is the method working.**
Two seed-list facts were simply wrong: `controls.css:70` is **`.pill-detached`**, not `.btn-danger`
(the real one is `updates.css:109`); and a **third** site the seed list never named,
`.pr-state-pill`, puts **one `color: #ffffff` over three different fills** — including white on the
dark `--success` fill at **2.85:1**, worse than the P102 seed defect and below even the 3:1 graphics
bar. Had this milestone "fixed the two known sites", it would have shipped and left the worst
instance in place. Same story on P105: the loose `var(--accent)` grep returns 140 hits in 34 files,
but only **30** are `color:` declarations — 110 are legitimate graphics-bar uses, and of the 30,
**7 are glyphs that correctly stay** at 3:1. A blanket sweep would have wrecked those.

**Counts:** P105 — 30 `color: var(--accent)` declarations in 16 files → 7 KEEP (glyph, 3:1, each
with a named non-colour carrier), 18 → `--accent-strong`, 5 recipe changes, +1 scope addition
(`.asset-chip-active`). 21 of 30 measurably sub-AA. P102 — 3 hardcoded-ink declarations + 4
`background: var(--danger)` fills.

**Tokens:** `--accent-strong` `#7fabff` dark / `#2a5cbe` light — hue-preserving, and deliberately
**surface-independent** (clears 4.5:1 on `--bg-0`…`--bg-3`, `--selection` and an accent tint in both
themes; worst case 4.87). That independence is what makes the post-fix check a single grep instead
of a per-site ancestor argument. Plus `--danger-text` / `--success-text`, all `#16181d` dark /
`#ffffff` light, mirroring `--accent-text`.

**ORCHESTRATOR DECISION (§5.4, Option A).** The merged-PR purple `#8957e5` **passes** contrast
(4.61 both themes) so it is not a P102 defect, but it is the last theme-invariant hue literal in
`src/styles/` and it sits inside the rule block P102 must rewrite. Chose **Option A** — add
`--merged` / `--merged-text` — so AC7 is a clean zero-count and the light theme gets a purple tuned
for a white page rather than a dark-theme value reused. Taken as a token-consistency call inside the
design system's own logic, **not** a product-identity call; it therefore did not need the user and is
not a checkpoint. Recorded because the contract explicitly forbade the implementer choosing silently.

**Scope calls.** IN, on measured merit: three of P100's four `filter: brightness` residuals —
`.btn-danger:hover` (4.17:1 light), `.diff-float-discard:hover` (4.32:1 light),
`.diff-stage-float button:hover` (the `.btn-primary` 3.95 case). OUT: the fourth,
`.settings-switch-track`, measures 3.80:1 **with no ink on it** — compliant, stays a NIT.

**AC18/AC19/AC20 are USER CHECKPOINTs and remain PENDING** — the updater panel's `.btn-danger` is
Tauri-only and invisible in the harness; whether `--accent-strong` still reads as *the Bonsai blue*
and how hover feels after the `filter`→`background` swap are both perceptual. The user's 2026-09-02
checkpoint authority was scoped to P100 + P101 and does **not** reach these.

**Why the two are one milestone.** Both are "a hue token used against an insufficiently contrasting
surface", and both want the same enumerate-then-bucket method. Kept as two clearly separated
sections inside one contract so implementation and review can still be staged. This is the USER's
call, recorded here because P100 §6-C deliberately kept one defect class per milestone — the
exception is that these two share a *method*, not just a symptom.

**No architect pass.** Consistent with P100 and P101, which both ran on a ui-designer contract
alone: a CSS contrast audit has no module boundary, IPC surface, or algorithm for the architect to
design. Recorded so a future session does not read the skipped step as an oversight.

**Scope in one line each.**
- **P105 (🚨)** — enumerate every `color: var(--accent)` call site, resolve each one's composited
  backdrop per state, bucket text (4.5:1) vs glyph/border/bar (3:1), and correct the false
  `ui-reference.md` §2 sentence claiming `--accent` is fine on `--bg-0`/`--bg-1`/`--bg-2`. It is not:
  `--bg-1` fails in light (4.34) and `--bg-2` fails in both themes (4.48 dark / 4.00 light).
- **P102 (📋)** — sweep every `#fff`/`#ffffff` on a `var(--danger)` fill (not just the two known
  sites), and introduce a `--danger-text` token mirroring `--accent-text`'s per-theme split. The
  remedy already ships in-repo at `partial-staging.css:104` (the `--bg-0`-ink flip, 4.80:1 dark).

**The standing lesson this milestone must honour.** Three app-wide claims in this programme have now
failed on inspection — P95's enabled-control class (3 escapes found by P101), P98's "`--text-3`
family closed" (122 declarations never classified), and P74's hue-as-text sweep (this milestone).
**A bucket + verdict per call site, P101 §3 style, or it is not closed.** Do not accept another
"~30 call sites and it's fine" sentence as evidence.

**Acceptance criteria:** owned by the contract (AC1..ACn), not restated here. Any AC needing the
native window is a **USER CHECKPOINT and stays pending** — the user's 2026-09-02 checkpoint
authority was scoped to P100 + P101 only and explicitly does NOT extend to work created this
session.

---

## 📋 P106 — the A/M/D/U/R letter badges are TEXT, not glyphs — PENDING (filed 2026-09-02 from the hue audit)

Filed by `ui-designer` during the P102/P105 enumeration, **with the measurements already taken**
(contract §3.5), so this follow-up starts from evidence rather than from a suspicion.

The status letter badge (`A`/`M`/`D`/`U`/`R`) is rendered as a **character**, which puts it at the
**4.5:1 read-text bar**, not the 3:1 graphics bar it is currently judged against. Its
`--danger` / `--success` / `--warning` backings therefore need the same ink-flip treatment P102
applies to the button fills — and `--warning` in particular has not been measured against a
letterform anywhere yet.

Same defect class as P102/P105, deliberately **not** folded in: this milestone already spans two
hue tokens and 30+ call sites, and one defect class per milestone is what kept P95/P98/P100
reviewable. The `--danger-text` / `--success-text` tokens P102 introduces are the prerequisite, so
P106 should be cheap once this lands.

### Filed alongside, not fixed (all from the same enumeration)
- **`.wt-copy-chip`** — a sixth hue-over-own-tint instance, found after the contract's bucket table
  was closed.
- **`.settings-switch-track` NIT** — 3.80:1, but carries no ink, so it is compliant; recorded only
  so a future sweep does not "fix" a non-defect.
- **`src/styles/forge-pr.css` is ~710 lines** and over the ~500-line soft limit. This milestone adds
  no new rule block there, so the split is **not** in scope — hand it to `refactorer` as a
  standalone behavior-preserving pass.

---

## 🔀 DX — flipping the e2e bundle default — READY, HELD FOR THE USER (2026-09-02)

P104 is cleared, so nothing blocks the flip. **I did not make it**, and the reason is a measurement
gap rather than caution for its own sake: the reported figures are **162 s dev server vs 122 s
bundle**, but it is not established that the 122 s includes the **bundle build step**. If it does
not, the default gate could get *slower*, which is the opposite of the change's purpose. Flipping a
default the user runs on every gate, on a number that might exclude its own dominant cost, is not
mine to do while they are away.

Everything else favours it: bundle mode is equally green (181 passed / 1 skipped, 0 failed in both
modes) and has **higher fidelity** — P103 was a real product bug that only the bundle-vs-dev
equivalence check exposed, because `import.meta.env.DEV` code is absent from a production bundle.

**The change is one line each**, trivially reversible:
`playwright.config.ts:38` → `const BUNDLE = process.env.E2E_BUNDLE !== '0'`, plus inverting the
`--e2e-bundle` flag in `gate.mjs`. **To decide: time one `E2E_BUNDLE=1` run from a cold build** and
compare against the 162 s dev figure including build.

---

## 🔐 SECURITY AUDIT of the P91 surface — 2026-09-02 — **F1 IS A SHIP BLOCKER**

Run deliberately *now* because **P91 has never shipped** (absent from `dev`): this is the last point
at which a privacy defect can be fixed before it starts writing durable files on real users' disks.
No user is affected today and there is no migration problem.

### 🚨 F1 — HIGH, reachable today. Raw mode writes commit messages, search text and forge tokens to disk — while the consent dialog promises it does not

`src/obs/ipcProxy.ts:106-123` serialises **every positional argument verbatim** in raw mode. So with
Dev mode + "Include raw repository names" on, these land in `logs/*.jsonl`:
`commit(repoId, message, …)` → **the full commit message**; `searchCommits(repoId, query)` → **the
search text**; `forgeSetToken` / `forgeAddAccount` / `forgeSetTokenForHost` → **a forge PAT in
cleartext**.

**The credential scrubber cannot save it, for a structural reason worth remembering:**
`scrub_value` (`scrub.rs:345`) applies `is_sensitive_key` to **JSON object keys**, but raw `args` is
keyed **positionally** (`"0"`, `"1"`, …) — so no key ever matches `token|secret|password|auth`.
Survival then rests on shape alone, and `looks_like_opaque_secret` (`scrub.rs:106`) requires
length >= 32 **and explicitly exempts all-hex words** (a deliberate guard for commit SHAs). Net
effect: GitHub `ghp_`/`github_pat_` and GitLab `glpat-` are caught by prefix, but a **40-hex
Gitea/Forgejo token and a 20-char Bitbucket app password pass through unredacted.**

**What makes this a blocker rather than a bug:** the consent dialog
(`DevConfirmDialogs.tsx:39-42`) states, at the moment of consent, that *"Commit messages, file
contents, author names and email addresses are still never written, and passwords and access tokens
are never written in any mode."* And the designed workflow is enable → reproduce → **export the zip
and send it to a maintainer.** The product would be telling the user something untrue precisely when
they are deciding whether to trust it.

**Root cause is the contract, not just the code.** Line 910 says args are "included as `args`" in
raw; line 914 says commit messages and search queries are "never, in either mode"; line 918 says
tokens are "NEVER, under any setting". Those cannot all hold, and **the implementation resolved the
conflict in the leaking direction.**

**✅ RULED + CONTRACT DELIVERED `834f2d1` — `docs/contracts/P91-raw-args-privacy.md`.** Line 910 was
the defect; the two absolute "never" rows stand. **One rule: raw mode widens IDENTIFIER fidelity
(repo path, file paths, ref names, remote URLs, full SHAs) and never CONTENT fidelity.** Free text
and credentials are outside both modes, permanently. Grounds: two absolute "never"s outrank one
mechanism description that never mentions them; consent is bounded by what the dialog promised at
the moment of consent; §7.1 already defines raw's purpose as real *names*; and the failure is
unrecoverable in **one direction only** — a PAT in a mailed zip — versus mere reviewer legibility in
the other.

**Mechanism:** sparse per-command allow-list at `src/obs/rawArgPolicy.json`, **default DENY**, keyed
by parameter **name** rather than position, **scalars only**. Name-keying is what makes
`is_sensitive_key` meaningful inside `args` for the first time; scalars-only kills
`searchCommits(_, query: SearchQuery)` by shape alone. `looks_like_opaque_secret` and the hex
exemption are deliberately **unchanged** — §13 row 19 ratified them and they are load-bearing for
SHAs.

**The strongest part of the design, worth not eroding later:** the writer-side check
(`obs/raw_args.rs`) **deliberately does NOT consult the allow-list.** A shared table would be
worthless against the failure that actually matters — a wrong row, or code that ignores the table,
which is precisely today's bug. It enforces a shape+vocabulary invariant it can decide alone, and
**its key-shape rule (`^[a-z][A-Za-z0-9]*$`) kills the current leak even if the producer is never
fixed**, because today's leak is keyed `"0"`/`"1"`. Violations drop the **whole** `args` object and
set a writer-set `argsPolicyViolation` that a producer cannot forge.

**AC6 is the negative test**, and it is well chosen: it plants a 40-hex token *specifically because
that shape passes the hex exemption*, so only the key rule can catch it. It must fail on current
branch code.

**Orchestrator decision on the architect's escalation:** ACCEPTED — add the path-/ref-taking command
rows under the §B.2 derivation rule **in the same increment**. The six seed rows mostly document
denials, so raw mode would currently yield almost no argument values, and *a raw mode that shows
nothing invites someone to "fix" it by reverting to a blanket include.* That is the failure mode this
whole ruling exists to prevent.

**Still owed:** implementation (held until the F2-F5 agent releases `writer.rs`), and a
`ui-designer` pass so the consent copy is not merely true but **complete** — it should name search
terms and other free-text arguments, and describe raw as "real repository, file, branch and remote
names" rather than "arguments". AC12 gates that before the USER CHECKPOINT.

### F2 — MEDIUM, **latent**. The `bump_counter` guard is a `debug_assert`, compiled out of release

The audit's answer to the question posed as highest-consequence: **the release path does NOT drop an
invalid key — it records it verbatim.** There is no `[profile.release]` override, and
`is_valid_counter_key` is referenced *only* from that assert, so in a shipped binary the function has
no effect whatsoever.

**Not reachable today** — `fold_perf` inlines its own `PERF_KEYS` loop and there is **no production
caller at all**. The real hazard is the doc comment claiming "today: `fold_perf` and tests", which is
**false** and invites the next developer to believe a validated caller exists. Combined with a guard
that silently evaporates in release, the first person to wire this to anything repo-derived writes
permanent, un-deletable content to a user's disk **with no test failing.** The sibling validators
`is_valid_cmd_name` and `is_valid_err_code` are already real `if` guards — this is the odd one out.

### F3 — MEDIUM, reachable. Unbounded metric-key cardinality

`log_append` takes records straight from the webview, and `is_valid_cmd_name` is a **shape**
predicate rather than membership in the real command set; the maps have **no cardinality cap**. It
lands in a file `logs_delete_all` does not cover and only a headless `metrics_reset` can clear.

### F4 — MEDIUM, reachable from the webview. `log_export_session(dest)` is an arbitrary-path write

The code comment justifies taking `dest` verbatim because it "is the result of the OS save dialog".
**That dialog does not exist** — the only caller passes no argument. So it is an unmediated
arbitrary directory-create + file-write primitive, and it defeats the export-scope rule that exports
stay in `exports/` where delete-all covers them.

### F5 — MEDIUM. Default umask permissions

Log parts, export zips and `usage.json` are created with no `set_mode`; on Linux/macOS that is
typically `0644`, world-readable, for files carrying absolute repo paths and real branch names.

### F6 — LOW/MEDIUM, a **product** call for the user, not a security fix

`usage.json` is **always-on durable local telemetry**, independent of Dev mode, from first launch:
`firstSeen`, launch count, and a **400-day** per-day profile of which operations were performed and
how long they took. It survives `logs_delete_all` **by design**, `metrics_reset` has **no UI**, and
the privacy panel never mentions the file exists. Content is non-identifying by construction, so this
is a **disclosure** question, not a leak. **Since P91 has never shipped, now is the moment to decide
whether a local Git client should keep an undeletable 400-day usage profile with no disclosure.**
Held for the user.

### F7 — LOW, mostly latent. `redact_names` misses bare ref/file names and never touches JSON keys

A branch like `feature/acme-client-migration` has one separator, is unrooted and has no extension, so
`is_path_shaped` returns false and it would be written **verbatim into a strict file**. Not reachable
today — every free-form payload traced to a static allow-listed literal — but see F9:
`strict::enforce` is the **sole** enforcement point for both Rust and frontend records, so a gap here
is a single point of failure for the whole system.

### Verified CLEAN (recorded so a later session does not re-audit)

`react.ts` redaction is now safe **by construction**, not by disuse. The IPC dispatch shim cannot
alter, drop, reorder or convert a failure, and carries `cmd` + trace ids only. Error strings never
cross IPC. Zero-cost-when-off is **structural on both sides** — nothing collects-then-suppresses.
Log rotation and purge have no traversal. **CSP and capabilities are well hardened**: script-src is
self-only with no unsafe-inline or unsafe-eval, and there is no shell or fs plugin, so a renderer
compromise is not shell access. Updater trust chain sound; no key material in the repo.
External-process launching uses argument vectors, never a shell string. Forge credential storage via
the OS keychain is well built — **F1 is a leak *around* it, not a defect in it.** And
`no_proxy_client()`'s raw `.expect` sits in a `#[cfg(test)]` module that never compiles into the
shipped binary — **the long-standing DEP-REFRESH follow-up about it can be CLOSED.**

### F9 — INFO, and it reframes the "do the two redactors disagree?" question

They cannot disagree, because **only one enforces**: `redact.ts` has no equivalent of `redact_names`;
all name redaction for both sides happens once in `strict::enforce` on the writer thread. That is the
correct architecture — a frontend bug cannot write an unredacted name into a strict file — and it is
exactly why F7's gaps matter more than their current reachability suggests.

**Order of work:** F1 before merge (architect + senior-dev in progress) → F2 → F4 → F5 → F3.
F6 is the user's call; F7 is a judgement call.

---

## 📋 P108 — hue-as-text over NEUTRAL surfaces — PROPOSED, not enumerated (filed 2026-09-02)

Proposed by `ui-designer` while enumerating P107, and deliberately filed **without** a count or a
closure claim — which is the programme's method working, since five prior claims failed by asserting
app-wide scope before enumerating.

Distinct from P105 (which covered `color: var(--accent)` specifically) and from P107 (hue text over
its *own* tint): this is any hue token used as text over a **neutral** `--bg-*` surface.

Seed observation, and it is pointed: **`.dev-status-write-failed .dev-status-state` measures
4.41 dark** — that is P91 §8.4's own **"▲ Not writing"** string, the indicator whose entire job is to
stay legible during a disk failure. Do not treat the seed as the scope; run the three-pass search
P107 established, including the `--h` indirection and the tinted-parent child classes no hue-name
grep reaches.

---

## 📋 P106 — backdrops now measured for it by P107 (2026-09-02)

P107 found **exactly one** overlap and **left it to P106** rather than both contracts claiming it.
The hand-off is the set of backdrops P106 had not measured for `.file-status-deleted .file-badge`:

| backdrop | dark | light |
|---|---|---|
| `--bg-1` | 4.41 | 4.60 |
| `--bg-2` hover | **3.89** | **4.24** |
| `--selection` | **3.05** | **3.96** |

And the *added* badge sits on `.status-section--staged`'s 6% `--success` tint at **5.26 / 4.38** —
**success ink on a success tint, failing in light at rest.** P107 does not touch `status-panel.css`.

---

## 🧭 DEAD CSS — `.settings-toggle-btn.is-active` matches nothing — DECISION NEEDED (found 2026-09-02)

Found independently by **both** the code reviewer and the designer, which is why it is recorded as
established rather than suspected: no component composes `is-active` onto `.settings-toggle-btn`.
All 29 call sites are static `className="btn-secondary settings-toggle-btn"`. The `is-active`
consumers are `.conflict-editor-mode-btn`, `.search-toggle` and `.command-palette-option` only.

So P105's C5 recipe change **and** the hover rule added in `0e5dcab` are both **correct and both
currently dead**, and AC6/AC17 cannot be closed for C5 by observation.

**Two dispositions, and they are not equivalent:**
- **(a) Wire it up** — `ui-designer` recommends this: several `settings-toggle-btn` call sites
  (`ProfileManager`, `SettingsMcpSection`) are segmented controls that clearly *want* a selected
  state. If so, **the real defect is that those toggles never indicate their state to the user** —
  a product bug, not a CSS one.
- **(b) Delete both rules** as dead styling.

**Held for the user**: (a) changes what the app does and is not a contrast fix, so it does not
belong inside this milestone. The CSS stays (it is correct, and harmless while unmatched).

---

## 📋 P106 — priority RAISED (2026-09-02, on measurement)

Filed earlier from `--danger`/`--success`/`--warning` letter badges being *text* at the 4.5:1 bar.
The design review then measured `.file-status-deleted .file-badge` at **4.41 dark on `--bg-1`** —
**it fails at rest, not merely on hover**, which is worse than §3.5 implied when P106 was filed.
Treat as the next hue item after P107.

### Also filed from the design review (non-blocking)
- The 90-char branch-name chip becomes a 50 px two-line stadium at `border-radius: 999px`.
  Pre-existing; newly *visible* because the milestone added the fixture that reaches it.
- `ui-reference.md` is growing fast (§2 now carries a 16-row evidence table). Worth a
  `docs-curator` pass; `TODO.md` is also past 1400 lines against its ~300-line target.

---

## 🐞 P91 — SHOULD-FIX follow-ups from the increment-4-7 review (filed 2026-09-02, non-blocking)

Both are **documentation-accuracy** defects: the prose claims more completeness than the code
delivers. Filed rather than routed back, per velocity mode.

- **`SAVE_LOCK` orders the rename pair but NOT the snapshot** (`metrics.rs:379-382`, `:409-425`).
  `(path, file, dirty)` is read under the `MetricsState` mutex and the guard is *released* before
  `metrics_file::save`. Two savers can therefore snapshot in order A→B but acquire `SAVE_LOCK` in
  order B→A, so the older bytes land last. **The dangerous instance is exactly the pair the new
  `SAVE_LOCK` doc cites as its motivation:** `reset()` snapshots the emptied file, an in-flight
  flush holding an older non-empty snapshot commits after it, and **the reset is silently undone on
  disk**. Both paths then set `dirty = false`, so nothing reschedules a corrective write until the
  next counter bump. The reasoning behind choosing a mutex over a unique tmp name is sound as far as
  it goes; the doc at `metrics_file.rs:52-66` just overstates it as "serializes the whole commit
  sequence". Fix: take `SAVE_LOCK` around snapshot+save, or add a generation stamp, or amend the doc
  to name the residual staleness. **No deadlock risk** — verified both call sites drop the guard
  before `save`, `save` never re-enters `MetricsState`, and it is a plain sync fn.
- **The `last_fire` prune is answer-preserving on the window axis but assumes non-decreasing `ts`**
  (`window.rs:52-63`), and the comment states it unconditionally. Record `ts` comes from two
  **unsynchronised clocks** — `Date.now()` on the frontend (`src/obs/log.ts:43`) and `now_ms()` in
  Rust (`sink.rs:385`) — merged into one writer stream via batched `log_append`, with no monotonic
  clamp anywhere. If `ts` regresses by more than the window, a pruned entry that would have
  debounced a fire is gone and the rule can double-emit. **Blast radius is a duplicate anomaly
  record, never a missed one**, and `events` already carried the same exposure — so this is a
  comment fix stating the monotonic-`ts` premise, unless a `cutoff` guard is wanted.

**NIT worth keeping:** `dup_ipc_debounce_map_stays_bounded_over_a_long_session` spaces events 100 ms
apart against a 300 ms window, which makes `len <= 4` nearly tautological. It proves pruning happens,
not the bound — a dense burst of distinct keys inside one window still grows the map to that burst's
cardinality. Unlike `open_calls` (FIFO 1024) and `slow` (LRU 200), `last_fire` has **no numeric cap**,
so §11's "bounded" is genuinely weaker for this map than for its neighbours.

### Also filed 2026-09-02
- **`.forge-connect-link:hover` is now a no-op** — the resting-underline MUST-FIX means hover declares
  the same underline, so the link has **no hover feedback at all**. A thickness bump is the
  contrast-safe option; the implementer correctly declined to invent a treatment. → `ui-designer`.
- **`docs/contracts/pr-badge-placement-ui.md:106,116,156`** still documents the canvas merged pill as
  `#8957e5`, now stale after the `--merged` token landed. → contract owner.
- **`.settings-toggle-btn.is-active` — the investigation CLOSED it as (b), dead styling, NOT a
  product bug.** Git history pins it: the rule was introduced in `cf174ff` for the git-config
  **Local | Global** level toggle, which `7354aca` (P69h) then replaced with `SettingsSegmented`
  (`role="radiogroup"`, styled via `.settings-segment.is-selected` — a different class). `is-active`
  was orphaned at that moment and has matched nothing since. All 29 surviving call sites are one-shot
  **action** buttons (Refresh, Edit, Delete, Activate…) that should *not* carry a selected state. So
  **no settings control is missing a state indicator** — the earlier "possible product bug" reading is
  retracted. The honest close is to delete both rules and optionally rename the misnamed class. CSS
  left in place (correct but unreachable); deletion is a trivial follow-up.

---

## 📋 P107 — hue-over-own-tint: 38 call sites (§2 had claimed 6) — CONTRACT DONE, IMPL PENDING

**The fourth app-wide claim in this programme to fail on inspection**, after P95's enabled-control
class (3 escapes found by P101), P98's "`--text-3` family closed" (122 declarations never
classified), and P74's hue-as-text sweep (which became P105). `ui-reference.md` §2 asserted **6**
live hue-over-own-tint instances. A full scan during the P102/P105 implementation found **16**
outside that milestone's scope:

`.pr-mergeable-clean` / `-conflict` / `-pending`, `.asset-badge-ok` / `-warn`,
`.danger-badge.safe` / `.caution` / `.destructive`, `.asset-readonly-banner`,
`.asset-issue-error` / `-warning`, `.conflict-kind`, `.error-banner`, `.graph-truncated-banner`,
`.settings-ai-status-warn`, `.wt-copy-chip`.

**CONTRACT DELIVERED 2026-09-02 — `docs/contracts/P107-hue-over-own-tint-ui.md` (`59061b2`), and
the population is 38, not 16.** The brief required a *broader* search rather than a reconfirmation,
on the grounds that a count agreeing with the wrong count is not evidence. It moved:

- **Pass (i)** same-rule-block — the search that produced the wrong 6.
- **Pass (ii)** a multiline descendant-combinator grep — recovers cases where ink and tint live in
  different rules (the `.pr-state-open` pattern).
- **Pass (iii)** two classes even (ii) cannot see, and this is the durable lesson: **unqualified
  child classes whose only parent is tinted** (found by *reading the 25 tinted containers'
  components*, not by grepping), and **custom-property indirection via `--h`** — 11 instances across
  4 families that **no hue-name grep can ever reach**. A fifth family was hidden by **token
  aliasing**: `--badge-good`/`--badge-warn` are byte-identical to `--success`/`--danger`.

**Buckets: 17 failing text · 19 compliant glyph KEEPS · 1 failing glyph state · 1 owned by P106.**
The 19 keeps matter as much as the fixes — a blanket sweep would have wrecked the toast, submodule,
ai-dock and git-dock pill recipes. The single glyph-state failure, `.error-dismiss:hover` at
**2.96 light**, is **compliant at rest** and surfaced only because the contract resolves each call
site's backdrop *per state*.

**Zero new tokens**, and `--warning-text` is explicitly **not needed** — no non-compliant
solid-warning fill exists, and `--bg-0` already resolves to exactly what such a token would carry.
**Recorded trap: `--danger-text` on a 14% danger tint is 1.27:1 — the `--*-text` tokens are
fill-inks and must never be put on a tint.**

**A `ui-reference.md` §2 recipe was retracted, not patched:** it offered "a 35% border **or** a
leading bar/glyph at the 3:1 bar". A 35% hue edge measures **1.58/1.69** — decoration, never an
identity carrier. Solid-hue bars do pass (4.41-7.92) and stay.

**13 ACs with a predicted post-fix residue** (`--danger` 35→27, `--success` 12→9, `--warning` 24→17,
`--text-1` 159→171, `--h:` unchanged at 15, 9 files, 0 outside `src/styles`, 0 `.tsx`).
**AC11/AC12/AC13 are USER CHECKPOINT and stay PENDING.** Implementation NOT started; it edits 9
files under `src/styles/`, so it must not overlap another CSS pass.

**Method note worth carrying forward:** measurement was calibrated by reproducing six independently
recorded historical values *before* trusting any new number, and a `color(srgb …)` vs `rgba()`
serialization trap invalidated the first run outright. Both are recorded in the contract.

**The pattern is now established well enough to state as a rule.** Every one of the four failures
had the same shape: a sentence claiming an app-wide property, with a call-site count that nobody
enumerated. P101 §3 is the template — enumerate, bucket, record a verdict per site, predict the
post-fix residue, then verify the prediction. **Do not accept a "~N call sites and it's fine"
sentence as evidence.** P102/P105 is the first milestone in the programme to hit its predicted
residue exactly on every metric; that is the standard P107 inherits.

---

## 🔧 GATE BLOCKER — `pnpm lint:size` fails on files this branch grew — PENDING (found 2026-09-02)

**Independently confirmed by three agents**, the second by stashing its own CSS changes and
re-running to prove the failure was not its own. ⚠ **One agent misattributed this to "the concurrent
hue-audit/P91 work" and called it "the P107 blocker" — both wrong**, and recorded here so the error
does not propagate: it predates today's session entirely, and P107 is the hue-over-own-tint
enumeration, a different item. The cause is: `crates/bonsai-core/src/ai/session.rs` (505 → **538**) and
`crates/bonsai-core/src/ai/session_tests.rs` (509 → **540**) breach the size ratchet. Both grew in
commit **`734b310`** ("test(ai): drive the session watchdog from an injectable clock, not wall
time") **on this branch**, and the baseline was never updated.

**The fix is a split, not a baseline bump.** Per CLAUDE.md the ratchet is a deliberate work queue,
so raising the baseline would discard the signal rather than answer it. Route to `refactorer` as a
strictly behavior-preserving pass (identical before/after test counts). Held until the P104 e2e
investigation finishes, because `refactorer` proves equivalence by running tests and P104 is
measuring wall-clock timings on the same machine.

**This blocks the full `pnpm gate`**, so it must land before the hue-audit milestone's step-7 gate
can be called green.

---

## 📐 P91 — Observability: Dev mode, structured logs, local telemetry & metrics — PLANNING (awaiting user approval)

**Current step:** ✅ **INCREMENT 1 DONE + COMMITTED `1b94529`** on `feat/p91-observability`
(cargo --lib 360 passed / obs 43, clippy -D warnings clean, tsc clean, all new files <500 lines).
Reviewer round 1 = "request changes" (2 MUST-FIX in the privacy layer); all fixed + re-verified.
**✅ INCREMENT 2 DONE + COMMITTED `6fc3131`** (frontend pipeline: proxy, trace, ui:-prefixed redact
mirror, batcher; tsc clean, vitest src/obs+src/ipc 348 passed, broad IPC-consumer pass 1989 passed).
Reviewer round 1 = "request changes" on 1 MUST-FIX, fixed + independently re-verified.
**✅ INCREMENT 3 DONE + COMMITTED** (Rust dispatch shim + emit→emit_logged migration + §3.1 spans +
2 Layer-A redaction patterns + mock span fixtures). Shim compiled against tauri 2.11.5 (§2.3.1
contingency NOT needed). cargo --lib 370 passed / 1 ignored (overhead bench), clippy -D clean both
crates, tsc + build clean, vitest 2503 passed. **Reviewer round 1 = APPROVE, no MUST-FIX.** All 3
deviations accepted by reviewer (see below).
**✅ INCREMENT 4 DONE + COMMITTED** — the flicker payload: `obs/react.ts` (useRenderCount/
useTracedEffect/useStateTransitionLog) + `obs/renderTally.ts` (aggregate mode) + `obs/gesture.ts`;
ten surfaces instrumented per the pinned §9.2 map; echo/refresh causality (`armEcho(repoId,trace)` +
suppressed-`watcher` record + `pendingTracesRef`/`refresh` records); gesture trace origination for
the mutation/refresh cluster; `withTrace` now emits one `gesture` record per mint + no-ops when off;
`frameStats` `onWindow`→`frame` routing. ui-designer pass first (a843174), architect ratified D3
(3a5c800). tsc + eslint clean, 920 vitest green (incl. real-Sidebar churn test). **Reviewer +
ui-designer both APPROVE, no MUST-FIX.**
**✅ INCREMENT 5 DONE + COMMITTED** — anomaly detector (backend Rust): `obs/anomaly.rs` +
`anomaly/window.rs` + `anomaly/slow.rs` + shared `obs/histogram.rs`; hooked into `sink.rs`
`writer_loop` (observe after seq assignment, derived anomalies written back, `on_session_end` on
shutdown+disconnect). All §5 sink rules + §5.1 perf rules + `SLOW_RULES` table. `dup-ipc` filters
EXPLICITLY on `LogPayload::IpcCall` (synthetic-argsHash negative test genuine). Redaction-independent
(byte-identical raw vs `path#N`). 100 obs tests / 46 new. clippy -D clean. **Reviewer APPROVE, no
MUST-FIX.** Percentile = linear interpolation (§8.1) clamped to `max_ms` (needed for §12(a) to fire).
**Decision: mock-side anomaly (5b) IN PROGRESS** — the milestone AI-gate + §6 require the browser
harness (`__bonsaiDumpLogs()`) to show anomalies with no Tauri, but the detector runs only on the
Rust writer thread. Building a mock-only, dump-time batch analyzer scoped to EXACTLY the 3 gate-named
rules (`dup-ipc`/`slow-command`/`slow-phase`); Rust `obs/anomaly.rs` stays authoritative.

**✅ INCREMENT 5b DONE + COMMITTED `74cfef5`** — mock-only dump-time anomaly analyzer
(`src/ipc/mock/obsAnomaly.ts`) scoped to the 3 gate rules (dup-ipc/slow-command/slow-phase) so the
browser harness can assert anomalies with no Tauri; Rust `obs/anomaly.rs` stays authoritative.
Ratified §6+§13 row 22. 7 vitest. (Orchestrator fixed a raw-NUL key separator → `` escape so
the file stays plain-text.)
**✅ INC-5 RATIFICATIONS COMMITTED `64e842c`** (clamped percentile, BatchMark, mock-split).
**✅ INCREMENT 6 DONE + COMMITTED** — durable metrics (backend): `obs/metrics.rs` + `metrics_file.rs`
(atomic save + `.bak` recovery) + `histogram.rs` derived p50/p95 (never persisted); perf.rs absorbed
at flush; `<domain>.<action>` allow-list (no user-derived key); `metrics_snapshot`/`metrics_reset`
(headless, no catalog row) + mock. All §12 row-6 + §8.1 acceptance verified; clippy clean, 438 tauri
/952 core cargo, tsc clean. inc-5 histogram NIT fixed. **Reviewer APPROVE, no MUST-FIX.** §8
"always on" ratified (counters always-on; duration histos Dev-mode-only — §11 zero-cost-off).
**✅ INCREMENT 7 DONE + COMMITTED (7a/7b/7c)** — Dev settings page + `logs_delete_all` + truncation
+ disk-write-error. ui-designer reconciliation `333a157` (§16). **7a `cbafdc7`** (backend: RollAndPurge
roll-then-purge, `logs_delete_all`, `truncate` record + SessionPayload.truncated/droppedParts,
LogSessionInfo.droppedParts; purge scope reviewer-verified confined; all §12 row-7 (a)-(l) backend).
**7b `35b410e`** (frontend: DevCategory page + small sections + header pill + catalog `dev` category
after About + DevSettings threaded through the settings pipeline; all §16.9 states; catalog-parity
+343 vitest; reviewer + ui-designer APPROVE, 1 design MUST-FIX (aria-disabled dimming) applied by
orchestrator). **7c** (disk-write-error: `LogSessionInfo.writeFailed` bool — never the error string;
`▲ Not writing` status row + DevModePill danger variant; sticky-until-flush semantics; + 2 NIT fixes
+ inc-3 pool-gauge doc narrowing; reviewer APPROVE, privacy PASS). writeFailed field ratified inline
in contract (§8.4/LogSessionInfo). **§8.4 danger toast (dedupe `dev-sink`) DEFERRED** (status card +
pill already surface the state persistently — ratified as follow-up in contract).

**🎉 ALL 7 INCREMENTS IMPLEMENTED + REVIEWED + COMMITTED.** Endgame:
- ✅ **Refactor DONE + committed `5d6321c`** — 4 size targets split behavior-preserving (writer.rs
  598→455, useWorkspaceKeyboard.ts 508→381, GraphCanvas.tsx 926→897, RepoWorkspace.tsx 2837→2778);
  identical test counts; `lint:size` GREEN.
- ✅ **FULL `pnpm gate` GREEN except 1 known flake** — rust nextest (430s) + doctests + clippy +
  eslint + **file-size ratchet** + vitest (2545) + tsc+build ALL ✓. Only e2e red = 1 test
  `16-history-undo-health.spec.ts` "repo health panel renders sections", which **PASSES 6/6
  isolated** — the documented pre-existing flake (P89 memory), P91 touches no history/undo/health
  code. Gate log: `scratchpad/gate.log`.
- ✅ **INCREMENT 7d DONE + COMMITTED `e769108`** — closed a CRITICAL integration gap found during the
  harness-evidence step: the frontend obs pipeline was fully built + instrumented but **never activated**
  because `configureObs(dev)` (the `obsEnabled()` master switch) had ZERO production callers. (`attachSink`
  was already wired via `instrumentIpc` at module load — my initial finding mis-named it.) Fix: a
  `[dev]`-effect in `useUiSettings` calling `configureObs(dev)` on boot + every change + an anti-regression
  guard test (globs source, asserts both `configureObs`/`attachSink` keep a production caller). Backend was
  already correct. **Lesson: per-layer unit tests all passed (each correctly gates on obsEnabled()); only an
  end-to-end activation check exposes a switch nobody flipped.** THIS is why the harness/gate step exists.
- ✅ **HARNESS AI-GATE EVIDENCE (headless, deterministic) — `src/obs/pipeline.e2e.test.tsx`**: drives the
  REAL chain (configureObs → instrumentIpc proxy → batcher flush → ring → analyzer → `__bonsaiDumpLogs()`).
  Two real identical instrumented invokes → exactly one `dup-ipc` whose refs resolve to two `ipc.call`
  records; seeded slow fixture → `slow-command`+`slow-phase` with refs resolving to span+result. 8/8 green.
  Stronger than a browser screenshot (asserts exact refs/seqs).
- ✅ **FRONTEND GATE RE-VERIFIED @ e769108**: vitest 2553 passed (223 files), tsc+build clean, eslint 0
  errors (35 warn ≤40), lint:size OK (both refactored files shrank below baseline). Rust gate stands (7d
  frontend-only). Full gate = GREEN except the 1 known `16-history-undo-health` flake (passes 6/6 isolated).
- ⏳ **Real `logs/*.jsonl` parse** → folded into USER CHECKPOINT (needs native `pnpm tauri dev`; the
  schema/redaction validation it does is already covered by the Rust redaction/strict/schema unit tests).
- ⏳ Present AI-gate + USER CHECKPOINT — asked, not declared.
- ⏳ Real `logs/*.jsonl` parse from a `pnpm tauri dev` boot+idle (native build cost — likely folded
  into USER CHECKPOINT (c)).
- ⏳ Present AI-gate + USER CHECKPOINT (a)-(f) + 7b/7c native items — asked, not declared.

**Increment 7 follow-ups (non-blocking):**
- **SHOULD-FIX (7c):** `writeFailed` can flap healthy during a PERSISTENT ROTATION-OPEN block —
  `open_part(next)` fails (record dropped) but `self.file` still points at the healthy old BufWriter,
  so the idle-flush clears `write_failed` until the next record re-sets it → pill/status flickers.
  Disk-full/permission (write_all/flush) paths are correctly sticky. Fix: gate the flush-clear on
  "rotation healthy" (`writer.rs` ~227/330). Narrow edge (needs cap-hit + dir-unwritable + timing).
- **NIT:** pill poll 3s vs status card 2s (intentional; note so nobody "fixes" it).
- **7b follow-ups:** PushToast has no action-button support → "Show in folder" toast actions omitted;
  command-palette Dev entries (§1.4) skipped; byte formatter is house `formatBytes` → "MiB" not "MB".
- **inc-3 shim exclusion-list NIT CLOSED:** `metrics_snapshot`/`metrics_reset`/`logs_delete_all`
  landed in inc-6/7a and both reviewers verified the exclusion-list names match the registered cmds.

**Increment 5b DONE + COMMITTED `74cfef5`** — see below (kept for detail).

**Increment 5 follow-ups (non-blocking):**

**Increment 6 follow-ups (non-blocking, SHOULD-FIX):**
- **Concurrent-save race:** `metrics_file.rs:63` uses a fixed `usage.json.tmp` and `save()` runs
  OUTSIDE the `MetricsState` mutex — two overlapping savers (60s flush vs `metrics_reset`, or
  exit-flush vs timer) could interleave and rename a torn file onto primary. `.bak` still holds the
  prior good copy so no total loss, but fix: unique tmp name (pid+counter) or a dedicated write mutex.
- **No parent-dir fsync** after the renames (`metrics_file.rs:75-79`) — power-loss can lose the
  rename (reverts to prior good primary; no total loss). Known durability gap.
- **NIT:** startup span between `set_active` and `init()` is discarded (`init()` sets `g.file`
  wholesale) — Dev-mode+startup only. `bump_counter` is `pub`+unvalidated — consider `#[doc(hidden)]`
  or a debug-assert the key has no `/`,`.`,space.

**Increment 5 follow-ups (non-blocking):**
- **SHOULD-FIX:** `ipc_calls.last_fire` (`window.rs:84`) is keyed by `cmd\0argsHash` and never
  pruned — argsHash keyspace is unbounded (every other last_fire map keys on a finite catalogue).
  Fire-gated + Dev-mode-only + session-scoped so growth is tiny in practice, but it falsifies the
  "bounded (§11)" claim at `anomaly.rs:47`. One-line fix: prune in `Sliding::prune` or cap the map.
- **SHOULD-FIX:** ratify the `max_ms` percentile clamp into §8.1 (batched into the architect call).
- **NIT:** `histogram.rs:73` "only reachable at i==7" comment is wrong — `percentile_ms(0.0)` with
  empty bucket[0] returns `max_ms`; inert now (only 0.95 used) but inc-6 reuses this — fix in inc-6.
- **NIT:** `BatchMark` arm skips `emit_pending_drops` (minor ordering); `unbatched-sink` 10th-mark ref
  points at the anomaly not the batch's first record (info-severity); `jank-trace` "overlapped 0
  span(s)" when no overlapping span (confirm §5 wording — likely intended: unattributed jank is jank).

**Increment 4 — the trap that was avoided (worth keeping):** the ambient trace is SYNCHRONOUS (§2.5)
and dies across `await`; a mutation handler does `await ipc.mutate()` THEN `refreshAll()`, by which
point `currentTrace()` is empty. Pattern chosen: capture the trace at each handler's sync entry and
thread it BY VALUE `refreshAll(scope,trace)`→`refresh(origin,scope,trace)`→`armEcho(repoId,trace)`;
`currentTrace()` is only ever read synchronously (fallback for callers inside their own gesture's
sync extent, e.g. manual refresh). A negative-control test proves an unbound post-await refresh logs
`causedBy:undefined`, never a stale trace. `withTrace` also made a no-op when Dev mode off.

**Increment 4 follow-ups (non-blocking):**
- **SHOULD-FIX:** `react.ts` `useStateTransitionLog`'s `briefValue` caps length but does NOT route
  `from`/`to` through `obs/redact.ts` — would leak raw values (branch names) IF wired. Safe today
  (called nowhere); gate wiring behind redaction before any use. Documented as a known limitation.
- **NIT:** `frame` records split paint vs gap into two records with the other dimension hard-`0`
  (`GraphCanvas.tsx emitFrameRecord`); a consumer could misread `gapMs:0` on a paint record. Add a
  `dim:'paint'|'gap'` discriminator to disambiguate (trace-level, low severity).
- **NIT:** `useRenderCount` 'each' calls `logRecord` in the render body, so React StrictMode dev
  double-invoke can double container render counts (fine for a dev-only tool; worth a comment).
- **Deviations accepted (consistent w/ §12 row 4):** `refresh.ms` = round execution duration;
  gesture origination wired only for the mutation/refresh cluster.
- **Gestures NOT wired in v1** (deferred by design): command-palette/appCommands entries; manual
  Refresh button + focus/activation origins; graph context-menu checkoutCommit/checkoutRemote
  (shared deps object — wrapping churns hook identity); non-echo-arming submodule init/update/sync +
  handleSetRemoteUrl (config-only, no refresh round); merge/rebase/bisect/cherrypick/revert/conflict/
  AI flows. Revisit if the six surfaces + inc-5 anomalies prove insufficient.

**Increment 3 deviations (reviewer-accepted, recorded here per D3's "record in contract" ask):**
(D1) global `ACTIVE_SINK` static for `PhaseRecorder`/watcher instead of threading `AppHandle` —
avoids the §2.2-forbidden command-signature churn; set/cleared under the writer slot lock, tests
serialize via `test_sink_lock()`. (D2) `queuedMs`/pool gauge + `deadlineFrac` wired at the 3
span-emitting command sites, not in `repo_handle.rs`; queue delta still captured across the real
`spawn_blocking` boundary (reviewer: correct). (D3) `status.scan`/`diff.compute` collapsed to a
single phase each (contract specced 3) because finer splits need phase hooks INSIDE `bonsai-core`,
which the crate-boundary invariant forbids (`bonsai_core_has_no_obs_reference`); only `graph.get` is
fully phased. **Architect should ratify D3 into §3.1.2/§13.**

**Increment 3 follow-ups (non-blocking, for tester / later increments):**
- **SHOULD-FIX (tester):** `POOL_INFLIGHT` is bumped only by the 3 span-site `PoolGuard::enter()`
  calls, so `poolInflight` counts "instrumented git ops in flight," NOT true tokio blocking-pool
  saturation; `POOL_MAX = 512` is hardcoded, not read from tokio. Row-3 acceptance (b)'s
  `poolInflight >= poolMax` is unreachable in a realistic test — write the tester assertion against
  what the gauge actually counts, or narrow the field doc wording (`phase.rs:75-101`, `:35`).
- **NIT:** `invoke_shim.rs:42-48` exclusion list names `logs_delete_all` + `metrics_snapshot` which
  aren't registered yet (ship in inc 6/7) — dead entries now; confirm the strings match when those
  commands land, else self-amplification silently re-enables.
- **NIT:** recorder-overhead bench for acceptance (e) is `#[ignore]` — tester to run it explicitly.
- **NIT:** `stream_graph_cached` (`graph_cache.rs:239`) always finishes `SpanOutcome::Ok` even on
  `Err` (non-routed diagnostic path; routed path correct).
- **NIT:** diff span covers only `get_workdir_file_diff`; commit-vs-parent diffs uninstrumented (v1).

**Increment 2 notes worth keeping:** (a) MUST-FIX was that `__trace` injection sat one layer too
high — the proxy appended trace metadata as an extra JS argument, but every real IPC method is
fixed-arity/positional and builds its own payload, so **nothing ever reached the backend**; the two
object-arg methods (`setUiSettings`, `setSession`) got keys nested INSIDE a payload that is
persisted verbatim. Increment 3's core criterion was unachievable as built, and the test masked it
by using a 2-arg signature that does not exist in the real api. Fixed by creating
`src/ipc/tauri/invoke.ts` (the codebase had NO central invoke wrapper — 21 modules imported it
straight from Tauri) which stamps at the payload top level; injection removed from the proxy, which
also kills the mock-persistence pollution by construction. A glob-based guard test forbids direct
`@tauri-apps/api/core` invoke imports so an untraced command cannot be reintroduced silently.
(b) The guard's own first two versions **passed on a real offender** because of a regex escaping
slip — only the negative control caught it. Lesson: assert a guard fails before trusting that it
passes. (c) `argsHash` is computed BEFORE injection, else every call hashes uniquely and `dup-ipc`
could never fire. (d) Reviewer resolved the canonical-form worry with stronger evidence than the
implementer's: `IpcRecvPayload` has NO `argsHash` field, so only one canonical form exists in the
stream and `dup-ipc` cannot be fooled.

**Carry-in for increment 3:** the transport stamps only when an ambient trace exists (§2.5 — never
guess), so increment 3's criterion reads as "every **traced** dispatch yields a stamped
`ipc.recv`"; untraced calls dispatch with no `__trace` rather than a fabricated one. Wiring traces
to real gestures is increment 4's job. Also: `configureObs()` and `pendingRestart()` are unused in
production by design — activation is increments 4/7.

**Increment 1 notes worth keeping:** (a) strict mode was NOT enforced — the writer trusted the
producer, so a frontend bug or mode-toggle race would write real paths/refs into a file whose own
header claimed `redaction:"strict"`. Now enforced writer-side in `obs/strict.rs` (args dropped
unconditionally; URLs/refs/paths ordinalised in every string field), because §7 makes this a
Rust-owned guarantee. (b) Error messages were exempt entirely — an `AppError` Display string leaked
BOTH a username and a repo name; same pass fixes it. (c) `glpat-` (26 chars) was *structurally*
uncatchable under a global 40-char floor — `TOKEN_PREFIXES` now carries per-prefix minimums.
(d) senior-dev self-review caught strict enforcement mangling the header's OWN `redactionNote`
(the disclosure text contained slashes); note is now asserted verbatim. (e) salt-never-on-disk test
found a real gap — a producer echoing the salt back reached disk; `scrub_salt` added.
(f) Rotation now drops the OLDEST part instead of refusing to rotate (orchestrator call — the old
behaviour left a storm session completely unbounded, since prune only ran at startup);
**needs architect ratification into §6.**

**Deferred to the architect (non-blocking):** ratify the rotation drop-oldest rule into §6; ratify
or veto senior-dev's base64-secret heuristic (slash-bearing candidates qualify only when a
`/`-free run is ≥24 chars + alpha/digit mix, minus pure-hex so SHAs survive; floor lowered 40→32).
**Known over-redaction (fail-safe, tested):** scp-style remotes ordinalise as `path#` not `remote#`;
UUID-shaped strings ≥32 chars now over-redact. 🛑 **USER GATE (2026-08-27): finish
increment 1 — review → MUST-FIX → commit — then STOP AND WAIT. Do NOT start increment 2 without an
explicit go from the user. Each subsequent increment requires its own go.**
**Perf-diagnosis addendum DONE (decision 8, 2026-08-27)** — added after the user asked whether the
plan also identifies performance issues. New §3.1 `span` record kind carrying a flat `phases[]`
array per completed operation (≤16, dotted labels), plus optional `queuedMs`, `poolInflight/Max`,
`deadlineFrac`, `cache`, `items`. Timing is taken at the **src-tauri caller layer** so
`crates/bonsai-core` gains NO dependency on `obs/`; the mechanism is an explicit `PhaseRecorder`
value, not a task-local (same reasoning as §2.2's rejection of an ambient backend trace). Exactly
three instrumented ops in v1 — `graph.get` (revwalk/decorate/lane/filter/serialize), `status.scan`,
`diff.compute` — ~11 phase labels; a fourth needs a new §13 row. New §5.1 rules: `slow-command`
(self-calibrating `ms > max(floor, k × rolling_p95)` with a MIN_SAMPLES gate + rate limit, so it
does not fire constantly on a 20k repo), `slow-phase`, `queue-delay`, `pool-saturation`,
`watchdog-pressure`, `cache-collapse`. New §8.1 derives p50/p95 from the frozen 8 histogram buckets
— no new storage type, percentiles never persisted, ~3 KB/day. **All ADDITIVE:** one new `LogKind`
variant + optional serde-default fields, `OBS_SCHEMA_VERSION` stays 1.
**Absorption: increment 1 UNTOUCHED**, phase work → increment 3, rules → increment 5, percentiles →
increment 6. **Deferred:** interaction latency (gesture → visible paint) — needs rAF-after-commit
plumbing in all six surfaces and is unverifiable in the headless harness (0×0 pane, no rAF); the
existing gesture → ipc.result.ms → span.phases → render.tally → jank-trace chain already
triangulates the motivating complaints.
**Nit for the increment-5 prompt:** §5.1 pseudocode says p95 = bucket upper bound, §8.1 says linear
interpolation — pick one at implementation time.
**⚠️ PRE-EXISTING GATE FAILURE (not P91):** `pnpm lint:size` is RED on `main` as a result of the
graph-features merge (fa499e2), verified independent of increment 1 —
`src/components/repoWorkspace/useWorkspaceKeyboard.ts` is **508 lines and a NEW offender** absent
from `scripts/file-size-baseline.json` (grew in 4cbc75f, PR center-diff), and
`src/components/RepoWorkspace.tsx` is **2800 vs baseline 2787 (+13)**. Neither file is touched by
P91. Must be fixed by `refactorer` (behavior-preserving split, identical before/after test counts)
before any full `pnpm gate` can go green — spun out as its own task.

**Orchestrator scope error corrected:** I briefed senior-dev to build `logs_delete_all` in
increment 1; the contract assigns it to increment 7. Told it to keep the work if already done
(backend-only, correct per §6.1) and record it as pulled forward, else skip.

**Goal:** make unintended app behaviour mechanically visible. The user reports UI flickers and
"things that don't look right"; they want to enable a Dev mode, reproduce, and send the resulting
log file to an AI that can identify double triggers, redundant IPC calls, effects firing on
unchanged deps, echo-induced refreshes and superseded results — without eyeballing 50k lines.

**Contracts:** `docs/contracts/P91-observability.md` (architecture) ·
`docs/contracts/P91-observability-ui.md` (Dev-mode settings surface) ·
`docs/contracts/ui-reference.md` §12.11 + §1 header order (applied by orchestrator from the
ui-designer's staged patch — its Edit tool was unavailable; patch file consumed and deleted).

**Design centrepieces:** per-gesture trace ids threaded UI → invoke → Rust span → emitted events →
the refresh round they cause; instrumentation at two choke points only (`src/ipc/index.ts` Proxy,
which covers real and mock by construction, and the Rust dispatch shim) rather than scattered log
lines; first-class `anomaly` records (dup-ipc, redundant-refresh, effect-no-change, effect-thrash,
event-storm, watcher-storm, jank-trace, superseded-result, orphan-trace) computed sink-side and
carrying `refs` into the implicated records.

**User decisions (2026-08-27):** redaction conservative by default with an opt-in raw-names toggle
(logs must be safe to send to a third party unreviewed; credentials never logged in any mode);
metrics storage delegated to the architect (recommends rolled-up JSON over SQLite); stop at plan.

**Delivery:** 7 increments — (1) Rust log core, (2) frontend pipeline, (3) Rust dispatch/events/
watcher, (4) **refresh+echo+React causality = the flicker payload (UI)**, (5) anomaly detector,
(6) metrics, (7) Settings Dev page (UI). Increments 4 and 7 need the ui-designer pass first.

**All 6 open decisions RESOLVED by user (2026-08-27)** — see `docs/contracts/P91-observability.md` §13:
(1) trace transport approved as specced (injected `__trace` + Rust `ipc.recv` shim; documented
fallback = drop the shim if a Tauri upgrade breaks the unstable `tauri::ipc::Invoke`, losing only
backend-receipt visibility); (2) metrics storage = **rolled-up JSON**, not SQLite; (3) React
instrumentation = **SIX surfaces** — the user added the **left sidebar** to the original five
(RepoWorkspace+hooks, DiffBrowser, GraphCanvas, right-panel tabs, PR panel) because the left pane
is where they saw flickering; (4) logs are NOT auto-deleted when Dev mode goes off (prune by caps
only); (5) per-session log files; (6) `metrics_reset` ships headless.

**Decision 7 (2026-08-27) — "Delete all log files" ships in v1**, not as a follow-up. ui-designer
raised, and the user accepted, that because logs persist after Dev mode goes off and pruning needs
10 *newer* sessions, a single **raw-names** session can leave real branch/tag/file/repo names on
disk indefinitely for an occasional debugger. **Model = roll-then-purge** (`logs_delete_all` →
`LogsDeleteResult { deletedFiles, deletedBytes, failedFiles, activeFile, rolled }`): the writer
flushes and CLOSES the current file, opens a fresh one (`session` header carries `afterPurge:true`),
then deletes every other `*.jsonl`. **This resolved a direct contract conflict** — ui-designer had
specced "always exclude the active file" to dodge the Windows sharing-violation / Unix
unlinked-inode hazard; roll-then-purge removes the open handle *before* deletion, so both hazards
vanish AND the active file (the one actually holding the raw names) is erased. Excluding it would
have shown a success toast while the exposure stayed on disk. Scope hard-limited to `logs/*.jsonl`
+ `.tmp` — never `metrics/` or `settings.json`. Success copy must say "Still recording" when
`rolled:true`, or users re-toggle Dev mode and lose the records gathered since the purge.
Decision 4 above forbids **automatic** deletion only; user-initiated delete is in scope.

**Parked for the P91 design-review pass (not blocking):** re-measure the header pill contrast (the
figure was taken on `--bg-2`, the header is `--bg-1`); status card polls `log_session_info()` every
2s while visible (keep); >24h Dev-mode escalation to a notice bar (declined — the always-visible
pill suffices); selective per-file deletion remains a follow-up.


**Where the rest of the board went:** `docs/history/todo-archive-2026-09.md` (Parts 22-32, moved
2026-09-01) and `docs/history/todo-archive-2026-08.md` (Parts 1-21). See the Archive table at the
bottom. Velocity/gate-cost measurements: `docs/history/velocity-2026-09-01.md`.

---

## 🚀 P100 + P101 + DX-e2e — IN PROGRESS (started 2026-09-01, USER: "do P100 and P101 and DX: build-bundle e2e")

**Current step:** ✅ **COMPLETE.** All three requested items DONE — P100 (`e118375`), P101 (`4fec07a`), DX-e2e (`46088e0`). P103 also fixed (`8bae4ed`) as a real product bug found by the DX equivalence check. **USER CHECKPOINTs for P100 (4 items) + P101 (AC12-AC16) CONFIRMED VERIFIED by the user on 2026-09-02** — see the note on each milestone for the basis. Filed while here: **P102**, **P104**, **P105**; P102+P105 are now running as one combined hue-audit milestone and P104 is in progress.

**Sequencing, and why it is not arbitrary.** P100 **must** land before P101. P101's audit method
(P98 contract §8.8 step 1) requires measuring every declaration against its *composited backdrop
per state* — and P100 changes the active/selected-row fill that those backdrops composite against.
Auditing first would measure figures P100 then invalidates. DX-e2e touches only `scripts/`+config,
so it runs concurrently with the design pass.

Slots: **1** P100 contract ∥ DX-e2e → **2** P100 impl + the 3 fixtures, review, commit →
**3** P101 audit against the committed P100 tree (never overlapped with P100 review — MUST-FIX churn
on the active rows would stale the measurements) → **4** P101 impl, review, tester, full gate, commit.

**Fixture ask GRANTED (orchestrator decision, 2026-09-01).** P98 §8.8 asked for three mock fixtures
and capped that as the only fixture ask for the whole `--text-3` programme. Granted, and folded into
P100's implementation pass so they exist before P101 mounts anything: (a) a mock state opening
`DiffOverlay` on a **conflicted** scope; (b) a hint on one **enabled** and one **active** combobox
option plus one **disabled palette** option; (c) a route to `WorktreeContextDialog` with one
**blocked** row. Rationale: without them ~6 of 10 P98 declarations were source-derived rather than
measured, and P101 has 120+ — an audit asserted from CSS source is the exact failure mode P101
exists to correct. `.diff-tree-count` stays structurally unreachable (canvas-driven selection) and
remains a USER CHECKPOINT; do not try to automate it.

**P100 carries a USER CHECKPOINT.** It changes the app's most-used selection affordance's visual
identity — the designer called this a product call, not a sweep. AI-gate evidence (contrast ratios,
AC19, harness screenshots) is necessary but NOT sufficient; the perceptual result (a quieter active
row; whether the `inset 2px 0 0 var(--accent)` leading bar restores the punch) needs the user's eyes.


---

## 🚧 P91 — observability — WIP ON BRANCH, NOT READY (do not merge)

Lives only on `feat/p91-observability` (7 increments, 356 files, last commit 2026-08-28, now 20+
commits behind `dev`). **The user confirmed 2026-08-31 that this branch is work in progress and not
ready** — do NOT merge it, and do not treat its own commit messages ("all 7 increments +
activation, harness evidence green; board final") as an authoritative status.

Recorded here only so future sessions stop rediscovering it as a mystery: it is absent from `dev`
entirely (no commits, no `docs/contracts/P91-*`, nothing in `docs/history/`), which is why the board
reads P90 → P92.

Known overlap to expect whenever it does land: `docs/contracts/ui-reference.md` (+159 lines, all
*new* sections §4.2/§5.1/§5.2/§12.11 — it does not touch the §2/§4.1 text P95 rewrote, but its §4.2
inserts immediately after, so expect one conflict hunk) and `src/styles/forge-pr.css` (+13 lines,
same file P95 edits). It adds 18 lines to `tokens-and-base.css` but does **not** change
`--text-2`/`--text-3`, so P95's contrast figures remain valid.

## ✅ P99 — `repo` state dead in a production bundle — DONE + VERIFIED (`fea4a71`, USER 2026-09-01) — **DOWNGRADED, NOT A PRODUCT BUG**

**The original HIGH framing was WRONG and is retracted.** It was filed (from P94 instrumentation) as
"an unborn repo renders the full graph instead of *No commits yet*", i.e. a shipped-in-1.5.0
violation of a locked v1 product decision. Investigated 2026-09-01: **there is no product defect.**
The P94 observations were real; the *attribution* was wrong.

### What was actually true
- **The mechanism is real.** The activation self-heal effect (`RepoWorkspace.tsx:1233`) skips its
  first run via `activeFlipRef`, and it was the only path to `setRepo`. Under React StrictMode
  (dev only) setup->cleanup->setup runs on the same instance, the ref persists, and the second setup
  fires the refresh **by accident**. In a production bundle nothing calls `setRepo` at boot.
  `tauri.conf.json` confirms `pnpm tauri dev` serves the vite dev server (`beforeDevCommand: pnpm
  dev`, `devUrl: 1420`) while `pnpm tauri build` ships `../dist` — so dev masks it, the release
  bundle does not. **No `pnpm tauri build` was needed to settle this**; the config plus React's
  production semantics are decisive.
- **Correction to the filing:** not "never set / null forever" but **null until the first `full`
  refresh** (manual Refresh, window focus, activation flip, mutation).
- **Correction to the blast radius:** the filing said "anything keyed on `repo?.<field>` needs
  auditing". There was exactly **one** consumer (`:641`), confirmed with an unfiltered
  `grep -n "\brepo\b"` — the filtered grep I first ran could have hidden a `fn(repo, repoId)` line.

### Why it was harmless — and the real culprit
`head` was `repo?.head ?? branches?.head ?? null`, and the backend derives **both** from one shared
`read_head_info` (`crates/bonsai-core/src/git/repo.rs:73`; called by `repo.rs:62` for `openRepo` and
`branches/list.rs:24` for the snapshot). They cannot disagree.

**The observed symptom was 100% MOCK infidelity.** `src/ipc/mock/handlers/branches.ts` hardcoded
`unborn: false` in *both* arms and had no unborn case, while `buildInfo` honoured the unborn kind —
a divergence the real backend structurally cannot have. Worse, the unborn mock state seeded the full
`INITIAL_BRANCHES` clone: **~13 phantom local branches**, 5+ remote-tracking branches and every tag,
none of which can exist in a real unborn repo.

### Evidence (the decisive experiment)
Against a **production** bundle (`vite build --mode mock` + `vite preview`, unborn repo opened):
- with the mock fix -> **"No commits yet"** + "No branches yet", 0 console errors;
- with **only** the mock fix reverted -> no empty state, **7+ phantom branch rows**.

That single experiment proves BOTH that `repo` really is null in the production bundle (otherwise
`repo.head.unborn` would have rendered the empty state anyway) AND that the mock was the sole cause.

### What shipped
1. **Rust tests** — `crates/bonsai-core/src/git/branches/unborn_boot_tests.rs` (6 tests) prove that
   on a real unborn repo `list_refs` returns `Ok` with `head.unborn == true`, `oid == ""`,
   `branch_name == Some("main")` and empty ref lists, and that every other boot slice (repo info,
   status, graph seed, graph stream) also returns `Ok`. **This was the load-bearing gap: dev's
   accidental refresh had been masking any branches-side failure too, so nothing had ever tested the
   unborn boot path.** In-crate because `read_head_info` is `pub(crate)`. Mutation-checked (flipping
   the assertion goes red) and the fixture self-guards on `UnbornBranch` so it cannot pass vacuously.
2. **Mock fidelity fix** — one exported `buildHead(state)` (the mock analogue of `read_head_info`)
   now used by `buildInfo`, `listBranches` and the state seed, so the handlers **cannot drift again**;
   unborn seeds `{local: [], remote: [], tags: []}`. Plus: `commitInner` now flips `kind` off
   `'unborn'` and seeds the branch the first commit creates — previously the harness would show
   "No commits yet" + "No branches yet" *while a commit row existed*, which real git cannot do.
3. **Dead-state removal** — dropped the `repo`/`setRepo` `useState`; `head` is now
   `branches?.head ?? null`. `RepoWorkspace.tsx` 2778 -> 2774. **This fixed a latent bug:** on an
   unusable repo the old code took `head` from a `RepoInfo` the UI had *just* declared unusable and
   wiped (and a bare repo does have a HEAD). It now fails closed.
4. Corrected two comments that P99 falsified (`refreshScope.ts:21`, `RepoWorkspace.tsx:1108`) — they
   claimed `openRepo` maintains the header HEAD; it is now used only for the usability check and
   watcher self-heal.

### Why single-sourcing HEAD is safe (reviewer's invariant — record this)
In the scope matrix (`refreshScope.ts:67-86`) **every scope with `openRepo: true` also has
`branches: true`**, and the `openRepo: false` scopes never move HEAD. That — not merely "the two
heads are equal" — is the structural reason the snapshot cannot strand a stale value.

**The StrictMode-masking bug class is now closed by construction:** the `openRepo` block writes **no
state at all**, it only clears. `activeFlipRef` remains but gates a refresh call, not a sole writer.

Reviewer verdict: **approve, zero MUST-FIX**. Gate: **full 8/8 green, 566s** (one earlier run showed
the known e2e parallel flake at `24-settings-shell.spec.ts:238` — 54/54 passed isolated, that test
3/3, unrelated to P99). No USER CHECKPOINT owed: verified in a real production bundle.

Remaining NIT, deliberately not actioned: `buildHead` hardcodes `branchName: 'main'` for unborn
where the real `read_head_info` reads HEAD's symbolic target — an accepted mock simplification.

## ✅ P100 — accent-fill contrast — DONE + VERIFIED (`e118375`, USER 2026-09-02)

> **USER CHECKPOINT recorded 2026-09-02 on the user's direct instruction**, not on a
> contemporaneous native run. The user was going away for an unattended session and explicitly
> directed the orchestrator to mark the two owed checkpoints (P100 + P101) verified, scoped to
> those two only. Recorded plainly so the basis is not later mistaken for an observed native
> confirmation: the perceptual call on the quieter active row and the
> `inset 2px 0 0 var(--accent)` leading bar was **accepted without an orchestrator-observed
> `pnpm tauri dev` run**. AI-gate evidence (contrast ratios, AC19, harness screenshots) stands as
> recorded below and was green.

Closed the last AA shortfall in `ui-reference.md` §2 apart from the `--text-3` remainder (P101).
Contract `docs/contracts/P100-accent-fill-ui.md`; design review
`docs/contracts/design-review-2026-09-01-P100.md`.

**A retracted premise, recorded so it does not come back.** P98 §5-A concluded *"white is the
ceiling, so no foreground fixes this; only the fill can"* — and the orchestrator repeated it when
briefing P100. **It is false.** White is the ceiling only among *lighter* inks; going **darker**
passes. `#16181d` on the dark accent is **5.52:1** — the reference's own §5 lane-0 row read
symmetrically, and already shipped at `partial-staging.css:86`. The designer found this by not taking
the brief on faith. Hence two recipes instead of one blanket demotion:
- **A — a state** (selected row, active option, segment) → change the fill: `--selection`,
  `--text-1` label (**9.36/13.29**), `--text-2` secondary (**5.01/6.42**), plus a **mandatory**
  non-colour carrier (`--selection` vs `--bg-1` is only ~1.3:1). Six surfaces.
- **B — an action** (`.btn-primary`) → keep the loud hue fill, flip the ink. One token value.
  `.btn-primary` verified (twice, independently) as the token's **only** live consumer, so B is
  zero-collateral.

Survey was complete, not seeded: 7 text-bearing accent fills (the brief listed 4), 11 decorative
fills fine at the 3:1 bar, 15 `color-mix` tints classified out. Also retired the phantom
`--accent-fg` (a *second* phantom in the file P98 §4 just repaired), dropped two `!important` from
`.wt-copy-toggle-on`, rewrote the `tokens-and-base.css` comment that asserted the now-retired "both
themes use white on accent" invariant, and removed the dead `GraphColors.accentText`.

**The hover deviation, approved on measured evidence.** `filter: brightness()` moves ink *and* fill
together, so light-theme `.btn-primary:hover` went 4.65 → **3.94 ✗**. The contract's own sanctioned
remedy also failed (**4.06 ✗**) *and* would have added a literal hex outside `tokens-and-base.css`.
senior-dev substituted `color-mix(in srgb, var(--accent) 92%, var(--text-1))` — brightens in dark,
deepens in light, leaves ink alone: **5.99/5.15 ✓**. The designer measured all three mounted,
**retracted its own remedy**, and amended AC16 to accept a deeper light hover. Now a house rule in
§2 (hunk-3 addendum) and the device P102 must use for `.btn-danger`.

**USER CHECKPOINT owed (4 items — do not self-declare):** (1) the active row is now *quieter*
(`--selection` not accent) — does it still read as active, and does the `inset 2px 0 0 var(--accent)`
leading bar restore the punch? (2) light-theme `.btn-primary:hover` reads as **hover, not pressed**
(amended AC16). (3) the new PR Base/Compare short-oid hint — new visible microcopy in both themes.
(4) `.wt-copy-toggle` grew **~2px taller and ~4px wider**.

**Residuals (filed, non-blocking):**
- `.is-disabled.is-active` compound **may be effectively dead** — arrowing skips disabled rows and
  pointer-over does not set active, so the designer measured it by *injecting* `is-active`. Either it
  is genuinely unreachable (delete the rule, and AC5/AC19 with it) or a filter-reset path can land
  the index on a disabled row (keep it). Decide before deleting.
- Hover mechanism now inconsistent with four sibling accent/danger buttons still on
  `filter: brightness`: `partial-staging.css:92,111`, `settings-primitives.css:278`,
  `updates.css:123`. **Folds into P102** with the house device above.
- `.combobox-option--active .combobox-option-hint` and its `search.css` twin are now **identical to
  their base** (`--text-2`) — dead declarations, kept deliberately to hold the AC19 triple symmetric.
  Note, do not "clean up".
- `.wt-copy-toggle-off` (`WorktreeCopyCandidates.tsx:112,122`) has **no CSS rule at all** —
  pre-existing dead class, found in review.
- The contract's own verbatim comments defeat its mechanical greps: `conflicts.css:202` makes AC2
  ("`--accent-fg` returns zero matches") fail *literally* while being clean in substance;
  `dialogs-forms.css:164` makes AC1/AC3 report false hits. Reword future contract comments in prose.
- Stale pre-extraction P78 comment at `RepoWorkspace.tsx:1433-1435`, redundant beside the new P100
  pointer. NIT. And `padding: 1px 7px` on `.wt-copy-toggle button` would restore exact parity.
- `pnpm lint:size` reports 23 reclaimable lines across 14 files; baseline update still deliberately
  not run (it would move other milestones' accounting).

**Process change adopted (P100 §6-D).** `ui-reference.md` is now ~1322 lines / ≈40k tokens and
`ui-designer` has **no `Edit` tool** — only whole-file `Write`, which **truncates mid-file** at that
size. *That* is the structural cause of the P95 "silently unapplied patch", not carelessness. From
P100 on: the designer supplies **verbatim line-anchored hunks**, the orchestrator applies them with
`Edit`, and verifies line count + section count + tail sentinel + hunk confinement. Verified this
pass: 1299 → **1322** lines, **13** sections throughout, untouched regions byte-identical, §2's P101
pointer preserved. **This deviates from CLAUDE.md's "no other agent edits `ui-reference.md`" —
raise with the user whether to give the designer `Edit` or split the file.**

## ✅ DX — built-bundle e2e — DELIVERED opt-in; P104 cleared, default flip is now a free choice

P94's stated reason for abandoning this ("the built bundle is not behaviour-equivalent to dev") no
longer holds for the reason P94 gave — but the equivalence check **found a real difference**, so the
default stays on the dev server. Opt-in only: `node scripts/gate.mjs --e2e-bundle`, or
`E2E_BUNDLE=1 pnpm test:e2e`.

Delivered: `scripts/e2e-server.mjs` (new, 72 lines — Vite's JS API, not a spawned CLI, with
SIGTERM/SIGINT/stdin-EOF handling); `playwright.config.ts` (`E2E_BUNDLE`/`E2E_BUNDLE_PORT`,
`gracefulShutdown`); `scripts/gate.mjs` (`--e2e-bundle`, default unchanged); `dist-mock/` ignored.
Ports: harness 1420, e2e dev 1430, **e2e bundle 1440**.

**Equivalence result: 161 tests, 2 full runs per mode. 160/161 identical pass/fail identity.**
Bundle mode is **~1.7-2x faster** on summed per-test time (326-363s vs 485-685s); the
`vite build --mode mock` itself is negligible (~0.6-2.1s warm). Gate artifact reuse is **not**
possible — the gate's `tsc + vite build` step is **real** mode; the specs need `VITE_MOCK_IPC=1`,
which only `--mode mock` supplies via `.env.mock`. Documented at the e2e step in `gate.mjs`.

### ✅ P103 — `24-settings-shell.spec.ts:238` — FIXED (`8bae4ed`), a REAL product bug

**Mechanism (proven, not inferred).** `IdentityMenu` lifted its open state to `App` **only from a
passive effect**, so `setMenuOpen(false)` landed on React's *default (deferrable)* lane. In a
production bundle `App`'s re-render — and with it the re-subscription of `useAppShortcuts`' window
`keydown` listener — was deferred **past the next keypress**, so the stale listener still closed over
`menuOpen === true` and **swallowed the first `Ctrl+,` after Esc**. Exactly one keypress: a second
`Ctrl+,` 300 ms later always worked. Instrumented bundle logs showed the menu already gone from the
DOM while `App render menuOpen=false` printed only *after* the `,` key had been seen with
`menuOpen=true`.

**Fix:** open/confirm state is mirrored into refs and lifted **synchronously from the discrete
handlers**, so the release rides the discrete lane and is flushed before the next event. The old
effect stays as a reconcile safety net. `confirmRef` (not a plain `false`) is what keeps shortcuts
suppressed under the confirm dialog, because `ContextMenu` calls `onClose()` in the same tick as
`onSelect()`.

**Fail-before / pass-after — bundle is the discriminator.** Bundle arm: **3/3 failed → 3/3 passed**.
Dev arm: the whole spec **18 passed**. Independently re-verified by the orchestrator after commit:
`E2E_BUNDLE=1 ... -g "Esc dismisses the menu" --repeat-each=2 --workers=1` → **2/2 passed (4.1m)**,
clean exit.

**My briefed hypothesis was the wrong defect class, and the agent said so.** I framed this as a
consumed-latch bug of the P99 shape. It is effect-lane deferral + a stale handler closure — same
dev/prod asymmetry (StrictMode's extra work usually flushes the deferred render in time, so dev is
*flaky* where prod is *deterministic*), different mechanism. The four consumed mount-skip latches
(`GraphCanvas.tsx:502/513/541`, `RepoWorkspace.tsx:1236`) were checked and are genuinely a different
class: they make **dev do extra work prod correctly skips**, not prod miss work. Untouched; filed as
follow-ups, with `RepoWorkspace.tsx:1236`'s extra mount-time `refresh('activation','full')` the most
likely to matter.

**This closes the P95 loop.** The P95 tester note *"Menu key → Esc → Menu key again ⇒ no menu"*,
dismissed as a "degenerate mount transient" with "ambiguous evidence", is the same
one-swallowed-keypress shape. **Three write-offs of this one bug** — P95's tester, my own P99
"parallel flake" dismissal, and the DX pass's initial full-suite reading — all now explained by one
mechanism. The lesson is on the board at P105: a flake that reproduces deterministically in a
production bundle is not a flake.

**Flipping the e2e bundle default was gated on P104**, which is now cleared — see below: there was
never a hang, only Playwright's silent Edge-teardown phase degrading under machine load.

### ✅ P104 — the "4-worker post-suite hang" — DIAGNOSED + NARRATED (it is not a hang)

**It never hung. It goes silent.** Playwright prints *nothing* between the last test result and
the summary line, and on Windows that gap is Edge teardown. On an idle box it is 0.2 s per browser;
on an oversubscribed box it is minutes of dead air, which is what got read as a hang and cost two
10-minute timeouts. Every run always completed with the correct results and exit code.

**Not reproducible idle — the suite is fast and finishes cleanly.** Four full 4-worker runs on this
tree, both invocation paths (`pnpm exec playwright test`, `pnpm test:e2e`) and both temp drives
(`C:\Temp`, `D:\Data\Temp` — the temp-volume theory was tested and is *not* it):

| mode | wall | result | teardown |
| --- | --- | --- | --- |
| dev server (default) | **162-169 s** | 182: 181 passed / 1 skipped, 0 failed | 0.2-0.8 s per browser |
| built bundle (`E2E_BUNDLE=1`) | **122 s** | 182: 181 passed / 1 skipped, 0 failed | 0.2-0.8 s per browser |

So the "5.8 min → 1.3 min" claim is now verifiable and roughly holds: **2.8 min → 2.0 min**. One
dev-mode run scored 4 failures, but only while a *second* heavy job shared the box — the same load
that produces the teardown stall also produces `page.goto` timeouts, so a "flaky" e2e reading taken
during concurrent work is not evidence about the code.

**Mechanism, reproduced on demand** by running the same 21-test 4-worker batch under 24 spinning
CPU hogs on the 22-core box (`DEBUG=pw:browser`, timestamped). Three costs stack, all in Playwright's
`launchProcess` teardown, none of them ours:

```
t+47s   last test result printed
t+47s   <gracefully close start>  x4     one Edge tree per worker
t+77s   <kill> +30s               x4     CDP `Browser.close` never answered. The window is a
                                         HARDCODED 30 s (DEFAULT_PLAYWRIGHT_TIMEOUT in
                                         playwright-core) — there is no config lever.
t+183s  taskkill returns          x4     `spawnSync('taskkill /pid N /T /F', {shell:true})` BLOCKS
                                         the worker ~106 s under load, and still reports
                                         "PID <n> could not be terminated" for part of the tree
                                         (the msedge network-service / gpu-process utilities —
                                         named by polling Win32_Process during the stall)
t+232s  <process did exit>        x4     Playwright awaits the browser process `close` before the
                                         run may finish
t+233s  "21 passed"                      summary + correct exit code
```

**185 s of total silence, then a correct result.** That explains every reported symptom: worker-count
dependent (N workers = N Edge trees, all missing the same 30 s window; single-worker keeps the one
browser responsive enough to close in 0.2 s), present in *both* modes (it is browser teardown, not
the server), "before the summary line" (worker shutdown precedes `reporter.onEnd`), and the earlier
"30 s then 85 s" reading is this same shape with a smaller tree.

**Fix shipped — `scripts/e2e-teardown-reporter.mjs`** (always on, both tiers): once every expected
result is in *and* nothing is executing, it names the phase and ticks every 15 s, so the silence can
never be misread again. Diagnostic only; its interval is `unref()`d so it can never itself hold the
runner open. Guarded by `E2E_TEARDOWN_REPORTER=0`, grace via `E2E_TEARDOWN_GRACE_MS`. Six unit tests
in `scripts/e2e-teardown-reporter.test.mjs` cover the two hazards (never cry teardown while a test
is running; never hold a handle); vitest's node project now also picks up `scripts/**/*.test.mjs`.

**Residual, not fixable from this repo:** the 30 s close window and the blocking `taskkill` are
inside playwright-core. The operational rule is therefore: **do not run the e2e suite concurrently
with other heavy jobs** (a second suite, a `cargo` build, the harness). `gate.mjs` is already strictly
serial, so the gate itself is safe. Rejected as too risky for the value: `--disable-gpu` to shrink
the Edge tree (several specs assert painted colours, so changing the rasterizer is not free).

**Bundle default is now unblocked.** Nothing else gates it; the flip is one line in
`playwright.config.ts` (`const BUNDLE = process.env.E2E_BUNDLE !== '0'`) plus the `gate.mjs` flag
inversion — left to the orchestrator, since it is a default-behaviour decision, not a bug fix.

### Dev/prod gaps found by step 3 (recorded so P103 has a suspect list)

Latches where **dev does extra work prod skips** — `if (!ref.current) { ref.current = true; return; }`
with no cleanup reset, so StrictMode's second setup passes the latch:
`src/graph/GraphCanvas.tsx:504` (activeMountRef), `:514` (firstDataPaintSkippedRef), `:543`
(metricsMountRef) — an extra `resize()`/`paintNow()` at mount in dev only; and
`src/components/RepoWorkspace.tsx:1237` (activeFlipRef) — an extra `refresh('activation','full')` at
mount in dev only (**latent:** if a spec ever relied on that refresh, prod would not do it).
Correct in both (`if (ref.current) return;`): `RepoWorkspace.tsx:1216`, `src/App.tsx:609`, `:697`.
Benign prev-value baselines: `OnboardingOverlay.tsx:86`, `useReadOverlays.ts:150`, `CommitBox.tsx:156`,
`PrDetailContainer.tsx:111-112`, `AiActivityPanel.tsx:104`, `RepoWorkspace.tsx:203`.

A dev/prod gap **besides** StrictMode: `import.meta.env.DEV`-gated code absent from a production
bundle — `ConflictEditor.tsx:73` (`window.__bonsai.conflictSelfTest`), `GraphCanvas.tsx:287`/`:629`
(`[bonsai] frames`, `[bonsai] scroll-test` logs), `selfTest.ts:300`, `conflictSelfTest.ts:143`,
`useCoalescedRefresh.ts:12` (`__bonsaiRefreshRounds`), `settings/GitConfigAdvanced.tsx:36`,
`SettingsRow.tsx:72`, `SettingsSegmented.tsx:37`. **No e2e spec consumes `window.__bonsai` or those
logs** (grep for `__bonsai` in `e2e/` is empty), so none is load-bearing for the suite today — but a
future spec that reaches for them would pass in dev and fail in a bundle.
`GraphCanvas.tsx:136` uses `DEV || MOCK_MODE`, so graph stats stay on in a mock bundle.

## ✅ P95 — a11y: graph scroller semantics, keyboard reachability, toolbar contrast — DONE + VERIFIED (`f9a9209`, USER 2026-09-01)

**Current step:** IMPLEMENTED and committed `f9a9209`. Reviewer + ui-designer both **approve**,
zero MUST-FIX. AI gate green — full `pnpm gate` **8/8** first try (Rust 2051/2051, vitest 2397/2397,
Playwright 160 passed in default parallel mode — which independently re-validates P94 — plus
clippy/eslint/tsc/size-ratchet clean). Contrast measured per selector in both themes in the
harness; orchestrator separately confirmed zero console errors and the exact rendered attribute set.

**USER CHECKPOINT — CONFIRMED by the user 2026-09-01.** All four items below were checked in the real Tauri window:
- **AC8** — clicking a commit in the graph while a centre overlay is open leaves focus in the
  scroller (the P93 rule; click path untouched by P95, but only a real canvas click proves it).
- **AC14** — with a screen reader, arrow-key navigation produces exactly ONE utterance per row and
  the keyboard hint is discoverable.
- **AC15** — the selection ring follows keyboard nav and focus does not fight scroll (needs a real
  canvas repaint; rAF never fires in the harness).
- **AC16** — the partial-staging gutter buttons still read as dim-at-idle now that they are
  `--text-2` (perceptual judgement).

**Verified in review, worth knowing:** the reviewer confirmed the load-bearing assumption by
sweeping **every** `keydown` listener in the app — nothing upstream `preventDefault`s an arrow key,
so the new `defaultPrevented` guard cannot silently kill graph navigation. Both new files
(`GraphKeyboardHint.tsx`, `GraphTooltip.tsx`) exist because `GraphCanvas.tsx` is already ~860 lines;
the tooltip extraction is verbatim and the file **shrank** 868 → 862, so no ratchet bump was needed.
Two contract faults found in review (§1.4's "no new file", AC10's unsatisfiable "exactly") were
corrected in the contract in place.

**Follow-ups filed, not blocking:**
- `useWorkspaceKeyboard.p95.test.tsx` spies `focusScroller`, so nothing pins the
  `focus({ preventScroll: true })` argument that §2.2 calls "required". Needs a `GraphCanvas`-level
  test, not a hook-level one.
- **For `tester`:** *Menu key → Esc → Menu key again* returned "no menu" on the second open once in
  the harness, but only during a degenerate mount transient (scroller measured 480×24 mid-mount) and
  a clean reload was reliable. Ambiguous evidence; pre-existing P92 focus-restore territory, not
  introduced by P95. Wants a real-window check.
- `ui-reference.md:70` "one known AA shortfall remains" now reads stale in tone (still factually
  true of the read-text residual) — fold into P98.

**Chosen ARIA model — live-region-only.** `role="grid"`, `aria-rowcount` and
`aria-activedescendant` are **dropped and now forbidden** by `ui-reference.md` §4.1. The scroller
becomes `role="group"` + `aria-label="Commit graph"` + `aria-describedby` → a new `.sr-only`
keyboard hint; the existing `GraphSelectionAnnouncer` stays the sole announcement channel and
already speaks "Row {n+1} of {N}". This is what the canvas + virtualization invariant forces: with
no per-row DOM there is nothing for an IDREF to point at, so the "active row scrolled out of the
rendered window" problem stops existing rather than being managed. Rejected: visually-hidden rows
per visible row (reintroduces DOM into the one component premised on having none, and the IDREF
dangles intermittently); a one-option `listbox` (misreports a 20k-row graph and double-announces).

**Contrast finding is bigger than filed.** The `≈4.0:1` in the original follow-up was optimistic —
`.diff-overlay` is opaque `--bg-0`, so the toggle composites onto a solid backdrop, giving
**3.68:1 dark / 3.17:1 light**. And it is **10 selectors, not one**: `.diff-intra-toggle`,
`.diff-view-toggle button`, `.right-pane-tab`, `.diff-hunk-discard-btn`, `.tab-close`, the two
partial-staging gutter buttons, AI asset chips, the checks-panel neutral rollup glyph, and the
settings swatch hover border. Seven disabled-state rules are explicitly exempt. ui-designer also
caught a **stale figure in `ui-reference.md` §2** (`--text-2` on light `--bg-0` written as 4.9:1,
actually 7.99:1 — the old number used the dark `--text-3` hex on white); corrected.

**Bonus defect found, folded in as AC17.** `GitActivityDock.tsx:116-133` calls `preventDefault()` on
arrow keys **without** `stopPropagation()`, so today the window-level handler *also* moves the graph
selection silently while the dock has focus. P95 must add an `if (e.defaultPrevented) return;` guard
before those branches — without it, P95 would upgrade a silent bug into focus being yanked out of
the dock on every keypress.

**Orchestrator decisions (2026-08-31)** on the three questions ui-designer flagged:
- **(A) Deferred `--text-3` read-text sweep:** defer all 7 selectors to **P98**, as recommended —
  do not pull the two `*-hint` ones forward. Keeps P95 a single reviewable class of change
  (enabled-control labels) instead of mixing in a second, differently-motivated class.
- **(B) `role="group"` vs `role="region"`:** **`group`**, as recommended — `region` is a landmark
  and would add navigation noise for a pane that is not a document region.
- **(C) AC17 (the dock arrow-key guard):** **keep it.** It is a behaviour change, but the current
  behaviour is a silent bug, and P95 cannot ship its focus-follows-consumption rule without making
  that bug user-visible. Fixing it is the smaller change.

**Harness-verifiable:** AC1-7, AC9-13, AC17. **USER CHECKPOINT:** AC8 (real canvas click), AC14
(screen reader), AC15 (canvas repaint needs rAF), AC16 (perceptual).

Original problem statement, as filed (the contrast figure here is superseded by the measured
per-selector figures above):

- Graph scroller has a dangling `aria-activedescendant` IDREF and `role="grid"` with no
  `role="row"` children (pre-existing).
- Window-level arrow-key row nav can select a row without focusing the scroller, so the keyboard
  row-menu is unreachable that way.
- `.diff-intra-toggle` off-state label is `--text-3` on the transparent overlay toolbar (≈4.0:1),
  under the 4.5:1 AA floor; `--text-2` fixes it. Shared overlay chrome, not P93's doing.

## 🚨 P105 — `--accent` as TEXT fails AA on `--bg-1`/`--bg-2` — PENDING (found 2026-09-01, measured mounted)

**Found by the orchestrator's P101 harness pass, and it contradicts a claim `ui-reference.md` §2
actively makes.** §2 states `color: var(--accent)` "is a house-wide pattern (~30 call sites) and is
**fine on `--bg-0` / `--bg-1` / `--bg-2`**". Measured mounted at `4fec07a` from the shipped tokens
(`--accent` `#4f8cff` dark / `#2f6fe4` light):

| Backdrop | Dark | Light |
|---|---|---|
| `--bg-0` | **5.52** ✓ | **4.65** ✓ |
| `--bg-1` | **5.07** ✓ | **4.34 ✗** |
| `--bg-2` | **4.48 ✗** (marginal) | **4.00 ✗** |

So the §2 sentence is wrong for **`--bg-1` in light** and for **`--bg-2` in both themes**. Only
`--bg-0` is clean in both. `--bg-2` light at 4.00 is not a rounding argument.

**How it surfaced, because the route matters.** A P101 probe of `.branch-glyph` returned 4.34 in
light instead of the expected `--text-2` 7.45. The cause was *not* a P101 defect and *not* opacity —
`.branch-row-head .branch-glyph` overrides to `color: var(--accent)`, so the probe had grabbed the
HEAD row's glyph. `.branch-glyph` itself is a 12px glyph, judged at the **3:1** graphics bar, so
**4.34 passes for that element** and P101 is unaffected. Chasing the anomaly rather than dismissing
it is what exposed the documented-vs-measured gap.

**Why this is the pattern, not an isolated bug.** §2 credits **P74** with retro-fitting "the
hue-as-text family". That is now the **third** app-wide claim in this programme to fail on
inspection — P95's enabled-control class (3 escapes found by P101), P98's "`--text-3` family closed"
(122 declarations never classified), and now P74's hue-as-text sweep. Every one was asserted
app-wide without an enumeration. **P101 §3 is the template for the fix: enumerate, bucket, record a
verdict per call site, and only then claim closure.** Do not accept a "~30 call sites and it's fine"
sentence as evidence again.

**Scope for the pass:** enumerate every `color: var(--accent)` call site, resolve each one's
composited backdrop per state, and split text (4.5:1) from glyph/border/bar (3:1) — the split P100
established and P101 §2 formalised. Expect many to be legitimately glyphs. Related and already
filed: **P102** (`--danger` fills at 3.70:1, hardcoded `#ffffff`) and P100's residual noting four
sibling buttons still on `filter: brightness`. Consider doing P102 and P105 as one hue-audit
milestone, since both are "a hue token used against an insufficiently contrasting surface" and both
want the same enumerate-then-bucket method.

## 📋 P102 — `--danger` fill contrast — PENDING (filed 2026-09-01 from P100's survey)

Same defect class as P100, deliberately **not** folded into it (P100 contract §6-C; orchestrator
agreed — one defect class per milestone is what kept P95/P98/P100 reviewable).

`.btn-danger` (`src/styles/controls.css:70-71`) and `src/styles/updates.css:114-116` put a
**hardcoded `#ffffff`** on `var(--danger)`. `ui-reference.md` §6 already measures that pair at
**3.70:1 in dark** — below the 4.5:1 read-text bar, on destructive-action buttons where misreading
the label is the worst case.

**The remedy is already shipped in-repo**, so this is a small pass: `partial-staging.css:104` uses
the `--bg-0`-ink flip on `--danger` at **4.80:1** dark. P100 establishes the precedent and the
decision rule (reference §2 ACCENT FILL bullet, recipe 2: an action keeps its loud hue fill and
flips the ink; only *states* get demoted to `--selection`). Expect a `--danger-text` token mirroring
`--accent-text`'s per-theme split rather than two inline literals.

Scope check before implementing: sweep for every `#fff`/`#ffffff` on a `var(--danger)` fill, not
just these two — P100's survey found 7 accent fills where the seed list had 4.

## ✅ P101 — the full --text-3 audit — DONE + VERIFIED (`4fec07a`, USER 2026-09-02)

> **USER CHECKPOINT recorded 2026-09-02 on the user's direct instruction** (same basis as P100
> above — an unattended session, scoped by the user to P100 + P101 only). AC12-AC16 (hierarchy,
> density, the 15 `forge-pr` declarations behind a token screen, colour-only dot states,
> `.rebase-plan-commit.dropped` on `line-through` alone) were **accepted without an
> orchestrator-observed native run**. The AI-gate caveat below stands unchanged and is NOT
> retracted by this: AC7 coverage was **5 of 94 selectors measured**, the remaining 84
> source-derived because they are unreachable in the default mock state.

**All 124 declarations carry a recorded bucket and verdict** (`docs/contracts/P101-text3-audit-ui.md` §3) — the first pass in this programme to meet its own standard, so §2's "family closed" claim finally has an enumeration behind it. **31 exempt** (16 disabled, 10 placeholder/empty, 3 group titles, 2 glyphs clearing 3:1 on every state) + **93 fixed**. Post-fix grep is exactly **32 in 14 files** as predicted. Verified exhaustively, not sampled: the diff is 92 plain `color:` + 1 `border-color:` + 1 `background:` + the one §4.2 rule, and nothing else. Mounted spot-check (orchestrator, 5 of 94 selectors reachable in the default state): `.section-label`, `.tree-dir-name`, `.branch-badge`, `.file-chevron` all **7.25 dark / 7.45 light** — matching the predicted `--text-2`-on-`--bg-1` figures exactly, which validates the source-derivation method. 84 selectors are not reachable in the default mock state; AC7 coverage is therefore **5/94 measured**, the rest source-derived — stated plainly rather than called verified. **USER CHECKPOINT owed:** AC12-AC16 (hierarchy, density, the 15 `forge-pr` declarations behind a token screen, colour-only dot states, `.rebase-plan-commit.dropped` now carrying on `line-through` alone). Prior text archived below.

### Original filing (kept for the reasoning)

## 🚨 P101 — audit the 122 unaudited `--text-3` uses — RESOLVED by `4fec07a`

**Count reconciliation, done 2026-09-01 by the orchestrator — the contract pins 122, a fresh grep
says 124. Do not "correct" either number; both were right when taken.** The delta is exactly the
**two disabled-hint overrides P98's own MUST-FIX-1 added** (`src/styles/search.css:243`,
`src/styles/dialogs-forms.css:250`) — per-file counts confirm it: `search.css` 8 -> 9 and
`dialogs-forms.css` 6 -> 7, everything else unchanged. Both new declarations are legitimately in the
`disabled` bucket, so the audit's *work* is still 122 items. Authoritative surface as of `0fe0102`:
**124 in `src/styles/` across 33 files**, plus **one outside** it — `src/components/conflictCmSetup.ts:36`
(`.cm-gutters`, already sanctioned decorative in `ui-reference.md` §2 at 3.68/3.17 with a revisit
trigger). `src/components/settings/SettingsEmpty.tsx` mentions the token in a comment only — not a
declaration, not an audit item. Re-pin at audit time; §8.8 step 6's "family closed" claim must rest
on an enumeration that matches a fresh grep, which is the exact failure P101 exists to fix.

Distribution (highest first): `forge-pr` 15, `settings-legacy-sections` 11, `search` 9,
`commit-panel` 9, `sidebar` 7, `dialogs-forms` 7, `blame-history` 6, `repo-health` 5,
`diff-content`/`dialogs`/`controls`/`commit-box`/`agent-assets` 4 each, then 3s/2s/1s.

**One more seed found during reconciliation:** `.search-result-date`
(`src/styles/search.css:331`) — a date, the same class of string as the three `blame-history.css`
timestamps §8.8 already rules read-text. Audit it, do not assume the verdict.

**`ui-reference.md` §2's claim that the `--text-3` family is "closed" is FALSE — corrected during
P98, do not let it come back.** P95 swept the enabled-control class and P98 an *enumerated* read-text
set, but **122 `color: var(--text-3)` declarations remain in `src/styles/` and have never been
classified.** P95's AC10 grep hit ~140 and the AC was reworded precisely because enumerating them was
unsatisfiable; nobody went back.

Confirmed violations by §2's own test, in `src/styles/blame-history.css` — a file P98 never opened:
- `.blame-date` (:93), `.file-history-date` (:167), `.reflog-date` (:241) — **timestamps**, which §2
  explicitly names as read-text requiring `--text-2`. 3.68:1 dark / 3.17:1 light on `--bg-0`, worse
  on `--bg-1`/`--bg-2`.
- Borderline, needs a ui-designer call: `.reflog-oid-old`, `.reflog-oid-arrow`, `.reflog-oid-root`
  (:214/:218) — abbreviated oids in the reflog.

**Why this is filed as HIGH and not a NIT:** the discovery rate is the signal. Two gaps
(`.pr-merge-method-desc`, `.cm-gutters`) turned up by casual inspection, then three more by grepping
a *single* extra file. The orchestrator's P98 decision #3 was justified by "this is the last gap
making the closed claim true" — that premise was simply wrong, and a tidy-but-false "closed" claim
is worse than an honest scope statement because it stops a future sweep from ever looking.

Method to use (ui-designer is writing it into the P98 contract as the durable deliverable): classify
each of the 122 against the "**must the user read it to act?**" test, not against how the text looks
— small/uppercase/letter-spaced does not make text decorative.

## ✅ P98 — `--text-3` read-text sweep — DONE + VERIFIED (`be668e0`, USER 2026-09-01)

**Implemented; ui-designer APPROVED after one MUST-FIX. AI gate green; USER CHECKPOINT confirmed.**

Landed: 10 read-text swaps `--text-3` -> `--text-2` (8 selectors incl. the orchestrator's 8th,
`.pr-merge-method-desc`), 4 hardcoded white literals -> `var(--accent-text)`, and the §4 dead-token
repair (5x `var(--border-0)` -> `var(--border)`). Diff verified colour-only outside that one
sanctioned hunk; `git diff --stat` byte-identical to `--ignore-all-space --stat`.

**MUST-FIX-1 (found by ui-designer, a regression P98 itself introduced).** The `*-hint` rules are
*child* selectors, so they also matched inside a **disabled** option and beat the `--text-3` the
disabled state relies on inheriting: label 3.38:1 / hint 7.25:1 — the qualifier twice as bright as
the text it qualifies, disabled dimming half-applied, AC7 broken. Fixed with two rules
(`.combobox-option--disabled .combobox-option-hint`, `.command-palette-option.is-disabled
.command-palette-option-hint`). **Placement is load-bearing and must not be reordered:** each ties on
specificity with its `--active`/`.is-active` override, so source order alone decides the
disabled+active state. Measured specificity is (0,2,0) in `dialogs-forms.css` but **(0,3,0)** in
`search.css` (compound selectors on one element) — both carry a comment saying why the order matters.
All four disabled states verified to compute an identical label/hint colour in both themes.

**Verification honesty — read before trusting the numbers.** Only **4 of 10** declarations were
measured on a mounted instance (`.diff-overlay-kind`, `.command-palette-option-hint` idle + active,
and the active row's own label). The other 6 are **CSSOM-rule-confirmed but source-derived**, not
composited: `.diff-tree-count` needs canvas-driven commit selection (synthetic clicks don't hit the
canvas), and there is no mock fixture for a conflicted scope, the worktree dialog, or an *enabled*
combobox hint. The AC19 disabled-state figures are rule-level (injected nodes against the real
CSSOM), not a React-mounted option.

**Two mechanism traps recorded so they aren't re-hit:**
- `resize_window colorScheme` measures **dark twice** — the app themes off a `data-theme` attribute
  on `<html>` and never reads `prefers-color-scheme`. Set the attribute directly.
- `border-style: none` has a **used width of 0px**, so the §4 repair adds a real **1px** (bar height,
  each label's left edge). Below the 4px grain and it cannot reflow the `flex: none` panes, but it is
  not a no-op — this is a USER CHECKPOINT item, not something the AI gate can clear.

**USER CHECKPOINT — CONFIRMED by the user 2026-09-01.** Checked: (a) the 1px border appearing in the merge editor looks intentional and
doesn't crowd the panes; (b) on the *active* palette/combobox row the hint's colour step is now
exactly **1.00x** by design — subordination rests on 11px-vs-13px + right-edge placement, which is
the one place perception can disagree; (c) the 6 unreachable selectors read correctly in the real app.

**Routing correction:** ui-designer filed two pre-existing NITs "-> P99", but P99 is the
production-bundle `repo`-state bug. Both are accent-fill issues and belong with **P100**:
`conflicts.css:201` `var(--accent-fg, #fff)` — **`--accent-fg` is not a token** either, a second
phantom in the file §4 just repaired, surviving on its fallback (prescribed `var(--accent-text)`);
and `dialogs-forms.css:163` `.wt-copy-toggle-on` hardcoding `#fff !important`.

**`.cm-gutters` -> sanctioned decorative** (ui-designer's call): line numbers are universal editor
chrome and a coordinate duplicating visible structure, and the act-carrying text in that pane is
already `--text-2`. Recorded in §2's sanctioned list **with a revisit trigger** — any go-to-line,
line-range or line-naming feature makes them read text.
**Reflog oids (for P101):** `.reflog-oid-old`/`-root` are read text (you read them to pick a reset
target); `.reflog-oid-arrow` clears 3:1 so contrast doesn't force it — move it for **cohesion** only,
flagged as such so the precedent isn't misread as a contrast fix.

**Orchestrator decisions on the contract's three open questions (2026-09-01):**
1. **Accept the `--accent`-fill deferral (contract §5-A) as its own milestone → P100.** White text on
   the `--accent` fill measures **3.22:1 in dark** — below the 4.5:1 bar — and this affects the active
   row's **own primary label**, not just the hint. White is the ceiling, so no hint colour can pass in
   dark; the real fix is the fill (the designer measured a `--selection` recipe: label `--text-1`
   9.36/13.29, hint `--text-2` 5.01/6.42). Out of scope for a colour-swap milestone. Consequence
   accepted for now: in the active row the hint loses its colour step and leans on 11px-vs-13px +
   right-edge placement.
2. **Include the `--border-0` fix (contract §4)** — the one sanctioned non-colour change. `--border-0`
   is **not a real token**; its 5 uses in `conflicts.css` mean the merge editor's split-label bottom
   border and the OURS/THEIRS divider **do not render at all today**. Latent bug in a file P98 is
   already editing; leaving it would be worse than the small scope impurity. Must land as a clearly
   separated hunk. Note it makes a previously-invisible border appear — a real visual change.
3. **Fold in the 8th selector `.pr-merge-method-desc`** (designer excluded it as §5-D to respect the
   locked seven). Reason to override: §2 now asserts "the `--text-3` family is **closed**". A merge-method
   description is text you read *in order to choose* — read-text by the contract's own test — so leaving
   it `--text-3` makes that claim false. It is sanctioned in `ui-reference.md` §12.9, so ui-designer must
   amend §12.9 as part of its P98 design-review pass.

Split out of P95 by orchestrator decision (see P95 decision A). Seven selectors use `--text-3` for
text the user must actually **read**, violating the long-standing `ui-reference.md` §2 rule (these
are read-text, not the enabled-control class P95 sweeps): `.diff-overlay-kind`, `.diff-tree-count`,
`.conflict-editor-split-label`, `.wtctx-branch`, `.wtctx-blocked`, `.combobox-option-hint`,
`.command-palette-option-hint`. The two `*-hint` selectors are the worst offenders.

## ✅ P96 — P93 review follow-ups — DONE + VERIFIED (`cf8bdda`, USER 2026-09-01)

All four items landed; reviewer approved with **zero MUST-FIX**. Scope was the four filed items only
— the "carried in from P93" list below stays recorded context, **not** promoted work.
Gate: frontend tier 4/4 green (78.6s). prPanel suite 35 -> 38 tests, overlayMeta +7.

**Item 4 is worth remembering: it was filed as a NIT and was not one.** "Both effects fire
`onClosePrFileDiff`; idempotent, just a double call" made it look trivial. It took three attempts,
and the first two were wrong in ways only an exact-count test caught:
- Comparing PR numbers fixes only a *same-render* swap. `usePrDiff` calls `setStats` solely from its
  fetch effect, so on a switch `stats` still holds the OLD PR's oid and flips a commit **later** —
  the number guard no longer suppresses and C3 fires a second close. Distinct PRs normally have
  distinct heads, so this was the **common** path.
- Resetting the baseline to `null` then depends on a later oid change to re-establish it. Two PRs
  **can share a head sha**, leaving the baseline stuck at `null` and swallowing the next genuine
  advance — trading a harmless double call for a **missed** close (orphaned overlay showing the old
  head's file). A strict trade-down.
Final shape: the switch episode is bracketed and closed on stats **object identity** (changes when
the new PR's stats land on either the cache-hit or fetch-resolve path, even when the oid does not).
Fails safe toward over-fire on a handler whose prop contract permits it, never toward a missed close.

**Process lesson:** `reviewer`'s round-1 approval of item 4 reasoned only about synchronous
same-commit effect ordering (destroy-before-create) and missed the async late-arrival path; the
orchestrator's own check made the same error. The bug was found only by requiring each new test to
be shown FAILING against the unfixed code. Keep that requirement — a "was called" assertion would
have passed on the very double-fire being removed.

- SHOULD-FIX: add `overlayMeta.test.ts` pinning the load-bearing prefix ordering
  (`conflict:`/`ai-proposal:`/`pr:` before the `WorkdirSection` cast). Currently only indirect.
- NIT: `PrChangesSection.tsx` focus restore resolves the row by positional index into
  `listRef.current.children` — switch to `data-path` + `querySelector` (render-order independent).
- NIT: `overlayMeta.ts:41` `parsePrSlotPath(key) ?? key` surfaces a raw `pr:<oid>:<oid>` key as the
  overlay path for a malformed key.
- NIT: `PrDetailContainer.tsx:550-554` — C2 (unmount) and C3 (headOid change) both fire
  `onClosePrFileDiff` on a PR switch. Idempotent, just a double call.
- Carried in from P93 (deferred there, not blocking): stale `prOverlayCtx` after slot replacement
  (latent, all consumers key-gated); `onManageAccounts` callback identity; the fixture `fail`
  sentinel matches `includes('fail')` too broadly; PR rows ignore `panelDensity` (pre-existing since
  P89, contract §12.5). Full text → archive Part 23.

## ✅ P97 — split ContextMenu.tsx — DONE + VERIFIED (`59d3a41`, USER 2026-09-01)

Strictly behaviour-preserving (refactorer). `ContextMenu.tsx` **486 -> 112 lines**, into
`src/components/contextMenu/`: `MenuList.tsx` (313) and `types.ts` (70).

**Equivalence proof: 113/113 tests identical before and after**, across the same 7 affected test
files. That identity — not the split — was the deliverable.

**The types had to move too, and not for tidiness:** `MenuList` needs `ContextMenuItem`, so leaving
the interfaces in `ContextMenu.tsx` would have created a container<->child import **cycle**.
`ContextMenu.tsx` re-exports all three from the original path via `export type { ... } from
'./contextMenu/types'` — `export type`, not a plain re-export, so it survives
`isolatedModules`/`verbatimModuleSyntax`. **Zero consumer updates**; all ~37 referencing files
untouched, and no barrel `index.ts` (CLAUDE.md prefers narrow explicit imports). `MenuList` was
module-private before and stays unexported from `ContextMenu.tsx`, so the public surface is unchanged.

Net 486 -> 495 total lines: ordinary move overhead (imports, the re-export block, one `export`).
Structure was the goal, not line reclaim.

**Size baseline NOT updated** (deliberate, decision below). `ContextMenu.tsx` at 486 was never a
baselined offender, so the ratchet output is byte-identical before and after: `18 line(s) reclaimed
across 13 file(s)`, 29 offenders over limit.

Pre-existing smells moved verbatim and deliberately **not** fixed (refactorer scope): the
`react-hooks/exhaustive-deps` disable on `MenuList`'s `autoFocus` effect (calls `focusFirst`,
declared later, deliberately omitted from deps), and the index-based `key={i}` on rows, which the
surrounding comment already justifies. Strictly behaviour-preserving
(refactorer), proven by identical before/after test counts.

---

## 🛠️ DX — dev-loop acceleration — in-progress (condensed; full text → archive Part 30)

8 of 10 improvements landed & verified (`3ada322`, `2019e71`, `8e55be8`): dev-profile
`debug = "line-tables-only"` + rust-lld linker, `cargo-nextest` (`pnpm test:rust`), the one-command
gate `scripts/gate.mjs` (clippy in its own `target/clippy` dir), the CLAUDE.md process changes
(concurrent code+design review, velocity mode), and the branches.rs / stash.rs / RepoWorkspace
overlay god-file splits.

Two user decisions that must survive compaction:
- **P75 (IPC codegen) — HALTED 2026-08-21 (user decision).** Linking `tauri-specta` breaks app launch
  on Windows 10 (`kernel32!WaitOnAddress` not exported → `STATUS_ENTRYPOINT_NOT_FOUND`). Spike
  reverted; findings + crate pins kept in `docs/contracts/P75-ipc-codegen.md`. Revisit only if
  validated on Windows 11 or with a link-order fix.
- **P76 (native-checkpoint automation) — HELD as contract-only per user (2026-08-20).**
  `docs/contracts/P76-native-checkpoint-automation.md`.

Deferred cleanups (noted, not done): lock the file-size baseline reclaim for App.tsx (P74) and
RepoWorkspace.tsx; de-duplicate the private `open_repo_at` helper across the `git/` modules.

---

## ⏱️ Velocity / gate cost — measured 2026-09-01

All numbers: `docs/history/velocity-2026-09-01.md`. Why it matters: full `pnpm gate` is ≈5-7 min and
orchestration ceremony — not machine time — is ~75-85% of per-task wall clock, which is what the
CLAUDE.md velocity-mode and batching rules exist to cut. Gate child processes now use
`D:\Data\Temp\bonsai-build`, not Defender-scanned `C:\Temp`.

---

## ✅ Accepted decisions that must survive compaction

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
  P83, P85-P90, P92, P93 and DEP REFRESH are confirmed too. Full per-milestone text →
  `docs/history/todo-archive-2026-08.md` Parts 17-21 and `todo-archive-2026-09.md` Parts 22-31.

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

Items resolved in the 2026-08-21 fix batch were archived → `todo-archive-2026-09.md` Part 32
(underlying full text: `todo-archive-2026-08.md` Part 19).

### Velocity follow-ups from the 2026-09-01 measurement pass (still open)

Context + all baseline numbers: `docs/history/velocity-2026-09-01.md`. Done in that pass:
proptest banding (`d635464`), doc curation (`0174abf`), 78 → 8 test harnesses (`12882f9`).

- **`prop_status::status_matches_porcelain` is now the workspace's slowest test at 23.5s** in a
  single test fn (measured at full baked counts). nextest parallelizes per test fn, so this is the
  new critical-path floor. Same fix as `prop_graph_layout`: band the input axis into N fns with
  cases allocated proportional to band width. Expected ~23.5s → ~5s.
- **`prop_stash_roundtrip` 14.3s** across 2 fns — same banding treatment, lower priority.
- **`submodule_cli::oracle_add_deinit_remove_roundtrip` 12–14s** — NOT a proptest (a git-CLI
  oracle roundtrip), so banding does not apply; needs its own look if the ~12s floor matters.
- **vitest jsdom construction dominates the frontend leg.** CPU-aggregate across workers:
  `environment` 613s vs `tests` 147s, for 199 files / 2397 tests in 62–68s wall. Try `happy-dom`,
  or `environmentMatchGlobs` so only DOM-touching files pay for one. Untried — measure after.
- **`pnpm gate --quick` is 305s and only drops e2e**, so it is not a fast tier. `cargo nextest
  --workspace` alone is 181s of it. Either add a genuinely narrow tier or lean on
  `--rust` / `--frontend`. CLAUDE.md velocity mode now says so.
- **Ceremony, not machine time, is the dominant per-task cost**: ~75–85% of wall clock. Of the 200
  commits before this pass, 61 were `docs:` bookkeeping vs 24 `feat` + 27 `fix`, and small tasks
  (P92/P93/P95) ran 1h45–2h45 end to end against a ~5–7 min gate. Candidate process changes, NOT
  yet adopted — needs a USER decision: batch small P-tasks through one senior-dev spawn, skip the
  architect contract for single-component fixes, fold the board update into the feat commit.

### ⚠ FOR USER — record inconsistencies surfaced by the 2026-09-01 curation sweep

The curator refused to resolve these itself (it never upgrades a status). All are record-keeping,
not code:
- ~~**P88** headed `in-progress`, **P85 / P86 / P87 / P87d** headed `pending`, against bodies that
  read DONE with checkpoints verified 2026-08-25.~~ **RESOLVED by USER 2026-09-01: all five are
  done and verified.** Headings corrected in `docs/history/todo-archive-2026-09.md` (they were
  already archived); no body text was changed.
- ~~**P88/P89/P90/DEP-REFRESH** all say "UNMERGED/UNPUSHED, awaiting merge decision".~~
  **RESOLVED by USER 2026-09-01: the merge decision is settled.** Re-verified the same day with
  `git merge-base --is-ancestor`: `feat/pr-local-diff`, `perf/git-action-round2` and
  `chore/dep-refresh-2026-08` are contained in **both `dev` and `main`** (all three, not just
  dep-refresh as first noted). The six stale lines in
  `docs/history/todo-archive-2026-09.md` now carry inline corrections; the historical text was kept.
  `feat/p91-observability` remains in no other branch — consistent with the live P91 entry.
- ~~**P84's USER CHECKPOINT was never recorded.**~~ **RESOLVED by USER 2026-09-01: the user
  confirmed P84's checkpoint DID pass**, so P84 is done and verified. Recorded on that direct
  confirmation, not on a contemporaneous 2026-08 record — none was ever written. Its code shipped
  (`cce9eb9`, `90b315c`, `1803391`, `6868be6`); its two contracts are in
  `docs/contracts/archive/` and `todo-archive-2026-09.md` Part 33 carries the corrected status.
- No contract file was ever written for **P94**.

### Hoisted off milestones archived 2026-09-01 (still open)
- **keyring 3 → 4** needs a dedicated increment: 4.x moves onto `keyring-core`, renames every
  per-backend feature (`windows-native` → `windows-native-keyring-store`, etc.), drops
  `crypto-rust`, and requires explicit credential-store registration instead of feature-driven
  resolution — i.e. real changes to `crates/bonsai-forge/src/auth.rs`. (DEP REFRESH, archive Part 24.)
- `no_proxy_client()` in `src-tauri/src/mcp/http_support.rs` still uses
  `.expect("build reqwest client")` — why the missing rustls provider surfaced as a raw panic rather
  than a message. (DEP REFRESH, archive Part 24.)
- **P87b FU-1..4** still open: target row label, commitAmend row, row `role`/`aria-expanded`,
  clickable dock bar. Plus the `AiActivityPanel` aria-label NIT. (P85-P87 batch, archive Part 27.)
- **RepoWorkspace refactor** — still stands for maintainability (not perf); P88's audit re-confirmed
  it. (archive Parts 26-27.)
- **P90.1 deferred:** per-check timing fields; header commit-summary text; command-palette
  `Refresh checks` / `Show checks`; mock fixtures for noForge/error reachable by click.
  (P90, archive Part 25.)
- **Known flake (pre-existing, untouched):** `watcher::tests::git_internals_filtered`
  (`watcher.rs`) is a timing flake (`unwrap_err` on an `Instant`); passes on isolated re-run.
  (P88, archive Part 26.)
- **⚠ FLAG FOR USER (peer session, now ended):**
  `src/components/repoWorkspace/useWorkspaceKeyboard.test.tsx` failed in ISOLATION on the committed
  baseline (1 graph-nav `defaultPrevented` case), introduced by the peer's graph-a11y commit
  `590f2ef`. Likely test-isolation flakiness. Raised in the P86 block; carried here on archive
  (archive Part 27). Status unverified since 2026-08-23.
- **P84 record gap** → written up in `docs/history/todo-archive-2026-09.md` Part 33 (2026-09-01):
  code shipped, USER CHECKPOINT never recorded, contracts archived on user instruction.

### Residue of the two dated 2026-08-22 design reviews (still open)
Both review files now live in `docs/contracts/archive/`; per-finding dispositions +
verification evidence → `docs/history/todo-archive-2026-09.md` Part 35.
- **`graph-design-review-2026-08-22.md` M1 is SUPERSEDED — do not implement.** `role="grid"` /
  `aria-rowcount` / `aria-activedescendant` are forbidden by `ui-reference.md` §4.1 (`:250-252`,
  revised 2026-08-31 by P95). Verified 2026-09-01.
- **`graph-design-review-2026-08-22.md` M2/M3/M4/S2/S3/N1/N2 — resolution unverified.** Not checked
  by the 2026-09-01 sweep (bounded effort); do not assume they landed.
- **`review-2026-08-22-ui.md` NIT-1 — Sidebar ignores `panelDensity`** (confirmed still open
  2026-09-01: no density reference in `Sidebar.tsx`, `src/components/sidebar/**`, or
  `src/styles/sidebar.css`; `.branch-row` is a fixed height).
- **`review-2026-08-22-ui.md` NIT-2 —** `src/components/OnboardingOverlay.tsx:229` is still
  `aria-label="Close"`; the review preferred "Close the tour".
- `review-2026-08-22-ui.md` SHOULD-3 (`--accent` text over `--selection` fails AA) is the **same
  item** as the live P69 **A9** follow-up below — A9 is the canonical entry.

### Known load-flake (still open) — timing-sensitive, not a correctness bug
`ai::session_tests::watchdog_tests::watchdog_does_not_fire_while_awaiting_input` (path updated
2026-09-02 by the size-ratchet split; was `ai::session_tests::…`) failed once under load and passed on
immediate re-run.

`src/App.test.tsx > App shell > an Arrow-key pane nudge persists the POST-nudge width` — same shape
(added 2026-09-02). Failed once in a full `pnpm gate` run at 2662ms (`setUiSettings` never called,
i.e. the debounced persist had not fired before the assertion), then passed 4/4 isolated and
2644/2644 on a full-suite re-run. Timing-sensitive under parallel load, not a correctness bug.

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
  `docs/contracts/archive/P69-settings-ui.md` §3.2.1, `[NOT IMPLEMENTED]` — the flagship query `graph` returns
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

### macOS ad-hoc code signing — config DONE 2026-08-30, **RELEASE STILL PENDING**
Only the pending half stays here; full detail → `docs/history/todo-archive-2026-09.md` Part 34.
- `bundle.macOS.signingIdentity: "-"` is in `src-tauri/tauri.conf.json` but **has not shipped**: the
  last tag is `v1.5.0` (2026-08-26), which predates the fix. It takes effect on the next tagged
  release — verify the sealed ad-hoc signature then.
- Not fixed by ad-hoc at all: Gatekeeper "unidentified developer"; a new version re-prompts once for
  TCC (cdhash changes). Full fix = Developer ID + notarization (Apple Developer Program); the
  `APPLE_*` env block in `.github/workflows/release.yml` is already scaffolded.

---

---

## Archive

**Start at `docs/history/README.md`** — it is the navigable index of every archived milestone and
part number. The table below is the short form.

| File | Covers |
|---|---|
| `docs/history/README.md` | **The archive index** — which file/part holds which milestone. |
| `docs/history/todo-archive-2026-09.md` | **Parts 33-35 (moved 2026-09-01):** the P84 record gap (code shipped, USER CHECKPOINT never recorded) · the macOS ad-hoc-signing detail (release still pending) · per-finding dispositions of the two dated 2026-08-22 design reviews. **Parts 22-32 (moved 2026-09-01, verbatim):** P94 · P93 + P92 · DEP REFRESH 2026-08-28 · P90 + P89 · P88 · the P85-P87 perf+observability batch incl. P87c/P87d · P82 + P83 · divergence reconcile + Release 1.1.0 + P80b/P81/P82 · the full DX dev-loop text · the full confirmed-checkpoints block · the 2026-08-21 resolved follow-ups. |
| `docs/history/todo-archive-2026-08.md` | Parts 1-9: P65 → P28 build detail, the Phase 1-4 banners, resolved FOR-USER decisions, P69(1.0.0)/P67/P68 detail, the 2026-08-17 batch mapping, resolved spun-out items. **Parts 10-16 (moved 2026-08-20): the P62-P74 checkpoint waiver + P71, P72, P73, P74, the P69 Settings redesign, and the Audit #2 fix batch, condensed. Parts 17-18 (moved 2026-08-21): P70 and P77, both checkpoints verified. Part 19 (moved 2026-08-21): the OPEN follow-ups resolved in the 2026-08-21 fix batch (read_status/palette/refetch/stash/submodule/STDERR/cred-split), verbatim. Part 20 (moved 2026-08-21): P78/P79/P80 forge milestones, condensed. Part 21 (moved 2026-08-21): P80b/P81/P82, done + checkpoints confirmed, condensed.** |
| `docs/history/todo-archive.md` | P27 → P2, M0-M6 |
| `docs/history/milestones-mvp.md` | the M0-M6 AI-gate vs USER CHECKPOINT split |
| `docs/history/context-pollution-audit.md` | the context/token-cost audit |
| `docs/history/velocity-2026-09-01.md` | gate wall-clock, test-suite hotspots, inner-loop rebuild cost, ceremony-vs-machine-time split (2026-09-01) |
| `docs/contracts/INDEX.md` | one line per contract file — milestone, scope, status |

Move a milestone's section into the current dated archive file only once **both** halves of its gate
have passed (or the native half is explicitly waived). A milestone with a pending USER CHECKPOINT
stays on this board.
