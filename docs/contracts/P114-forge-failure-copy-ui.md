# P114 — Forge credential-failure copy (UI contract)

Scope: microcopy only. No new components, no new tokens, no geometry change. Owner surface:
`src-tauri/src/commands/forge_remove_account.rs`, `src-tauri/src/commands/forge_clear_host.rs`,
their mock mirrors, and `src/components/settings/SettingsAccountsSection.tsx` (render site).
Implemented by `senior-dev`. Branch context: `feat/post-p91-rulings`, HEAD `14c1188`.

> **Dormancy note (read before implementing).** `forge_clear_token_for_host` has **no
> `#[tauri::command]` wrapper and no UI caller** (security audit INFO-2, 2026-09-17); its module is
> `#[cfg(test)]`. The clear-host copy below is specified **for the documented rewire only**. It is
> not a commitment to ship host-wide sign-out, and **no control is designed for it here** — whether
> it ever gets a button is a separate product decision. Its rendered forms assume the same
> verbatim-render rule as the reachable sibling.

---

## 1. Rule 1 — who owns the sentence (resolves Problem 1)

**A backend message that returns a bare *cause* does not own the sentence: the caller prefixes it.
A backend message that returns an *outcome* — which half of a two-step operation happened — owns the
whole sentence, and the caller renders it verbatim.**

These five/seven strings are outcomes, not causes. That is the structural reason the current prefix
contradicts outcome R2/C6: a caller that does not know *which half succeeded* cannot write a
truthful lead clause, and `Could not remove {host}: the credential was removed…` is that
impossibility made visible. So:

- **The container prefix goes.** `SettingsAccountsSection.tsx:148` becomes
  `setRemoveError(errorMessage(e))` — no `Could not remove ${host}: ` wrapper, no other change to
  that handler (the dialog still stays open for in-place retry, per §12.14).
- **The backend strings become standalone sentences**, capitalised, terminal period.
- To keep "render verbatim" **total** for this command, the two non-outcome rejections that can also
  surface through it must be wrapped into sentence form **inside the command file** (see rows N1/N2).
  `settings::settings_file`'s own lowercase cause string is shared by many commands and is **out of
  scope** — wrap it at this call site, do not edit it.

**Relation to `ui-reference.md` §5 (toast copy shape), verified.** §5 already pins
`Couldn't <verb> <target>. <what to do next>` with the frontend supplying the prefix and the
backend's `AppError.message` surfaced **verbatim**, and already requires backend messages to be
complete, capitalised, period-terminated sentences. Rule 1 does **not** overturn that: it carves out
**outcome-shaped** messages from two-step operations, where no caller-side prefix can be truthful.
Single-step failures keep the §5 prefix (which is why L97/L98/L124 of
`SettingsAccountsSection.tsx` are untouched here). §5's "Couldn't" form also confirms the
contraction pin below.

**`errorMessage` is a pure passthrough** (`src/utils/errors.ts:16-20`:
`AppError.message | Error.message | String(e)` — no kind prefix, no case or punctuation fixup), so
"rendered verbatim" is literally true and the const text is the rendered text. `gitNotFound.ts:73`
is the existing precedent for rendering it bare. One residual: a transport-level rejection that is
neither an `AppError` nor an `Error` renders as `String(e)` — pre-existing, applies to every call
site in the app, and **out of scope** here.

This is the **first** capitalised, sentence-form `AppError::Other` in Rust (grep found only
`"MCP state lock poisoned"` / `"MCP server is not running"`, neither user-rendered prose). Flagged
deliberately. The alternative — returning structured error variants and composing the sentence in
TS — is an architect-side shape change; named here as the rejected-for-now option, not specced.

## 2. Rule 2 — state, not act (resolves Problem 2)

**Describe the end state of the keychain, never the act of deleting.** Say *"the credential is no
longer in the OS keychain"*, never *"was removed"*.

Why this is required, in both directions of the same mechanism
(`crates/bonsai-forge/src/auth.rs:53` folds `keyring::Error::NoEntry` into `Ok(())`):
- **First failure:** the delete really did succeed, then the settings write failed. State-shaped
  wording is true.
- **Retry after a partial failure:** there is no key left, `delete_token` returns `Ok(())` without
  removing anything, and the settings write fails again. Act-shaped wording ("was removed") is then
  **false**; state-shaped wording is still true.

The fold is load-bearing — it is what makes every "you can try again" below honest — so the copy
must be written to survive it.

## 3. Rule 3 — the other standing rules

