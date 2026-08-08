# P58 — Real commit signing + verification — USER CHECKPOINT checklist (native-only)

These items require the native Tauri window, the user's **real key material** (an SSH signing key
and/or a GPG key with its agent), and human perception of the on-canvas badge — they CANNOT be
self-declared by the orchestrator. The AI gate proves the **plumbing** end-to-end with an ephemeral,
hermetic SSH key and mock verify statuses; it CANNOT prove that *your* real key signs, that a real
remote (GitHub/GitLab) shows "Verified", that a real GPG agent prompt behaves, or that the badge
glyphs read correctly to a human (the headless harness renders the graph pane at 0×0, so badge
VISUALS are unverifiable there).

Run via `pnpm tauri dev` against a **scratch/throwaway repo** (signing genuinely creates commits).
`gpg.format` + `user.signingkey` come from your git config (global or repo-local); the CommitBox
"Sign commit" toggle defaults to the effective `commit.gpgsign`. Bonsai NEVER prompts for a
passphrase — any prompt is your own gpg/ssh agent, and a locked/failed agent surfaces as a plain
`git` error toast.

> SAFETY REMINDER: use a scratch repo, not a real project. Note each branch tip before you commit so
> you can `git reflog` back. NEVER push/reset against a real repo while testing.

## Already proved by the AI gate (do NOT re-verify manually)

- **SSH signing is fully hermetic + green** (`cargo test -p bonsai-core signing`, ~33 tests): an
  ephemeral empty-passphrase ed25519 key signs a real commit via `git commit-tree -S` + `update-ref`;
  `git verify-commit` exits 0; `git log --format=%G?` returns `G`/`U`; author/committer identity and
  the `commit:` reflog entry match; **amend preserves the original author + author-date and re-signs**.
- **Unsigned path is byte-identical to pre-P58** — a `sign=None` + `commit.gpgsign` unset commit has
  **no `gpgsig` header** (asserted via `git cat-file`); `commit.gpgsign=true` ⇒ signed;
  `sign=Some(false)` overrides `gpgsign=true` ⇒ unsigned.
- **Config gate:** SSH signing requested with no `user.signingkey` ⇒ `ConfigMissing` (naming the key),
  **NO commit created** — the exact "missing key" backend behavior.
- **Verification units:** `map_status_code` full table (`G/U/B/X/Y/R/E/N`), `parse_verify_output`
  (US-record split, empty signer/key → None), `build_verify_args` (exact argv, non-hex oids dropped),
  empty-oids ⇒ no spawn, and the wholesale-degrade path (verification impossible ⇒ `CannotCheck`,
  never an error). Wire-shape tests lock `VerifyStatus`/`CommitVerification`/`SigningStatus` to the
  camelCase TS mirror.
- **Frontend pure mapping** (`src/graph/verifyBadge.test.ts`): every `VerifyStatus` → its OQ7 badge
  bucket (`good`/`warn`/`unknown`/blank) and its panel label. This is the single source shared by the
  canvas badge and the CommitPanel line, so they cannot drift.
- **Browser harness (`VITE_MOCK_IPC=1`):** `signingStatus` + `verifyCommits` are mockable; `?sign=ssh`
  flips the commit-box indicator to "will sign (SSH)"; mock statuses light each badge bucket;
  `showSignatureBadge=false` stops verify requests. (Mock fixtures carry NO real signatures — the
  crypto verdict is the native part below.)

So below is strictly what **real keys + a real remote + human perception** must confirm.

## A. SSH signing — the headline flow (real SSH key)

Set up (global or repo-local): `git config gpg.format ssh`; `git config user.signingkey <your key>`
(a private key path, or a literal `ssh-ed25519 …` public key when using an agent); optionally
`git config commit.gpgsign true`. If you want a **green** (`good`) badge rather than
"signed, unverified signer", also configure `gpg.ssh.allowedSignersFile` naming your key.

- [ ] The CommitBox shows a **"Sign commit"** toggle; with `commit.gpgsign=true` it is **on by
      default**, and the hint reads **"Commits will be signed (SSH)"**.
- [ ] Stage a change and commit (toggle on). It succeeds with no console-window flash (see E).
- [ ] `git log --show-signature -1` reports a **good ssh signature** for the new HEAD commit; the
      commit object carries a `gpgsig` header (`git cat-file -p HEAD` shows it).
- [ ] Bonsai's graph badge on that row is a **green filled check** (with `allowedSignersFile`) or a
      **solid neutral disc** = "signed, unverified signer" (without it) — matching `%G?` `G` vs `U`.
- [ ] Selecting that commit shows a **signature line in the CommitPanel**: the matching icon + status
      text (`Good signature` / `Signed, unverified signer`) + the signer and short key.
- [ ] Push to GitHub/GitLab → the web UI shows **"Verified"** for that commit (real remote-side
      proof; the AI gate cannot reach a remote).

## B. GPG / OpenPGP signing (real GPG key — native-only; CI throwaway keys are flaky)

