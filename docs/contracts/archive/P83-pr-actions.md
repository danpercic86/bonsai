# P83 — Merge & Close/Decline Pull Requests

Status: DESIGN (architect contract). UI visuals live in `docs/contracts/P83-ui.md` (ui-designer).

## Goal

Extend the forge PR surface from read/create to **act**: merge a PR and close/decline (abort)
a PR from the PR detail panel, implemented for all four forges (GitHub, GitLab, Bitbucket,
Azure DevOps). Destructive outward actions → explicit UI confirmation required; never
force-merge or auto-resolve conflicts.

---

## 1. Module boundaries

- `crates/bonsai-forge/src/provider.rs` — add two trait methods + `MergePrInput`/`MergeMethod`
  imports.
- `crates/bonsai-forge/src/types.rs` — add `MergeMethod`, `MergePrInput` neutral DTOs (+ wire-shape
  tests). Add `MergeMethod::supported_for(kind)` helper for the UI filter.
- `crates/bonsai-forge/src/http.rs` — add `HttpMethod::{Put, Patch}` + reqwest mapping; add `put`/
  `patch` transport helpers per provider `rest.rs` (mirroring the existing `post`).
- `crates/bonsai-forge/src/{github,gitlab,bitbucket,azure}/rest.rs` — endpoint URL builders +
  method-specific transport calls for merge and close/decline.
- `crates/bonsai-forge/src/{github,gitlab,bitbucket,azure}/dto.rs` — request-body builders
  (`merge_body`, `close_body` where applicable) and reuse existing `parse_pr_detail`.
- `crates/bonsai-forge/src/{...}/mod.rs` — implement `merge_pr` / `close_pr` per provider.
- `src-tauri/src/commands/forge.rs` — `forge_merge_pr`, `forge_close_pr` commands (+ `_inner`).
- `src-tauri/src/commands/mod.rs` (or wherever the invoke_handler list lives) — register both.
- `src/ipc/{types.ts,tauri.ts,index.ts}` + `src/ipc/mock/handlers/forge.ts` — TS mirror + mock.

Invariant: only neutral `crate::types` cross the trait boundary; wire JSON stays in `dto.rs`.
Token never in a URL/log. Heavy calls run in `spawn_blocking` (command layer, existing pattern).

---

## 2. Trait extension (`provider.rs`)

```rust
use crate::types::{ /* existing */ MergePrInput};

pub trait ForgeProvider: Send + Sync {
    // ... existing methods ...

    /// Merge PR `number` with the given method. REQUIRES a token
    /// (`ForgeAuthRequired` when none). If the forge reports the PR is not
    /// mergeable (conflicts / needs review / already merged / blocked), returns
    /// a clear `AppError` and changes NOTHING — never forces, never resolves.
    /// Returns the updated `PrDetail` (state should read `Merged`).
    fn merge_pr(&self, number: u64, input: &MergePrInput) -> Result<PrDetail, AppError>;

    /// Close (GitHub/GitLab) / decline (Bitbucket) / abandon (Azure) PR
    /// `number` WITHOUT merging. REQUIRES a token. Returns the updated
    /// `PrDetail` (state should read `Closed`).
    fn close_pr(&self, number: u64) -> Result<PrDetail, AppError>;
}
```

Both are added to the trait and implemented in all four providers this milestone (unlike P62's
"defined-early" methods).

### `MergeMethod` + `MergePrInput` (`types.rs`)

```rust
/// Neutral merge strategy. Not every variant is valid on every forge — the UI
/// filters via `MergeMethod::supported_for(kind)`; the provider maps the chosen
/// variant to its wire value and rejects an unsupported one with `ForgeApi`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MergeMethod {
    Merge,       // standard merge commit
    Squash,      // squash then merge
    Rebase,      // rebase then merge (fast-forward-ish)
    FastForward, // Bitbucket-only fast_forward
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePrInput {
    /// Chosen strategy. UI defaults to the forge's default (see mapping table).
    pub method: MergeMethod,
    /// Optional override commit title/message (GitHub/Bitbucket support this;
    /// GitLab uses `merge_commit_message`; Azure ignores). `None` ⇒ forge default.
    pub commit_title: Option<String>,
    pub commit_message: Option<String>,
    /// Delete the source branch after a successful merge. Maps per forge
    /// (GitHub: N/A on merge → ignore; GitLab `should_remove_source_branch`;
    /// Bitbucket `close_source_branch`; Azure `deleteSourceBranch`). Default false.
    pub delete_source_branch: bool,
    /// Azure ONLY: the head commit id required by its completion call. The
    /// command layer fills this from the PR's `head_sha` before calling; other
    /// forges ignore it. `None` for non-Azure.
    pub head_sha: Option<String>,
}
```

