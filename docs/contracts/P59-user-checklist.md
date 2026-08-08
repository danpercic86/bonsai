# P59 — Git hooks execution + force-push-lease hardening — USER CHECKPOINT checklist (native-only)

These items require the native Tauri window, **real-world hook managers** (Husky / `pre-commit` /
lint-staged / gitleaks), a **real remote** (a throwaway GitHub/GitLab repo you own), your **real
credential helper**, and human perception of the `HookOutputDialog` — they CANNOT be self-declared by
the orchestrator. The AI gate proves the **mechanism** hermetically (real `#!/bin/sh` hooks run via
`git hook run` on git 2.51, and git's atomic `--force-with-lease` against a local `file://` bare
remote); it CANNOT prove that *your* Husky/gitleaks config wires up correctly, that a *real* remote
refuses a stale lease, that *your* credential helper answers with no prompt, or that the dialog's
output reads legibly to a human.

Run via `pnpm tauri dev`. Hooks genuinely execute and commits/pushes genuinely happen, so use a
**scratch repo** and a **throwaway remote**. `bonsai.runHooks` (default true) + the per-commit "Skip
hooks" checkbox mirror `git commit --no-verify`. Bonsai NEVER prompts for credentials — any prompt is
your own git credential helper; the force-push lease shells `git push` under the same never-prompt
policy (`GIT_TERMINAL_PROMPT=0`, `-c core.askpass=`) used for reads.

> SAFETY REMINDER: use a scratch repo + a throwaway remote you own. Note each branch tip before you
> commit / force-push so you can `git reflog` back. NEVER force-push-with-lease against a real shared
> project while testing.

## Pixel-free note (no canvas visuals this milestone)

Unlike P58 (the on-canvas signature badge), P59 adds **no canvas/pixel** UI. The only new surfaces are
DOM: the `HookOutputDialog` modal, the CommitBox "Skip hooks" checkbox, and the Settings "Run git
hooks" toggle. Those are **overlay dialogs the 0×0 headless harness cannot meaningfully drive or
perceive**, so their real interaction + legibility is the native part below. The mock seams
`?hooks=fail`, `#hookfail` (message sentinel), `?hooks=failpush`, and `?remote=leasefail` exercise the
underlying DATA paths (rejection surfaces as `hookRejected`; the skip-hooks retry succeeds; the lease
refuses) — that is what the AI gate covers.

## Already proved by the AI gate (do NOT re-verify manually)

- **Hooks oracle** (`cargo test -p bonsai-core hooks`, 12 tests) with **real hook scripts on git 2.51,
  Windows** — this is the whole point of running hooks via `git hook run` (A-D1), so the Windows
  `#!/bin/sh` shell path is already exercised hermetically:
  - a failing `pre-commit` (`echo … >&2; exit 1`) ⇒ `HookRejected` carrying the hook's output; `HEAD`
    UNCHANGED (no commit);
  - a passing `pre-commit` (`exit 0`) ⇒ commit succeeds;
  - a `commit-msg` hook that appends `Signed-off-by:` ⇒ the committed message contains it (rewrite);
  - a failing `commit-msg` ⇒ `HookRejected`, no commit;
  - `post-commit` that `exit 1` ⇒ commit STILL succeeds (`HEAD` moved), `HookRunInfo.success == false`
    captured (non-blocking);
  - `core.hooksPath` pointed at a sibling dir whose hook fails ⇒ blocks (discovery is git's, not a
    hardcoded `.git/hooks`);
  - opt-out: `bonsai.runHooks=false` and (separately) `skip_hooks=true` ⇒ a failing `pre-commit` is NOT
    run, commit succeeds;
  - re-stage: a `pre-commit` that `git add`s a file ⇒ the reloaded index includes it in the tree.
  - Pure units: `build_hook_run_args` exact argv (with/without `--to-stdin`, args after `--`);
    `hooks_enabled` truth table (`skip` × `bonsai.runHooks` true/false/unset).
