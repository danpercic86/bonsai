# P113 FU — the forge failure mock seams, as they actually exist (2026-09-17)

**Owner:** architect · **Written:** 2026-09-17 · **Status:** descriptive — records shipped code, specs
one open re-wiring.
**Why a separate file:** `P113-settings-inline-notes.md` is ui-designer-owned and already 1301 lines.
Its §14 seam table and §3.3 backend note went stale on 2026-09-17 and the corrections belong somewhere
an architect can maintain. **This file is authoritative for the `?forgeRemoveFail=` /
`?forgeClearHostFail=` surface**; the P113 passages it supersedes are named in §5.

**Measurement note, binding on this file.** Every line number, count and character length below was
read out of the tree on 2026-09-17 at `14c1188`, and the method is stated inline per claim. The two
figures that are **not** mine are marked as inherited and attributed (§2.1, §3.1). No figure is
silently carried over from another document.

---

## 1. Two knobs, two namespaces — do not conflate them

`forgeRemoveFailure.ts` and `forgeClearHostFailure.ts` each carry a seam table whose **keys are URL
knob names**, and each also carries the 2026-09-17 ruling's **outcome numbers** (1 / 3 / 3') in its
header prose. **These are different namespaces.** `forgeRemoveFailure.ts:16-17` says so explicitly:

> `NB the '1' seam KEY below is a URL knob name, not outcome 1 — it selects the config-dir failure.`

`=1` is kept only because it is the legacy P113 §14 knob; it predates the outcome numbering and maps
to the pre-core `cannot resolve app config dir` rejection, which has **no** outcome number. A contract
or test that reads `=1` as "outcome 1" is wrong.

---

## 2. `?forgeRemoveFail=` — `src/ipc/mock/handlers/forgeRemoveFailure.ts`

Type: `ForgeRemoveFailSeam` (`:37-44`). Selector: `removeAccountRejection(seam, attempt)` (`:73`).
Wired at `handlers/forge.ts:389-404` (`forgeRemoveAccount`), with a per-`accountId` attempt ledger at
`:396-397`.

| Seam key | Outcome no. | Message source | Reachable via |
|---|---|---|---|
| *(absent / unrecognised)* | — | `null` ⇒ removal succeeds | default |
| `1` | **none** (legacy key) | `'cannot resolve app config dir: unknown path'` (`:76`) | Remove ▸ confirm |
| `long` | none (legacy key, pathological `{e}`) | `cannot resolve app config dir: ${LONG_CAUSE}` (`:78`); `LONG_CAUSE` is a space-free UNC path, described at `:61` as ~330 chars | Remove ▸ confirm |
| `keychain` | **1** | `REMOVE_KEYCHAIN_FAIL_MESSAGE` (`:47`) | Remove ▸ confirm |
| `settings` | **3** | `REMOVE_SETTINGS_FAIL_MESSAGE` (`:52`) | Remove ▸ confirm |
| `settings-no-credential` | **3'** | `REMOVE_SETTINGS_FAIL_NO_CREDENTIAL_MESSAGE` (`:58`) | Remove ▸ confirm |
| `keychain-then-ok` | 1, then success | outcome-1 text on `attempt === 1`, `null` after (`:86`) | Remove ▸ confirm twice |

Every failing seam leaves the Remove confirm dialog **open** (P113 §1.3 / §3.3 — unchanged, and still
the reason row 8 lands in `.dialog-error`).

### 2.1 Seeding widened — any value now seeds accounts

`forgeAccountStore.ts:58` is `const FORGE_REMOVE_FAIL_CASE = urlParam('forgeRemoveFail') !== null;`,
and it is OR-ed into both the account list (`:64`) and the host defaults (`:70-77`). Consequences,
each a behaviour change from the P113-era knob:

- `?forgeRemoveFail=<anything>` — **including an unrecognised value** — seeds
  `[FORGE_ACCOUNT_GITHUB, FORGE_ACCOUNT_LONG]` plus a `github.com` host default, with no
  `?forge=auth` needed. An unrecognised value therefore *seeds but never fails*: two accounts, Remove
  succeeds. That is a legitimate control state, not a bug.
- **`=1` was affected retroactively.** Before the widening it needed `?forge=auth` to have a row to
  remove; now it seeds on its own and shows **two** accounts rather than one.
