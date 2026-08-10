# P64 — More providers + AI PR descriptions: native USER CHECKPOINT checklist

P64 shipped in four sub-increments, all AI-gate-passed and committed:
**P64a** AI PR-description generation (local `claude` CLI), **P64b** GitLab, **P64c** Bitbucket
Cloud, **P64d** Azure DevOps. All providers implement one `ForgeProvider` trait; the frontend
renders every provider through the same neutral PR panel.

The AI gate is done and cannot go further without real accounts/tokens: `cargo test -p bonsai-forge`
**153/0/0** (GitHub 60 + GitLab 29 + Bitbucket 28 + Azure 31 + shared, all offline via a fake
`HttpTransport` — no network/keychain), `cargo clippy -p bonsai-forge --all-targets -D warnings`
clean, `cargo check -p bonsai` links, `pnpm tsc` + `pnpm build` green, `pnpm vitest` green, and a
`pnpm dev:mock` click-through for **each** provider (`?forge=gitlab|bitbucket|azure`): connect →
list → detail, host-aware connect copy + token-help link, Working subtree stays hidden under the PR
tab. Command count after P64: **156** (147 + P62 7 + P63 1 + P64a 1).

Everything below requires a real `pnpm tauri dev` build and a **real account + token per provider**,
so the orchestrator cannot self-verify it (the harness proves wiring on canned mock data only).

## A. AI PR descriptions (P64a) — works for ALL providers, local-only
1. **Generate** — on the PR create form, with the local `claude` CLI installed and AI consent on,
   click **✨ Generate**: it fills a structured title + body from the real commits in
   `compare`-vs-`base` (WHY-oriented, not a diff restatement), WITHOUT auto-submitting. You still
   click **Create pull request**.
2. **Gating** — with AI disabled / not consented / CLI absent, the Generate button is shown but
   disabled with a tooltip (mirrors CommitBox); it never calls out.
3. **Privacy** — confirm generation runs the local CLI only (no network egress to a cloud AI). This
   is OD1 (local-`claude`-CLI-only), unchanged.
4. **Empty range** — Generate with `compare == base` (no commits) surfaces a clean "nothing to
   describe" message, not a spinner or crash.

## B. GitLab (P64b) — real gitlab.com or self-managed
1. **Detection** — open a repo whose `origin` is `gitlab.com/group/proj` (also try a **nested group**
   `gitlab.com/group/subgroup/proj`). The PR tab shows "Connect to gitlab.com", not "unsupported".
2. **Auth / keychain (SECURITY)** — paste a real PAT (needs `api` scope). It's accepted, MRs list.
   Then confirm out-of-band: the token IS in the OS keychain keyed by host; it is **ABSENT** from
   `settings.json`, logs, and any error text/URL (the `PRIVATE-TOKEN` header is redacted). A revoked
   PAT is rejected with a sensible "authentication failed" message.
3. **List / detail / create** — real MRs render (state opened→Open / merged→Merged / closed→Closed,
   draft tag, author, `source→target`, notes count). Open/Closed/All re-queries. Detail shows body,
   labels, mergeable, +/−/files, and comments (notes + discussions, de-duped). Create an MR from a
   scratch/test project.

## C. Bitbucket Cloud (P64c) — real bitbucket.org
1. **Detection** — origin `bitbucket.org/workspace/repo` → "Connect to bitbucket.org".
2. **Auth / keychain (SECURITY)** — paste a **repository/workspace access token** (Bearer). Accepted,
   PRs list. Out-of-band: token in keychain, ABSENT from settings/logs/URLs (Bearer redacted); bad
   token → clean auth error.
3. **State filter (VERIFY THE P64c FIX)** — Bitbucket has no `state=all` and defaults to OPEN when
   omitted, so `list_prs` now fans out repeated `state` params. **Confirm:** the **All** filter shows
   OPEN **and MERGED and DECLINED** PRs, and the **Closed** filter shows MERGED + DECLINED — merged
   PRs must NOT be missing (that was the bug this fix closes).
4. **Detail / create** — body (rendered `raw` if present), inline vs general comments split correctly,
   `head→base`, comment count. Create a PR (note: Bitbucket has no draft PRs — the draft toggle is a
   server-side no-op there).

## D. Azure DevOps (P64d) — real dev.azure.com org
1. **Detection (3-part identity)** — try `https://dev.azure.com/{org}/{project}/_git/{repo}`, the SSH
   form `git@ssh.dev.azure.com:v3/{org}/{project}/{repo}`, and a legacy
   `{org}.visualstudio.com/{project}/_git/{repo}` remote. Each → "Connect to dev.azure.com" (all
   normalized to that host), and org/project/repo are parsed correctly.
   - **Known limitation (reviewer NIT):** the org-default shorthand
     `dev.azure.com/{org}/_git/{repo}` (where repo == project) is NOT recognized and falls through to
     the friendly "unsupported forge" state (no crash). Flag if a user hits it.
2. **Auth / keychain (SECURITY)** — paste an Azure **PAT with Code (Read & Write)**. Backend sends
   `Authorization: Basic base64(":"+PAT)`. Out-of-band: token in keychain, ABSENT from
   settings/logs/URLs (redaction test proves plaintext AND base64 absent from Debug). Identity is
   validated via the cross-host `app.vssps.visualstudio.com/.../profiles/me` call.
   - **Known rough edge (reviewer NIT):** a bad/expired Azure PAT can return **HTTP 203** (an HTML
     sign-in page) rather than 401/403; that currently surfaces as a "malformed response"
     (`forgeApi`) rather than a clean "authentication failed". No leak/crash — but if you see a
     confusing error on a bad Azure PAT, this is why. Candidate follow-up: treat 203 / non-JSON on an
     authed call as `forgeAuthRequired`.
3. **List / detail / create** — real PRs (active→Open, completed→Merged, abandoned→Closed; isDraft;
   `sourceRefName`/`targetRefName` shown with `refs/heads/` stripped; `head→base`). Open/Closed/All
   re-queries (Azure supports `status=all`). Detail shows body, labels, mergeable (mergeStatus),
   comments (threads; inline vs conversation). Create a PR (backend re-adds `refs/heads/`).

## E. Cross-provider
1. **"Open in browser ↗"** (PR detail) — the label is now provider-neutral (was "Open on GitHub"). In
   the native Tauri webview, confirm it reaches the system browser at the provider's real PR URL. If
   `target="_blank"` doesn't open the system browser (the P62 follow-up), wire it to the P49
   external-open path (extended to accept an https URL).
2. **Sign-out** — clearing the token per provider removes the keychain entry and returns the panel to
   the connect state.
3. **`keyring` on Linux** — confirm token storage works on the target Linux desktop
   (Secret-Service / GNOME Keyring / KWallet present) for each provider.
