# Phase 4 — Forge / PR integration — shared conventions (P62–P64 anchor)

The consistency anchor for the Phase-4 forge milestones (P62 foundation + PR panel, P63 PR/CI
status badges on the commit graph, P64 AI PR descriptions). It does NOT design those features — it
records the SHARED conventions every Phase-4 contract must follow so they stay coherent, and it
documents the strategic OPEN DECISIONS the user confirms at build time (auth mode, new deps).

References read (verified, not guessed): `crates/bonsai-core/src/git/cred_cache.rs` (P35 in-process
HTTPS cred cache — single-flight, never-log discipline, `LazyLock` global, injectable `FillFn` seam),
`crates/bonsai-core/src/git/remote.rs` (`list_remotes(workdir) -> Vec<RemoteInfo>`, `RemoteInfo{name,
url:Option<String>}`), `crates/bonsai-core/src/error.rs` (`AppError` + serde `{kind,message}`),
`src-tauri/src/commands/{mod.rs,shared.rs,compose.rs}` (the `#[command] fn X → fn X_inner →
spawn_blocking(core)` triple; `repo_path(state,repo_id)`), `src-tauri/src/lib.rs` (`generate_handler!`
— 147 commands today), `src/ipc/{index.ts,types.ts,tauri.ts,mock.ts}`, `src/ipc/mock/handlers/
compose.ts` (mock handler shape + `#fail` sentinel), `src/components/WorkspaceRightPanel.tsx` (right-
pane tri-state container). Cargo scan: `keyring`/`keychain`/`ureq` = ABSENT; `reqwest` present as a
**dev-dependency only** (`src-tauri/Cargo.toml`, v0.12, `default-features=false, features=["json"]`);
`rustls`/`security-framework`/`windows-sys` already transitively in `Cargo.lock`.

---

## F1 — The `bonsai-forge` crate boundary (NEW pure library crate)

All forge logic lives in a NEW `crates/bonsai-forge/`, mirroring `crates/bonsai-core/` (pure library,
no Tauri, no UI). Invariants:

- **Blocking + runtime-free public API.** Every provider method is synchronous and BLOCKING; the
  command layer runs it inside `spawn_blocking`. The crate exposes NO async fn and starts NO async
  runtime of its own (consistent with the bonsai-core "no tokio" stance — see `cred_cache.rs`).
- **Depends on `bonsai-core`** for `AppError` (single error taxonomy across the app) and, where handy,
  `git::remote::list_remotes` to read the `origin` URL. It does NOT re-open repos for anything git2
  already gives us.
- **Injectable HTTP seam (`HttpTransport`)** exactly like cred_cache's `FillFn`: the trait +
  provider logic are unit-tested with a FAKE transport (canned JSON, zero network, cross-platform,
  deterministic). The concrete networked transport is a thin adapter behind the trait, injected at
  construction. This is what lets the AI-gate run offline.
- **File-size discipline:** provider-neutral types, detection, http, auth, and each concrete provider
  are separate modules; the GitHub impl is split (`github/{mod,rest,dto}.rs`) so no file passes ~500
  lines and GitHub's wire JSON never leaks past `github/dto.rs`.

## F2 — The `ForgeProvider` trait (the shared abstraction)

One trait; GitHub is the first impl; GitLab/Bitbucket/GraphQL slot in later by adding a module + a
`ForgeKind` arm. Provider-NEUTRAL types only cross the trait boundary (never a `serde_json::Value`,
never a GitHub struct). The trait carries the FULL surface for the whole phase (P62 implements it all;
P63/P64 only wire new IPC to methods that already exist):

```rust
pub trait ForgeProvider: Send + Sync {
    fn repo_context(&self) -> ForgeRepoContext;                              // identity, known at build
    fn list_prs(&self, q: &PrListQuery) -> Result<PrPage, AppError>;
    fn get_pr(&self, number: u64) -> Result<PrDetail, AppError>;
    fn create_pr(&self, input: &CreatePrInput) -> Result<PrDetail, AppError>;// requires auth
    fn list_review_comments(&self, number: u64) -> Result<Vec<ReviewComment>, AppError>;
    fn combined_status(&self, sha: &str) -> Result<CommitStatus, AppError>;  // defined now; IPC in P63
}
```

`combined_status` + its `CommitStatus` shape (F4) are DEFINED AND IMPLEMENTED in P62 (trait
completeness forces the GitHub impl) but are NOT exposed as an IPC command until P63 — so P63 is pure
wiring + canvas badge rendering.

## F3 — Auth / credential model (cross-cutting — the security spine)