- **`FORGE_LONG_HOST_CASE` (`:51`, `=== 'long'`) is now fully subsumed by `:58` and is load-bearing
  for nothing** — `!== null` already covers `'long'`, and both flags feed the same two expressions.
  It is harmless and is a cleanup candidate, not a distinction to preserve. Its header comment
  (`:45-50`) still explains *why the seed exists at all* — `=long`'s second pathological half is the
  **host** length, `FORGE_ACCOUNT_LONG`'s self-hosted GitLab host, described at `:49` as 61 chars —
  and that reasoning stays valid under `:58`.

*P113 §14 cites "~300-char" and "60-char" for these two. The two figures above (~330, 61) are the
source comments' own, quoted with attribution, not independently counted. Measure before introducing a
third pair.*

---

## 3. `?forgeClearHostFail=` — `src/ipc/mock/handlers/forgeClearHostFailure.ts`

Type: `ForgeClearHostFailSeam` (`:45-53`). Selector: `clearHostRejection(seam, host)` (`:93`), with an
internal per-host attempt ledger (`:85`, `:94-95`) — note the ledger is **inside** this module, unlike
the remove side where `forge.ts` owns it.

| Seam key | Outcome no. | Message source |
|---|---|---|
| *(absent / unrecognised)* | — | `null` ⇒ sign-out succeeds |
| `config-dir` | **none** (pre-core rejection) | `'cannot resolve app config dir: unknown path'` (`:98`) |
| `keychain` | **1** (k of N refused) | `CLEAR_HOST_KEYCHAIN_FAIL_MESSAGE` (`:60`) |
| `keychain-all` | **1**, all N+1 refused | `CLEAR_HOST_KEYCHAIN_FAIL_ALL_MESSAGE` (`:63`) — three causes joined with `'; '` |
| `keychain-no-account` | **1'** (host has no accounts) | `CLEAR_HOST_KEYCHAIN_FAIL_NO_ACCOUNT_MESSAGE` (`:68`) — must not claim rows are still listed |
| `settings` | **3** | `CLEAR_HOST_SETTINGS_FAIL_MESSAGE` (`:75`) |
| `settings-no-credential` | **3'** | `CLEAR_HOST_SETTINGS_FAIL_NO_CREDENTIAL_MESSAGE` (`:80`) — must not claim credentials were removed |
| `keychain-then-ok` | 1, then success | outcome-1 text on the host's first attempt, `null` after (`:110`) |

### 3.1 These seams are currently **INERT**, not merely console-only

**Do not document a way to reach them that does not exist.** Measured 2026-09-17:

