# P87b FU-1 (backend half) — where a git-activity run's `target` comes from

**Owner:** architect · **Written:** 2026-09-03 · **Status:** **shipped** in `1d8c6f9` (full 8-step gate green)
**Corrected 2026-09-10, post-implementation** — implementing FU-1 proved three statements here wrong;
each is fixed in place rather than silently: §4's unborn-HEAD rationale (the `null` is enforced by a
**load-bearing guard**, not "free" — `read_head_info` *does* name the branch-to-be), §3 + §9.2's test
name (`push_target_matches_push_result` → `push_target_agrees_with_push_result_remote`, with the
reason for the rename), and §8's `?gitLongTarget` literal (the illustration measured 83 chars, not
≥90; the shipped fixture is 95). §6's enforcement table never named the renamed test, so nothing
there changed. A contract that silently changes after the fact is worse than one that records the
correction.
**Renders to:** `docs/contracts/P87b-FU1-FU4-git-dock-ui.md` §3.3/§3.6/§3.10 (ui-designer, authoritative
for everything user-visible). This file supplies only what §3.6 commissioned.
**Parent:** `docs/contracts/archive/P87-ui.md`, `crates/bonsai-core/src/git/activity.rs`

> Line numbers below are as of HEAD (`1be3a85`). A senior-dev is mid-change in
> `src/components/GitActivity*`, `src/ipc/mock/handlers/stash.ts`, `src/styles/` — treat §7/§8 as
> shape, re-locate before editing.

---

## 1. Scope

`target` is fire-and-forget observability, exactly like the rest of the P87 seam: it must never gate
or change an op's success/error, and it must cost nothing when nobody is subscribed. One optional
string, on `started` only.

| File | Change |
|---|---|
| `crates/bonsai-core/src/git/activity.rs` (353) | `+ ActivityTarget` newtype + cap const beside `activity_line`; `GitActivityEvent.target`; `ActivityEmitter::new` takes the target. **~+75 → ~430** |
| `crates/bonsai-core/src/git/activity_target.rs` | **NEW** — `resolve_activity_target()` + `configured_upstream()`. The only git2 in this feature. ~110 |
| `crates/bonsai-core/src/git/activity_target_tests.rs` | **NEW** — fixture-repo table tests. ~150 |
| `crates/bonsai-core/src/git/activity_tests.rs` | `+` newtype tests (§6) |
| `crates/bonsai-core/src/git/mod.rs` | `+ pub mod activity_target;` |
| `src-tauri/src/commands/activity.rs` | `with_activity` gains a `target` param; `+ activity_target()` helper. ~+30 |
| `src-tauri/src/commands/{remotes,staging,merge}.rs` | one line each, at the 7 call sites in §5.2 |
| `src-tauri/src/commands/shared.rs` | re-export `ActivityTarget`, `activity_target` |
| `src/ipc/types/activity.ts` | `+ target?: string` |
| `src/components/repoWorkspace/gitActivityState.ts` | `+ target: string \| null` + `newGitRun` param |
| `src/components/repoWorkspace/useGitActivity.ts:118` | pass `ev.target ?? null` |
| `src/ipc/mock/gitActivity.ts` | §8 |

No new IPC command, no new event kind, no new channel. The activity channel
(`git_activity_subscribe`) is unchanged apart from one extra optional field on one kind.

---

## 2. Rust — the type (`git/activity.rs`, beside `activity_line`)

```rust
/// Cap on a run target, in CHARS. Git's practical ref bound is far under this, so
/// 255 never truncates a real name; it exists so a hostile ref cannot park a
/// megabyte string in the frontend's 200-run store (FU-1 §3.6-4).
pub const MAX_ACTIVITY_TARGET_CHARS: usize = 255;

/// A run's target ref: a RAW git identifier, sanitized and capped. Never a human
/// phrase — the frontend derives all copy (`all remotes`, prepositions) from the
/// category (FU-1 §3.3). The inner `String` is private and there is NO public
/// constructor: outside this crate the only way to obtain one is
/// [`crate::git::activity_target::resolve_activity_target`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityTarget(String);

impl ActivityTarget {
    /// THE funnel. `None` when nothing survives sanitation.
    fn new(raw: &str) -> Option<Self>;

    pub(crate) fn remote(remote: &str) -> Option<Self>;
    /// `branch` may be a short name or `refs/heads/<x>`; the prefix is stripped.
    pub(crate) fn remote_branch(remote: &str, branch: &str) -> Option<Self>;
    /// Branch SHORT name; `refs/heads/` is stripped.
    pub(crate) fn branch(branch: &str) -> Option<Self>;

    pub fn as_str(&self) -> &str;
    pub fn into_string(self) -> String;
}
```