- **Force-push lease oracle** (`cargo test -p bonsai-core --test force_push_cli`, 12 tests) against a
  local `file://` bare remote (no creds ⇒ hermetic):
  - **A. lease refuses** — a 3rd-party push advanced `origin` to Y; rewriting locally to Z ⇒
    `Err(PushRejected)` from git's OWN stale-info stderr; the remote ref is UNCHANGED (git's atomic
    check, not our old client-side compare);
  - **B. lease succeeds** — no third-party push ⇒ `Ok(Pushed{set_upstream:false})`, remote == Z;
  - **C. up-to-date** — baseline == local tip ⇒ `Ok(UpToDate)` with **no git spawn** (asserted via a
    fake `GitExec` that panics if called);
  - **D/E** — `NoUpstream` / no-baseline `PushRejected` asserted before any push; plus a `pre-push`
    skip case; pure `build_force_push_args` + `classify_push_stderr` (stale/rejected ⇒ `PushRejected`,
    auth ⇒ `AuthFailed`, connect ⇒ `NetworkError`, else ⇒ `Git`).
- **`remote_cli` (20 tests)** regression stayed green (push/pre-push plumbing unaffected).
- **No new IPC command** (per contract): `commit` / `commitAmend` / `commitMerge` / `push` /
  `forcePush` only gained an optional `skipHooks` param; `forcePush`'s signature is unchanged.
- **Browser-harness seams** (`VITE_MOCK_IPC=1`): `?hooks=fail` / a `#hookfail` message sentinel ⇒
  `hookRejected` (drives `HookOutputDialog`); the "Commit anyway (skip hooks)" retry re-invokes with
  `skipHooks:true` and succeeds; `?hooks=failpush` ⇒ pre-push rejection + "Push anyway"; the "Run git
  hooks" toggle round-trips `bonsai.runHooks` through the existing `read_config`/`set_config`;
  `?remote=leasefail` ⇒ lease refuse. The mock gate truth table + the load-bearing hook-output
  prefixes that drive the dialog's Commit/Push-anyway label are locked by
  `src/ipc/mock/hooksGate.test.ts` (12 tests).

So below is strictly what **real hook managers + a real remote + human perception** must confirm.

---

## A. Commit hooks — the headline flow (real hook manager, native repo)

Set up a scratch repo with a real hook manager, e.g. `pre-commit` (`pip install pre-commit &&
pre-commit install`) with a `gitleaks` / `end-of-file-fixer` / lint hook, or Husky + lint-staged, or
commitizen/gitmoji as a `commit-msg` hook. Confirm `git commit` on the CLI runs them first.

- [ ] **Failing pre-commit BLOCKS + shows real output.** Stage a change that trips a hook (e.g. a
      planted secret for gitleaks, or a lint error). Commit in Bonsai → the commit is **blocked**, the
      `HookOutputDialog` opens, and the tool's **own output** (the gitleaks/lint report) is rendered
      **legibly** in a scrollable preformatted block; `git log`/`git rev-parse HEAD` shows **no new
      commit**. *(AI gate proved the block + `HookRejected` message plumbing; native adds a real tool's
      output + human legibility.)*
- [ ] **"Commit anyway (skip hooks)" then commits.** In that dialog, click **Commit anyway (skip
      hooks)** → the commit now succeeds with the hook bypassed (≡ `--no-verify`). *(AI-gate: harness
      proved the `skipHooks:true` retry path.)*
- [ ] **Passing run commits.** Fix the offending change (or use a clean change) → committing runs the
      hooks and **succeeds** with no dialog.
- [ ] **commit-msg rewrite reflects in the message.** With a `commit-msg` hook that rewrites the
      message (commitizen/gitmoji, or an `add-msg-hook` that appends `Signed-off-by:`), commit a plain
      message → the **created commit's message shows the rewrite** (`git log -1 --format=%B`). *(AI
      gate proved the rewrite mechanism hermetically; native confirms your real hook.)*
- [ ] **post-commit runs but never blocks.** With a `post-commit` hook (e.g. one that fails or prints)
      → the commit **still succeeds** and the post-commit failure never turns into an error/rollback.

## B. Toggle + skip + settings behavior

- [ ] **"Skip hooks" checkbox (per-commit).** With a failing hook present, tick the CommitBox **"Skip
      hooks"** checkbox BEFORE committing → the commit **skips the hooks and succeeds** directly (no
      dialog). Un-tick → the failing hook blocks again.
