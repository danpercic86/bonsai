# P37 — Force-push with lease — USER CHECKPOINT checklist

Native `pnpm tauri dev` verification. The AI gate (CLI-oracle tests, clippy, tsc/build,
browser harness) is verified by the orchestrator; these steps require the real Tauri
window + a real remote and are confirmed by the user.

**All work on a SCRATCH repo — never a real repository. No push/reset against anything
but the scratch clone.**

---

## Scratch setup (bare origin + working clone)

Run once in a throwaway location (e.g. `D:\Temp\bonsai-scratch\p37-manual`):

```sh
mkdir -p /d/Temp/bonsai-scratch/p37-manual && cd /d/Temp/bonsai-scratch/p37-manual
git init --bare -b main origin.git
git clone ./origin.git work
cd work
git config user.name  "Test User"
git config user.email "test@example.com"
printf 'hello\n' > hello.txt
git add -A && git commit -m "initial"
git push -u origin main            # origin/main now exists + is tracked
```

Open **`work`** in Bonsai (`pnpm tauri dev`, then open the folder). You should see the
`main` branch with an upstream and the Push control enabled.

---

## Step (a) — Rewrite local history, force-push succeeds

1. In `work`, rewrite the committed history (either works):
   - **Amend:** `printf 'more\n' >> hello.txt && git add -A && git commit --amend -m "initial (amended)"`, or
   - **Interactive rebase / reset:** drop or reword the tip so `HEAD` diverges from `origin/main`.
   In Bonsai, refresh (toolbar refresh or window focus) — the branch should now show as
   **diverged / non-fast-forward** vs `origin/main` (ahead and behind, or a rewrite marker).
2. Click the **caret (▾) beside the Push button** → **"Force-push with lease…"**.
3. The **danger confirm dialog** appears, naming the branch (`main`) and remote (`origin`)
   and warning it rewrites published history. Click **Force-push**.
4. Expect a **success toast** ("Force-pushed main → origin/main") and the ahead/behind
   counters clear to 0.
5. **Cross-check with real git** — the origin now holds the rewrite:
   ```sh
   git --git-dir=/d/Temp/bonsai-scratch/p37-manual/origin.git rev-parse main
   git -C /d/Temp/bonsai-scratch/p37-manual/work rev-parse HEAD
   # the two oids must be EQUAL
   git ls-remote /d/Temp/bonsai-scratch/p37-manual/origin.git refs/heads/main
   git --git-dir=/d/Temp/bonsai-scratch/p37-manual/origin.git log --oneline -3
   # log shows the rewritten message, not the pre-amend one
   ```

---

## Step (b) — Someone else pushed: lease is REFUSED

1. Simulate a teammate advancing the origin ref from a second clone (Bonsai must NOT
   fetch this in between):
   ```sh
   cd /d/Temp/bonsai-scratch/p37-manual
   git clone ./origin.git other
   cd other && git config user.email t2@example.com && git config user.name "Teammate"
   printf 'teammate\n' > t.txt && git add -A && git commit -m "teammate work"
   git push origin main            # origin/main advances; `work` has NOT fetched this
   ```
2. Back in `work`/Bonsai, rewrite the local tip again (amend as in step a) **without
   fetching**. Do NOT click refresh-then-fetch — the point is `work`'s remote-tracking
   ref is stale.
3. Caret → **"Force-push with lease…"** → confirm **Force-push**.
4. Expect a **refusal**: an **error toast** whose message says the branch **"has moved on
   the remote… fetch"** (with the "— fetch and retry" hint). No success.
5. **Cross-check** — the origin is UNCHANGED (still the teammate's commit, not your rewrite):
   ```sh
   git --git-dir=/d/Temp/bonsai-scratch/p37-manual/origin.git log --oneline -1
   # still shows "teammate work" — your force-push did NOT land
   ```

---

## Step (c) — After a fetch, the retry succeeds

1. In Bonsai, **Fetch** (toolbar). The graph/branch now shows the teammate's commit and
   the divergence updates.
2. Re-apply your local rewrite on top if needed (rebase onto the fetched tip / amend so
   the new tip is a genuine rewrite past the fetched baseline).
3. Caret → **"Force-push with lease…"** → confirm. Now the lease baseline matches the
   live remote → expect a **success toast**.
4. **Cross-check** — origin `main` now equals your local `HEAD`:
   ```sh
   git --git-dir=/d/Temp/bonsai-scratch/p37-manual/origin.git rev-parse main
   git -C /d/Temp/bonsai-scratch/p37-manual/work rev-parse HEAD    # equal
   ```

---

## Step (d) — No upstream: the caret is hidden/disabled

1. In `work`, create a branch with **no upstream** and check it out:
   ```sh
   git -C /d/Temp/bonsai-scratch/p37-manual/work checkout -b local-only
   ```
2. Refresh Bonsai. On `local-only` (no upstream), the **caret / "Force-push with lease…"
   action is disabled** (tooltip: "Force-push needs a branch with an upstream."). It must
   be impossible to force-push a branch that has no upstream.
3. Switch back to `main`; the caret becomes enabled again.

---

## Pass criteria

- (a) Force-push publishes the rewrite; `git` on the bare origin shows the rewritten oid/log.
- (b) With the origin advanced (unfetched), the lease **refuses** with a readable
  "has moved / fetch" message and the origin is **unchanged**.
- (c) After fetch, the retry **succeeds** and the origin matches local `HEAD`.
- (d) The force-push action is **disabled** on a branch with no upstream.
- Confirm-dialog wording is clear (names branch + remote, warns about rewriting published
  history); the fetch-and-retry hint on refusal is actionable.

## Cleanup

```sh
rm -rf /d/Temp/bonsai-scratch/p37-manual
```
