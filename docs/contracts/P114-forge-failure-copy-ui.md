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
| Retry cue? | **Every outcome in §4 gets one, because every §4 outcome is safely retryable by design** (nothing is mutated on keychain refusal; the delete is idempotent). Phrase it as **the correct next action**, never as a promise about what will happen: "so you can try again" / "Try again to finish removing it". **Carve-out (Addendum A): R4 gets NO cue** — it is the one outcome where the removal *succeeded*, so there is no target left to retry and a cue would be false. The test is idempotence, not tone. |
| Partial failure (one half landed)? | **State what is true of each half, succeeded half first, in one sentence joined by "but"**, then the action. Never a lead clause that denies the whole operation. |
| Where does the interpolated cause go? | **Last**, after all human sentences, introduced by `Details: `. The plain-language sentence and the action must be spoken before any `os error 5` / absolute path. This is the copy analogue of "no raw libgit2 text": the raw cause is permitted **only** as trailing detail, and is never truncated or hidden. **`Details: ` is a first use** — the in-app precedent is P113's `Couldn't load your accounts. ${listError}` (bare cause after a period). Justified: in a single `role="alert"` utterance a bare `io error: write C:\…` after a period is indistinguishable from a third sentence, and the lead-in is the audible boundary. If the orchestrator prefers strict consistency, the fallback is to drop `Details: ` and keep `… try again. {e}`; everything else in the table is unchanged. |
| Contraction style | **"Couldn't"**, not "Could not". Pins the P113 error-banner precedent (`Couldn't load your accounts.`). The four surviving `Could not …` strings in `SettingsAccountsSection.tsx` (L97, L98, L124, and the L225 comment) are a **follow-up sweep**, not part of this increment. |
| Sentence budget | ≤ 2 human sentences + the `Details: ` fragment. §12.14's outcome notes are one-line; a dialog error may run to two because it must carry both halves of a partial failure and the action. **Carve-out (Addendum A): R4 is a two-sentence note**, the only one. It earns the second sentence for the same reason a dialog error does — it carries both halves of a partial outcome — plus a third obligation no other note has: naming a fix that lives outside the app. `overflow-wrap: anywhere` is already on `.settings-row-note--warn` (§12.14), so the length is safe there. Do not generalise this to other notes. |

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
**Updated by Addendum A (2026-09-17): ten live strings + one retired** — R4 is added,
`LEGACY_KEYCHAIN_FAIL_HEAD` is retired.

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
`?forgeClearHostFail=` is **fully inert** (orchestrator correction, 2026-09-17): the command was
dropped from the invoke surface and its module is `#[cfg(test)]`, so C4–C7 are **not console-reachable
either** — they are verified by the cross-language guard test alone. An earlier revision of this line
said "console-only"; that was wrong. Documentation-only verification, not a USER CHECKPOINT to open;
do not add a control to make it visible.

**Ambiguity flagged for the orchestrator:** rows N1/N2 extend scope beyond the five strings named in
the brief. My recommendation is to include them — without them, "the caller renders verbatim" is not
total and a bare lowercase `cannot resolve app config dir: …` reaches the dialog. If the orchestrator
wants a minimal diff instead, the fallback is for `SettingsAccountsSection` to keep a prefix
**only** on those two paths, which requires string sniffing and is worse; say so before choosing it.

---

# Addendum A (2026-09-17) — the best-effort legacy sweep, outcome R4

User ruling, on the security auditor's reasoning: the legacy bare-host sweep in
`forge_remove_account` becomes **best effort**. Fail-closed can permanently strand a user — the
account's own token is already gone on a retry (`NoEntry → Ok`) while the legacy key refuses again,
leaving a listed, disconnected, **unremovable** account fixable only by hand in the OS credential
store. So the removal completes and the leftover is reported honestly.

## A.1 The row is retired, and why

**`LEGACY_KEYCHAIN_FAIL_HEAD` (`forge_remove_account.rs:55`) is REMOVED, not reworded.** Its text —
*"Nothing was changed — the account is still listed, so you can try again."* — is false under
best effort in all three clauses: something *was* changed, the account is *not* still listed, and
there is nothing to try again. It was added after this contract's signature and never had a row
here; it gets one now, as retired. Three-part change (§A.6): Rust const deleted, mock
`LEGACY_KEYCHAIN_FAIL_TEXT` / `REMOVE_LEGACY_KEYCHAIN_FAIL_MESSAGE`
(`forgeRemoveFailure.ts:94,108`) replaced, guard fragment swapped.

