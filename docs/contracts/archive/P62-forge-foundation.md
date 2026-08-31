# P62 — Forge foundation (provider-abstracted; GitHub first)

First milestone of Phase 4. Shared conventions live in `docs/contracts/phase4-forge-overview.md`
(read it first: F1 crate boundary, F2 trait, F3 auth, F4 status shape, F5 IPC, F6 mock parity, F7 UI).
This file is the implementable spec: types, IPC surface, module boundaries, algorithms, acceptance.

Reuse (by path, do NOT reinvent): `crates/bonsai-core/src/git/remote.rs::list_remotes(workdir:&Path)
-> Result<Vec<RemoteInfo>,AppError>` (`RemoteInfo{name:String,url:Option<String>}`) for origin;
`crates/bonsai-core/src/git/cred_cache.rs` as the pattern for the never-log, process-global, injectable-
seam token cache; `crates/bonsai-core/src/error.rs::AppError`; the command triple in
`src-tauri/src/commands/compose.rs`; the mock handler shape in `src/ipc/mock/handlers/compose.ts`;
the right-pane container `src/components/WorkspaceRightPanel.tsx`.

## 1. Goal & scope

Detect the forge from `origin`, authenticate with a pasted PAT stored in the OS keychain, and drive a
right-pane PR panel (list → detail → create) against a `ForgeProvider` trait whose first impl is GitHub
REST v3. Define + implement the `CommitStatus` shape (P63 renders it). Everything runs in a plain
browser offline through the mock. IN scope: the crate, trait, DTOs, GitHub impl, auth, detection, 7
commands, PR panel. OUT of scope: graph badges (P63), AI descriptions (P64 — seam only), OAuth,
GraphQL, GitLab/Bitbucket, PR merge/close/review-submit, comment posting, webhooks, auto-poll.

## 2. Module boundaries

### 2a. New crate `crates/bonsai-forge/` (pure library — F1)

| File | Responsibility |
|---|---|
| `Cargo.toml` | deps: `bonsai-core` (path), `serde`, `serde_json`, `thiserror` (workspace); `reqwest{blocking,json,rustls-tls}` (OQ-3); `keyring` (OQ-2). dev: `tempfile`. |
| `src/lib.rs` | Public surface: `open(workdir:&Path) -> Result<Box<dyn ForgeProvider>,AppError>`; re-export trait + all DTOs; module decls. No logic. |
| `src/types.rs` | ALL provider-neutral DTOs (§3) + serde + `*_wire_shape_is_camel_case` tests. Pure. |
| `src/detect.rs` | PURE `detect_provider(remote_url:&str) -> Option<ForgeTarget>` (§6) + a normalize table test (mirrors `cred_cache::normalize_key` tests). No I/O. |
| `src/provider.rs` | The `ForgeProvider` trait (§F2) + `ForgeKind`. |
| `src/http.rs` | `HttpTransport` trait (injectable seam) + `HttpRequest`/`HttpResponse` + concrete `ReqwestTransport`; header redaction. No provider logic. |
| `src/auth.rs` | `TokenStore`: `get/set/delete(host)` over the `keyring` keychain + a process-global never-log in-process cache (cred_cache pattern). |
| `src/github/mod.rs` | `GitHubProvider { target, token:Option<String>, http:Box<dyn HttpTransport> }` implementing `ForgeProvider`. |
| `src/github/rest.rs` | Endpoint URL builders + paginated GET helper (Link-header `has_next`) + auth-header assembly. |
| `src/github/dto.rs` | GitHub wire structs (`Deserialize`) + `into_*` mappers to `types.rs`. GitHub JSON NEVER escapes this file. |

`open()` = read origin via `remote::list_remotes` → `detect_provider` (`None` ⇒ `forgeUnsupported`) →
`TokenStore::get(host)` → build `GitHubProvider` with a `ReqwestTransport`. Tests build providers
directly with a fake `HttpTransport` + explicit token, bypassing `open()`.

### 2b. New Tauri command module

`src-tauri/src/commands/forge.rs` — the 7 `#[command] fn / fn _inner` triples (§4). Register in
`src-tauri/src/commands/mod.rs` (`mod forge; pub use forge::*;`) and each fn in
`src-tauri/src/lib.rs`'s `generate_handler!`. Re-export the DTO names the command layer NAMES from
`shared.rs` (mirror the `compose_apply` note there).

### 2c. Frontend

