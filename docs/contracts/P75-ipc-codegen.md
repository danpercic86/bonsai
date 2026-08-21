# P75 — Generate the IPC boundary from Rust: Implementation Contract

> **STATUS: HALTED 2026-08-21 (user decision).** The Phase 6.1 spike proved the RC crates
> build and pin, but linking `tauri-specta` breaks app launch on Windows 10 (the blocker
> below). P75 is a developer-velocity refactor (it kills the 4-file IPC lockstep) with no
> user-facing value, built on release-candidate crates, and it cannot be completed without
> linking tauri-specta into the app (Phase 6.5) — so the Win10 regression is unavoidable by
> sequencing. Decision: keep the hand-written IPC lockstep; do NOT adopt tauri-specta. The
> 6.1 code/dep changes were reverted; this contract + the findings below are retained so a
> future revisit (e.g. once validated on Windows 11, or with a link-order fix for the
> kernel32 api-set import) starts from the known state rather than re-discovering it.

> **Pinned trio (P75, resolved empirically in Phase 6.1):**
> `specta =2.0.0-rc.22` / `tauri-specta =2.0.0-rc.21` / `specta-typescript =0.0.9`
> (the contract's documented-coherent set). The newest set (`tauri-specta =2.0.0-rc.25`
> + `specta =2.0.0-rc.25` + `specta-typescript =0.0.12`) was **rejected**: it *builds* but
> (a) `specta-typescript 0.0.12` removed `BigIntExportBehavior::Number` (the §1.3-mandated
> global `u64/usize → number` mapping; 0.0.12 only offers forbid/`enable_lossless_bigints`
> (→`bigint`) or per-field `#[specta(type = Number)]`), and (b) the export test fails to
> RUN on this toolchain (same blocker as below). rc.22/rc.21/0.0.9 retains
> `BigIntExportBehavior::Number` and matches the contract's API snippets.
>
> **⚠ BLOCKER (Phase 6.1, Windows 10):** linking `tauri-specta` forces the `tauri/specta`
> feature, and the resulting binary statically imports `WaitOnAddress`/`WakeByAddress*`
> from **`kernel32.dll`**. Windows 10 (this dev box: 19045) does NOT export those from
> `kernel32.dll` (they live in KernelBase/the `api-ms-win-core-synch` api-set), so the
> `bonsai_lib` test binary — and, by extension, the real app — fails to load with
> `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)`. `bonsai-core` (specta only, no tauri) loads
> fine; the trigger is specifically `tauri-specta`+`tauri`. This very likely works on
> Windows 11 (kernel32 forwards those symbols). Must be resolved before Phase 6.5 wires
> the builder into the app: verify on Win11 and/or pursue a linker fix (force the api-set
> import lib ahead of `kernel32.lib`). See senior-dev's Phase 6.1 report.

Status: authoritative for P75. Implementer: senior-dev, ~9 fresh-context sub-increments (§6). This
is a **structural / behavior-preserving** milestone: the shipped IPC surface (command names, arg
order, return shapes, error semantics, events, channels) must be **byte-for-byte identical** on the
wire before and after. No product behavior changes. No call-site churn.

Motivation (velocity analysis): every one of the **173 `#[tauri::command]`s** is hand-synced across
four top-8-churned files — `src/ipc/types.ts` (2701 LOC), `src/ipc/tauri.ts` (1084 LOC),
`src/ipc/index.ts` (barrel), and the mock (`src/ipc/mock/handlers/*.ts` + `fixtures/*.ts`). Adding
one command = 4–5 coordinated manual edits and a constant drift/bug source. This contract makes the
**Rust command signatures + serde types the single source of truth** and generates the TS types +
typed invoke wrappers + typed event listeners from them.

Current file state (read before implementing):
- `src-tauri/src/lib.rs` — the `generate_handler![… 173 commands …]` list (lines 100–263) and the
  `setup`/`run` closures.
- `src-tauri/src/commands/mod.rs` — the domain module list + `pub use <domain>::*` re-exports.
- `src-tauri/src/commands/{repo,status,history,ai_stream}.rs` — the **4 `tauri::ipc::Channel<T>`
  commands** (`clone_repo`/`CloneProgress`, `stream_graph`/`GraphChunk`, `history_index_build`/
  `IndexProgress`, `ai_resolve_conflict_stream`/`AiRunEvent`).
