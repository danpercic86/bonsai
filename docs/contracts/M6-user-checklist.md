# M6 — USER CHECKPOINT checklist (remotes: fetch / pull / push)

Run these steps yourself in the native app. The AI gate (CLI-oracle remote tests over LOCAL bare
repos + adversarial probes, full `cargo test`, `pnpm build`, browser-harness smoke) has already
passed; this checklist covers what only a human can verify. It has two halves:

- **Part A (local bare remote)** — button/busy/notice/error UI against a scratch repo. No
  network, no credentials.
- **Part B (real network remote)** — **the actual M6 USER CHECKPOINT**: one round-trip against a
  remote of YOURS with the Git credential helper. This is the one thing the AI cannot test — the
  local transport never invokes the credentials callback, so the helper/agent path is exercised
  ONLY here.

M6 is done only when both parts behave as described. Everything local stays on **D:**.

## Part A — local bare remote (UI behavior)

### A1. One-time setup (PowerShell)

Builds: a bare `origin`, a `publisher` clone (simulates "someone else" pushing), and the repo
Bonsai opens — cloned from the bare so `origin` + upstream tracking are already configured, one
commit **behind** upstream, plus a local branch `feature/local` with **no upstream**. Safe to
re-run; it deletes and recreates all three folders.

```powershell
$repo = 'D:\Temp\bonsai-scratch\m6-checkpoint'
$bare = 'D:\Temp\bonsai-scratch\m6-checkpoint-remote.git'
$pub  = 'D:\Temp\bonsai-scratch\m6-checkpoint-publisher'
foreach ($p in @($repo, $bare, $pub)) { if (Test-Path $p) { Remove-Item -Recurse -Force $p } }

git init --bare -b main $bare
git clone $bare $pub
git -C $pub config user.name  "Publisher"
git -C $pub config user.email "publisher@example.com"
git -C $pub config core.autocrlf false
git -C $pub checkout -B main
Set-Content "$pub\hello.txt"  "hello v1"
Set-Content "$pub\shared.txt" "shared v1"
git -C $pub add -A
git -C $pub commit -m "base commit"
git -C $pub push -u origin main

# The repo Bonsai opens: cloned AT the base commit...
git clone $bare $repo
git -C $repo config user.name  "Checkpoint User"
git -C $repo config user.email "checkpoint@example.com"
git -C $repo config core.autocrlf false
git -C $repo branch feature/local        # no upstream — for the Push-sets-upstream step

# ...then the publisher moves upstream ahead by one (repo is now behind 1).
Set-Content "$pub\hello.txt" "hello v2 (upstream update)"
git -C $pub add -A
git -C $pub commit -m "upstream update 1"
git -C $pub push origin main
```

### A2. Launch + toolbar renders

```powershell
cd D:\Repos\Playground\bonsai
pnpm tauri dev
```

- [ ] Open `D:\Temp\bonsai-scratch\m6-checkpoint`. The header shows, LEFT of the Refresh button,
      three buttons: **↓ Fetch**, **⇣ Pull**, **↑ Push**.