`MergeMethod::supported_for(kind: ForgeKind) -> &'static [MergeMethod]`:

| ForgeKind    | supported methods                          | default    |
|--------------|--------------------------------------------|------------|
| GitHub       | Merge, Squash, Rebase                      | Merge      |
| GitLab       | Merge, Squash                              | Merge      |
| Bitbucket    | Merge, Squash, FastForward                 | Merge      |
| AzureDevOps  | Merge, Squash, Rebase (+ RebaseMerge*)     | Merge      |
| Unknown      | (empty) → merge unsupported                | —          |

*Azure has both `rebase` and `rebaseMerge`. **Recommendation (flag for orchestrator):** expose only
the four neutral variants; map `MergeMethod::Rebase → "rebase"` for Azure and do NOT surface
`rebaseMerge` in v1 (keeps the enum forge-neutral). Revisit if users need rebase-with-merge-commit.

TS mirror in `src/ipc/types.ts`:

```ts
export type MergeMethod = 'merge' | 'squash' | 'rebase' | 'fastForward';
export interface MergePrInput {
  method: MergeMethod;
  commitTitle: string | null;
  commitMessage: string | null;
  deleteSourceBranch: boolean;
  headSha: string | null;
}
```

The UI filter list is duplicated in TS (small pure map) so the mock/browser needs no backend:
`SUPPORTED_MERGE_METHODS: Record<ForgeKind, MergeMethod[]>`.

---

## 3. Per-forge REST (endpoint table)

All merge/close calls REQUIRE auth (call `require_token()` before sending, as `create_pr` does).
On success parse the response (or re-`get_pr`) into `PrDetail`. Add `HttpMethod::Put`/`Patch` to
`http.rs` + reqwest mapping, and `put`/`patch` helpers alongside each `post`.

### GitHub
- **Merge:** `PUT /repos/{owner}/{repo}/pulls/{n}/merge`
  body `{ "merge_method": "merge|squash|rebase", "commit_title"?, "commit_message"? }`.
  Success 200. `merge_method` map: Merge→`merge`, Squash→`squash`, Rebase→`rebase`,
  FastForward→**unsupported** (`ForgeApi("fast-forward merge is not available on GitHub")`).
  Response body has no full PR → after 200 call `get_pr(n)` to return `PrDetail`.
- **Close:** `PATCH /repos/{owner}/{repo}/pulls/{n}` body `{ "state": "closed" }`. Success 200 →
  parse returned PR into `PrDetail`.
- **Error mapping (extend existing `map_status`):** 405 Method Not Allowed / 409 Conflict on merge →
  `ForgeApi("GitHub could not merge this PR — it is not mergeable (conflicts, failing required
  checks, or already merged)")`. 404 already `ForgeApi("not found")`. Note: GitHub's
  `mergeable` may be `null` (still computing) — the UI disables Merge until known; the backend
  still surfaces the 405/409 verbatim-mapped message and changes nothing.

### GitLab
Base `https://{host}/api/v4`, project id already URL-encoded (see existing `merge_request_url`).
- **Merge:** `PUT /projects/{id}/merge_requests/{iid}/merge`
  body `{ "squash": bool, "should_remove_source_branch": bool, "merge_commit_message"? }`.
  Map: Merge→`squash:false`, Squash→`squash:true`; Rebase/FastForward→unsupported (`ForgeApi`).
  Success 200 returns the MR → parse to `PrDetail`.
- **Close:** `PUT /projects/{id}/merge_requests/{iid}` body `{ "state_event": "close" }`.
  Success 200 → parse to `PrDetail`.
- **Error mapping:** 405/406 (not mergeable / has conflicts / needs approval) →
  `ForgeApi("GitLab could not merge this MR — it is not mergeable (conflicts, unresolved
  discussions, or pending approvals)")`. 409 → same class.