```
new(raw):
    clean = strip_control_chars(raw).trim()      # the SAME fn activity_line uses
    if clean.is_empty(): return None
    return Some(ActivityTarget(truncate_chars(clean, MAX_ACTIVITY_TARGET_CHARS)))

remote(r)              = new(r)
branch(b)              = new(strip_prefix(b, "refs/heads/"))
remote_branch(r, b):
    rp = remote(r)?                              # each part validated separately, so a
    bp = branch(b)?                              # blank part yields None, not "/main"
    return new(format!("{}/{}", rp.as_str(), bp.as_str()))
```

`truncate_chars` already appends `…` past the cap; for a target that is the right marker (§3.6-4
says the UI's `title` simply shows the capped value).

### 2.1 Event + emitter

```rust
pub struct GitActivityEvent {
    // …unchanged…
    /// `Started` ONLY — the run's target ref. Raw identifier, sanitized, ≤255 chars.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

pub struct ActivityEmitter {
    id: String,
    /// Set at construction, read only by `started`. No setter, no `&mut self`.
    target: Option<String>,
    // …unchanged…
}

impl ActivityEmitter {
    pub fn new(
        id: String,
        target: Option<ActivityTarget>,
        emit: Box<dyn Fn(GitActivityEvent) + Send + Sync>,
    ) -> Self;                              // stores `target.map(ActivityTarget::into_string)`
}
```

- `base()` sets `target: None`, unconditionally.
- `started()` — signature unchanged — sets `ev.target = self.target.clone()`. Nothing else ever does.
- `GitActivityRecorder` gains **no** method. Core cannot touch the target.

---

## 3. Rust — the resolver (`git/activity_target.rs`, NEW)

```rust
use std::path::Path;
use crate::git::activity::{ActivityTarget, GitActivityCategory};

/// Best-effort target for a run that is ABOUT to start. INFALLIBLE by design —
/// every failure path yields `None` (the row renders the bare noun), because this
/// is observability and must never fail an op. Blocking (git2): callers run it
/// inside `spawn_blocking`.
pub fn resolve_activity_target(
    workdir: &Path,
    category: GitActivityCategory,
) -> Option<ActivityTarget>;

/// `branch.<name>.remote` + `branch.<name>.merge`, read-only, never erroring.
/// Returns (remote, remote-branch SHORT name).
fn configured_upstream(repo: &git2::Repository, branch: &str) -> Option<(String, String)>;
```

```
resolve_activity_target(workdir, category):
    if category == Fetch: return None            # §4, row "fetch"
    repo = open_repo_at(workdir).ok()?
    head = read_head_info(&repo).ok()?           # git/repo.rs:73
    if head.unborn or head.detached: return None # LOAD-BEARING — see §4
    branch = head.branch_name?                   # short name

    match category:
      Commit | Amend | MergeCommit:
          return ActivityTarget::branch(&branch)

      Pull | ForcePush:
          (remote, rbranch) = configured_upstream(&repo, &branch)?   # None => op will error anyway
          return ActivityTarget::remote_branch(&remote, &rbranch)

      Push:
          if let Some((remote, rbranch)) = configured_upstream(&repo, &branch):
              return ActivityTarget::remote_branch(&remote, &rbranch)
          # no upstream => push_current defaults to origin/<branch> and sets it
          if repo.find_remote("origin").is_ok():
              return ActivityTarget::remote_branch("origin", &branch)
          return None                            # no origin => op errors; bare `Push`

configured_upstream(repo, branch):
    refname = format!("refs/heads/{branch}")
    remote  = repo.branch_upstream_remote(&refname).ok()?.as_str()?.to_string()
    merge   = repo.config().ok()?.get_string(&format!("branch.{branch}.merge")).ok()?
    return Some((remote, strip_prefix(merge, "refs/heads/").to_string()))
```

**Why a second resolution rather than reusing the op's.** `push_current_with_activity`
(`remote_push_activity.rs:67-94`) and `force_push_with_lease_with_activity` (`:226-256`) resolve the
same upstream, but their versions are error-carrying: each `Err` branch is load-bearing for the op's
error taxonomy (`NoUpstream` vs `NoRemote` vs `PushRejected`). Folding an infallible observability
read into them would either weaken that taxonomy or wrap it in `Option` at the wrong layer. The
resolver is therefore an independent read-only mirror, and the drift is pinned by **two assertions
across two fixtures** rather than by sharing code. This is the one deliberate duplication in this
contract.

The two assertions are not interchangeable, and the split is why the anti-drift test was renamed
during implementation (`push_target_matches_push_result` → the shipped name):

1. **`push_target_agrees_with_push_result_remote`** (§9.2) pins the **remote name** only.
   `PushResult::UpToDate { remote, branch }` carries `branch` = the **local** branch
   (`remote_push_activity.rs:105-107`, where `branch` is filled from `head.branch_name`) — never the
   upstream branch. So it can compare the whole `remote/branch` string just in the special case where
   the local and upstream branch names happen to be equal, which is what its fixture arranges. Under
   the old name — and the old, stronger claim — the test would have **failed falsely** on a divergent
   upstream (local `main` tracking `upstream/trunk`) while the resolver was perfectly correct.
2. **`resolves_table`'s `renamed` fixture** (`main` → `upstream/trunk`, asserted for **Pull and
   Push**) pins Push's use of `branch.<x>.merge`. This is the gap the old single test left: without
   the Push assertion the Push arm could read `branch.<x>.remote` and ignore `branch.<x>.merge`
   (yielding `upstream/main`) with every test still green.