- `crates/bonsai-core/src/error.rs` — `AppError` (thiserror enum with a **manual `serde::Serialize`
  impl** at line 182 producing `{ kind, message }` — the one type specta cannot derive; §1.4).
- `src/ipc/types.ts` — hand-written TS mirrors + the `IpcApi` interface (170 methods, line 1984).
- `src/ipc/tauri.ts` — hand-written `invoke()` wrappers, the Channel bridges, event `listen`
  subscriptions, and the JS-side updater/dialog/window-focus methods.
- `src/ipc/index.ts` — barrel: dynamic `mock` vs `tauri` selection + a 200-line type re-export.
- `src/ipc/mock.ts` + `src/ipc/mock/handlers/*.ts` (31 files) — `mockIpc: IpcApi` assembled from
  per-domain handler groups spread together.

---

## 0. Tool choice — tauri-specta v2 (RECOMMENDED), not ts-rs

**Recommendation: `tauri-specta` v2 + `specta` + `specta-typescript`.**

| Criterion | `ts-rs` | `tauri-specta` v2 |
|---|---|---|
| Generates TS **types** from serde types | yes | yes (via `specta`) |
| Generates typed **`invoke` wrappers** | **no** | **yes** (a `commands` object) |
| Generates typed **event** listeners | no | yes (`collect_events!`) |
| Honors `#[serde(rename_all)]`, `rename`, tag modes | partial | yes |
| Kills which of the 4 churned files | only `types.ts` | `types.ts` + `tauri.ts` + shrinks the barrel |

The dominant churn is the **invoke wrappers** (`tauri.ts`, 85 touches) and the **event/channel
plumbing**, which `ts-rs` cannot generate — it would leave the lockstep 3-files-wide. `tauri-specta`
collapses it to one generated file. Its two costs are both small and bounded for this codebase:
1. It is a **release candidate** (must version-pin). Mitigated by pinning with `=` (§1.1) and the
   staleness CI gate (§5) which catches any rc-to-rc output drift immediately.
2. **`Channel<T>` is unsupported** ("coming soon"). Bonsai has exactly **4** channel commands, all
   already hand-shaped as callback-bridge methods in today's facade — they stay hand-written (§4.3).

### 1.1 Exact crates / versions

Pin all three with `=` (rc APIs break between patches). Add to `src-tauri/Cargo.toml`:

```toml
[dependencies]
specta = { version = "=2.0.0-rc.22", features = ["derive"] }
tauri-specta = { version = "=2.0.0-rc.21", features = ["derive", "typescript"] }

[build-dependencies]  # unchanged
```
```toml
[dev-dependencies]
specta-typescript = "=0.0.9"
```

`specta` also needs `derive`s on the workspace type crates. Add to `crates/bonsai-core/Cargo.toml`,
`crates/bonsai-forge/Cargo.toml` (any crate defining a wire type):
```toml
specta = { version = "=2.0.0-rc.22", features = ["derive"] }
```
Add `specta = "=2.0.0-rc.22"` to `[workspace.dependencies]` in the root `Cargo.toml` and reference
it as `specta = { workspace = true, features = ["derive"] }` from each member, mirroring the
existing `serde`/`git2` workspace-dep pattern.

> **AMBIGUITY (flag to orchestrator):** rc.21/rc.22 is the last set the maintainer documented as
> coherent (`specta 2.0.0-rc.22` + `tauri-specta 2.0.0-rc.21` + `specta-typescript 0.0.9`).
> `tauri-specta 2.0.0-rc.25` exists but its coherent `specta`/`specta-typescript` pairing is not
> documented. **Recommendation:** senior-dev pins the newest set that `cargo build` + the export
> test (§5) accept, and records the exact trio in this contract's header. Do not float any of them.

---

## 2. Module layout — what is generated vs hand-written

```
src/ipc/
  generated/
    bindings.ts        # GENERATED, do-not-edit. specta types + `commands` + `events`. Committed.
  adapter.ts           # hand-written, generic. Result<T,E> -> throw-AppError unwrap (§3.2).
  extras.ts            # hand-written. The non-command surface: 4 channels, 4 events, dialog,
                       #   updater, window-focus (§4.3). ~10 methods, built on generated types.
  index.ts             # barrel. Assembles `ipc`, swaps mock, re-exports types. Public API STABLE.
  types.ts             # SHRINKS to a thin re-export from ./generated/bindings (§2.2). Kept so the
                       #   ~200 `from './ipc/types'` / `from './types'` import sites do not churn.
  mock.ts, mock/**     # unchanged structure; now typed against the generated IpcApi (§3, §6 phase).
```