- [ ] Hover Push → tooltip `Push main to origin/main` (upstream is configured).
- [ ] Sidebar BRANCHES: `feature/local`, `main` (current). REMOTES: `origin/main`. `main` shows
      **no** ahead/behind badge yet (the upstream move hasn't been fetched).

### A3. Fetch

- [ ] Click **Fetch** → all three buttons (and Refresh) disable briefly (busy), then a notice
      line appears under the header: `Fetched 1 remote — 1 ref updated`.
- [ ] `main` now shows a **`↓1`** badge; the graph shows the `origin/main` pill one commit ahead
      of `main`.
- [ ] The notice disappears on its own after ~5 s.
- [ ] Click **Fetch** again → `Fetched 1 remote` (no "ref updated" suffix); nothing else changes.

### A4. Pull — fast-forward, then up to date

- [ ] Click **Pull** → notice `Fast-forwarded main to <short-oid>`; the header oid changes; the
      `↓1` badge clears; `main` and `origin/main` pills sit on the same commit.
- [ ] `Get-Content 'D:\Temp\bonsai-scratch\m6-checkpoint\hello.txt'` prints
      `hello v2 (upstream update)` (the worktree really moved).
- [ ] Click **Pull** again → notice `Already up to date`.

### A5. Divergence — wouldNotFastForward warning + push rejection

Create real divergence: a local commit in Bonsai AND a different upstream commit.

- [ ] In Bonsai: create `local-note.txt` in the repo folder
      (`Set-Content 'D:\Temp\bonsai-scratch\m6-checkpoint\local-note.txt' "local work"`),
      refocus, stage it, commit `local note` → `main` shows **`↑1`**.
- [ ] In PowerShell (publisher pushes a competing commit):
      ```powershell
      $pub = 'D:\Temp\bonsai-scratch\m6-checkpoint-publisher'
      Set-Content "$pub\upstream2.txt" "upstream work 2"
      git -C $pub add -A
      git -C $pub commit -m "upstream update 2"
      git -C $pub push origin main
      ```
- [ ] Click **Fetch** → badge becomes **`↑1 ↓1`**.
- [ ] Click **Pull** → a **warning-tinted** notice (not an error banner):
      `Cannot fast-forward: 'main' has 1 local commit(s) not on upstream. Bonsai v1 does not
      merge — push your commits or reconcile via the CLI.` Nothing changed: badge still
      `↑1 ↓1`, header oid unchanged, `upstream2.txt` does NOT exist in the repo folder.
- [ ] Click **Push** → a red **error banner**: `push rejected: the remote contains commits you
      do not have. Fetch/pull first — Bonsai v1 never force-pushes.` Dismiss it with its ✕.
- [ ] Reconcile via CLI (scratch repo only):
      `git -C 'D:\Temp\bonsai-scratch\m6-checkpoint' pull --rebase origin main` → refocus →
      badge shows **`↑1`** (local commit rebased on top, no longer behind).
- [ ] Click **Push** → notice `Pushed main → origin/main`; the `↑1` badge clears.
- [ ] Verify the commit really landed:
      `git -C 'D:\Temp\bonsai-scratch\m6-checkpoint-remote.git' log --oneline -1` shows
      `local note`.

### A6. Push sets upstream on a new branch

- [ ] Checkout `feature/local` in the sidebar. Hover Push → tooltip
      `Push feature/local to origin/feature/local and set upstream`.
- [ ] Click **Push** → notice `Pushed feature/local → origin/feature/local (upstream set)`;
      REMOTES gains `origin/feature/local`; the branch shows no ahead/behind badge (0/0).
- [ ] Checkout `main` again.

### A7. Dirty pull is blocked cleanly

- [ ] Publisher pushes a change to `shared.txt`:
      ```powershell
      $pub = 'D:\Temp\bonsai-scratch\m6-checkpoint-publisher'
      git -C $pub pull --rebase origin main
      Set-Content "$pub\shared.txt" "shared v2 (upstream)"
      git -C $pub add -A
      git -C $pub commit -m "upstream touches shared"
      git -C $pub push origin main
      ```
- [ ] Locally dirty the SAME file (uncommitted):
      `Set-Content 'D:\Temp\bonsai-scratch\m6-checkpoint\shared.txt' "local uncommitted edit"`
      → refocus; `shared.txt` appears under Unstaged.
- [ ] Click **Pull** → error banner: `cannot pull: local changes would be overwritten by the
      update. Commit or discard them first.` Nothing changed: header oid unchanged, `shared.txt`
      still contains `local uncommitted edit`, still listed as Unstaged.
- [ ] Discard: `git -C 'D:\Temp\bonsai-scratch\m6-checkpoint' restore shared.txt` → refocus →
      click **Pull** → `Fast-forwarded main to <short>`; `shared.txt` now reads
      `shared v2 (upstream)`.

### A8. Broken second remote fails fast

- [ ] `git -C 'D:\Temp\bonsai-scratch\m6-checkpoint' remote add aaa-broken 'D:\Temp\bonsai-scratch\no-such-remote.git'`
- [ ] Click **Fetch** → an error banner appears: `network error talking to 'aaa-broken':
      failed to resolve address for D: ...` (libgit2 parses the nonexistent `D:\` path as an
      scp-style host — observed and pinned in the adversarial tests); `origin` was NOT fetched
      (fail-fast). The app does not hang or crash. Dismiss the banner.
- [ ] `git -C 'D:\Temp\bonsai-scratch\m6-checkpoint' remote remove aaa-broken` → Fetch works
      again.

## Part B — REAL network remote (the M6 USER CHECKPOINT)

Use a remote **of your own** — best: create a brand-new empty private repo on GitHub (or your
Git host) just for this test, so nothing real is at risk. Bonsai **never asks for credentials**:
it delegates to your configured Git credential helper (HTTPS) or SSH agent. No prompt of any
kind should ever appear inside Bonsai.

Pre-flight (once): `git config --global credential.helper` should print `manager` (Git
Credential Manager, the Git-for-Windows default) — and a plain CLI `git fetch` in the clone must
work without an interactive password prompt. If the CLI prompts, fix that first; Bonsai cannot
work around a missing helper.

- [ ] **Clone + open:** `git clone <your-remote-url> D:\Temp\bonsai-scratch\m6-real` (empty repo
      is fine), set `user.name`/`user.email` if needed, open it in Bonsai.
- [ ] **Fetch:** click **Fetch** → completes with a notice, no credential prompt, no hang. (On
      an empty repo: `Fetched 1 remote`.)
- [ ] **Push:** create a file, stage, commit in Bonsai, click **Push** → notice
      `Pushed <branch> → origin/<branch>` (with `(upstream set)` on an unborn-remote first
      push); the commit is visible on the host's web UI; the `↑` badge cleared. No credential
      prompt appeared in Bonsai at any point (the helper may have been used silently; a
      first-ever GCM authentication can open a BROWSER window — that is the helper, not Bonsai,
      and is OK).
- [ ] **External change + pull:** edit a file directly on the host's web UI (creates an upstream
      commit) → in Bonsai: **Fetch** (badge `↓1`) → **Pull** → `Fast-forwarded ...`; the change
      is in the worktree.
- [ ] **Divergence warning over the network:** make another web-UI edit AND a local Bonsai
      commit → Fetch → **Pull** shows the warning notice (`Cannot fast-forward: ...`), changes
      nothing; **Push** shows the `push rejected` error banner. Reconcile via CLI
      (`git pull --rebase`), then Push succeeds.
- [ ] **Bogus URL yields networkError, not a hang:**
      `git -C D:\Temp\bonsai-scratch\m6-real remote add bogus https://bonsai-does-not-exist.invalid/x.git`
      → **Fetch** → error banner `network error talking to 'bogus': ...` within a reasonable
      time; app stays responsive; dismiss, then
      `git -C D:\Temp\bonsai-scratch\m6-real remote remove bogus`.
- [ ] **Busy behavior:** during the network ops, Fetch/Pull/Push/Refresh were disabled and
      re-enabled afterwards; no double-fire from impatient clicking.

**If auth fails** (an `authentication failed for 'origin': no usable credentials...` banner):
Bonsai intentionally has no fallback prompt. Check `git config credential.helper`, confirm plain
`git fetch` works non-interactively in the same clone, and for SSH URLs confirm an SSH agent is
running with your key loaded (`ssh-add -l`). Then retry in Bonsai.

## Cleanup

Close the app (Ctrl+C in the `pnpm tauri dev` terminal), then:

```powershell
Remove-Item -Recurse -Force 'D:\Temp\bonsai-scratch\m6-checkpoint'
Remove-Item -Recurse -Force 'D:\Temp\bonsai-scratch\m6-checkpoint-remote.git'
Remove-Item -Recurse -Force 'D:\Temp\bonsai-scratch\m6-checkpoint-publisher'
Remove-Item -Recurse -Force 'D:\Temp\bonsai-scratch\m6-real'   # if created
```

Delete the throwaway remote repo on your Git host if you created one. Leftover `bonsai-*`
folders under `D:\Temp\bonsai-scratch` are abandoned test temp dirs and can be deleted anytime.