| Question | Rule |
| --- | --- |
| Name the OS keychain? | **Yes**, as "the OS keychain". It is where the user's PAT physically is, it is the thing that refused, and it is the only place they could clean up by hand. Never "keyring", never a platform-specific name (Credential Manager / Keychain Access) — one string ships to three OSes. |
| "credential" or "token"? | **"credential"** in user-facing copy (singular per account; plural for a host). "token" only where the user chose the word themselves — the add-account flow's "personal access token" field. Never "PAT" on screen. |
| Retry cue? | **Every outcome gets one, because every outcome is safely retryable by design** (nothing is mutated on keychain refusal; the delete is idempotent). Phrase it as **the correct next action**, never as a promise about what will happen: "so you can try again" / "Try again to finish removing it". |
| Partial failure (one half landed)? | **State what is true of each half, succeeded half first, in one sentence joined by "but"**, then the action. Never a lead clause that denies the whole operation. |
| Where does the interpolated cause go? | **Last**, after all human sentences, introduced by `Details: `. The plain-language sentence and the action must be spoken before any `os error 5` / absolute path. This is the copy analogue of "no raw libgit2 text": the raw cause is permitted **only** as trailing detail, and is never truncated or hidden. **`Details: ` is a first use** — the in-app precedent is P113's `Couldn't load your accounts. ${listError}` (bare cause after a period). Justified: in a single `role="alert"` utterance a bare `io error: write C:\…` after a period is indistinguishable from a third sentence, and the lead-in is the audible boundary. If the orchestrator prefers strict consistency, the fallback is to drop `Details: ` and keep `… try again. {e}`; everything else in the table is unchanged. |
| Contraction style | **"Couldn't"**, not "Could not". Pins the P113 error-banner precedent (`Couldn't load your accounts.`). The four surviving `Could not …` strings in `SettingsAccountsSection.tsx` (L97, L98, L124, and the L225 comment) are a **follow-up sweep**, not part of this increment. |
| Sentence budget | ≤ 2 human sentences + the `Details: ` fragment. §12.14's outcome notes are one-line; a dialog error may run to two because it must carry both halves of a partial failure and the action. |

## 4. Copy table — rendered form is what is signed off

`{e}` = the interpolated cause, verbatim from the backend. Rendered form assumes **Rule 1**, i.e.
the container prefix is gone, so const text and rendered text are identical.

### `forge_remove_account` (singular) — rendered in the Remove dialog's `.dialog-error`

**R1 — keychain refused the delete** (`KEYCHAIN_FAIL_*`)
- Current: `Could not remove github.com: could not remove the credential from the OS keychain: {e}. Nothing was changed — the account is still listed, so you can try again.`
- **Proposed:** `Couldn't remove the account's credential from the OS keychain. Nothing was changed — the account is still listed, so you can try again. Details: {e}`

**R2 — credential gone, settings write failed** (`SETTINGS_FAIL_*`)
- Current: `Could not remove github.com: the credential was removed from the OS keychain, but the account list could not be saved: {e}. The account may still appear until settings can be written.`
- **Proposed:** `The credential is no longer in the OS keychain, but the account list couldn't be saved. Try again to finish removing it. Details: {e}`
- Drops "was removed" (Rule 2) and drops "may still appear until settings can be written", which told the user nothing to do. Note for the implementer: a *backend* `settings::update` failure does **not** raise §12.14's global settings banner (that banner is driven by the frontend `useUiSettings` failure streak), so this cue is the only guidance the user gets.

**R3 — unknown / already-removed id, settings write failed** (`SETTINGS_FAIL_NO_CREDENTIAL_PREFIX`)
- Current: `Could not remove github.com: the account list could not be saved: {e}.`
- **Proposed:** `The account list couldn't be saved. Try again to finish removing it. Details: {e}`
- Says nothing about a credential — correct, none was touched.

**N1 — config dir unreachable** (outer command, currently `cannot resolve app config dir: {e}` leaking bare)
- **Proposed:** `Couldn't remove the account — Bonsai can't reach its settings folder. Details: {e}`
- New const in `forge_remove_account.rs`; wrap `settings::settings_file(&app)?` at this call site.

**N2 — blocking task panicked** (currently `task join error: {e}`)
- **Proposed:** `Couldn't remove the account. It may or may not have been removed — check the list and try again. Details: {e}`
- The hedge is deliberate: after a panic the actual state is genuinely unknown, and this is the one place we must not promise.