Cost: one extra `Repository::open` + ≤2 config reads per op, **only while the dock is subscribed**
(§5.1 gates on `is_active`). Sub-millisecond against a network push; for commit it precedes a repo
open that was happening anyway.

---

## 4. The per-operation table — the exact expression, and what "none" means

`b` = `read_head_info(&repo).branch_name` (short); `(r, rb)` = `configured_upstream(&repo, b)`.

| Category | Call site (HEAD) | Expression | Yields `None` when | Row then reads |
|---|---|---|---|---|
| `Push` | `remotes.rs:164` | `remote_branch(r, rb)`, else `remote_branch("origin", b)` if `origin` exists | unborn / detached / no upstream **and** no `origin` | `Push` |
| `ForcePush` | `remotes.rs:203` | `remote_branch(r, rb)` | unborn / detached / no upstream (op returns `NoUpstream`) | `Force-push` |
| `Pull` | `remotes.rs:128` | `remote_branch(r, rb)` | unborn / detached / no upstream (op returns `NoUpstream`) | `Pull` |
| `Fetch` | `remotes.rs:90` | **`None`, always** | always — `fetch_all_with_activity` fetches *every* remote | `Fetch all remotes` (frontend-derived) |
| `Commit` | `staging.rs:75` | `branch(b)` | unborn HEAD, detached HEAD | `Commit` |
| `Amend` | `staging.rs:177` | `branch(b)` | detached HEAD (unborn cannot amend) | `Amend` |
| `MergeCommit` | `merge.rs:82` | `branch(b)` | detached HEAD | `Merge commit` |

**Commit/amend/merge on detached HEAD emit `null`, and that is right.** §3.3 renders a bare `Commit`
with no placeholder; there is no ref the commit is "on", and inventing `HEAD` or `(detached)` would
claim a fact. The commit's own hash is not a target — it does not exist at `started`.

**Unborn HEAD emits `null` per §3.6-1, and the guard that enforces it is LOAD-BEARING — do not
remove it.** `read_head_info` (`crates/bonsai-core/src/git/repo.rs`, the `UnbornBranch` arm) returns
`HeadInfo { branch_name: Some(<branch-to-be>), unborn: true }`: on an unborn HEAD it falls back to
reading HEAD's **symbolic target**, which does name the branch that the first commit will create. So
`branch_name` is `Some`, `head.branch_name?` does **not** short-circuit, and nothing about the `null`
is free.