The dormant `forge_clear_host` sibling **C5 still encodes fail-closed** ("Nothing was changed, so you
can try again") and is **out of scope** — that command is unreachable (see §8) and the ruling was
about `forge_remove_account`. Named here so it is not rediscovered as a contradiction: if
`forge_clear_token_for_host` is ever rewired, C5 must be re-ruled against Addendum A first.

## A.2 Shape first — this outcome must NOT land in `.dialog-error`

**Design requirement (the mechanism is the architect's call — three options ranked below).** R4 must
be **distinguishable from a failure at the IPC boundary, without inspecting the message text.**
Reporting a *success* through the remove dialog's `.dialog-error` produces a defect the copy cannot
fix, because `confirmRemove` (`SettingsAccountsSection.tsx:143-154`) deliberately keeps the dialog
open on `Err` for in-place retry:

- the dialog is titled with `removeLabel` — an account that **no longer exists**;
- its destructive **Remove** button is live, and a second click hits `rec == None → Ok` and closes
  the dialog **silently**, reporting nothing about the leftover that is still there;
- the error path does **not** `refetch()`, so the list behind the dialog still shows the removed
  row until something else refreshes it — a second untruth on the same screen.

R2/R3 tolerate that container because a retry there is real and useful. Here it is impossible.

**Ranked options — FLAGGED for the architect/orchestrator. The copy (§A.3), the caller behaviour
(below) and the mock ordering (§A.5) are identical under all three; only the routing check moves.**

1. **RECOMMENDED — return `Ok` with a payload.** The command resolves with a small result struct
   carrying the leftover message (e.g. `leftover: Option<String>`; the architect names the type and
   field). This is the only option in which a success is typed as a success: an `Err` that means
   success is counted as a failure by every generic layer above it — `src/obs/types.ts`'s
   `ipc.result` / `error` / `anomaly` payloads, §12.14's DEV `console.error` guard, any future retry
   wrapper — and it is exactly the act/state lie rejected at the const-name level, relocated to the
   transport. Cost: `forgeRemoveAccount`'s return type changes for all callers.
2. **Fallback — a distinguishable error kind**, if the architect wants the minimal diff. Routing on
   kind has a house precedent (`isAppError(e) && e.kind === 'authFailed'`, `PrPanel.tsx:258`);
   proposed name `forgeLeftoverCredential`. Accepts the mis-classification above knowingly.
3. **Last resort — `.dialog-error`, like R2.** The increment then knowingly ships a dialog naming a
   deleted account with a live Remove button whose second click is a silent no-op. If this is chosen,
   file that as a defect in `TODO.md` rather than leaving it implicit.

**Caller behaviour in `confirmRemove` (options 1 and 2):**

1. R4 detected (fulfilled-value field, or `e.kind`) → `setRemoving(false)`,
   `setRemoveTarget(null)` (**close** the dialog), `refetch()` (the row is genuinely gone), and
   report the message through a `SettingsOutcomeNote` in its **`--warn`** tone (`--warning`, not
   `--danger`: nothing was lost) plus the section announcer, written from the same `report(...)`
   call — the §12.14 "result of the action you just took" shape. No new component, no new token.
2. Any other rejection → unchanged (`setRemoveError(errorMessage(e))`, dialog stays open).

**Which note slot — this is load-bearing, and the host-group slot is WRONG.** R4's precondition is
`last_on_host`, so after `refetch()` that host has no accounts and `remove_forge_host`
(`forge_remove_account.rs:199-204`) has dropped its settings entry: the `SettingsAccountGroup`
unmounts, and §12.14 clears a note on unmount. `report(host, …)` (the `setDefault` shape at
`SettingsAccountsSection.tsx:124`) would therefore leave **only the announcer** — the sighted user
sees nothing, which is the placement-is-not-a-carrier failure §12.14 already rules out.

Use the **existing section-level slot**, `ADD_SLOT` (`SettingsAccountsSection.tsx:234`, rendered as
the `accounts.add` row's `hint` at :244-249). It exists for precisely this reason — its own comment
at :232 reads "the section slot, not a host slot: the new host's group does not exist until
`refetch` resolves". Always mounted, `:empty`-collapsed, `aria-describedby`-composed, contrast and
tint already verified. **No new slot, no new element.**

**One ambiguity, flagged rather than decided:** `ADD_SLOT` currently lives in the hint of the row
labelled *"Add a token for a host"*, so a **removal** outcome would render under an add control.
My recommendation is to **rename the constant to `SECTION_SLOT`** (and `ADD_OUTCOME_ID` to
`SECTION_OUTCOME_ID`) with **no DOM or placement change** — it is already a cross-host section slot
carrying a host-independent outcome, and the label mismatch is cosmetic against the cost of a second
slot with its own live-region arithmetic. The alternative, if the orchestrator finds the mismatch
unacceptable, is a dedicated always-mounted section note **above** the group list, which then needs
the §12.14 parent-`gap`/`:empty` check on its container and a second `aria-describedby` target with
no control to attach to. Say which before implementing.

## A.3 Copy — outcome R4

**R4 — the account was removed; a leftover legacy credential for the host was refused**
(`LEGACY_LEFTOVER_HEAD`)

- **Proposed:** `The account is no longer listed, but a leftover credential for {host} is still in
  the OS keychain. Bonsai can't remove it — clear it there by hand if you want it gone. Details: {e}`
- "clear it **there**", not "clear it from the OS keychain": the phrase is named once and referred
  back to, so the second sentence does not echo the first.

Why each clause is the way it is:

- **Succeeded half first, joined by "but"** (Rule 3, partial-failure row), and **state-shaped both
  halves** (Rule 2): "is no longer listed" / "is still in the OS keychain", never "was removed" /
  "couldn't be removed". This string can never re-render on a retry, so Rule 2 buys no truth here —
  it is applied for consistency, at no cost.
- **`{host}`, not `login`.** §7 finding 1 forbids `host` as a *subject in the remove dialog*; this
  message is no longer in that dialog, its subject is the leftover, and the leftover **is** keyed to
  the bare host. `login` would be wrong: nothing named `login` is left behind. `r.host` is in scope
  at the `format!` site.
- **No retry cue** — §12.14 permits one only where the operation is provably idempotent, and
  retrying is not merely non-idempotent here, it is **impossible**: the account is gone, so a second
  Remove has no target and would never reach the sweep.
- **It points at the OS keychain, and that is deliberate.** Options were (a) state the fact only,
  (b) point at the OS keychain. **(b), because the house rule is "errors say what happened and what
  to do next"**, and this is the one outcome where the next step exists but is outside the app.
  Omitting it leaves the user with an unexplained fact and no exit. "the OS keychain" is already the
  sanctioned, platform-neutral vocabulary (Rule 3), so the clause costs one phrase and names no
  platform. `Bonsai can't remove it` is doing real work: it pre-empts the reasonable belief that
  some button in the app would finish the job.
- **`Details: {e}` last**, `CAUSE_LEAD` shared (Rule 3).
- Two human sentences + the fragment — at budget, not over.

**Const name: `LEGACY_LEFTOVER_HEAD`, and the absence of `FAIL` is intentional.** Every sibling is
`*_FAIL_HEAD` because the command failed; here it **succeeded**. Naming this one `..._FAIL_...`
would repeat the act/state lie at the identifier level and would mislead the next reader into
routing it with the failures. Mock mirror: `LEGACY_LEFTOVER_TEXT` /
`REMOVE_LEGACY_LEFTOVER_MESSAGE`.

## A.4 Combination truth conditions (P114 §5 style)

The sweep now records its refusal instead of returning early, so two failures can coexist. Ruling:

- **legacy refused + account-key refused (R1):** **R1 wins.** R1 is actionable and its "Nothing was
  changed — the account is still listed" is still true (the settings write never ran).
- **legacy refused + settings write failed (R2/R3):** **R2/R3 wins.** It is the actionable one, and
  the legacy fact is **not lost**: the account record still exists, so the recommended retry runs
  the sweep again and surfaces R4 on the attempt that finally succeeds.
- **legacy refused + everything else succeeded:** **R4**, the only case where R4 is returned.
- **legacy refused + task panic (N2):** N2 wins; state is unknown and N2 is the one message that
  promises nothing.

So R4 is returned **only** from the tail of a fully successful removal, which is precisely what
makes its first clause true. Implementer: the refusal must be captured (e.g. `Option<String>`) and
returned **after** `update_settings` succeeds — never with `?`.

## A.5 Harness state

Repurpose the existing seam key **`?forgeRemoveFail=legacy-keychain`**: the mock must now **remove
the account from its list first, then reject** with `REMOVE_LEGACY_LEFTOVER_MESSAGE` under the new
kind. That ordering is the fixture's whole point — it is what lets the harness show the dialog
closing, the row disappearing and the warn note appearing in one pass. Its `forgeRemoveFailure.ts`
header comment (lines 26-33, which currently documents the fail-closed behaviour and calls
`"Nothing was changed"` "literally true") must be rewritten in the same edit. `LEGACY_HOST =
'github.com'` stays — it is the seeded account's host, so the rendered sentence names a real row.
No new fixture, no other seam touched.

## A.6 Every string is a THREE-part change

Restating §8 with the current guard behaviour, because it now constrains the *text itself*:
`mock_copy_mirrors_the_rust_copy` `include_str!`s the mock handler and asserts each whole Rust HEAD
(+ `CAUSE_LEAD`) appears as **one quoted literal on a non-comment line, exactly once**. So for
**every** row added, changed or retired here:

1. the Rust const (`forge_remove_account.rs`),
2. the TS mirror in `forgeRemoveFailure.ts` — **one whole literal**, never a composed fragment, and
   never only inside a comment (a comment occurrence does not satisfy the guard, and a *second*
   occurrence breaks the exactly-once match),
3. the guard's fragment list, plus any test asserting the old text.

A retired const must be removed from all three, or the guard fails on a literal with no const.

---

# Addendum B (2026-09-17) — the toast double-framing in PrPanel and ChecksPanel

Scope extension: these two panels call the **same** backend command as the Settings surface, so
Rule 1 already governs them; P114 simply never looked here.

## B.1 The defect

`src/components/PrPanel.tsx:247` and `src/components/checksPanel/ChecksPanel.tsx:85` both do:

```
setConnectError(errorMessage(e));                                 // correct — verbatim
pushToast('error', `Could not connect: ${errorMessage(e)}`);      // the defect
```

Rendered with an outcome-shaped message that is the prefix-stutter class removed from the Settings
dialog: `Could not connect: The credential in the OS keychain is up to date, but the account
details couldn't be saved…`. `src/ipc/mock/handlers/forgeAddFailure.ts:22` already documents this
in a comment; nobody acted on it.

## B.2 Ruling — **the toast goes away entirely.** Not the prefix, not the message: the toast.

The deciding argument is Rule 1's own, and it is not about width. `forgeSetToken` rejects with
**both** classes of message: cause-shaped ones (`authFailed`, rate-limited, network) where §5's
`Couldn't <verb> <target>.` prefix is *correct*, and outcome-shaped ones
(`?forgeSetTokenFail=rolled-back|kept|rollback-failed`) where it is *false*. A single call site
cannot pick correctly, and telling them apart at runtime means **string sniffing** — which P114 §8
already rejected as "worse". So:

- "drop the prefix" is wrong: the cause-shaped rejections would surface bare and lowercase in a
  toast, breaking §5.
- "keep the prefix" is wrong: the outcome-shaped ones stutter, breaking Rule 1.
- Removing the toast is correct for **both** classes, and costs nothing, because the inline
  `connectError` render **is** the notification. Precedent, in the same file:
  `PrPanel.tsx:252-256` — "the reauth banner IS the notification (OD-3)".

Length is supporting evidence only: a two-sentence outcome + `Details: {e}` with an absolute path
does not belong in a 360px auto-dismissing toast with no `overflow-wrap: anywhere` guarantee, and
the user would lose it before reading it.

## B.3 Caller change (exact)

- **Delete** `PrPanel.tsx:247` — the `pushToast('error', ...)` line. Nothing else in that handler
  changes; `setConnectError(errorMessage(e))` on :246 stays.
- **Delete** `ChecksPanel.tsx:85` — same line, same rule. `setConnectError` on :84 stays.
- **Do not remove the `pushToast` prop** from either panel — verified, not assumed:
  `ChecksPanel.tsx:93` (`Could not open the check page: …`) and five other `PrPanel` call sites
  (`:177, :214, :285, :305, :311`) still use it legitimately (single-step failures and one success,
  §5 prefix correct). §12.14's "delete the capability where it is no longer needed" does not apply
  here; remove the **call**, not the parameter.
- **Rewrite the comment at `src/ipc/mock/handlers/forgeAddFailure.ts:22`** in the same edit — it
  documents the toast double-framing as a live defect, and would read as a stale accusation once
  the toast is gone.
- Remove any test asserting `Could not connect` for these two paths; replace with an assertion that
  the inline banner shows the message **and** that no toast is raised (the absence is the fix).

**A11y verified — no live-region gap.** `ForgeConnect.tsx:223-225` renders the `error` prop in
`<div className="error-banner error-banner-dismissible pr-error" role="alert">`, so the failure is
still announced once after the toast is gone. Both panels pass the state in (`PrPanel.tsx:384`,
`ChecksPanel.tsx:138`). Had it not been a live region, this ruling would have had to add
`role="alert"`; it does not. One utterance before, one utterance after — and today's two
(toast + banner) are in fact a **double** announcement, which is an additional reason to delete.

## B.4 Findings beyond the brief

1. **`forgeSetToken`/`forgeAddAccount` outcome copy now exists in Rust that this signed table does
   not cover** (`forge_set_token.rs`, `forge_add_account.rs`, and the `rolled-back|kept|
   rollback-failed` seams). It reads as rule-conformant, but it is **unsigned copy**. Flagged for a
   follow-up pass; deliberately not re-ruled here.
2. **The add-account callers are clean of the stutter.** `SettingsAccountAddForm.tsx:44-52` maps by
   `e.kind` to its own written sentences with no prefix, and `SettingsAccountCard.tsx:96` renders
   `errorMessage(e)` bare. No change needed — but `addError`'s fall-through must keep rendering
   outcome messages verbatim if it is ever edited.
3. **`ChecksPanel.tsx` has no `connectError` clear on success-then-reopen** beyond
   `setConnectError(null)` at submit; matches the §12.14 clear-at-start rule. No defect.