| File | Responsibility |
|---|---|
| `src/ipc/types.ts` | Add the DTO mirrors (§3) + 7 methods on `IpcApi`; add 4 `AppError.kind` values (§5). |
| `src/ipc/tauri.ts` | 7 `invoke` wrappers (§4). |
| `src/ipc/index.ts` | Re-export the new type names. |
| `src/ipc/mock/handlers/forge.ts` | `forgeHandlers` (§8) — canned, offline, sentinel-aware. Spread into `mockIpc` in `src/ipc/mock.ts`. |
| `src/ipc/fixtures/forge.ts` | Canned `ForgeRepoContext`, PR list/detail, comments, viewer, one `CommitStatus`. |
| `src/components/PrPanel.tsx` | CONTAINER (F7): view state (`connect`/`list`/`detail`/`create`), effects, IPC calls, last-wins req-id guard, error→toast. |
| `src/components/PrList.tsx` / `PrListItem.tsx` | Presentational list + row (title, `#num`, state pill, draft, author, `head→base`, comment count). |
| `src/components/PrDetailView.tsx` | Presentational detail (title, meta, labels, mergeable, +/− stat, body) + a slot for comments. |
| `src/components/PrReviewComments.tsx` | Presentational comment thread. |
| `src/components/PrCreateForm.tsx` | Presentational create form (title, body, base/head, draft) + the P64 `onGenerateDescription?` seam (OQ-4). |
| `src/components/ForgeConnect.tsx` | Paste-PAT affordance (password input, keychain note, no prefill). |

Right-pane integration: add `rightPaneTab: 'work' | 'prs'` state in `RepoWorkspace`; render a small tab
switcher + `PrPanel` in `WorkspaceRightPanel.tsx` when `'prs'`. The existing tri-state stays under
`'work'`.

## 3. Rust DTOs (`bonsai-forge/src/types.rs`) + TS mirrors (`src/ipc/types.ts`)

All Rust: `#[derive(Debug,Clone,Serialize,Deserialize)] #[serde(rename_all="camelCase")]`; enums
`#[serde(rename_all="camelCase")]` (unit variants ⇒ plain string on the wire).

```rust
pub enum ForgeKind { GitHub, Unknown }                       // "gitHub" | "unknown"
pub struct ForgeViewer { pub login: String, pub avatar_url: Option<String> }
pub struct ForgeRepoContext {
    pub provider: ForgeKind, pub host: String, pub owner: String, pub repo: String,
    pub remote_name: String, pub web_url: String,
    pub authenticated: bool,                 // token present in keychain for host (NO network)
    pub viewer: Option<ForgeViewer>,         // Some only when a validated viewer is cache-warm
}

pub enum PrState { Open, Closed, Merged }
pub enum PrStateFilter { Open, Closed, All } // list query filter
pub struct PrSummary {
    pub number: u64, pub title: String, pub state: PrState, pub is_draft: bool,
    pub author: String, pub author_avatar_url: Option<String>,
    pub source_branch: String,               // head ref (branch name only)
    pub target_branch: String,               // base ref
    pub comments: u32, pub created_at: String, pub updated_at: String,  // ISO-8601
    pub url: String,                         // html_url (browser)
    pub head_sha: String,                    // for P63 status lookup
}
pub struct PrDetail {
    pub summary: PrSummary, pub body: String,           // markdown, may be ""
    pub mergeable: Option<bool>,                         // null while GitHub computes
    pub additions: u32, pub deletions: u32, pub changed_files: u32,
    pub labels: Vec<String>,
}
pub struct PrListQuery { pub state: PrStateFilter, pub page: u32, pub per_page: u32 } // per_page capped <=50
pub struct PrPage { pub items: Vec<PrSummary>, pub page: u32, pub has_next: bool }
pub struct CreatePrInput {
    pub title: String, pub body: String,
    pub source_branch: String, pub target_branch: String,
    pub draft: bool, pub maintainer_can_modify: bool,     // default true
}
pub enum CommentKind { Review, Conversation }             // diff-line vs PR conversation
pub struct ReviewComment {
    pub id: u64, pub author: String, pub author_avatar_url: Option<String>,
    pub body: String, pub path: Option<String>, pub line: Option<u32>,
    pub created_at: String, pub url: String, pub kind: CommentKind,
}
// CheckRollup / StatusContext / CommitStatus: see overview §F4 (defined + implemented here).
```

TS mirror (add to `src/ipc/types.ts`; discriminated-string enums as unions):