### Bitbucket
Base `https://api.bitbucket.org/2.0`, `Authorization: Basic` (existing).
- **Merge:** `POST /repositories/{workspace}/{slug}/pullrequests/{id}/merge`
  body `{ "type": "commit", "merge_strategy": "merge_commit|squash|fast_forward",
  "close_source_branch": bool, "message"? }`.
  Map: Merge→`merge_commit`, Squash→`squash`, FastForward→`fast_forward`; Rebase→unsupported.
  Success 200 returns the merged PR → parse to `PrDetail`. (NOTE: decline reason cannot be set
  via API; do not send one.)
- **Decline:** `POST /repositories/{workspace}/{slug}/pullrequests/{id}/decline` (empty body `{}`).
  Success 200 → parse to `PrDetail`.
- **Error mapping:** 400/409 (has conflicts / not open / merge checks failing) →
  `ForgeApi("Bitbucket could not merge this PR — it is not mergeable (conflicts or unmet merge
  checks)")`.

### Azure DevOps
Base `https://dev.azure.com/{org}/{project}/_apis/git/repositories/{repo}`, every URL carries
`api-version=7.1`; `Authorization: Basic` PAT (existing).
- **Merge (complete):** `PATCH .../pullRequests/{id}?api-version=7.1`
  body `{ "status": "completed",
          "lastMergeSourceCommit": { "commitId": "<head_sha>" },
          "completionOptions": { "mergeStrategy": "noFastForward|squash|rebase|rebaseMerge",
                                  "deleteSourceBranch": bool } }`.
  `lastMergeSourceCommit.commitId` is REQUIRED — the command layer fills `MergePrInput.head_sha`
  from the PR summary's `head_sha` before calling. Map: Merge→`noFastForward`, Squash→`squash`,
  Rebase→`rebase`, FastForward→unsupported. Success 200 → parse to `PrDetail`.
- **Abandon (close):** `PATCH .../pullRequests/{id}?api-version=7.1` body `{ "status": "abandoned" }`.
  Success 200 → parse to `PrDetail`.
- **Error mapping:** 400/409 (conflicts / branch policies not satisfied / already completed) →
  `ForgeApi("Azure DevOps could not complete this PR — merge conflicts or unmet branch policies")`.

**Safety across all four:** the provider NEVER retries, NEVER changes the strategy on failure, and
NEVER resolves conflicts. A non-mergeable response maps to a clear `ForgeApi` message and the local
repo/remote are left untouched.

---

## 4. IPC surface

New commands in `src-tauri/src/commands/forge.rs`, mirroring `forge_create_pr` exactly
(resolve workdir → `resolved_key` → `open_with_key` in one `spawn_blocking`). Neither emits
`repo-changed` (they mutate the remote, not the local repo; the panel refetches).

```rust
#[tauri::command]
pub async fn forge_merge_pr(
    app, state, repo_id: String, number: u64, input: MergePrInput,
) -> Result<PrDetail, AppError>;

#[tauri::command]
pub async fn forge_close_pr(
    app, state, repo_id: String, number: u64,
) -> Result<PrDetail, AppError>;
```

Azure `head_sha` fill: `forge_merge_pr_inner` should, when `input.head_sha` is `None`, fetch it via
`get_pr(number)` head_sha inside the same `spawn_blocking` before calling `merge_pr` — so the UI is
not required to know the sha. **Recommendation:** always fill it backend-side (one extra GET only
when absent) rather than trust a possibly-stale UI value.

Errors (both): `noRepo | forgeAuthRequired | forgeUnsupported | noRemote | forgeApi |
forgeRateLimited | authFailed | networkError | git`.

TS additions:
- `src/ipc/types.ts` `IpcApi`:
  ```ts
  forgeMergePr(repoId: string, number: number, input: MergePrInput): Promise<PrDetail>;
  forgeClosePr(repoId: string, number: number): Promise<PrDetail>;
  ```
- `src/ipc/tauri.ts`:
  ```ts
  forgeMergePr(repoId, number, input) { return invoke<PrDetail>('forge_merge_pr', { repoId, number, input }); }
  forgeClosePr(repoId, number)        { return invoke<PrDetail>('forge_close_pr', { repoId, number }); }
  ```
- `src/ipc/index.ts` — export as usual.

