# P91 — Amendment A26: raw-mode `args` is an allow-list, not a blanket include

**Status:** RATIFIED (architect, 2026-09-02). **Binding on the next P91 increment; blocks merge to `dev`.**
**Supersedes** `docs/contracts/P91-observability.md` §7.1 line 910 (the "Argument **values** … included as
`args`" row). Where this file and P91-observability.md disagree, **this file wins**. Everything else in
§7 (ordinals, §7.2 argsHash rules, §7.2.1 scrubber layers, §7.3 disclosure) is unchanged.

**Trigger:** security audit traced a shipped-in-branch leak to a self-contradiction in §7.1 —
line 910 (arg values included in raw) vs. line 914 (commit messages / search queries "never, in either
mode") vs. line 918 (tokens "NEVER, under any setting"). `src/obs/ipcProxy.ts:106-123` resolved it in
the leaking direction: every positional argument is serialised verbatim in raw mode.

**Not shipped.** P91 is branch-only and absent from `dev`, so there is no affected user, no migration
and no corpus to protect.

---

## A. Ruling

**§7.1 lines 914 and 918 are the contract. Line 910 is the defect.**

> **The one rule, stated once:** `raw` mode widens **identifier** fidelity — repo path, file paths, ref
> names, remote URLs, full SHAs — and **never** content fidelity. Free text and credentials are outside
> both modes. There is no setting, present or future, under which a commit message, a search string, a
> blame excerpt, file contents, an author name/email, or a token/password reaches a log file.

Justification (recorded so this is not re-litigated):

1. **Two absolute prohibitions beat one mechanism description.** "never, in either mode" and "NEVER,
   under any setting" are stated twice, in absolute terms, in the same table. Line 910 describes *how*
   arg capture works; it does not carve an exception out of a prohibition it never mentions.
2. **Consent is bounded by what the user was told.** `DevConfirmDialogs.tsx:39-42` promises, at the
   moment of consent, that commit messages and tokens are never written. Copy written from 914/918 is
   what the user agreed to; an implementation that exceeds it is a leak regardless of the table.
3. **§7.1 line 922 already defines raw's purpose** as "the only mode that puts **real names** on disk,
   and the user must be able to remove them". The entire raw column is de-ordinalisation of *names*.
   Content was never in that set.
4. **Fail-safe direction is mandatory here.** The designed workflow is enable → reproduce → export zip
   → send to a maintainer. A leaked PAT in a mailed archive is unrecoverable; a missing argument value
   costs only reviewer legibility. This is the same trade §13 row 19 already ratified permanently
   ("precision is subordinate to recall").

**Replacement text for §7.1 line 910** (apply verbatim; two rows replace one):

```
| Argument **values** — allow-listed scalar params only (§A26) | **elided** → `argsHash` + `argsShape` | allow-listed scalars only, keyed by param NAME; default **deny** |
| Free-text args (message, query, prompt, note, body) and credential args | **never, in either mode** | **never** |
```

---

## B. Producer mechanism (frontend) — allow-list, default deny

### B.1 Where the allow-list lives

**`src/obs/rawArgPolicy.json`** — data only, no logic, so it is diffable and reviewable on its own.

```jsonc
// key   = IpcApi method name (exact)
// value = one entry per positional parameter, in order:
//           "<paramName>" → MAY be included in raw mode
//           null          → always elided
// Trailing/extra actual args beyond the array are elided.
{
  "commit":               ["repoId", null, "sign", "skipHooks"],
  "commitMerge":          ["repoId", null, "skipHooks"],
  "searchCommits":        ["repoId", null],
  "forgeSetToken":        ["repoId", null],
  "forgeAddAccount":      ["host", "kind", null],
  "forgeSetTokenForHost": ["host", "kind", null]
}
```

These six rows are the **verified seed** (signatures read from `src/ipc/types/ipc-api.ts:65,207,289` and
`src/ipc/types/ipc-api-forge.ts:55,91,93`). Every other `IpcApi` method is **unlisted**.

**Default for an unlisted command: DENY — no `args` at all.** An unlisted command behaves exactly like
strict mode: `argsHash` + `argsShape` and nothing else. This is the whole point: a new IPC command
added tomorrow cannot leak, because silence means deny.

**The list is sparse and opt-in, not exhaustive.** An exhaustive `Record<keyof IpcApi, …>` was
considered and rejected: 199 rows of churn buys no additional safety once the default is deny, and a
renamed command silently becomes deny (safe) rather than breaking the build. The forcing function is
placed on the *dangerous* action instead — **adding** a row (§B.3).

### B.2 Derivation rule for adding a row

A parameter name may be listed **only if all four hold**:
1. its declared TS type is a **scalar** (`string`, `number`, `boolean`, a string-literal union, or an
   `undefined`/`null` union thereof) — never an object, array, `unknown`, or `Record<…>`;
2. it is an **identifier**: id/handle (`repoId`, `sha`, `oid`), path, ref/branch/tag/remote name, URL,
   enum/flag, or count;
3. its name fails `isFreeTextParam` and `isSensitiveParam` (§B.3);
4. it cannot carry user-authored prose or a secret **by value**, whatever the type says.

Anything else gets `null`. When in doubt: `null`.

### B.3 Vocabulary guards (shared by producer and writer)

```ts
// src/obs/rawArgPolicy.ts
export const RAW_ARG_MAX_STR = 512;

/** Credential vocabulary. Mirrors Rust `scrub.rs::is_sensitive_key` + explicit extras. */
export function isSensitiveParam(name: string): boolean;
//   /token|secret|password|passphrase|auth|credential|apikey|api_key|privatekey|sshkey|\bpat\b/i

/** Free-text vocabulary — raw `args` keys ONLY. Never applied to general JSON keys. */
export function isFreeTextParam(name: string): boolean;
//   /message|msg|query|search|text|body|prompt|descri|note|content|comment|title|
//    subject|summary|patch|diff|blurb|input|reason/i
```

`isFreeTextParam` is **scoped to raw `args` keys and nothing else**. It must NOT be folded into the
general scrubber's `is_sensitive_key`: `ErrorPayload.message` is a legitimate, already-scrubbed field
and collapsing it would blind every error record.

### B.4 Emission

```ts
// src/obs/rawArgPolicy.ts
export interface RawArgsResult { args?: Record<string, unknown>; omitted: number }
export function buildRawArgs(cmd: string, args: readonly unknown[]): RawArgsResult;
```

`buildRawArgs` includes position `i` iff **all** of:
- `RAW_ARG_POLICY[cmd]?.[i]` is a non-null string `name`;
- `!isFreeTextParam(name) && !isSensitiveParam(name)`;
- the actual value is a **scalar**: `string | number | boolean | null` (objects, arrays and functions
  are always elided, regardless of the table — this alone kills `searchCommits(_, query: SearchQuery)`
  and any nested `{ token }`);
- if a string: `length <= RAW_ARG_MAX_STR` and it contains no `\n` or `\r`.

Result keying: **by parameter name** (`{ "repoId": "r1", "sign": false }`), never by `"0"`/`"1"`.
`omitted` = count of positions the policy or the checks removed. `args` is omitted entirely when empty.

`src/obs/ipcProxy.ts:106-123` is replaced by a call to `buildRawArgs`; the inline
`Object.fromEntries(args.map((v, i) => [String(i), v]))` construction is deleted.

### B.5 What does NOT change

- **`argsHash`** — still the salted FNV-1a-64 over the **full positional JSON array of all arguments**,
  computed pre-injection. Denied args still participate: the digest never leaves the process, the salt
  never reaches a file, and `dup-ipc` needs full discrimination (§7.2, §13 row 20 intact).
- **`argsShape`** — still keyed `"0"`, `"1"`, … for **all** positions, values-free. It is how a reader
  aligns the name-keyed `args` back to arity, and it is the record of what was elided.
- **`ipc.recv`** still carries no `argsHash`/`argsShape`/`args` (§13 row 20 prohibition intact).

### B.6 Schema additions (§3 `IpcCallPayload`)

```ts
interface IpcCallPayload {
  cmd: string; argsHash: string; argsShape?: ArgShape;
  /** raw mode only; keyed by PARAM NAME, allow-listed scalars only (§A26). */
  args?: Record<string, unknown>;
  /** raw mode only, emitted only when > 0: positions the policy elided. */
  argsOmitted?: number;
  /** WRITER-SET ONLY. A producer must never emit it; Rust's serde struct has no
   *  such field, so a forged one is dropped at deserialisation. (§D) */
  argsPolicyViolation?: boolean;
}
```

All three are optional and additive. **`OBS_SCHEMA_VERSION` stays 1**, explicitly, under the §3
amendment rule / §13 row 23 pre-release carve-out: P91 is branch-only, no reader parses records from
disk, and no v1 corpus exists. The carve-out still expires on merge to `dev`.

---

## C. Writer-side enforcement (Rust) — independent of the table, by design

The producer proposes; the writer enforces. **The writer does not consult `rawArgPolicy.json`.** A
shared table would make writer enforcement worthless against the failure mode that actually matters
(a wrong row, or code that ignores the table — today's bug). The writer instead enforces a
**shape + vocabulary invariant** it can decide alone, which catches *both* a producer code bug and a
bad table row.

```rust
// src-tauri/src/obs/raw_args.rs   (new file, ~120 lines)
pub const RAW_ARG_MAX_STR: usize = 512;

/// §A26 — enforces the raw-mode `args` invariant on a serialized record, IN PLACE.
/// Runs in BOTH redaction modes, on every record. Returns true iff it dropped an
/// `args` object, in which case it also sets `argsPolicyViolation: true`.
pub fn enforce(v: &mut Value) -> bool;

fn is_valid_param_key(k: &str) -> bool;   // ^[a-z][A-Za-z0-9]*$
fn is_free_text_param(k: &str) -> bool;   // §B.3 vocabulary; NOT used by scrub.rs
fn is_denied_param(k: &str) -> bool;      // is_free_text_param || scrub::is_sensitive_key + extras
fn is_allowed_scalar(v: &Value) -> bool;  // bool|number|null|string(<=MAX, no \n/\r)
```

`scrub.rs::is_sensitive_key` becomes `pub(super)` so `raw_args.rs` reuses the one vocabulary rather
than forking it. Its behaviour is otherwise untouched (including the deliberate `auth`→`author` match,
scrub.rs:62-66).

**Rules — any violation drops the WHOLE `args` object, never just the offending entry.** A producer
that mislabelled one argument is untrusted about the rest.

| | Rule |
|---|---|
| W1 | `args` may appear only on `kind == "ipc.call"`. Anywhere else ⇒ remove (strict mode already removes it unconditionally at `strict.rs:167`; keep that). |
| W2 | `args` must be a JSON **object**. |
| W3 | Every key matches `^[a-z][A-Za-z0-9]*$`. **A numeric key (`"0"`, `"1"`) fails ⇒ drop.** This is the single rule that kills today's leak at the writer even if the frontend is never fixed. |
| W4 | No key satisfies `is_denied_param`. |
| W5 | Every value satisfies `is_allowed_scalar` — objects and arrays are categorically ineligible. |
| W6 | `argsOmitted`, if present, must be a non-negative integer; otherwise remove that field. |

Pipeline order in `writer.rs::append_record` (currently `:272-283`):

```
serialize → strict::enforce (strict only) → raw_args::enforce (ALWAYS) → scrub_value → scrub_salt
```

`raw_args::enforce` runs **before** the credential scrubber so a violating object is gone before
anything can partially "rescue" it, and the surviving allow-listed scalars still pass through
`scrub_value` and `scrub_salt` normally.

`argsPolicyViolation` is injected into the `serde_json::Value` after serialisation. Rust's
`IpcCallPayload` struct must **not** declare the field, so a frontend that sends one has it dropped as
an unknown field — the flag cannot be forged.

---

## D. The positional-key blind spot in scrubbing

The audit is right: `scrub_value` (`scrub.rs:346`) applies `is_sensitive_key` to JSON object **keys**,
so positionally-keyed `args` (`"0"`, `"1"`, …) matched nothing, and survival fell to
`looks_like_opaque_secret` alone — which requires length ≥ 32 and exempts all-hex words to protect
SHAs, so a 40-hex Forgejo token and a 20-char Bitbucket app password walked straight through.

**Both halves of the fix are required, not either/or:**

1. **Thread the names through** — §B.4 keys `args` by parameter name, so `is_sensitive_key` becomes
   meaningful inside `args` for the first time (`token`, `password`, `authHeader` now match).
2. **Make numerically-keyed containers ineligible** — W3 drops any `args` whose keys are not parameter
   names. Name-keying is a producer property; W3 is the writer's independent guarantee that it holds.

No change to `looks_like_opaque_secret`, `MIN_SECRET_LEN`, `MIN_B64_RUN`, or the hex exemption — §13
row 19 ratified those and they are load-bearing for SHAs. The shape heuristic was never the right
defence for a known-credential parameter; the key vocabulary is.

---

## E. Mock IPC / browser harness

The proxy wraps `IpcApi` identically in mock and Tauri modes, so `buildRawArgs` is exercised under
`VITE_MOCK_IPC=1` with no Rust present. `src/ipc/mock.ts` needs **no change** — this amendment adds no
command, event or channel. The harness asserts the **producer** half via `__bonsaiDumpLogs()`; the
**writer** half has no mock counterpart (no Rust in the browser) and is covered by AC6/AC7 in Rust.

---

## F. Consent copy — FLAGGED TO `ui-designer` (not mine to write)

Under this ruling the existing copy becomes **true**, so the required change is small but it is not
zero, and it is `ui-designer`'s file either way:

- `src/components/settings/DevConfirmDialogs.tsx:39-42` — "Commit messages, file contents, author names
  and email addresses are still never written, and passwords and access tokens are never written in any
  mode." **Now accurate.** Should additionally state that **search terms and other free-text arguments**
  are not written, since the user's mental model of "raw" is "everything".
- `src/components/settings/SettingsDevPrivacySection.tsx` — the §7.3 always-visible content statement
  must describe raw mode as "**real repository, file, branch and remote names**", not "arguments", so
  the identifier/content distinction is legible at the point of consent.

**No copy change is required for correctness under this ruling** — only for completeness. If the
orchestrator instead ever chose the opposite ruling, both files would need a hard warning and the
export flow would need a re-consent; that is recorded here only to show the asymmetry, not as an option.

---

## G. §13 decision-record row (paste into `P91-observability.md` §13)

| 26 | **§7.1 raw-mode `args` self-contradiction — a shipped-in-branch privacy leak** (found by security audit, 2026-09-02) | **RESOLVED — lines 914/918 are the contract; line 910 is the defect and is REPLACED.** The one rule: **`raw` widens *identifier* fidelity (paths, refs, remotes, SHAs) and never *content* fidelity**; free text and credentials are outside both modes, permanently. `ipcProxy.ts:106-123` serialised every positional argument verbatim, so Dev + raw wrote **full commit messages** (`commit`), **search text** (`searchCommits`) and **cleartext forge PATs** (`forgeSetToken`/`forgeAddAccount`/`forgeSetTokenForHost`) into `logs/*.jsonl` — into the very zip the workflow tells the user to mail a maintainer, while the consent dialog promised the opposite. The scrubber could not save it: `is_sensitive_key` matches JSON **keys**, and `args` was keyed `"0"`,`"1"`, so survival fell to `looks_like_opaque_secret`, which needs len ≥ 32 and exempts all-hex (to protect SHAs) — a 40-hex Forgejo token and a 20-char Bitbucket app password passed through. **Mechanism: a sparse per-command allow-list (`src/obs/rawArgPolicy.json`), default DENY for any unlisted command**, allow-listed positions keyed by **parameter name**, **scalars only** (objects/arrays categorically ineligible), plus free-text and credential name vocabularies. **Writer-side enforcement is INDEPENDENT of the table** (`obs/raw_args.rs`, runs in both modes, before the scrubber): non-`ipc.call`, non-`^[a-z][A-Za-z0-9]*$` keys (kills positional keys), denied key names, non-scalar or >512-char/multiline values ⇒ **drop the whole `args` object** + writer-set `argsPolicyViolation`. A shared table was rejected precisely because it would not defend against a bad row. `argsHash`/`argsShape` unchanged (§13 row 20 intact). **`OBS_SCHEMA_VERSION` stays 1** — additive optional fields under the §13 row 23 pre-release carve-out; P91 has never shipped, so there is no migration. Consent copy becomes true; completeness edits flagged to ui-designer | §7.1 (row replaced), §7.4/A26, §3 `IpcCallPayload`, §6 writer pipeline, `docs/contracts/P91-raw-args-privacy.md` |

---

## H. Acceptance criteria

1. `src/obs/rawArgPolicy.json` exists with the six seed rows of §B.1 verbatim, and
   `src/obs/rawArgPolicy.ts` exports `RAW_ARG_POLICY`, `RAW_ARG_MAX_STR`, `isSensitiveParam`,
   `isFreeTextParam`, `buildRawArgs`.
2. `src/obs/ipcProxy.ts` no longer contains any `String(i)` / positional-key construction of `args`;
   its only path to `args` is `buildRawArgs`. Grep for `Object.fromEntries(args` returns nothing.
3. **Default-deny test (vitest):** an `ipc.call` for a command absent from the policy, in raw mode,
   emits **no** `args` key and still emits `argsHash` + `argsShape`.
4. **Producer test (vitest):** `commit('r1', 'FIXTURE_MESSAGE_ZQX', false, true)` in raw mode emits
   `args === { repoId: 'r1', sign: false, skipHooks: true }`, `argsOmitted === 1`, and the serialised
   record contains no occurrence of `FIXTURE_MESSAGE_ZQX`. Same for `forgeSetToken('r1', 'ghp_…')`
   (only `repoId`), `forgeAddAccount('h','github','tok…')` (only `host`, `kind`), and
   `searchCommits('r1', { text: 'FIXTURE_QUERY_ZQX' })` (object arg ⇒ elided by shape alone).
5. **Vocabulary test (vitest):** every non-null name in `rawArgPolicy.json` fails both
   `isFreeTextParam` and `isSensitiveParam`, and every key of the JSON is an existing `IpcApi` method
   name. This test is the forcing function on future additions — it must fail if someone lists
   `message`, `token`, `prompt`, …
6. **NEGATIVE TEST — writer, the one that must exist (`cargo test`):** open a `LogWriter` in
   `RedactionMode::Raw`; append (a) an `ipc.call` for `commit` whose `args` is
   `{"0":"r1","1":"NEGTEST_COMMIT_MESSAGE_ZQX","2":false}` — i.e. exactly what the buggy producer
   emits — and (b) an `ipc.call` for `forgeSetToken` whose `args` is
   `{"repoId":"r1","token":"a1b2c3d4e5f60718293a4b5c6d7e8f9012345678"}` (40-hex: passes
   `looks_like_opaque_secret`'s hex exemption, so only the key rule can catch it). Read the file back
   and assert: the bytes contain **neither** `NEGTEST_COMMIT_MESSAGE_ZQX` **nor** the token substring;
   both records carry `argsPolicyViolation: true`; both records still carry `cmd`, `argsHash` and
   `argsShape`. This test must fail on today's `main`-of-branch code.
7. **Writer unit tests (`cargo test`):** `raw_args::enforce` drops `args` on each of W1–W5
   independently; preserves a conforming `{"repoId":"r1","sign":false}`; is a no-op on records without
   `args`; and a producer-supplied `argsPolicyViolation: true` on an otherwise-clean record does not
   survive (unknown field dropped at deserialisation).
8. `raw_args::enforce` is called unconditionally in `writer.rs::append_record`, positioned after
   `strict::enforce` and **before** `redact::scrub_value`. A test asserts strict mode still removes
   `args` entirely (existing `strict.rs:167` behaviour unregressed).
9. `OBS_SCHEMA_VERSION` is still `1`; no field added by this amendment is required.
10. **Harness (`VITE_MOCK_IPC=1`):** with raw mode on, `__bonsaiDumpLogs()` shows name-keyed `args` for
    an allow-listed command and no `args` for an unlisted one. Existing P91 harness assertions
    (anomalies, dup-ipc) still pass — `argsHash` is unchanged, so `dup-ipc` must still fire.
11. `pnpm gate` green; no new clippy/eslint suppressions.
12. Consent copy (§F) re-verified true by `ui-designer` before the P91 USER CHECKPOINT.

---

## I. Flags for the orchestrator

- **F1 — I could not patch `P91-observability.md` in place** (architect has no Edit tool; the file is
  1455 lines and a full rewrite is neither safe nor token-affordable). Line 910 still reads the wrong
  thing. Please apply the §A replacement rows at line 910 and paste the §G row into §13, or route that
  one-line edit to `docs-curator`. Until then this file's "supersedes" clause is the only thing holding
  the contradiction closed — that is a documentation risk, not an implementation one, since senior-dev
  gets this path.
- **F2 — seed allow-list size.** I listed six rows, all of which exist to *document a denial* while
  letting `repoId`/`sign`/`host`/`kind` through. That means raw mode currently yields almost no
  argument values, which is a real debuggability reduction. If you want raw mode to remain useful for
  path/ref debugging, the follow-up is to add rows for the path- and ref-taking commands under §B.2 —
  a mechanical pass senior-dev can do in the same increment. **My recommendation: do it in this
  increment**, because a raw mode that shows nothing invites someone to "fix" it by reverting to a
  blanket include.
- **F3 — no escalation needed on the ruling itself.** It is unambiguous and the leaking alternative is
  not defensible given the consent copy and the export-and-mail workflow.