The `null` comes solely from the explicit guard in §3's pseudocode, shipped as

```rust
if head.unborn || head.detached { return None }
```

in `crates/bonsai-core/src/git/activity_target.rs`, and pinned by `resolves_table`'s unborn fixture
(which asserts `None` for both `Commit` and `Push` on a repo whose `read_head_info(..).unborn` is
`true`). Delete the `unborn ||` half and a repo's **first commit silently renders `Commit main`**,
contradicting §3.6-1 with no compile error and no other test failing.

*(This is the same fact as **F-1**, stated from the other side: because `read_head_info` already
surfaces the branch-to-be, `Commit main` is available by dropping one token from that guard — it is
not extra code to write, it is a guard to remove. §3.6-1 explicitly lists unborn under `null`, so the
guard stays. Corrected 2026-09-10: this passage previously claimed `read_head_info` returns
`branch_name: None` for unborn "so this is free", which reads as though the guard were redundant.)*

**`Fetch origin` is unreachable in the real backend.** §3.3's "fetch (one remote)" row exists as
copy, but there is exactly one fetch entry point and it is fetch-all. The row is therefore mock-only
until a per-remote fetch command exists; `ActivityTarget::remote()` is specified above so that
command needs no contract change. Flagged as **F-3**.

---

## 5. Command layer

### 5.1 The helper (`src-tauri/src/commands/activity.rs`)

```rust
/// Resolve a run's target before the run starts. `None` when nobody is
/// subscribed (no repo open is paid), when the repo id is unknown, or when the
/// resolver declined. NEVER returns an error — a failure here must not fail the op.
pub(crate) async fn activity_target(
    state: &AppState,
    repo_id: &str,
    category: GitActivityCategory,
) -> Option<ActivityTarget>;
```

```
if !state.git_activity.is_active(): return None
path = repo_path(state, repo_id).ok()?           # cheap map lookup; errors swallowed
spawn_blocking(move || resolve_activity_target(&path, category)).await.ok().flatten()
```

Swallowing the `repo_path` error is deliberate: the op's own `repo_path?` inside the closure still
produces the real error and the failed run row, exactly as today. Ordering is unchanged.

### 5.2 `with_activity` gains one parameter

```rust
pub(crate) async fn with_activity<T, F, Fut>(
    hub: GitActivityHub,
    category: GitActivityCategory,
    target: Option<ActivityTarget>,     // NEW — third position
    run: F,
) -> Result<T, AppError>
```

Body change is two lines: pass `target` to `ActivityEmitter::new`. The `!hub.is_active()`
passthrough is unchanged (a target computed under a race is simply dropped).

Each of the 7 op inners gains one line before its existing `with_activity(…)` call, e.g.:

```rust
let target = activity_target(state, repo_id, GitActivityCategory::Push).await;
with_activity(state.git_activity_hub(), GitActivityCategory::Push, target, move |emitter| async move {
    // …body byte-identical to today…
})
```

The existing unit test at `activity.rs:100` gains `None` in the new position.

---

## 6. Guarantee enforcement — structural vs. test

| §3.6 guarantee | Enforcement | Named test |
|---|---|---|
| **1. Raw identifier, never a phrase** | **Structural at the crate boundary.** `ActivityTarget.0` is private, `new` is private, and the three named constructors are `pub(crate)` — so `src-tauri` **cannot construct one at all**; the sole cross-crate producer is `resolve_activity_target`. The guarantee reduces to "the resolver's table is right", which is §4. `refs/heads/` stripping is inside the constructors, so no call site can leak a full refname. Residual, stated honestly: *within* `bonsai-core` someone could write `ActivityTarget::branch("all remotes")`. Nothing structural prevents that; the table test does. | `activity_target_tests::resolves_table` (one fixture repo per row of §4) + `no_target_contains_prose` — asserts every §4 output contains no `' '`, `'\u{2192}'` (→), `'\''`, `'"'` |
| **2. Set on `started`, immutable** | **Structural.** The target lives on `ActivityEmitter` as a private `Option<String>` set at construction; every method is `&self`; there is no setter and `GitActivityRecorder` gains no method, so a mid-run change is **not representable**. `base()` hard-codes `target: None`; only `started()` reads the field. | `activity_tests::target_appears_only_on_started` (drive an emitter through phase/line/hookDone/progress/finished, assert exactly one event carries `target`) |
| **3. Sanitized like `activity_line`** | **Structural, one funnel.** `ActivityTarget::new` calls the *same* `strip_control_chars` (`activity.rs:324`) that `activity_line` calls. The rule has one implementation; the two boundaries are thin wrappers around it — the shape that already exists for lines. See §6.1 for why not two funnels. | `activity_tests::target_strips_bidi_and_zero_width` — input `origin/ma\u{202E}in`, plus `\u{200B}`, C0 `\n`, C1 `\u{0085}` |
| **4. ≤255 chars** | **Structural.** Same funnel, `truncate_chars(.., MAX_ACTIVITY_TARGET_CHARS)`. Unreachable by any other path. | `activity_tests::target_capped_at_255_chars` (400-char ref → exactly 255 chars ending `…`) |