### 2.1 Generated `bindings.ts` surface (what tauri-specta emits)

- **All wire types** as `export type` (every `#[derive(specta::Type)]` reachable from a collected
  command or event, transitively). camelCase fields (§1.3).
- **`export const commands = { … }`** — one typed function per `#[specta::specta]` command, keyed by
  the **camelCase** of the snake_case command name (`open_repo` → `commands.openRepo`), **positional
  args in Rust declaration order** (special args `AppHandle`/`State`/`Window` dropped). Each returns
  `Promise<Result<T, AppError>>` where `Result` is specta's discriminated union
  `{ status: "ok"; data: T } | { status: "error"; error: AppError }`.
- **`export const events = { … }`** — typed `listen`/`once`/`emit` handles for each `collect_events!`
  type.
- **`export type Result<T, E>`** — the union above.

Bonsai's facade returns `Promise<T>` and **rejects** with `AppError`; the generated `commands` return
the non-throwing union. `adapter.ts` (§3.2) reconciles this **generically, once** — not per command.

### 2.2 `types.ts` after migration (keeps import sites stable)

`types.ts` becomes a re-export barrel so the ~200 `import type { … } from './ipc/types'` sites are
untouched:
```ts
export * from './generated/bindings';       // all wire types
export type { IpcApi } from './index';       // the assembled facade type (§3.1)
// Hand-owned types that are NOT Rust-derived stay here or move to a sibling and are re-exported:
//   Unsubscribe, CloneProgress-callback shapes only if not Rust-derived, UI-only helper aliases.
```
Any type currently in `types.ts` that has **no** Rust origin (pure-frontend helper types) must be
identified during phase 6.1 and relocated to `src/ipc/frontendTypes.ts`, re-exported from `types.ts`.
The generated file must never be hand-edited.

---

## 3. The `IpcApi` facade — stable public API, generically derived

### 3.1 `IpcApi` type

`IpcApi` stays the single interface every call site and the mock implement, but it is now **derived**
from the generated `commands` (so a new Rust command auto-extends it) plus the hand-written
`ExtrasApi`:

```ts
// index.ts
import { commands } from './generated/bindings';
import type { ExtrasApi } from './extras';

/** A generated command fn `(...args) => Promise<Result<T, AppError>>` reshaped to the
 *  facade contract `(...args) => Promise<T>` (rejects with AppError). */
type Unwrapped<F> =
  F extends (...args: infer A) => Promise<Result<infer T, AppError>>
    ? (...args: A) => Promise<T>
    : never;

type CommandsApi = { [K in keyof typeof commands]: Unwrapped<(typeof commands)[K]> };

export type IpcApi = CommandsApi & ExtrasApi;
```

Adding a `#[specta::specta]` command in Rust → regenerate → `commands` gains a key → `CommandsApi`
gains a method → `IpcApi` gains it → **`mockIpc: IpcApi` fails typecheck until a mock is added**
(§3.3) and `tauriIpc: IpcApi` is satisfied automatically by the adapter (§3.2). This is the whole
point: the 4-file lockstep collapses to "add the Rust command + add one mock handler."

### 3.2 `adapter.ts` — generic Result→throw unwrap (one function, no per-command code)

```ts
import { commands } from './generated/bindings';
import type { Result } from './generated/bindings';
import type { AppError } from './generated/bindings';

function unwrap<T>(r: Result<T, AppError>): T {
  if (r.status === 'ok') return r.data;
  throw r.error;                         // preserves the reject-with-AppError contract verbatim
}

/** Maps every generated command to its throw-on-error facade form. */
export function unwrapCommands(): CommandsApi {
  const out = {} as Record<string, (...a: unknown[]) => Promise<unknown>>;
  for (const key of Object.keys(commands) as (keyof typeof commands)[]) {
    const fn = commands[key] as (...a: unknown[]) => Promise<Result<unknown, AppError>>;
    out[key] = (...args) => fn(...args).then(unwrap);
  }
  return out as unknown as CommandsApi;
}
```

