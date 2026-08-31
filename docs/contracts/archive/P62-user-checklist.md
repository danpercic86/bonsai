# P62 — Forge foundation: native USER CHECKPOINT checklist

Everything below requires a **real** `pnpm tauri dev` build + a real GitHub account/PAT and
therefore CANNOT be self-verified by the orchestrator (the browser harness proves the wiring on
canned mock data only). The AI gate already passed: `cargo test -p bonsai-forge` 60/0 (offline via a
fake `HttpTransport` — no network/keychain/git), `cargo test -p bonsai --lib` 117/0 (incl.
`forge_commands_require_an_open_repo`), `cargo clippy --all-targets -D warnings` clean, `cargo build
--workspace` 0-warn, `pnpm tsc` + `pnpm build` green, and a full `pnpm dev:mock` click-through
(connect → list → detail → create → offline) with a clean console.

Command count after P62: **154** (147 + 7 `forge_*`).

## Verify in `pnpm tauri dev` against a real GitHub repo

1. **Detection** — open a repo whose `origin` is a real `github.com/OWNER/REPO(.git)`. The right pane's
   **Pull requests** tab shows the "connect" state (not "unsupported forge").
2. **Auth / keychain (SECURITY-CRITICAL)** — paste a real PAT into ForgeConnect. It is accepted and
   the open PRs list appears. Then confirm out-of-band:
   - the token IS in the OS keychain (Windows **Credential Manager** → Generic Credentials; macOS
     **Keychain Access**; Linux **Secret Service**), keyed by host;
   - the token is **ABSENT** from `settings.json`, any Bonsai log, and any error text/URL;
   - a **bad/revoked** PAT is rejected with a sensible "authentication failed" message (not a raw dump).
3. **List** — the real OPEN PRs render (state pill, draft tag, author, `head→base`, comment count).
   The Open/Closed/All filter re-queries. Refresh re-fetches.
4. **Detail** — open a real PR: real title/author/`head→base`, body (markdown as text), labels,
   mergeable chip (open PRs only), +/−/changed-files, and both **review** (with `path:line`) and
   **conversation** comments, sorted by date.
5. **Create** — from a scratch/test remote, create a PR (title/body/base/compare/draft). It succeeds,
   opens the new PR's detail, and the success toast carries the browser URL.
6. **Sign-out** — clearing the token (`forge_clear_token`) removes the keychain entry; the panel
   returns to the connect state.
7. **Rate limit** — (best-effort) hammering or a low-limit token surfaces a "rate limited" message
   that includes the reset time, not a generic failure.
8. **Non-GitHub origin** — a repo whose origin is GitLab/Bitbucket/etc. shows the friendly
   "unsupported forge" empty state (P64 adds those providers), and a data action on it errors as
   `forgeUnsupported` rather than crashing.

## Known follow-ups (flagged during review — NOT bugs, scheduled)

- **"Open on GitHub" link** uses `<a target="_blank" rel="noreferrer noopener">`. In the browser
  harness this is correct, but in the **native Tauri webview** `target="_blank"` may not reliably
  reach the system browser. If it doesn't during this checkpoint, wire it to an opener command
  (extend the P49 external-open path to accept an https URL) in a follow-up increment.
- **Create-form `Base` field** starts empty (no reliable default-branch signal is threaded yet;
  only `Compare` is seeded from the current branch). The user types the base. `defaultBase` is a
  dormant prop seam if we later resolve the repo's default branch.
- **`keyring` on Linux** pulls a Secret-Service/D-Bus backend; confirm token storage works on the
  target Linux desktop (GNOME Keyring / KWallet present).