### 6.1 Where sanitation lives, and why exactly there

**One choke point per boundary, one implementation of the rule.** `strip_control_chars` +
`truncate_chars` are the rule; `activity_line` and `ActivityTarget::new` are the two boundaries that
apply it. I did **not** sanitize again at `ActivityEmitter::started` (the emit site) even though that
is where `line` does it, because `started` cannot receive an unsanitized target — the type it takes
is already proof. Sanitizing at both would be the "rule enforced in two places drifts" failure the
brief names. Conversely I did not push it down into the resolver, because then a future producer
would have to remember; the type is the memory.

**The one place the rule genuinely is implemented twice: the mock.** `src/ipc/mock/gitActivity.ts`
is a second backend and never runs Rust, so §3.10's `?gitBidiTarget` seam can only prove the contract
is *modeled* if the mock strips too. That is the same compromise already made for
`MAX_ACTIVITY_LINE_CHARS` (mirrored at `gitActivity.ts:66` with a `MIRRORS` comment). See §8 —
flagged **F-2**.

---

## 7. TypeScript

```ts
// src/ipc/types/activity.ts — inside GitActivityEvent
  /** `started` only — the run's target ref. A raw git identifier (`origin/main`,
   *  `main`), sanitized and <=255 chars. Absent = this run has no target; the
   *  frontend derives all copy from the category (never a human phrase here). */
  target?: string;
```

```ts
// src/components/repoWorkspace/gitActivityState.ts — inside GitActivityRun
  /** Set once from `started`; never changes for the run's life. */
  target: string | null;

export function newGitRun(
  id: string,
  category: GitActivityCategory,
  phase: GitPhase,
  seq: number,
  now: number,
  target: string | null,      // NEW, last position
): GitActivityRun;
```

`useGitActivity.ts:118` passes `ev.target ?? null`. **The store must never write `target` again** —
no other branch of the reducer touches it (guarantee 2, frontend side).
Test: `useGitActivity.test.tsx::target_survives_later_events` — a `started` with a target followed by
`phase`/`line`/`finished` leaves `run.target` unchanged; call site at `:152` gains the new arg.

**No migration.** The run store is session-scoped — `useGitActivity.ts:14` "nothing persists across
app restart" — and nothing serializes `GitActivityRun`. The only "old record" case is a frontend
running against an older backend (dev/HMR), where `target` is absent → `undefined ?? null` → `null` →
§3.3's last row, the bare noun. That path is exercised by the `?gitNoTarget` seam, so it is covered
without a migration step.

The frontend does **not** re-sanitize or re-truncate. `runTarget()` stays a pure formatter; a
defensive strip there would mask a backend regression instead of surfacing it.

---

## 8. What the mock owes (`src/ipc/mock/gitActivity.ts`)

```ts
/** MIRRORS `ActivityTarget::new` (P87b-FU1-run-target §2). The mock is a second
 *  backend: fixtures cross the same boundary, so they get the same funnel.
 *  Strips C0/C1 + bidi (U+200E/200F, U+202A–202E, U+2066–2069) + zero-width
 *  (U+200B–200D, U+FEFF), trims, then caps at 255 chars. */
function mockActivityTarget(raw: string | null): string | null;

export function runMockActivity<T>(
  category: GitActivityCategory,
  target: string | null,        // NEW, second position (ui-designer's suggested shape)
  fn: () => Promise<T>,
): Promise<T>;
```