- **PAT-only for v1** (recommended default; OQ-1). The user PASTES a Personal-Access-Token into the
  app. The app NEVER autofills, NEVER reads the user's tokens from anywhere, NEVER puts a token in a
  URL, and NEVER logs `token`, `Authorization`, or raw response bodies (copy cred_cache.rs's security
  note verbatim in spirit). The token reaches the network ONLY as that provider's auth header,
  constructed in the provider's `rest` module and carried by `HttpTransport`: GitHub and Bitbucket
  `Authorization: Bearer <token>`, Azure DevOps `Authorization: Basic base64(":" + PAT)` (empty
  username), GitLab `PRIVATE-TOKEN: <token>`. (Corrected by P72 — the earlier "Bearer only" wording
  was never true for Azure or GitLab. Base64 is encoding, not secrecy: the `http.rs` redaction seam
  is what keeps the value off logs and `{:?}`.)
- **Store of record = OS keychain** via the `keyring` crate (OQ-2 — NEW dep, flagged). Key: service
  `com.bonsai.app` (the app identifier), account = **host** (e.g. `github.com`) so two repos on the
  same host share one token. NEVER settings.json (hard invariant).
- **In-process token cache** mirrors cred_cache's pattern (process-global `LazyLock`, never-log, a
  `Mutex<HashMap<host,String>>` warmed lazily from the keychain) so we don't hit the keychain on every
  API call. It is a get/set over the keychain, not over `git credential fill`.
- **Optional git-credential bridge (default OFF, deferred):** after validating a PAT we COULD seed it
  into the user's git credential helper (`git credential approve`, reusing `remote.rs` plumbing) so
  `git push/fetch` reuse the same token. Left OFF in v1 to avoid clobbering an existing git credential;
  documented as the "extend cred_cache plumbing" future path.
- **Validation on paste:** `forge_set_token` performs ONE lightweight call (`GET /user`) to validate +
  resolve the viewer, then stores. A bad token → `authFailed` and NOTHING is stored.
- **Read paths work unauthenticated** for public repos (GitHub REST allows anon reads at a low rate
  limit); `create_pr` REQUIRES a token → `forgeAuthRequired` when absent.

## F4 — Provider-neutral data model + the status shape (define once, here)

All DTOs are `#[derive(Serialize, Deserialize)] #[serde(rename_all="camelCase")]` with a
`*_wire_shape_is_camel_case` test, and are the ONLY things the trait returns. The normalized CI/commit
status (P62 defines + implements; P63 renders as graph badges) is:

```rust
pub enum CheckRollup { Success, Pending, Failure, Error, Neutral, None } // serde camelCase
pub struct StatusContext { pub name: String, pub state: CheckRollup,
                           pub description: Option<String>, pub target_url: Option<String> }
pub struct CommitStatus {
    pub sha: String, pub state: CheckRollup,
    pub total: u32, pub passed: u32, pub failed: u32, pub pending: u32,
    pub contexts: Vec<StatusContext>,          // capped (<=50); individual checks
}
```

GitHub's impl MERGES the legacy combined-status API and the check-runs API into this one rollup
(algorithm in P62 §7). Other providers map their own concepts onto `CheckRollup`. The full PR DTO set
(`PrSummary`/`PrDetail`/`PrPage`/`PrListQuery`/`CreatePrInput`/`ReviewComment`/`ForgeRepoContext`) is
specified in `P62-forge-foundation.md` §3.

## F5 — IPC conventions (align with existing `*_inner` triple)

- **Commands only.** PR lists/detail/comments are bounded (paginated pages, one detail, capped
  comment threads) ⇒ request/response `#[tauri::command]`. NO channel (no streaming), NO new event in
  P62. The panel refetches on demand (manual refresh + active-repo-tab change); it does NOT auto-poll
  (rate limits). If P63 wants push status refresh, THAT is a future event — out of scope now.
- **Naming:** snake_case `forge_<verb>[_<noun>]`; TS camelCase mirror (`forge_list_prs` ↔
  `forgeListPrs`). Every command takes `repo_id: String` first (the backend resolves origin+host from
  it), mirroring every other repo-scoped command.
- **Triple (verbatim from `commands/compose.rs`):** `forge_x(state, repo_id, …) →
  forge_x_inner(state, &repo_id, …)`; the inner does `repo_path(state, repo_id)?` then
  `spawn_blocking(move || { let p = bonsai_forge::open(&workdir)?; p.method(…) })` then
  `.map_err(join)`. Forge commands are NOT AI-gated. They do NOT emit `repo-changed` (`create_pr`
  changes the remote, not the local repo).
- **Errors:** reuse `noRemote` (no origin), `authFailed` (token rejected), `networkError`
  (DNS/TLS/connect). ADD four `AppError` variants (P62 §5): `forgeUnsupported` (origin host isn't a
  known provider), `forgeAuthRequired` (op needs a token; none stored), `forgeRateLimited` (carries
  reset hint), `forgeApi` (4xx/5xx/malformed response). Never a raw string kind.
