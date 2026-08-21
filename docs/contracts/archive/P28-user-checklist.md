# P28 — "What changed" digest — USER CHECKPOINT checklist

Native app (`pnpm tauri dev`), real `claude` CLI on PATH, AI enabled + consented in settings.
Use a real repo with history (e.g. this Bonsai repo is fine — **the digest is read-only /
write-free**, so no scratch repo is required). Any git *experimentation* you do to set up ranges
(creating branches, resets, etc.) must still happen in a SCRATCH repo, never a real one.

## 1. Between two real refs

- [ ] Fetch first, then click **"✨ What changed…"** in the toolbar → dialog opens with three modes.
- [ ] Mode "Between refs": from `origin/main`, to `main` (or any two branches that differ).
      Submit → `AiOutputPanel` shows a loading spinner, then plain-English prose.
- [ ] Digest names real files/areas and (if several contributors) real author names; cost shown.
- [ ] Title reads `What changed: origin/main..main`.
- [ ] A tag also works as `from` (e.g. an old release tag → expect a longer digest).

## 2. Last N days

- [ ] Mode "Last N days", default 7 → plausible narrative of the last week on the current branch
      (mainline only — side-branch-only commits should not dominate).
- [ ] Title reads `What changed: last 7 days`.
- [ ] N = 0 is rejected (input min 1 or a clear `invalidName` error, never a crash).

## 3. Since a pasted commit

- [ ] Copy a short oid of an older commit (`git log --oneline`), mode "Since commit", paste it →
      digest covers that oid..HEAD; title `What changed since <short7>`.
- [ ] Garbage input (e.g. `not-a-ref`) → clear `git` error banner in the panel, app stays usable.

## 4. Empty range & truncation

- [ ] Same ref twice (e.g. `main` → `main`) → error banner
      "no changes in the selected range"; no CLI spinner beforehand.
- [ ] Big-range truncation note: pick a very large range (e.g. first commit → HEAD). The digest
      still returns; the commit list is capped at 200 (older ones collapse to "... and N more
      commits") and very large diffs are byte-capped at 256 KiB with a truncation note in the
      payload — so the narrative may say the diff was truncated. This is expected, not a bug.

## 5. AI disabled gating

- [ ] Disable AI in settings (or revoke consent) → the "✨ What changed…" toolbar button is
      hidden/blocked like the other ✨ actions; no way to trigger a digest.
- [ ] Re-enable → button returns without restart.

## 6. Sanity

- [ ] `git status` in the repo after all runs shows NO changes made by the digest (write-free).
- [ ] Cancel button in the dialog closes it without any request being sent.