### 3.3 Real `tauriIpc` and the barrel

```ts
// index.ts
import { unwrapCommands } from './adapter';
import { tauriExtras } from './extras';

const tauriIpc: IpcApi = { ...unwrapCommands(), ...tauriExtras };

export const ipc: IpcApi =
  import.meta.env.VITE_MOCK_IPC === '1'
    ? (await import('./mock')).mockIpc
    : tauriIpc;
```
`tauri.ts` is **deleted** at the end of §6; its only surviving logic (the 4 channel bridges, updater,
dialog, window-focus) moves verbatim into `extras.ts`.

> **AMBIGUITY (flag):** a handful of facade methods have **optional** args that the generated
> positional signature makes **required-nullable** — `commit(repoId, message, sign?: boolean|null,
> skipHooks?)`, `listStaleBranches(repoId, base?)`, `deleteBranches(repoId, names, base?)`,
> `listAiAssets(repoId, canonical?)`, `registerMcpWithClaude(scope, repoPath|null)`. tauri-specta
> emits these as required (`sign: boolean | null`). **Recommendation:** for the ≤10 affected
> commands, add a tiny per-command trailing-default shim in `extras.ts` that overrides the unwrapped
> version (spread order: `{ ...unwrapCommands(), ...tauriExtras }` lets extras win). This keeps
> call sites (`ipc.commit(id, msg)`) unchanged. senior-dev enumerates the exact set during phase 6.6
> by diffing generated arity vs the old `IpcApi` optionals; it is small and bounded.

---

## 4. Commands, events, channels under the new scheme

### 4.1 Commands (request/response) — generated

Every non-channel `#[tauri::command]` gains `#[specta::specta]`. All are collected in a single
builder (§5). Positional args, `Result<T, AppError>` return → generated `commands.<camel>`.

### 4.2 Events (push signals) — generated typed listeners

The 3 Rust-emitted events become `#[derive(specta::Type, tauri_specta::Event)]` on their payloads
and are registered with `collect_events!`:

| Event name | Payload type | Emitted from |
|---|---|---|
| `repo-changed` | `RepoChangedPayload` | `commands/repo.rs`, `watcher.rs`, `scheduler.rs` |
| `job-status-changed` | `JobStatusChangedPayload` | `scheduler.rs` |
| `mcp-server-changed` | `McpStatus` | `mcp.rs` |

**Emit-site change:** raw `app.emit("repo-changed", payload)` becomes the typed
`RepoChanged(payload).emit(&app)` form tauri-specta generates. The event **names on the wire stay
identical** (`tauri_specta::Event` derives the kebab name from the struct, or is pinned via
`#[serde(rename)]`/the derive attr — senior-dev asserts the emitted string equals the current name
in a unit test so no listener breaks). The frontend `events.repoChanged.listen(cb)` handle wraps
`@tauri-apps/api/event::listen` and returns an unlisten fn; `extras.ts` adapts it to the existing
`onRepoChanged(cb) => Promise<Unsubscribe>` facade signature (thin, generic).

`onWindowFocus` is **not** a Rust event (it is `getCurrentWindow().onFocusChanged`) — it stays fully
hand-written in `extras.ts`.

### 4.3 Channels (streaming) — hand-written in `extras.ts`, payload types still generated

The 4 channel commands are **excluded from `collect_commands!`** (they keep plain `#[tauri::command]`,
no `#[specta::specta]`, because `Channel<T>` is not a `specta::Type`). Their wrappers move verbatim
from `tauri.ts` into `extras.ts` and are part of `ExtrasApi`:

| Facade method | Command | Channel payload |
|---|---|---|
| `cloneRepo(url, dest, onProgress)` | `clone_repo` | `CloneProgress` |
| `streamGraph(repoId, onChunk)` | `stream_graph` | `GraphChunk` |
| `historyIndexBuild(repoId, onProgress)` | `history_index_build` | `IndexProgress` |
| `aiResolveConflictStream(…, onEvent)` | `ai_resolve_conflict_stream` | `AiRunEvent` |