`GitSequencer.start(category, target)` emits `target` on the `started` event only, passing it through
`mockActivityTarget` first, and drops the key when it is `null` (so the wire shape matches serde's
`skip_serializing_if`).

New seams, in the file's existing `const X = query('x') !== null` idiom at `:58-63`:

| Seam | Fixture target | Applies to |
|---|---|---|
| *(default)* | `'origin/main'` / `'main'` | push, forcePush, pull → `origin/main`; commit, amend, mergeCommit → `main`; fetch → `null` |
| `?fetchAll` | `null` on fetch | already the default; the seam exists so the case is addressable by name |
| `?gitNoTarget` | forces `null` for **every** category | the absent-field path (§7) |
| `?gitLongTarget` | `'origin/feature/very-long-experimental-branch/with-many-nested-path-segments/retry-budget-tuning'` (**95 chars**) on push | 22ch ellipsis + `title` recovery |
| `?gitBidiTarget` | `'origin/ma‮in'` (write the escape, not the literal char) fed through `mockActivityTarget` | proves the funnel is modeled; the emitted string must be exactly `origin/main` |
| `?pushSlow` | unchanged + a target | immutability across a 1500 ms Network phase |

**Corrected 2026-09-10 — the `?gitLongTarget` literal.** This row previously read
`'…/with-many-segments/retry-budget-tuning'` and described it as `(≥90 chars)`; that literal was an
elided illustration and measured **83**, so it failed its own stated constraint. The 95-char string
above is what shipped (`MOCK_LONG_TARGET` in `src/ipc/mock/gitActivity.ts`) and what ui-designer
records in `P87b-FU1-FU4-git-dock-ui.md` §3.10. §9.10 depends on the length: it compares row heights
for a ref that must overflow the 22ch box several times over. The leaf must stay
`retry-budget-tuning` — the leaf is what P111's split protects, and at 19 chars it fits the 22ch box
while the head needs roughly 4× the space, which is exactly the case §9.10 exercises.

Call sites to update (7): `handlers/remotesSync.ts:58,62,66,73`, `handlers/status.ts:145`,
`handlers/merge.ts:83`, `handlers/stash.ts:148`. **Note:** `stash.ts:148` *is* activity-wrapped at
HEAD (with `handlers/amendActivity.test.tsx` covering it), so ui-designer's flag **F-E**
("`commitAmend` is still not activity-wrapped") does not hold as of `1be3a85`; amend needs a target
like the rest. Flagged **F-4**.

---

## 9. Acceptance criteria (AI gate — **no USER CHECKPOINT item**)

**Rust** (`cargo nextest -p bonsai-core -p bonsai`):
1. `activity_target_tests::resolves_table` — one fixture repo per §4 row: push with upstream, push
   without upstream but with `origin`, push with neither, force-push, pull, fetch, commit on a
   branch, commit detached, commit unborn, merge. Each asserts the exact `Option<&str>`. Includes the
   `renamed` fixture (local `main` → `upstream/trunk`), asserted for **Pull and Push** — the Push
   assertion is the only thing pinning the resolver's use of `branch.<x>.merge` (§3).
2. `activity_target_tests::push_target_agrees_with_push_result_remote` — anti-drift: on a fixture
   where `push_current_with_activity` returns `PushResult::UpToDate { remote, branch }`,
   `resolve_activity_target(.., Push) == Some(format!("{remote}/{branch}"))`. This pins the **remote
   name**; it can only compare the whole string because that fixture's local and upstream branch
   names are equal (`UpToDate.branch` is the LOCAL branch). The upstream branch for Push is pinned
   separately, by `resolves_table`'s `renamed` fixture — see §3. *(Corrected 2026-09-10: this item
   and §3 previously named the test `push_target_matches_push_result`, which overstated what
   `PushResult` can witness and would have failed falsely on a divergent upstream.)*
3. `activity_target_tests::resolver_never_panics_on_broken_repo` — non-repo path, repo with a
   corrupt HEAD, and a path that does not exist all return `None`.
4. `activity_tests::{target_strips_bidi_and_zero_width, target_capped_at_255_chars,
   target_appears_only_on_started, target_none_omits_the_field}` — the last asserts serde omits
   `target` entirely when `None`.