```ts
export type ForgeKind = 'gitHub' | 'unknown';
export type PrState = 'open' | 'closed' | 'merged';
export type PrStateFilter = 'open' | 'closed' | 'all';
export type CheckRollup = 'success' | 'pending' | 'failure' | 'error' | 'neutral' | 'none';
export type CommentKind = 'review' | 'conversation';
export interface ForgeViewer { login: string; avatarUrl: string | null; }
export interface ForgeRepoContext {
  provider: ForgeKind; host: string; owner: string; repo: string;
  remoteName: string; webUrl: string; authenticated: boolean; viewer: ForgeViewer | null;
}
export interface PrSummary {
  number: number; title: string; state: PrState; isDraft: boolean;
  author: string; authorAvatarUrl: string | null;
  sourceBranch: string; targetBranch: string;
  comments: number; createdAt: string; updatedAt: string; url: string; headSha: string;
}
export interface PrDetail {
  summary: PrSummary; body: string; mergeable: boolean | null;
  additions: number; deletions: number; changedFiles: number; labels: string[];
}
export interface PrListQuery { state: PrStateFilter; page: number; perPage: number; }
export interface PrPage { items: PrSummary[]; page: number; hasNext: boolean; }
export interface CreatePrInput {
  title: string; body: string; sourceBranch: string; targetBranch: string;
  draft: boolean; maintainerCanModify: boolean;
}
export interface ReviewComment {
  id: number; author: string; authorAvatarUrl: string | null; body: string;
  path: string | null; line: number | null; createdAt: string; url: string; kind: CommentKind;
}
export interface StatusContext { name: string; state: CheckRollup; description: string | null; targetUrl: string | null; }
export interface CommitStatus {
  sha: string; state: CheckRollup;
  total: number; passed: number; failed: number; pending: number; contexts: StatusContext[];
}
```

## 4. IPC surface (7 commands; exact wire shapes)

`repo_id`/`repoId` first on every command. Rust triple mirrors `compose.rs`. Wire args are the exact
camelCase keys `invoke` sends.

| Command (snake) | TS method | Wire request | Response |
|---|---|---|---|
| `forge_repo_context` | `forgeRepoContext(repoId)` | `{ repoId }` | `ForgeRepoContext` |
| `forge_list_prs` | `forgeListPrs(repoId, query)` | `{ repoId, query: PrListQuery }` | `PrPage` |
| `forge_get_pr` | `forgeGetPr(repoId, number)` | `{ repoId, number }` | `PrDetail` |
| `forge_create_pr` | `forgeCreatePr(repoId, input)` | `{ repoId, input: CreatePrInput }` | `PrDetail` |
| `forge_list_review_comments` | `forgeListReviewComments(repoId, number)` | `{ repoId, number }` | `ReviewComment[]` |
| `forge_set_token` | `forgeSetToken(repoId, token)` | `{ repoId, token }` | `ForgeViewer` |
| `forge_clear_token` | `forgeClearToken(repoId)` | `{ repoId }` | `void` |

`forge_set_token` (auth plumbing implied by the paste flow — the 5 task-listed data commands can't
authenticate without it): resolves the host from `repoId`'s origin, validates `token` via `GET /user`,
stores it keyed by host, returns the viewer. `forge_clear_token` deletes the keychain entry + evicts
the cache. NEITHER token nor `Authorization` ever appears in a log, error message, or URL.

Rust signatures (`commands/forge.rs`):

```rust
#[tauri::command]
pub async fn forge_list_prs(state: tauri::State<'_, AppState>, repo_id: String, query: PrListQuery)
    -> Result<PrPage, AppError> { forge_list_prs_inner(state.inner(), &repo_id, query).await }
pub(crate) async fn forge_list_prs_inner(state: &AppState, repo_id: &str, query: PrListQuery)
    -> Result<PrPage, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || bonsai_forge::open(&workdir)?.list_prs(&query))
        .await.map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
// forge_repo_context / forge_get_pr / forge_create_pr / forge_list_review_comments: identical shape.
// forge_set_token / forge_clear_token: spawn_blocking over bonsai_forge::auth on the resolved host.
```

TS wrappers (`tauri.ts`): thin `invoke<T>('forge_x', {...})` (mirror `applyComposedCommits`/
`listRemotes`). Add all 7 to the `IpcApi` interface with doc comments naming the rejectable
`AppError` kinds per command (e.g. `forge_create_pr` rejects `forgeAuthRequired | forgeApi |
networkError | git`).

No new events; no channels (F5).

## 5. `AppError` additions