**Payload type inclusion (the one gotcha):** `CloneProgress`, `GraphChunk`, `IndexProgress`,
`AiRunEvent` (and their nested types) are referenced by no collected command/event, so specta will
not emit them by default. Force them into the collection via the builder's explicit type API:
```rust
builder.typ::<CloneProgress>();   // or the rc's TypeCollection::register equivalent
builder.typ::<GraphChunk>();
builder.typ::<IndexProgress>();
builder.typ::<AiRunEvent>();
```
> **AMBIGUITY (flag):** the exact method name (`.typ::<T>()` vs collecting via a `TypeCollection`)
> depends on the pinned rc. **Recommendation:** use whichever the pinned rc exposes; **guaranteed
> fallback** if none exists — add a `#[specta::specta] fn __ipc_channel_types(a: CloneProgress, b:
> GraphChunk, c: IndexProgress, d: AiRunEvent)` marker collected into the builder but **never**
> added to `generate_handler!` (never callable at runtime); it exists only to drag the four types
> into the generated output. senior-dev picks the clean path first, falls back to the marker.

The frontend still bridges a plain callback to a `new Channel<T>()` exactly as today; only the
payload **types** are now imported from `generated/bindings`.

### 4.4 Non-Rust facade methods (stay in `extras.ts`)

`pickFolder` (dialog plugin), `checkForUpdate`/`downloadAndInstallUpdate` (updater plugin, stateful
JS-side `pendingUpdate`), `onWindowFocus` (window API). No Rust command backs these — moved verbatim
from `tauri.ts`, unchanged.

---

## 5. Generation trigger + CI staleness gate

**Single-source builder.** Factor the builder into one function used by BOTH the app mount and the
export test, so the mounted handler and the exported bindings can never diverge:

```rust
// src-tauri/src/ipc_export.rs (new)
pub fn ipc_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            commands::open_repo, /* … all NON-channel commands … */
        ])
        .events(tauri_specta::collect_events![
            commands::RepoChangedPayload,
            commands::JobStatusChangedPayload,
            /* McpStatus */
        ]);
        // + channel-payload type registration (§4.3)
}
```
`lib.rs` uses it: `let builder = ipc_export::ipc_builder();` then
`.invoke_handler(builder.invoke_handler())` — and the 4 channel commands are appended via a second
plain `generate_handler!`/`invoke_handler` merge (or kept in the same handler; senior-dev confirms
the rc's compose API). `builder.mount_events(app)` runs in `setup`.

**Export from a test (CI-friendly, no debug-only side effects at runtime):**
```rust
// src-tauri/src/ipc_export.rs  (#[cfg(test)])
#[test]
fn export_bindings() {
    ipc_builder()
        .export(
            specta_typescript::Typescript::default()
                .bigint(specta_typescript::BigIntExportBehavior::Number),  // §1.3 — match `number`
            "../src/ipc/generated/bindings.ts",
        )
        .expect("export IPC bindings");
}
```
Regenerate with `cargo test -p bonsai export_bindings`.

**CI gate** (add to the existing gate script, before `pnpm test`):
```
cargo test -p bonsai export_bindings
git diff --exit-code src/ipc/generated/bindings.ts
```
A stale checkout (someone changed a command/type without regenerating) fails CI with a diff. The
generated file is **committed** so the frontend/browser-harness build never depends on a Rust build.

### 1.3 serde ↔ specta fidelity rules (senior-dev must verify each)

- `#[serde(rename_all = "camelCase")]` → specta honors it; generated fields are camelCase (matches
  today's hand-written TS). Assert on a spot-check type in the parity test (§6.6).
- **`BigIntExportBehavior::Number`** is mandatory: specta maps `u64`/`i64`/`usize`/`isize` to
  `bigint` by default, but the hand-written types use `number` (e.g. counts, `usize`). Setting
  `Number` reproduces the current wire/TS shape. (`u32` like `GraphNode.lane` maps to `number`
  regardless.)
- `#[serde(skip_serializing_if = "Option::is_none")]` on an `Option<T>` → specta emits `T | null`,
  **not** `field?:`. Today's hand-written types use a mix; where an existing type has `field?: T`
  the generated `field: T | null` is wire-compatible (both accept absent/null) but is a **type
  diff** the parity test will surface. **Resolution:** accept `T | null` as the canonical form and
  fix the ≤N call sites that relied on optional (`in`-checks still work). Enumerate during §6.6.
- Enum tag modes: Bonsai enums are `#[serde(rename_all=…)]` externally/internally tagged; specta
  reproduces these. `#[serde(untagged)]` (if any) is supported.
- **`AppError` (manual `Serialize`) — see §1.4.**

### 1.4 `AppError` — hand-written `specta::Type`

`AppError`'s wire shape (`{ kind: <camelCaseVariant>, message: string }`) comes from a **manual
`serde::Serialize`**, so `#[derive(specta::Type)]` would describe the *enum* shape, not the emitted
object. Provide a **manual `impl specta::Type for AppError`** (or a `#[specta(remote = …)]` shadow
struct) that declares exactly:
```ts
export type AppError = { kind: AppErrorKind; message: string };
export type AppErrorKind = "git" | "io" | "other" | "noRepo" | /* … all variants, camelCase … */;
```
The `AppErrorKind` union must be kept in sync with the enum; add a Rust unit test that
round-trips-serializes one value of every variant and asserts its `kind` string is a member of a
hand-listed set, so a new variant without a codegen update fails a test. This is the **only** type
needing a manual specta impl (verified: it is the sole `impl Serialize` in the two crates).

---

## 6. Incremental migration — phased, green at every step

Hard rule: **`cargo test` (~1870), `vitest` (~1850), and the 24 e2e specs stay green after every
sub-increment; each is committed separately.** Wire output must not change until the very end, and
even then only in type-shape (§1.3), never in runtime bytes.

**Phase 6.1 — deps + AppError + builder skeleton (no output yet).**
Add the crates (§1.1). Add the manual `specta::Type` for `AppError` (§1.4). Create `ipc_export.rs`
with an **empty** `ipc_builder()` and the `export_bindings` test writing to a scratch path. Prove
`cargo build` + the test run. No `#[specta::specta]` yet, no frontend change. *Gate: cargo green.*

**Phase 6.2 — `#[derive(specta::Type)]` on all wire types, crate by crate.**
Add the derive to every serde wire type in `bonsai-core` (34 files), then `bonsai-forge`, then the
command-payload structs in `src-tauri/src/commands/*.rs`. Mechanical; ~436 sites. Split into 2–3
commits by crate/domain to keep review diffs small. *Gate: cargo green after each.*

**Phase 6.3 — `#[specta::specta]` + `collect_commands!` (non-channel), batch 1.**
Annotate repo/status/staging/diff/branches/remotes commands; add them to `ipc_builder()`. *Gate:
cargo green; export test emits a partial `bindings.ts` to scratch (not yet the real path).*

**Phase 6.4 — `#[specta::specta]` + `collect_commands!`, batch 2 (the rest).**
merge/rebase/bisect/stash/config/undo/search/signing/submodule/worktree/health/tags/ai(non-stream)/
profiles/assets/external/mcp/session/ui/compose/reset/revert/cherrypick/forge. Register the 4
channel-payload types (§4.3). *Gate: cargo green; export emits full type set to scratch.*

**Phase 6.5 — typed events + wire `ipc_builder()` into `lib.rs`.**
Derive `tauri_specta::Event`, `collect_events!` the 3 payloads, swap emit sites to the typed form,
`mount_events` in setup, feed `builder.invoke_handler()` into the app alongside the 4 channel
commands. Add the event-name-stability unit test (§4.2). *Gate: cargo green; native still mounts.*

**Phase 6.6 — generate for real + type-parity reconciliation.**
Point `export_bindings` at `src/ipc/generated/bindings.ts`; commit it. Add a **temporary**
`bindings.parity.test.ts` that structurally asserts the generated types are assignable to/from the
current hand-written `types.ts` types for a representative sample (GraphLayout, StatusSnapshot,
AppError, CommitResult, a tagged enum). Resolve every diff (`skip_serializing_if` optionality,
bigint, enum casing) per §1.3. *Gate: cargo + tsc green; parity test green.*

**Phase 6.7 — `adapter.ts` + `extras.ts` + rebuild the real `ipc`; delete `tauri.ts`.**
Add `adapter.ts` (§3.2) and `extras.ts` (4 channels + 4 events + dialog/updater/window-focus + the
≤10 optional-arg shims §3.3). Rebuild `index.ts` `ipc` from `{ ...unwrapCommands(), ...tauriExtras }`.
Redefine `IpcApi` as the derived type (§3.1). Delete `tauri.ts`. *Gate: `tsc` + full `vitest` + e2e
green (real-path build; the harness runs mock so verify mock separately in 6.8).*

**Phase 6.8 — re-type the mock against generated `IpcApi`.**
`mockIpc: IpcApi` now derives from generated commands, so it fails typecheck for any un-mocked
command — fix each handler-group `satisfies Partial<IpcApi>` and add missing handlers. No fixture
data changes (the shapes are wire-identical). Run the browser harness (`VITE_MOCK_IPC=1`) + all e2e.
*Gate: vitest + e2e green; harness screenshot of the graph unchanged.*

**Phase 6.9 — collapse `types.ts`, relocate frontend-only types, add CI gate, remove parity test.**
Reduce `types.ts` to the re-export barrel (§2.2); move non-Rust types to `frontendTypes.ts`. Add the
staleness gate (§5) to CI. Delete the temporary parity test. Update `docs/architecture-reference.md`
IPC section + `INDEX.md`. *Gate: full gate green; `git diff --exit-code` on generated file clean.*

**Estimated sub-increments: 9** (6.2 and 6.4 may each split once more for review-diff size → up to
~11 commits).

---

## 7. Acceptance criteria (measurable)

1. `src/ipc/generated/bindings.ts` exists, is committed, and is produced solely by
   `cargo test -p bonsai export_bindings`. `git diff --exit-code src/ipc/generated/bindings.ts`
   after regeneration is clean.
2. `src/ipc/types.ts` no longer hand-declares any Rust-derived type; it only re-exports. `tauri.ts`
   is deleted. Combined hand-written LOC across `types.ts` + `tauri.ts` + `adapter.ts` + `extras.ts`
   is **< 500** (down from ~3785).
3. Adding a throwaway `#[specta::specta]` command and regenerating makes `mockIpc: IpcApi` **fail
   `tsc`** until a mock handler is added (demonstrated once, then reverted). This is the anti-drift
   guarantee.
4. The wire surface is unchanged: all 173 command names, arg orders, and the 3 event names are
   byte-identical (asserted by a Rust test listing them; the event-name test from §4.2).
5. Full gate green with no skips: `cargo test` (~1870), `vitest` (~1850), 24 e2e specs.
6. Browser harness (`VITE_MOCK_IPC=1`) renders the commit graph and status identically to pre-P75
   (final screenshot proof).
7. CI fails on stale generated bindings (staleness gate wired, demonstrated by a deliberate un-regen
   commit reverted).
8. No call site outside `src/ipc/**` changed its import path or its `await ipc.foo(...)` usage
   (except the ≤N `skip_serializing_if` optionality fixes enumerated in §6.6, which are listed in
   the phase commit message).

## 8. Risks / rollback

- **R1 (top): rc instability / rc-to-rc output churn.** `tauri-specta` is pre-1.0. *Mitigation:* pin
  all three crates with `=`; the committed generated file + CI staleness gate make any silent output
  change a hard, visible failure rather than latent drift. *Rollback:* the whole migration lives
  behind the stable `ipc` barrel — reverting to the pre-P75 `tauri.ts`/`types.ts` is a clean
  git revert of the 6.7+ commits; call sites are untouched so nothing downstream needs changing.
- **R2 (top): `skip_serializing_if`/bigint type-shape diffs** silently break a `tsc`-passing call
  site at runtime (e.g. a field that was optional is now `T | null`). *Mitigation:* the §6.6 parity
  test + `BigIntExportBehavior::Number` + explicit enumeration of every optionality change in the
  phase commit; these are wire-compatible so runtime behavior is unchanged, only the TS type widens.
- **R3: channel-payload types missing from generated output.** *Mitigation:* the §4.3 explicit
  registration with the guaranteed marker-command fallback; a `tsc` failure in `extras.ts` catches
  it immediately.
- **R4: event name drift** from the `tauri_specta::Event` derive naming. *Mitigation:* the §4.2
  emitted-string unit test pins each name.
- **R5: build-time coupling** — a broken Rust build would block frontend codegen. *Mitigation:* the
  generated file is committed, so `pnpm dev`/harness/e2e never invoke Rust; only regeneration does.

Rollback granularity: each phase is its own commit; phases 6.1–6.5 are additive (derives/annotations
only, old files still authoritative) so they can sit shipped indefinitely if 6.7 is deferred.