### Mock (`src/ipc/mock/handlers/forge.ts`)
Maintain a module-level `Map<number, PrState>` overlay so merged/closed transitions persist within
the session and `forgeGetPr`/`forgeListPrs` reflect them.
- `forgeMergePr`: `offGuard()`; require `authenticated` else throw `forgeAuthRequired`; if the PR's
  effective state is not `open` throw `forgeApi("mock: PR is not open")`; if
  `input.method` not in `SUPPORTED_MERGE_METHODS[FORGE_KIND]` throw
  `forgeApi("mock: {method} not supported on {forge}")`; a PR number whose fixture `mergeable===false`
  (add one such fixture row) throws `forgeApi("mock: not mergeable — conflicts")`; else set overlay
  state `merged` and return the detail with `summary.state:'merged'`.
- `forgeClosePr`: `offGuard()`; require auth; set overlay `closed`; return detail with
  `summary.state:'closed'`.
- Have `forgeGetPr`/`forgeListPrs`/`forgeCreatePr` consult the overlay so the panel updates live.

---

## 5. Safety (mandated; visuals owned by ui-designer)

- Both actions MUST be gated behind an explicit confirmation dialog (ui-designer's `P83-ui.md`).
  This contract only mandates that the dialog exists and that the destructive command is not fired
  until confirmed.
- Merge is disabled in the UI while `mergeable === null` (unknown) or `false`; the method picker
  is filtered to `SUPPORTED_MERGE_METHODS[provider]`.
- On any `forgeApi`/`authFailed` error the panel surfaces the forge's reason verbatim and refetches
  the PR (nothing changed).

---

## 6. Acceptance criteria

### AI gate (orchestrator-verifiable)
1. `cargo build -p bonsai-forge` compiles: `merge_pr`/`close_pr` implemented in all four providers;
   `MergeMethod`/`MergePrInput` have passing `*_wire_shape_is_camel_case` tests.
2. Per-forge unit tests (FakeTransport spy pattern, as in `github/mod.rs`):
   - correct URL + HTTP method for merge and close/decline;
   - request body construction per method (assert JSON fields, e.g. GitHub `merge_method`,
     GitLab `squash`, Bitbucket `merge_strategy`, Azure `status`/`completionOptions.mergeStrategy`
     + `lastMergeSourceCommit.commitId`);
   - unsupported method → `ForgeApi` and NO request sent (assert spy empty), mirroring
     `create_pr_requires_token`;
   - not-mergeable status (405/409/400 per forge) → mapped `ForgeApi` with a clear message;
   - unauthenticated → `ForgeAuthRequired`, nothing sent.
3. `forge_merge_pr_inner`/`forge_close_pr_inner` round-trip unit test (backend fills Azure head_sha).
4. `tsc` clean; mock serves merge→`merged` and close→`closed` transitions reflected by
   `forgeGetPr`/`forgeListPrs`; browser harness (`VITE_MOCK_IPC=1`, `?forge=auth` and
   `?forge=gitlab|bitbucket|azure`) shows the action buttons and state change.

### USER CHECKPOINT (native, per forge — cannot run headless)
Against a real disposable PR on each of GitHub, GitLab, Bitbucket, Azure DevOps: confirm merge
(one supported strategy each) succeeds and the panel shows `merged`; confirm close/decline succeeds
and shows `closed`; confirm a deliberately-conflicting PR surfaces the not-mergeable message and
changes nothing.

---

## 7. Sub-increments (each one fresh-context senior-dev pass)

1. **P83a — neutral core + IPC skeleton:** `types.rs` (`MergeMethod`/`MergePrInput` + tests +
   `supported_for`), `http.rs` (`Put`/`Patch` + helpers), `provider.rs` trait methods, GitHub impl
   (`github/{rest,dto,mod}.rs`), commands `forge_merge_pr`/`forge_close_pr` + registration,
   TS types/tauri/index + mock. Gate: GitHub end-to-end in the harness.
2. **P83b — GitLab** (`gitlab/{rest,dto,mod}.rs` + tests).
3. **P83c — Bitbucket** (`bitbucket/{rest,dto,mod}.rs` + tests).
4. **P83d — Azure DevOps** (`azure/{rest,dto,mod}.rs` + tests; includes backend head_sha fill).

---

## Open questions (flag for orchestrator)
- **OQ1 Azure rebaseMerge:** recommend NOT exposing it in v1 (four neutral variants only). Confirm.
- **OQ2 Delete-source-branch default:** recommend `false` and letting the UI offer a checkbox.
  Confirm whether it should default to the repo's forge setting instead (would need an extra field).