### `forge_clear_token_for_host` (plural, dormant — see the boxed note)

**C4 — keychain refused, host has N accounts listed** (`KEYCHAIN_FAIL_PREFIX` + `KEYCHAIN_FAIL_SUFFIX`)
- Current: `could not remove the credentials from the OS keychain: {e1}; {e2}. Nothing was changed — the accounts are still listed, so you can try again.`
- **Proposed:** `Couldn't remove this host's credentials from the OS keychain. Nothing was changed — the accounts are still listed, so you can try again. Details: {e1}; {e2}`
- `KEYCHAIN_FAIL_JOIN` stays `"; "` and stays a named const (the guard covers it).

**C5 — keychain refused, host has NO accounts listed (legacy leftover)** (`KEYCHAIN_FAIL_NO_ACCOUNT_SUFFIX`)
- Current: `could not remove the credentials from the OS keychain: {e}. Nothing was changed — this host has no accounts listed; a leftover credential for it could not be removed, so you can try again.` — one and a half sentences, and it says the same failure twice.
- **Proposed:** `A leftover credential for {host} couldn't be removed from the OS keychain. Nothing was changed, so you can try again. Details: {e}`
- **Restructure: this variant becomes its own HEAD const, not a suffix swap**, so the sentence can start with the thing that actually failed and can name the host. `host_l` is in scope at the `format!` site (`forge_clear_host.rs:181`), so use it.

**C6 — credentials gone, settings write failed** (`SETTINGS_FAIL_*`)
- Current: `the credentials were removed from the OS keychain, but the account list could not be saved: {e}. The accounts may still appear until settings can be written.`
- **Proposed:** `The credentials are no longer in the OS keychain, but the account list couldn't be saved. Try again to finish signing out of {host}. Details: {e}`
- **Plural truth verified:** the loop at `forge_clear_host.rs:153-168` attempts every key *including* the legacy bare-host entry and pushes **all** refusals into `failures`, and this branch is reachable only when `failures.is_empty()`. So "the credentials" (all of them, legacy included) is true here — no need to scope it to "the accounts' credentials".

**C7 — no account named a credential, settings write failed** (`SETTINGS_FAIL_NO_CREDENTIAL_PREFIX`)
- Current: `the account list could not be saved: {e}.` (byte-identical to R3 today)
- **Proposed:** `The account list couldn't be saved. Try again to finish signing out of {host}. Details: {e}`
- **No longer byte-identical to R3** — deliberate: the action differs ("removing it" vs "signing out of {host}"). The two cross-language guards therefore check different fragments; do not de-duplicate the consts across the files.

**Count correction:** the brief says five strings; there are **seven** ruled outcome variants
(3 + 4), plus the two non-outcome wrappers N1/N2 that Rule 1 pulls into scope. Nine strings total.

## 5. Q3 answer — does the settings-save outcome need a retry cue?

**Yes, and it is honest.** R2/R3/C6/C7 all now end in "Try again to finish …". Truth conditions,
stated so the next writer can check them:
- Keychain half: the `NoEntry` fold means a retried `delete_token` finds no key and returns
  `Ok(())`. It cannot re-fail because of the already-deleted credential.
- Settings half: `settings::update` is a load→mutate→save transaction with no partial commit, so a
  retry either lands fully or fails the same way, leaving state unchanged.

So "try again" is a true statement about a genuinely idempotent operation, not optimism. The only
outcome where it is hedged is **N2** (panic), where state is unknown and the copy says "may".

## 6. Q4 answer — a11y and length in `.dialog-error`

Container: `src/styles/dialogs.css:104-117` — `font-size: 12px`, `color: var(--danger-strong)`,
`overflow-wrap: anywhere`, `margin: 6px 0 0`, `:empty { margin: 0 }`, always mounted with empty text,
`role="alert"` on the element in `SettingsAccountsSection`.

**Two sentences plus a trailing interpolated cause containing an absolute path is acceptable in this
control, unchanged.** Findings:
- Contrast: `--danger-strong` on `--bg-1` is **7.62:1** dark / **6.01:1** light (§12.14) — AA and AAA
  for this size. No token change.
- `overflow-wrap: anywhere` is already present, so the §14 pathological cause (the ~330-char
  space-free UNC path in the `long` seam) wraps instead of overflowing. This is why the cause may be
  a path at all.
- `role="alert"` reads the whole node in one utterance. **Cause-last ordering is the a11y
  requirement, not a style preference**: the actionable sentence is spoken first, and a user who
  stops listening at the path has already heard what to do.