Set up: `git config gpg.format openpgp` + a real secret key (`user.signingkey <keyid>` or let git
pick by committer email). Have your gpg-agent unlocked or ready to prompt.

- [ ] Commit with signing on. If the key needs a passphrase, **your gpg-agent prompts** (a pinentry
      dialog / terminal) — **Bonsai itself never prompts**. Enter it there; the commit completes.
- [ ] `git log --show-signature -1` reports a **good gpg signature**; badge + CommitPanel line read
      correctly (green check / signer / key), same as SSH.
- [ ] **Locked/failed agent:** cancel the passphrase prompt (or lock the agent) and commit again →
      Bonsai shows a **clear `git`-error toast** ("gpg failed to sign …"), makes **NO commit**, and
      does not hang or prompt inside Bonsai.

## C. Verifying OTHERS' commits (mixed real history)

Open a repo whose history has a **mix** of verified, signed-but-unverifiable, and unsigned commits
(e.g. clone a repo with signed commits from signers not in your keyring/allowedSigners).

- [ ] Scroll the graph — each visible row's badge matches `git log --show-signature` for that commit:
      green check = good/trusted; solid neutral disc = signed but signer not established
      (`goodUnknown`) or uncheckable (`cannotCheck`); **amber/red warning triangle** = bad / expired /
      expired-key / revoked; **nothing** = unsigned.
- [ ] Off-screen rows show only the **faint hollow "not yet checked" stub** until scrolled into view
      (verification is virtualized to visible rows — confirm it does not block scrolling).

## D. Missing-key + toggle + settings behavior

- [ ] **Missing key (ssh):** with `gpg.format=ssh` and **`user.signingkey` UNSET**, tick "Sign
      commit" and commit → a **clear, actionable error** ("commit signing requires user.signingkey …",
      linking to Settings) and **NO commit is created** (HEAD unchanged). This mirrors the AI-gate
      `ConfigMissing` unit — confirm the message reaches the UI as a toast.
- [ ] **Toggle overrides config:** with `commit.gpgsign=true`, un-tick "Sign commit" → the resulting
      commit is **unsigned** (`git cat-file -p HEAD` has no `gpgsig`). With `commit.gpgsign=false`,
      ticking the box **does** sign.
- [ ] **Settings → Graph → "Signature badge":** turning it **off** hides the lit badge on all rows
      **and** the CommitPanel signature line, and stops verify requests (rows fall back to the P51
      faint stub). Turning it back **on** re-lights them.

## E. Cross-platform / no flashing console (perception — Windows especially)

- [ ] On **Windows**, committing-with-signing and scrolling a signed history show **NO flashing
      console/cmd window** (the signer/verify subprocesses use `CREATE_NO_WINDOW`). Repeat on
      **macOS** and **Linux**: signer + agent invocation works and the badge/panel behave identically.

## F. Badge iconography — CONFIRM or request a change (OQ7 perception)

The implementation's glyph set (please judge that each reads clearly and distinctly):

- **green filled check** = `good` (verified);
- **SOLID neutral disc** = `goodUnknown` / `cannotCheck`. NOTE: OQ7 originally proposed a *hollow*
  neutral glyph, but the impl uses a **solid** disc **deliberately** so it is not confused with the
  P51 **faint hollow "not yet checked" stub**. **CONFIRM this solid disc reads well**, or ask for the
  hollow variant if you find it ambiguous.
- **amber/red warning triangle** (white `!`) = `bad` / `expired` / `expiredKey` / `revoked`;
- **nothing** = `unsigned` (blank slot — no clutter).

- [ ] The four states are visually distinct at normal zoom and in both light and dark themes, and the
      warning triangle is unmistakably an alarm (not just "different color").

## G. KNOWN CAVEAT to observe (OQ8 — flagged, NOT a blocker)

- [ ] Verification results are **cached by oid for the session**. Editing your keyring /
      `allowedSigners`, or an **external** commit landing at an already-verified visible range, will
      **NOT** re-color the badge until you hit **Refresh** (which drops the cache) — or until a new
      commit made **in Bonsai** re-verifies the new HEAD. Confirm Refresh updates a stale badge after
      you change trust config. This is expected behavior; note if it feels surprising.

## Sign-off
- [ ] A (SSH: box defaults + hint correct; real key signs; `--show-signature` good; badge green/neutral
      matches `%G?`; panel line correct; remote shows "Verified")
- [ ] B (GPG: real key signs via your agent; Bonsai never prompts; badge+panel correct; locked agent =
      clear error, no commit)
- [ ] C (others' commits: badges match `--show-signature` across good/unknown/warn/unsigned; virtualized)
- [ ] D (missing ssh key = actionable error + no commit; toggle overrides config both ways; badge
      toggle hides lit badge + panel line + stops requests)
- [ ] E (no flashing console window on Windows; works on macOS + Linux)
- [ ] F (badge iconography confirmed — or hollow requested for the neutral state)
- [ ] G (OQ8 caveat observed: stale badge until Refresh after a trust-config change)
