# P53 — AI "why" layer — USER CHECKPOINT checklist (native-only)

These items require the native Tauri window, a **real `claude` CLI**, and human judgement of AI output
quality — they CANNOT be self-declared by the orchestrator. The AI gate only proves argv/prompt
correctness (unit tests), grounding/parsing/error mapping (unit tests), and the **mock-driven** UI
wiring (browser harness with canned `aiHandlers`). Run via `pnpm tauri dev` against a **REAL repo**
with the real, authenticated `claude` binary on PATH and **AI consent enabled** in Settings.

## Already proved by the AI gate (do NOT re-verify manually)
- Unit tests: single-line blame targeting (`blame_line`), blame-why grounding shape
  (`LINE`/`COMMIT`/`MESSAGE`/file-block), bad-path → `invalidName`; commit-explain grounding now
  carries the full `MESSAGE:` body (`commit_payload_prefix_carries_full_message`); branch-name
  `sanitize`/`parse` rules, dedupe+cap, both `BranchNameSource` deserialize variants,
  `BranchNameProposal` camelCase wire shape, empty-grounding (clean worktree AND empty commit range)
  → `AiFailed` BEFORE any CLI spawn; all prompts single-line.
- Browser harness (`VITE_MOCK_IPC=1`): every entry point renders and is wired — blame gutter "Why?"
  button opens `AiOutputPanel`; graph-node "Explain this commit" opens `AiOutputPanel` titled
  `Explain commit <short7>`; branch-create "Suggest name ✨" is enabled only when the worktree is
  dirty, produces candidate chips, and a chip click fills the name field; `?ai=off` yields the
  disabled/inline-error states; the two IPC calls resolve with the documented shapes.

So the native checkpoint is about **real-model quality, real grounding, and real create/consent
behavior** — not whether the buttons exist.

## A. Blame "Why?" — real explanation quality (P53a)
- [ ] Open blame on a real, non-trivial source file; each blame gutter block shows a **"Why?"**
      affordance. Click one.
- [ ] The `AiOutputPanel` returns a 2–3 sentence **intent-focused** explanation of WHY that line
      exists — it references the introducing commit's purpose, **not** a restatement of the diff
      ("this line was changed to…"). Judge on a line whose reason isn't obvious from the code alone.
- [ ] The explanation is anchored to the **specific line/region**, not a summary of the whole
      (possibly multi-file) commit.
- [ ] Blame a line as of an **older revision** (blame overlay pinned to a non-HEAD `at_oid`): the
      explanation reflects the commit that introduced it at that revision.
- [ ] A cost estimate is shown in the panel; `Esc` closes the AI panel first, leaving the blame
      overlay open beneath (layering).

## B. "Explain this commit" from a graph node (P53b)
- [ ] Right-click a commit dot/row (and a branch/tag pill) → **"Explain this commit"** appears in the
      read-only group (after "Compare with HEAD", before Cherry-pick/Revert). Run it.
- [ ] The explanation **reflects the commit's own message/intent**, not just a mechanical file-by-file
      diff summary — pick a commit with a meaningful message and confirm the stated intent surfaces
      (this is the D2 `MESSAGE:` grounding enrichment doing its job with a real model).
- [ ] Panel title is `Explain commit <short7>`; a merge commit and a large-diff commit both return
      without erroring or hanging.

## C. "Suggest name ✨" — usable, creatable branch names (P53c)
- [ ] Open **New branch** with a **dirty** worktree (staged/unstaged/untracked changes). "Suggest
      name ✨" is **enabled**. Click it.
- [ ] 1–5 candidate chips appear; each looks like a sensible kebab-case name reflecting the actual
      change intent (e.g. `feat/…`, `fix/…`). Clicking a chip fills the name field.
- [ ] **Create the branch from a suggested (or lightly edited) name** — the existing create flow
      **accepts it** and the branch is created (proves every surfaced candidate is a valid, creatable
      git ref; the backend sanitizes, so no chip should ever be rejected by create).
- [ ] With a **clean** worktree, "Suggest name ✨" is **disabled** (no grounding to name from).
- [ ] Sanity: suggested names never contain spaces, uppercase, or illegal ref characters, and are
      never empty.

## D. Consent / AI-off — disabled + error states (real gate)
- [ ] Turn **AI consent OFF** in Settings: blame "Why?" and "Explain this commit" are disabled (or
      error clearly via the consent gate); branch "Suggest name ✨" is disabled. Nothing spawns the CLI.
- [ ] Remove/rename the `claude` binary from PATH (consent ON): triggering any of the three surfaces
      a clear `aiUnavailable`-style message ("Claude Code CLI not found …"), not a crash or silent
      no-op.
- [ ] Re-enable consent + restore `claude`: all three work again without restarting the app.

## E. Privacy — no code leaves the device except via your local CLI
- [ ] The ONLY egress path is the local `claude` CLI you already authenticated: with `claude` off
      PATH (Section D) the features are fully disabled/error — Bonsai has **no built-in or remote AI
      fallback**.
- [ ] (Optional, with a process/network monitor) When an AI action runs, **Bonsai spawns a local
      `claude` child process** and passes grounding via stdin/argv; Bonsai itself opens **no** network
      connection to any AI endpoint. Egress is identical to you running `claude` yourself.

## Sign-off
- [ ] A (blame-why quality)  - [ ] B (explain-commit reflects message)
- [ ] C (suggest names usable + creatable + clean-tree disabled)
- [ ] D (consent-off / CLI-missing states)  - [ ] E (privacy: local CLI only)