5. `commands::activity::tests` still green with the new parameter; the inactive-hub passthrough still
   performs **zero** repo opens (assert via a non-existent workdir producing no error).

**Frontend** (`pnpm vitest`):
6. `useGitActivity.test.tsx::target_survives_later_events` (§7) and a `started` without `target`
   yielding `run.target === null`.
7. `gitActivityFormat.test.ts` — §3.3's table, incl. `fetch` + `null` → `all remotes`, and
   `runRowName` for §3.7's six rows (ui-designer's half; listed so the gate is complete).
8. Fixture guard: no mock target string contains a space, `→`, `'`, or `"` (guarantee 1, mock
   side); `mockActivityTarget('origin/ma‮in') === 'origin/main'`.

**Browser harness** (`pnpm dev`, `VITE_MOCK_IPC=1`) — all assertions read via `javascript_tool`, no
screenshot required:
9. `?gitBidiTarget` + push → `document.querySelector('.git-run-target').textContent` is exactly
   `'origin/main'`, and
   `!/[​-‏‪-‮⁦-⁩﻿]/.test(el.textContent)` is `true`.
   **This is the §3.10 bidi proof.**
10. `?gitLongTarget` + push → `.git-run-target`'s `title` equals the full ≥90-char ref, and its row's
    `offsetHeight` equals the `offsetHeight` of a `?gitNoTarget` row (no reflow, no wrap).
11. `?gitNoTarget` + commit → no `.git-run-target` node exists in the row, and the row's text contains
    none of `(none)`, `—`, `unknown`.
12. `?fetchAll` → the row reads `Fetch all remotes` (DOM text) — the derived string, proving the
    backend/mock sent no human phrase.
13. `?pushSlow` → sample `.git-dock-target` `textContent` at t≈200 ms and t≈1400 ms of the Network
    phase: byte-identical (guarantee 2, observable).

**Static:** `cargo clippy -D warnings`, `tsc`, `eslint` clean; no file crosses 500 lines
(`activity.rs` lands ~430).

---

## 10. Flags for the orchestrator

- **F-1 (unborn HEAD).** Spec'd as `null` per §3.6-1, so a repo's first commit renders a bare
  `Commit`. `HEAD`'s symbolic target *does* name the branch-to-be — and `read_head_info` already
  returns it — so `Commit main` is available by dropping `head.unborn ||` from the §4 guard.
  Recommend shipping `null` as specced; raise with ui-designer only if the empty-repo
  onboarding flow wants it.
- **F-2 (the rule is implemented twice — Rust and mock).** Unavoidable: §3.10's `?gitBidiTarget` seam
  can only be green if the mock strips, and the mock never runs Rust. Mitigated by the existing
  `MAX_ACTIVITY_LINE_CHARS` mirroring precedent (`gitActivity.ts:66`) plus acceptance items 8 and 9,
  which fail loudly if the mirror is dropped. Recommend accepting it; the alternative
  (frontend-side stripping) would mask a real backend regression.
- **F-3 (one §3.3 row is unreachable).** `Fetch origin` cannot occur — the only fetch entry point is
  fetch-all. The copy is correct and `ActivityTarget::remote()` is spec'd for it, but until a
  per-remote fetch command exists that row is mock-only. Not a defect in either contract; noted so
  nobody hunts for the missing code path.
- **F-4 (ui-designer's F-E is stale).** `commitAmend` **is** activity-wrapped at HEAD
  (`src/ipc/mock/handlers/stash.ts:148`, `src-tauri/src/commands/staging.rs:177`), with
  `handlers/amendActivity.test.tsx` covering it. Amend is in §4's table like any other category. A
  senior-dev is mid-change in `stash.ts` — confirm before editing.
- **F-5 (sequencing).** This contract is independent of FU-3; ui-designer's F-C recommendation to land
  FU-1 + FU-3 in one senior-dev pass still stands, and this backend half can land first or together.
  Nothing here blocks on `RefLabel` / P111.
- **Nothing in §3.6's four guarantees is unmeetable as stated.** Three are structural; guarantee 1 is
  structural at the crate boundary and test-backed inside `bonsai-core`. §6 says precisely where the
  line falls.