`crates/bonsai-core/src/error.rs`: add `ForgeUnsupported(String)`, `ForgeAuthRequired(String)`,
`ForgeRateLimited(String)`, `ForgeApi(String)` — each a `#[error("{0}")]` newtype; extend the `kind()`
(`"forgeUnsupported" | "forgeAuthRequired" | "forgeRateLimited" | "forgeApi"`) and `message()` arms +
the doc comment. Mirror in `src/ipc/types.ts` `AppError.kind` union. Reuse existing `noRemote` /
`authFailed` / `networkError` where they already fit (F5). (Touches the shared error file — low risk,
additive.)

## 6. Provider detection (`detect.rs`) — pseudocode

```
fn detect_provider(remote_url) -> Option<ForgeTarget{kind, host, owner, repo, web_url}>:
    (host, path) = parse(remote_url):        # accept all origin URL forms
        "https://H/owner/repo(.git)?[/...]"        -> host=H,   path="owner/repo"
        "ssh://git@H(:port)?/owner/repo(.git)?"    -> host=H,   path="owner/repo"
        "git@H:owner/repo(.git)?"  (SCP-like)      -> host=H,   path="owner/repo"
        else -> return None
    strip a trailing ".git" and any trailing "/" from path; host = lowercase(host)
    segs = path.split('/'); require exactly 2 non-empty -> owner, repo  (else None)
    kind = if host == "github.com" { GitHub } else { Unknown }   # enterprise host override = future OQ
    web_url = "https://{host}/{owner}/{repo}"
    return Some(ForgeTarget{kind, host, owner, repo, web_url})
```

`open()` maps `None` (unparseable) or `kind==Unknown` behavior: return `ForgeRepoContext{provider:
Unknown,…}` from `forge_repo_context` (friendly empty state — NOT an error), but any DATA command on
an `Unknown`/unparseable origin ⇒ `AppError::ForgeUnsupported`. No origin remote at all ⇒
`AppError::NoRemote`. Add a `detect_table` test (github.com https/ssh/scp with/without `.git`,
trailing slash, enterprise host ⇒ Unknown, non-git host ⇒ None).

## 7. GitHub REST mapping + status rollup

Base `https://api.github.com`. Headers: `Accept: application/vnd.github+json`,
`X-GitHub-Api-Version: 2022-11-28`, `User-Agent: Bonsai`, and `Authorization: Bearer <token>` ONLY
when a token exists. Paginate via `?per_page=&page=`; `has_next` = presence of `rel="next"` in the
`Link` header.

| Trait method | Endpoint(s) |
|---|---|
| `repo_context` | none live (identity from `detect` + keychain presence; `viewer` from set-token cache) |
| `list_prs` | `GET /repos/{o}/{r}/pulls?state=&per_page=&page=` → `PrSummary[]` (head.ref, base.ref, head.sha, user.login/avatar, draft) |
| `get_pr` | `GET /repos/{o}/{r}/pulls/{n}` → `PrDetail` (body, mergeable, additions/deletions/changed_files, labels[].name) |
| `create_pr` | `POST /repos/{o}/{r}/pulls` `{title,head,base,body,draft,maintainer_can_modify}` → `PrDetail` |
| `list_review_comments` | `GET /repos/{o}/{r}/pulls/{n}/comments` (line/`Review`) **+** `GET /repos/{o}/{r}/issues/{n}/comments` (`Conversation`); merge, sort by `created_at` (REC — a line-only thread is confusing; OQ-5) |
| `combined_status` | `GET …/commits/{sha}/status` **and** `GET …/commits/{sha}/check-runs`; merge (rollup below) |

HTTP status mapping in `rest.rs`: 401/403-with-bad-creds ⇒ `AuthFailed`; 403 w/ `X-RateLimit-
Remaining: 0` or 429 ⇒ `ForgeRateLimited` (message includes the `X-RateLimit-Reset` epoch); 404 ⇒
`ForgeApi("not found")`; other 4xx/5xx or JSON parse failure ⇒ `ForgeApi`; transport/DNS/TLS error ⇒
`NetworkError`. A method that needs auth with `token==None` ⇒ `ForgeAuthRequired` BEFORE any request.

Rollup (`combined_status`), unit-tested pure over the merged context list:

```
normalize each source item -> CheckRollup:
    legacy status.state:  success->Success  pending->Pending  failure|error->Failure
    check_run: status!="completed" -> Pending
               else conclusion: success->Success; neutral|skipped->Neutral;
                    failure|timed_out|cancelled|action_required|startup_failure->Failure; _->Error
overall state (precedence): if any Failure|Error -> Failure
                            elif any Pending      -> Pending
                            elif any Success      -> Success
                            elif any Neutral      -> Neutral
                            else                  -> None
counts: passed=#Success, failed=#(Failure|Error), pending=#Pending, total=len(contexts capped 50)
```