- Never truncate, never clamp, never put the cause behind a disclosure. The dialog grows; that is
  correct for a failure the user must act on. Implementer: confirm the remove dialog's body is the
  scrolling container at 1440×900 with the `long` seam; if it is not, file it as a follow-up — it is
  a pre-existing layout property, not part of this copy increment.
- The section announcer stays **silent** for these outcomes (§12.14): the `role="alert"` is the one
  utterance. Unchanged by this pass.
- `overflow-wrap`, tone and geometry are identical in **dark and light** and in **cozy and compact**
  — this control has no density variant, and copy length is density-independent. Nothing to spec per
  theme/density beyond the contrast figures above.

## 7. Findings the brief did not list

1. **The prefix interpolated `host`, but the dialog labels the account by `login`**
   (`removeLabel = removeTarget?.login ?? removeTarget?.host ?? 'this account'`,
   `SettingsAccountsSection.tsx:153`). With the prefix gone, the **dialog title is the only subject**,
   so it must keep using `removeLabel` — the identifier the user chose. Do not reintroduce `host` as
   a subject anywhere in this dialog.
2. **Contraction style is split** across the same file (`Could not …` ×4 vs `Couldn't load …` ×1).
   Pinned in Rule 3; the sweep is a follow-up.
3. **C5 stated its failure twice** ("could not remove the credentials … a leftover credential …
   could not be removed"). Fixed by the head-const restructure.
4. **The mock handlers hold FULL strings, not composed halves.** `forgeRemoveFailure.ts` (and its
   clear-host sibling) declare each message as one complete literal. "Mirror" therefore means
   *rewrite the whole literal*, not *edit a fragment*.

## 8. Implementation notes for `senior-dev`

**This is a three-file-per-command change.** A cross-language guard pins the copy:
`forge_remove_account_tests::mock_copy_mirrors_the_rust_copy` and
`forge_clear_host_tests::mock_copy_mirrors_the_rust_copy` `include_str!` their mock handler and
assert every Rust fragment appears in it. So each string change touches:
1. the Rust consts (`forge_remove_account.rs:28-37`, `forge_clear_host.rs:45-63`),
2. the TS mirror (`src/ipc/mock/handlers/forgeRemoveFailure.ts`,
   `src/ipc/mock/handlers/forgeClearHostFailure.ts`) — full literals,
3. the fragments the guard asserts, plus any Rust/TS test asserting the old text
   (`SettingsAccountsSection.remove.test.tsx` asserts the rendered prefix — it must lose it).

N1/N2 also change the mock's non-outcome literals (`forgeRemoveFailure.ts:76` and `:78`, the `'1'`
and `'long'` seams), and the guard test currently checks only the three ruled outcomes — **extend it
to assert the N1 and N2 consts too**, or they ship as unguarded copy that can drift.

Const shape changes: `KEYCHAIN_FAIL_PREFIX`/`SUFFIX` no longer bracket the cause. Rename to
`*_FAIL_HEAD` (the human sentences, ending `. `) + a shared `CAUSE_LEAD: &str = "Details: "`, so
every message is `format!("{HEAD}{CAUSE_LEAD}{e}")` and there is exactly one place the ordering
rule lives. `KEYCHAIN_FAIL_NO_ACCOUNT_SUFFIX` becomes `KEYCHAIN_FAIL_NO_ACCOUNT_HEAD` and takes
`{host}`. Note the cause no longer needs a trailing `.` appended (rows R3/C7 currently append one).

**Harness states** (no new fixtures needed; existing seams already cover them):
`?forgeRemoveFail=keychain` (R1), `=settings` (R2), `=settings-no-credential` (R3), `=1` (N1),
`=long` (§14 pathological cause), `=keychain-then-ok` (the honest-retry path end to end).
`?forgeClearHostFail=` renders C4–C7 **console-only** — the clear-host copy **cannot be seen in the
harness UI** because the command is unreachable. That is a documentation-only verification, not a
USER CHECKPOINT to open; do not add a control to make it visible.

**Ambiguity flagged for the orchestrator:** rows N1/N2 extend scope beyond the five strings named in
the brief. My recommendation is to include them — without them, "the caller renders verbatim" is not
total and a bare lowercase `cannot resolve app config dir: …` reaches the dialog. If the orchestrator
wants a minimal diff instead, the fallback is for `SettingsAccountsSection` to keep a prefix
**only** on those two paths, which requires string sniffing and is worse; say so before choosing it.