- `forge_clear_token_for_host` is **not** on the invoke surface. `src-tauri/src/commands/mod.rs:58-59`
  is `#[cfg(test)] mod forge_clear_host;`, and `forge_clear_host.rs:9-10` carries the re-wiring recipe
  for the Rust half. *(That the drop was the security audit's INFO-2 and that there has been no UI
  caller since `323f8c5` is per the orchestrator's brief — not re-measured here.)*
- There is **no frontend method either.** A case-insensitive grep for `forgeClearTokenForHost` across
  `src/` returns only the doc comment at `forgeClearHostFailure.ts:88`. `ipc-api-forge.ts:58`,
  `tauri/forge.ts:66` and `mock/handlers/forge.ts:280` all declare the *per-repo*
  `forgeClearToken(repoId)` — a different command.
- Nothing imports `clearHostRejection`; nothing reads `urlParam('forgeClearHostFail')`. The module
  takes `seam` as a **parameter**, so the URL knob has no reader at all.
- `accountStore.removeAccountsForHost` (`forgeAccountStore.ts:144`) has **no caller**: a grep for the
  name across `src/` returns the declaration only. It is the other half of the same orphan.

So: **there is no control to click and no `ipc.*` method to call from the devtools console.** The knob
name is reserved, the copy is live (it is what the guard test pins, §4), and the behaviour is
unreachable until re-wired.

### 3.2 Re-wiring, when a caller returns (spec, not yet owed)

Four edits, in this order — anything less leaves the knob inert:

1. **Rust:** follow `forge_clear_host.rs:9-10` — re-add `#[tauri::command] pub async fn
   forge_clear_token_for_host` over `forge_clear_token_for_host_inner`, add it to `generate_handler!`,
   and drop the `#[cfg(test)]` from `commands/mod.rs:59`.
2. **IPC type:** `forgeClearTokenForHost(host: string): Promise<void>` on `src/ipc/types/ipc-api-forge.ts`.
3. **Tauri adapter:** `invoke<void>('forge_clear_token_for_host', { host })` in `src/ipc/tauri/forge.ts`.
4. **Mock handler:** in `src/ipc/mock/handlers/forge.ts`, mirroring `forgeRemoveAccount`
   (`:389-404`):

```ts
async forgeClearTokenForHost(host: string): Promise<void> {
  await delay(120);
  offGuard();
  const rejection = clearHostRejection(
    urlParam('forgeClearHostFail') as ForgeClearHostFailSeam,
    host,
  );
  if (rejection !== null) throw rejection;
  accountStore.removeAccountsForHost(host);   // forgeAccountStore.ts:144
}
```

The `keychain-no-account` / `settings-no-credential` seams need a host with **zero** accounts, so they
are only observable on a host the store was never seeded for (e.g. `'gitlab.com'` with no
`?forge=gitlab`), not on `github.com` under §2.1's seeding.

---

## 4. Changing any of this copy is a **three-part change** (four on the remove side)

Both mock modules are `include_str!`-ed by a Rust test that asserts every backend message fragment
appears in them:

| Guard test | Pins | Fragments asserted |
|---|---|---|
| `forge_remove_account_tests::mock_copy_mirrors_the_rust_copy` (`:193` onward; `include_str!` at `:194`) | `forgeRemoveFailure.ts` | `KEYCHAIN_FAIL_PREFIX`, `KEYCHAIN_FAIL_SUFFIX`, `SETTINGS_FAIL_PREFIX`, `SETTINGS_FAIL_SUFFIX` (`:196-199`), plus a quoted-start check built at `:211` (`format!("'{SETTINGS_FAIL_NO_CREDENTIAL_PREFIX}")`) — a plain `contains` is insufficient because the no-credential prefix is a **substring** of the ordinary one (`:206-209`) |
| `forge_clear_host_tests::mock_copy_mirrors_the_rust_copy` (`:308-338`) | `forgeClearHostFailure.ts` | `KEYCHAIN_FAIL_PREFIX`, `KEYCHAIN_FAIL_SUFFIX`, `KEYCHAIN_FAIL_NO_ACCOUNT_SUFFIX`, `SETTINGS_FAIL_PREFIX`, `SETTINGS_FAIL_SUFFIX` (`:311-315`), a `}{KEYCHAIN_FAIL_JOIN}${` check for the `'; '` separator (`:326`), and a backtick-quoted no-credential-prefix check (`:335`) |

**The mock is therefore NOT free-form.** Editing one user-facing message means editing:

1. the Rust `const` in `forge_remove_account.rs` / `forge_clear_host.rs`,
2. the TS mirror in the matching `handlers/forge*Failure.ts`,
3. the guard's fragment list / quoted-start probes if the message's *shape* changed (a prefix that
   stops being a prefix, a new suffix variant, a different join).

On the **remove** side there is a fourth: `src/components/settings/SettingsAccountsSection.remove.test.tsx:115-137`
enumerates every `?forgeRemoveFail` value and asserts the exact messages, including
`'long'.length > 300` (`:122`) and that outcome 3' never contains `'keychain'` (`:130`). Adding a seam
there without extending that test leaves the new value unpinned. **There is no clear-host equivalent**
— the only guard on that side is the Rust `include_str!`.

---

## 5. What this supersedes in `P113-settings-inline-notes.md`

Recorded here rather than edited there, because that file is ui-designer-owned and a full rewrite to
change three passages is not worth the collision risk. **Recommend the orchestrator apply a pointer at
each site**; until then, this file wins.

| P113 site | Why stale |
|---|---|
| §3.3, lines **119-126** | Claims `forge_remove_account_inner` "swallows both substantive failures", cites `src-tauri/src/commands/forge_accounts.rs:280-310`, and calls row 8 "near-unreachable in the real app" with the seams "mock-only" and the swallowing "a backend defect, filed separately". All four are false as of the 2026-09-17 ruling: the command no longer swallows (`forgeRemoveFailure.ts:11-17`), the code moved to `src-tauri/src/commands/forge_remove_account.rs`, and outcomes 1 / 3 / 3' are live. Row 8's design needed **no** change — which is what §3.3 predicted would happen. |
| §14 table, line **832** | `?forgeRemoveFail=1` is no longer the only remove seam — see §2 for all six. |
| §14 table, line **837** | "~300-char" / "60-char" pathological figures (source comments say ~330 and 61); also omits that any value now seeds (§2.1). |
| §14 intro, lines **816-820** | "four new knobs are required" was a pre-implementation count and no longer enumerates the surface. |

`?forgeClearHostFail=` is **not** in P113 at all — it postdates that contract entirely, which is the
other reason this file exists.

---

## 6. Acceptance criteria

1. **Seam inventory matches source.** The union of `ForgeRemoveFailSeam` (`forgeRemoveFailure.ts:37-44`)
   and `ForgeClearHostFailSeam` (`forgeClearHostFailure.ts:45-53`) equals §2's and §3's tables, minus
   the `null` arm. Checked by reading the two unions, not by grepping the contract.
2. **Reachability statement holds.** A case-insensitive repo grep for `forgeClearTokenForHost` outside
   `forgeClearHostFailure.ts` and this contract returns nothing, **or** §3.1 is rewritten and §3.2 is
   marked done. This is the criterion that expires first.
3. **Both guard tests green** — `cargo nextest run -p bonsai mock_copy_mirrors_the_rust_copy` (two
   tests; the filter is positional). They are the enforcement §4 describes.
4. **Harness, remove side only.** With `VITE_MOCK_IPC=1`, each of `?forgeRemoveFail=1|long|keychain|
   settings|settings-no-credential|keychain-then-ok` puts its §2 message into the Remove dialog's
   `.dialog-error` and leaves `role="dialog"` mounted. `keychain-then-ok` closes the dialog on the
   second confirm.
5. **Seeding.** `?forgeRemoveFail=zzz-not-a-seam` renders **two** accounts and a successful Remove —
   the §2.1 seed-but-never-fail state.
6. **No harness AC for the clear-host seams.** They are inert (§3.1); an AC that claimed to exercise
   them would be unrunnable. This absence is deliberate and is itself the flag.

---

## 7. Flags for the orchestrator

- **FL-1 (correction to the task brief).** The brief said the clear-host seams are reachable "by
  calling `ipc.forgeClearTokenForHost('github.com')` from the devtools console". **They are not** —
  there is no such method on the IPC API, in the Tauri adapter, or in the mock (§3.1). The seams are
  fully inert. `forgeClearHostFailure.ts` and `accountStore.removeAccountsForHost` are kept alive only
  by the Rust `include_str!` guard.
- **FL-2 (decide the orphan's fate).** Options: (a) leave inert with §3.2 as the recipe — the copy
  stays pinned to the Rust and costs nothing; (b) re-wire per §3.2 so the sign-out-a-host UI can
  return; (c) delete both the TS module and the `#[cfg(test)]` Rust module, losing the audited copy.
  **Recommend (a).** The copy is the audited artifact and the guard test keeps it honest; deleting it
  would have to be re-derived the next time host sign-out ships, and re-wiring a command with no UI
  caller re-opens the INFO-2 surface for nothing.
- **FL-3 (asymmetric test coverage).** The remove seams have a vitest enumeration; the clear-host
  seams have none (§4). If (b) is ever taken, a `forgeClearHostFailure` sibling of
  `SettingsAccountsSection.remove.test.tsx:115-137` should land in the same increment.
- **FL-4 (P113 pointers).** §5 lists four stale P113 sites with line numbers. They need a ~4-line
  dated pointer each; I did not edit that file (ui-designer-owned, 1301 lines, concurrent agent, and
  no `Edit` tool this session — see the report).
- **FL-5 (dead flag).** `forgeAccountStore.ts:51`'s `FORGE_LONG_HOST_CASE` is subsumed by `:58`
  (§2.1). One-line deletion; not urgent, but it will read as a meaningful distinction to the next
  person.