## 8. Frontend flow + mock (`handlers/forge.ts`)

`PrPanel` container flow:

```
on mount / active-repo-tab -> 'prs' / manual refresh:
  ctx = await forgeRepoContext(repoId)                     [req-id guarded]
  if ctx.provider == 'unknown'      -> render "unsupported forge" empty state
  else if !ctx.authenticated        -> render <ForgeConnect> (paste PAT)
  else                              -> page = await forgeListPrs(repoId,{state:'open',page:1,perPage:30}); render <PrList>
onSelectPr(n)   -> detail = forgeGetPr; comments = forgeListReviewComments; render <PrDetailView>+<PrReviewComments>
onCreate(input) -> forgeCreatePr -> on success open the new PR's detail + toast (open in browser link)
ForgeConnect submit(token) -> forgeSetToken(repoId,token) -> on success re-run repoContext -> list
```

Mock (`forgeHandlers`, offline, sentinels per overview F6): keep a module-level `authenticated` flag
(seeded true when `?forge=auth`); `forgeSetToken` sets it true and returns a canned viewer (throws
`authFailed` if the token contains `bad`); `forgeClearToken` sets it false; `?forge=off` ⇒ every
handler throws `networkError`; data handlers return `src/ipc/fixtures/forge.ts` data (≥3 PRs incl. one
draft + one with comments; one `PrDetail` with labels + `mergeable`; a `CommitStatus` for P63). Spread
`...forgeHandlers` into `mockIpc`. `satisfies Partial<IpcApi>` (compose.ts pattern).

## 9. Command-count delta

+7 commands (5 data + 2 auth) → `generate_handler!` goes from 147 to **154**. Absolute count is
approximate — RECOUNT against `src-tauri/src/lib.rs` at implementation and adjust the `TODO.md` line.

## 10. Acceptance criteria

**AI-gate (orchestrator verifies alone):**
- `cargo build` + `cargo clippy -D warnings` green for the new `bonsai-forge` crate + `src-tauri`.
- `cargo test -p bonsai-forge`: `detect_table`, the rollup precedence cases, every `*_wire_shape_is_
  camel_case`, and GitHub mapping tests driven through a FAKE `HttpTransport` with canned JSON (NO
  network). No test spawns git or hits `api.github.com`.
- Never-log proof: a unit test asserts the redaction helper elides the token from a formatted request/
  error (grep the crate for `token`/`Authorization` in any `log`/`format!` that could surface).
- `tsc` + `pnpm build` green; mock↔real parity: every `forge_*` in `tauri.ts` has a `forgeHandlers`
  entry (a small parity check / eyeball).
- Browser harness (`pnpm dev:mock`): with `?forge=off` the panel shows the offline-error state; default
  shows ForgeConnect → after pasting any token (not containing `bad`) the canned PR list renders →
  open a PR shows detail + comments → the create form submits and opens the new PR. Screenshot the PR
  list + detail as the single visual proof.

**USER CHECKPOINT (native, human — orchestrator must NOT self-pass):**
- Against a REAL GitHub account in `pnpm tauri dev`: paste a real PAT, confirm it is accepted, the
  real open PRs list, a PR opens with real body/comments, and creating a test PR on a scratch remote
  succeeds and opens in the browser. Confirm the token is in the OS keychain (Credential Manager /
  Keychain / Secret Service) and ABSENT from settings.json and any log. Confirm sign-out removes it.
- Rate-limit / bad-token messages read sensibly (force with a revoked token).

## 11. Open decisions (recap; recommended default baked in — do NOT block build)

- **OQ-1** PAT-only v1 (REC) vs OAuth device-flow now — recommend PAT.
- **OQ-2** Add `keyring` crate (REC — guarantees the OS-keychain invariant) vs reuse `git credential`
  helper (no new dep but no keychain guarantee). FLAG: NEW dep on the already-dirty Cargo files + Linux
  Secret-Service/D-Bus backend.
- **OQ-3** Promote `reqwest` dev-dep → real dep with `blocking,json,rustls-tls` (REC) vs add `ureq`.
  FLAG: touches the dirty Cargo files.
- **OQ-4** P64 hook: bake the `onGenerateDescription?` prop seam into `PrCreateForm` now, wire no
  command (button hidden) — REC.
- **OQ-5** Review comments: merge line + conversation comments (REC) vs line-only.
- **OQ-6** Enterprise/self-hosted GitHub host detection (non-`github.com`) — deferred to a later host-
  override setting; v1 treats non-`github.com` as `Unknown`.