- **Serde:** inputs `#[derive(Deserialize)] #[serde(rename_all="camelCase")]`; discriminated unions
  use `#[serde(tag="kind")]` matching a TS `{kind:…}` union (see `ForgeKind`/`PrState`).

## F6 — Mock ↔ real IPC parity (hard rule — offline harness)

Every new `forge_*` command gets a handler in a NEW `src/ipc/mock/handlers/forge.ts` (spread into
`mockIpc` in `src/ipc/mock.ts`), backed by canned fixtures in a NEW `src/ipc/fixtures/forge.ts`.
`pnpm dev:mock` (`VITE_MOCK_IPC=1`) must exercise list → detail → create → comments and the
connect/paste-token flow with ZERO network. Sentinels mirror the AI mock's `?ai=off`:

| URL sentinel | Mock behavior |
|---|---|
| (default) | authenticated:false → ForgeConnect flow; canned PRs after "connecting" |
| `?forge=auth` | starts authenticated (canned viewer) → PR list renders immediately |
| `?forge=off` | every forge command throws `{kind:'networkError'}` (offline-error path) |
| token containing `bad` in `forgeSetToken` | throws `{kind:'authFailed'}` (mirrors compose `#fail`) |

## F7 — UI conventions (right-pane PR panel)

- The PR panel is a NEW selectable view in the RIGHT pane, alongside the existing
  compare/commit-details/status tri-state (`WorkspaceRightPanel.tsx`). A small segmented tab
  (`Working` | `Pull requests`) at the top of the right pane switches between them; `RepoWorkspace`
  owns the active-tab state. (Exact tab affordance is a small senior-dev/orchestrator call — F7 fixes
  the seam, not the pixels.)
- `PrPanel.tsx` is the CONTAINER (state/effects/IPC + last-wins req-id guard like `AiOutputPanel`);
  it composes small presentational children (`PrList`, `PrListItem`, `PrDetailView`,
  `PrReviewComments`, `PrCreateForm`, `ForgeConnect`), each in its own file (~<500 lines).
- **`ForgeConnect`** renders the paste-token affordance ONLY when unauthenticated: a password-type
  input, an explicit "stored in your OS keychain, never in a file" note, and NO prefill. External
  "open on GitHub" links go through the existing `openExternal`/external-tool path, never a raw
  in-app navigation.

## P62 → P63 → P64 map

- **P62 (this phase's foundation):** the crate, the trait + all DTOs incl. `CommitStatus`, GitHub REST
  impl, PAT+keychain auth, origin→provider detection, the 5 data commands + 2 auth commands, and the
  right-pane PR panel (list → detail → create). `combined_status` implemented but not yet exposed.
- **P63 (status badges):** adds ONE command `forge_commit_status(repoId, sha) → CommitStatus` wiring
  the existing trait method, plus a status cache + canvas badge rendering beside graph nodes (consumes
  `PrSummary.head_sha` / `CommitStatus`). No new provider code.
- **P64 (AI PR descriptions):** fills the `onGenerateDescription` seam baked into `PrCreateForm` in
  P62 with a new `ai_pr_description` command (Phase-2 AI conventions: grounding payload → `run_claude`
  → editable proposal, writes nothing on its own).

## Open decisions (flag to orchestrator; recommended default baked in — do NOT block)

- **OQ-1 — Auth mode.** REC: PAT-only for v1 (paste + keychain). Alt: OAuth device-flow now (more
  code, a callback server, refresh-token storage). Recommend deferring OAuth.
- **OQ-2 — Keychain dep.** REC: add the `keyring` crate (guarantees "OS keychain, never settings.json"
  regardless of the user's git config). FLAG: NEW dependency touching the already-dirty
  `Cargo.toml`/`Cargo.lock`; on Linux it pulls a Secret-Service/D-Bus backend (a runtime concern on
  headless boxes). Alt: reuse the `git credential` helper (zero new dep) but it does NOT guarantee an
  OS keychain (a user with `credential.helper=store` gets plaintext) — so it fails the hard invariant.
- **OQ-3 — HTTP client.** REC: promote the existing `reqwest` dev-dep to a real dep of `bonsai-forge`
  with `default-features=false, features=["blocking","json","rustls-tls"]` ("reuse what's there";
  `blocking` keeps the crate synchronous for `spawn_blocking`; `rustls-tls` avoids OpenSSL on Windows).
  FLAG: touches the dirty Cargo files (dev→normal + feature/lock changes). Alt: `ureq` (leanest, truly
  no async runtime) — but a brand-new crate name in the tree.
- **OQ-4 — P64 hook now?** REC: bake the `onGenerateDescription?` PROP seam into `PrCreateForm` in P62
  but wire NO command (button hidden while the prop is undefined). Zero-cost seam; P64 just flips it on.