- [ ] **"Run git hooks" toggle (per-repo).** In Settings, turn the **"Run git hooks"** toggle **off**
      → committing with a failing hook present **succeeds** (hooks not run); the value persists in the
      repo's `.git/config` as `bonsai.runHooks=false` (`git config --local --get bonsai.runHooks`).
      Turn it back **on** → the failing hook blocks again. *(AI-gate: harness proved the round-trip via
      `read_config`/`set_config`.)*

## C. Windows shell-script hooks (cross-platform perception)

- [ ] **Windows: a `#!/bin/sh` hook executes.** On **Windows**, a shell-script hook (what
      pre-commit/Husky install) runs correctly through git's bundled `sh` via `git hook run` — a
      failing one blocks, a passing one commits — with **no flashing console/cmd window** (the exec
      uses `CREATE_NO_WINDOW`). *(AI gate already ran a real `#!/bin/sh` hook via `git hook run` on git
      2.51/Windows; native confirms it with a REAL hook manager + no window flash to the eye.)* Repeat
      the failing/passing pair on **macOS** and **Linux** to confirm parity.

## D. Pre-push hooks (P59a-2)

Add a `pre-push` hook to the scratch repo (e.g. `gitleaks protect --staged` style, or a script that
reads the pushed refs from stdin and exits non-zero on a match). Point the repo at your throwaway
remote.

- [ ] **Failing pre-push BLOCKS + "Push anyway".** Push in Bonsai → the push is **blocked**, the
      `HookOutputDialog` shows the pre-push tool's output, and the primary action reads **"Push anyway
      (skip hooks)"** (not "Commit anyway"); the remote is **unchanged**. Clicking **Push anyway** then
      pushes. *(AI-gate: `?hooks=failpush` proved the reject + label + skip retry.)*
- [ ] **Passing pre-push pushes.** With the hook passing (or "Skip hooks"/`bonsai.runHooks=false`) →
      the push **succeeds** normally.

## E. Force-push-with-lease against a real remote (P59b)

Use a **throwaway remote you own**. Simulate a teammate: clone a 2nd working copy (or push from the
web UI) to advance the remote branch after Bonsai last fetched it.

- [ ] **Lease succeeds — publishes a rewrite.** Amend or rebase a local commit, then force-push (with
      the UI confirm). With NO third-party change since your last fetch, git's `--force-with-lease`
      **succeeds** and the remote branch shows your rewritten commit. *(AI gate proved case B against a
      local remote; native confirms against a REAL remote + your creds.)*
- [ ] **Lease REFUSES a moved remote — changes nothing.** Advance the remote branch from the 2nd copy
      (do NOT fetch it into Bonsai), then amend locally and force-push → git's **atomic** lease
      **refuses** with a **readable message** (the stale-info / "remote ref updated since checkout"
      text surfaces in the toast), and the remote branch is **UNCHANGED** (verify on the web UI / from
      the 2nd copy). *(AI gate proved case A hermetically; native confirms a real server enforces it.)*
- [ ] **No interactive credential prompt.** The whole force-push resolves credentials via **your
      configured git credential helper** with **NO prompt inside Bonsai** and no popup terminal — even
      the first time in a session. A missing/misconfigured helper surfaces as a clear
      auth-failed/`git`-error toast, not a hang. *(Validates the `-c core.askpass=` + never-prompt env
      on the real credential path.)*

---

## Sign-off
- [ ] A (real hook manager: failing pre-commit blocks + `HookOutputDialog` shows the tool's output
      legibly; "Commit anyway" then commits; passing run commits; commit-msg rewrite reflects;
      post-commit never blocks)
- [ ] B ("Skip hooks" per-commit checkbox bypasses; "Run git hooks" Settings toggle persists to
      `.git/config` and re-enables)
- [ ] C (Windows `#!/bin/sh` hook executes with no console-window flash; macOS + Linux parity)
- [ ] D (pre-push: failing hook blocks with "Push anyway (skip hooks)"; passing hook pushes)
- [ ] E (real remote: lease succeeds on a clean rewrite; atomically REFUSES a moved remote with a
      readable message and changes nothing; credentials via your helper with NO prompt)
