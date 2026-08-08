# P64 — More forge providers + AI PR descriptions

Third Phase-4 milestone. Builds STRICTLY on `docs/contracts/P62-forge-foundation.md` (the
`ForgeProvider` trait, neutral DTOs, `detect_provider`, `HttpTransport` seam, `open()` factory,
`ForgeKind`, keychain `TokenStore`, the `onGenerateDescription?` seam in `PrCreateForm`) and the
shared conventions in `docs/contracts/phase4-forge-overview.md`. Do NOT redesign the trait — every
new provider implements it verbatim; the additions here are strictly additive and flagged.

## 1. Goal & scope

Two independent parts:

- **(A) More providers behind the SAME trait** — GitLab, Bitbucket, Azure DevOps, each a new
  `crates/bonsai-forge/src/<provider>/{mod,rest,dto}.rs`, selected by extending `detect_provider`
  and the `open()` `match`. This VALIDATES the P62 abstraction: provider JSON never escapes
  `<provider>/dto.rs`; each maps onto the P62 neutral DTOs + `CommitStatus`/`CheckRollup`. **Zero new
  commands** (they slot behind P62's `forge_*` surface).
- **(B) AI PR-description generation** — one new command `ai_generate_pr_description(repoId, base,
  head) → PrDescription{title, body}`, grounded in the real branch commits/diff, shown in the
  create-PR form via P62's `onGenerateDescription` seam. Human-in-the-loop: generate → user
  reviews/edits → the form fills → the user still clicks Create. WRITES NOTHING; never auto-posts.
  Provider-AGNOSTIC (pure local git + `run_claude`), so it ships for ALL providers incl. GitHub.

**Recommended sequencing (OQ-A1):** ship **Part B (all providers) + GitLab (Part A)** in P64; land
**Bitbucket** and **Azure DevOps** as P64b/P64c reusing the proven recipe. Each extra provider is a
full senior-dev increment (new `dto/rest/mod` + fixtures + mock parity + tests); Azure additionally
forces `ForgeTarget.project`, Basic auth, and ref-name stripping — doing all three at once is a
god-PR. This contract fully specifies ALL THREE so the split is a scheduling choice, not a design gap.

OUT of scope: OAuth/device-flow, GraphQL, webhooks, auto-poll, PR merge/close/review-submit,
comment POSTING, self-managed/enterprise host override (deferred, P62 OQ-6).

## 2. Reuse (by path — do NOT reinvent)

**AI core (Part B):**
- `crates/bonsai-core/src/ai/mod.rs::run_claude(cwd,&prompt,Some(&payload),RunOpts)` + `RunOpts`
  (`strip_fence` already applied to `result.text` inside).
- `crates/bonsai-core/src/git/ai_explain.rs::resolve_digest_range(&repo,&AiDigestRange::BetweenRefs
  {from,to}) -> (Header, Vec<Commit>, Option<Tree> /*old*/, Tree /*new*/)` + `cap_review_payload(String)
  -> String` (256 KiB `MAX_REVIEW_PAYLOAD_BYTES` byte-cap). Same reuse `ai_changelog.rs` does.
- `crates/bonsai-core/src/ai/payload.rs::{render_commit_list, render_headers, CommitLine}`.
- `crates/bonsai-core/src/git/diff.rs::{build_diff_options, apply_find_similar, collect_headers}`;
  `crates/bonsai-core/src/git/stage.rs::open_workdir_repo`.
- `crates/bonsai-core/src/git/ai_summary.rs::AI_SUMMARY_MAX_COMMITS` (=200 commit cap; the whole
  base..target-unique-commits + net-diffstat grounding in `summarize_range` is the template).
- Command gate template: `src-tauri/src/commands/ai.rs::ai_summarize_range` / `_inner` (settings gate
  BEFORE `repo_path`).
- Frontend gate: the `aiEligible` prop pattern in `src/components/CommitBox.tsx`
  (`aiEnabled && aiConsented && installed`).

**Forge foundation (Part A):** everything in P62 §2/§3/§6/§7 — the trait, `HttpTransport`,
`ForgeTarget`/`detect_provider`, `TokenStore`, DTO set, the GitHub `{mod,rest,dto}.rs` as the
per-provider template.

---

## PART A — More providers

## 3a. Module boundaries

**New (each mirrors `github/{mod,rest,dto}.rs`; ~<500 lines/file; wire JSON confined to `dto.rs`):**

| File | Responsibility |
|---|---|
| `crates/bonsai-forge/src/gitlab/{mod,rest,dto}.rs` | `GitLabProvider` (REST v4) impl `ForgeProvider` |
| `crates/bonsai-forge/src/bitbucket/{mod,rest,dto}.rs` | `BitbucketProvider` (Cloud REST 2.0) |
| `crates/bonsai-forge/src/azure/{mod,rest,dto}.rs` | `AzureDevOpsProvider` (REST 7.1) |

Each `mod.rs`: `struct <X>Provider { target: ForgeTarget, token: Option<String>, http: Box<dyn
HttpTransport> }` + the 6 trait methods + `viewer()` (§3d). Each `rest.rs`: endpoint URL builders +
per-provider auth-header assembly (§3c) + pagination + HTTP-status→`AppError` mapping (same taxonomy
as P62 §7: 401/403-bad-creds→`AuthFailed`; rate-limit→`ForgeRateLimited`; 404→`ForgeApi("not found")`;
other 4xx/5xx/parse→`ForgeApi`; transport→`NetworkError`; `token==None` on an auth-required call →
`ForgeAuthRequired` before any request). Each `dto.rs`: `#[derive(Deserialize)]` wire structs +
`into_*` mappers to `types.rs`.

**Extended P62 files (all ADDITIVE — flag on the dirty crate):**

| File | Change |
|---|---|
| `src/provider.rs` | `ForgeKind` += `GitLab, Bitbucket, AzureDevOps`; add `fn viewer(&self) -> Result<ForgeViewer, AppError>` to the trait (§3d, OQ-A4) |
| `src/detect.rs` | host table + per-provider path parsing (§3b); `ForgeTarget.project: Option<String>` |
| `src/lib.rs` | `open()` `match target.kind { GitHub=>…, GitLab=>…, Bitbucket=>…, AzureDevOps=>…, Unknown=>ForgeUnsupported-on-data }`; 3 `mod` decls |
| `src/types.rs` | `ForgeKind` arms; `ForgeRepoContext.project: Option<String>`; extend camelCase tests |
| `src/auth.rs` | `forge_set_token` validation now goes through `provider.viewer()` (was GitHub-hardcoded) |

## 3b. Detection extension (`detect.rs`) — pseudocode

```
extend detect_provider(remote_url) after P62's (host,path) parse:
  host = lowercase(host)
  match host:
    "github.com"                 -> kind=GitHub;      owner,repo = 2-seg(path);          project=None
    "gitlab.com"                 -> kind=GitLab;      owner=path[..last], repo=path.last; project=None   # subgroups: owner may contain '/'
    "bitbucket.org"              -> kind=Bitbucket;   owner,repo = 2-seg(path);           project=None    # owner = workspace
    "dev.azure.com" | "ssh.dev.azure.com"
                                 -> kind=AzureDevOps; from path "{org}/{project}/_git/{repo}" (ssh: "v3/{org}/{project}/{repo}")
                                    owner=org, repo=repo, project=Some(project)
    endswith ".visualstudio.com" -> kind=AzureDevOps; org = host-subdomain; path "{project}/_git/{repo}"
                                    owner=org, repo=repo, project=Some(project)
    else                         -> kind=Unknown
  web_url:
    GitHub/GitLab/Bitbucket -> "https://{host}/{owner}/{repo}"
    AzureDevOps             -> "https://dev.azure.com/{org}/{project}/_git/{repo}"
  return Some(ForgeTarget{ kind, host, owner, repo, project, web_url })
```

Notes: GitLab relaxes P62's exactly-2-segments rule (namespace may nest → owner = everything-but-last;
API uses URL-encoded full path, §3c). Azure needs 3 identifiers → the `project` field (OQ-A3).
`detect_table` test extends with each provider's https/ssh/scp forms, gitlab subgroup, legacy
`visualstudio.com`, and non-provider host ⇒ `Unknown`.

## 3c. Per-provider mapping onto the neutral DTOs

All PR/MR concepts map onto the P62 DTOs unchanged (`PrSummary`/`PrDetail`/`PrPage`/`ReviewComment`/
`CommitStatus`). Only the wire structs + endpoints differ. `head_sha` MUST be populated (P63 needs it).

### GitLab (REST v4) — base `https://{host}/api/v4`
Auth header: `PRIVATE-TOKEN: <token>`. Project id = url-encode(`owner/repo`) → `{id}`.

| Trait method | Endpoint / mapping |
|---|---|
| `list_prs` | `GET /projects/{id}/merge_requests?state=&per_page=&page=` → `PrSummary[]`. state `opened→Open, closed→Closed, merged→Merged, locked→Closed`; filter `open→opened, closed→closed, all→all`. `iid`→number (NOT `id`), `draft`→isDraft, `source_branch`/`target_branch`, `author.username`/`avatar_url`, `user_notes_count`→comments, `web_url`→url, `sha`→headSha |
| `get_pr` | `GET /projects/{id}/merge_requests/{iid}` → body=`description`; `detailed_merge_status=="mergeable"`→`Some(true)` / conflict→`Some(false)` / checking→`None`; `labels[]`; additions/deletions/changed_files via `?...` diff summary or `/changes` (heavy — v1 may leave `0`, OQ-A2) |
| `create_pr` | `POST /projects/{id}/merge_requests` `{source_branch,target_branch,title,description}`. draft ⇒ prefix title `"Draft: "` (GitLab convention; no `draft` param on create) |
| `list_review_comments` | `GET .../merge_requests/{iid}/notes` (`Conversation`) + `.../discussions` diff notes (`Review`, carry `position.new_path`/`new_line`); drop system notes; sort by `created_at` |
| `combined_status` | `GET /projects/{id}/repository/commits/{sha}/statuses` → contexts. `success→Success, running|pending|created→Pending, failed→Failure, canceled|skipped→Neutral, manual→Neutral`, else `Error` |
| `viewer` | `GET /user` → `{login:username, avatarUrl:avatar_url}` |
Pagination: `has_next` = `X-Next-Page` header non-empty (or returned count == per_page).

### Bitbucket (Cloud REST 2.0) — base `https://api.bitbucket.org/2.0`
Auth header: `Authorization: Bearer <token>` (workspace/repo/project **access token** — REC, keeps the
single-secret model). App-password fallback = paste `user:app_password`, backend base64 → `Basic`
(OQ-A5). owner=workspace, repo=repo_slug.

| Trait method | Endpoint / mapping |
|---|---|
| `list_prs` | `GET /repositories/{ws}/{slug}/pullrequests?state=&page=&pagelen=` → state `OPEN→Open, MERGED→Merged, DECLINED|SUPERSEDED→Closed`; filter maps `open→OPEN, closed→DECLINED, all→` (omit param). `id`→number, `draft`→isDraft, `title`, `author.display_name`/`links.avatar.href`, `source.branch.name`/`destination.branch.name`, `comment_count`→comments, `links.html.href`→url, `source.commit.hash`→headSha |
| `get_pr` | `GET .../pullrequests/{id}` → body=`description` (rendered `raw` if present); mergeable ⇒ `None` (not directly exposed); labels ⇒ `[]`; additions/deletions via `.../diffstat` (heavy — v1 may leave `0`, OQ-A2) |
| `create_pr` | `POST .../pullrequests` `{title,description,source:{branch:{name}},destination:{branch:{name}},draft}` |
| `list_review_comments` | `GET .../pullrequests/{id}/comments` → `inline`≠null ⇒ `Review` (`inline.path`/`inline.to`) else `Conversation`; skip deleted; sort by `created_on` |
| `combined_status` | `GET /repositories/{ws}/{slug}/commit/{sha}/statuses` → `SUCCESSFUL→Success, INPROGRESS→Pending, FAILED→Failure, STOPPED→Neutral`, else `Error` |
| `viewer` | `GET /user` → `{login:username, avatarUrl:links.avatar.href}` |
Pagination: body has `next` (absolute URL) + `values[]`; `has_next` = `next` present. (No Link header.)

### Azure DevOps (REST 7.1) — base `https://dev.azure.com/{org}/{project}/_apis/git/repositories/{repo}`
Auth header: `Authorization: Basic base64(":" + <PAT>)` (empty username). Every URL carries
`api-version=7.1`. Ref names are `refs/heads/x` → strip prefix for source/target branch.

| Trait method | Endpoint / mapping |
|---|---|
| `list_prs` | `GET .../pullrequests?searchCriteria.status=&$top=&$skip=` → state `active→Open, completed→Merged, abandoned→Closed`; filter `open→active, closed→abandoned, all→all`. `pullRequestId`→number, `isDraft`→isDraft, `title`, `createdBy.displayName`/`imageUrl`, `sourceRefName`/`targetRefName` (stripped)→branches, `lastMergeSourceCommit.commitId`→headSha |
| `get_pr` | same PR object → body=`description`; `mergeStatus`: `succeeded→Some(true), conflicts→Some(false), queued|notSet→None`; labels=`labels[].name`; additions/deletions/changed_files ⇒ `0` in v1 (needs `/iterations/{i}/changes`, OQ-A2) |
| `create_pr` | `POST .../pullrequests` `{sourceRefName,targetRefName,title,description,isDraft}` (add back `refs/heads/`) |
| `list_review_comments` | `GET .../pullrequests/{id}/threads` → each thread's `comments[]`; `threadContext.filePath`/`rightFileStart.line` ⇒ `Review` else `Conversation`; skip system/deleted |
| `combined_status` | `GET .../pullrequests/{id}/statuses` (or commit statuses) → `succeeded→Success, pending→Pending, failed|error→Failure, notApplicable|notSet→Neutral` |
| `viewer` | `GET https://app.vssps.visualstudio.com/_apis/profile/profiles/me?api-version=7.1` → `{login:displayName, avatarUrl:None}` |
Pagination: `$skip`/`$top`; `has_next` = returned count == `$top`.

## 3d. Trait / DTO additions (additive)

- `ForgeKind` gains `GitLab, Bitbucket, AzureDevOps` (Rust) / `'gitLab' | 'bitbucket' | 'azureDevOps'`
  (TS union) + wire-shape test coverage.
- `ForgeTarget` + `ForgeRepoContext` gain `project: Option<String>` (Rust) / `project: string | null`
  (TS) — `None` for GitHub/GitLab/Bitbucket, `Some(project)` for Azure (OQ-A3).
- Trait gains `fn viewer(&self) -> Result<ForgeViewer, AppError>` so `forge_set_token` validation is
  provider-aware (each hits its own identity endpoint via its own auth header) instead of a
  GitHub-hardcoded call in `auth.rs` (OQ-A4). No other trait change.

## 3e. Frontend deltas (Part A)

- `src/ipc/types.ts` — extend `ForgeKind` union; add `project: string | null` to `ForgeRepoContext`.
- `src/components/ForgeConnect.tsx` — provider-specific paste hint + token-help link (GitLab PAT with
  `api` scope; Bitbucket access token / app-password; Azure PAT with Code=Read&Write). Presentational
  only; still password input, keychain note, no prefill.
- `src/ipc/fixtures/forge.ts` + `src/ipc/mock/handlers/forge.ts` — `?forge=gitlab|bitbucket|azure`
  URL sentinel sets `ForgeRepoContext.provider` (+ `project` for azure); the PR list/detail/comment
  fixtures stay provider-neutral (same DTO shape — the abstraction payoff). Existing sentinels
  (`?forge=auth|off`, `bad`-token) unchanged.
- PR list/detail/comment components: **unchanged** (they render neutral DTOs).

---

## PART B — AI PR description

## 4a. Rust — `crates/bonsai-core/src/git/ai_pr_description.rs` (new)

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrDescription {
    pub title: String,      // first output line, trimmed; imperative, no trailing period
    pub body: String,       // Markdown, may be "" (why-not-what)
    pub base: String,       // echoed resolved base ref
    pub head: String,       // echoed resolved head ref
    pub commit_count: u32,  // commits listed (capped at AI_SUMMARY_MAX_COMMITS)
    pub cost_usd: Option<f64>,
}

/// Blocking, READ-ONLY. Grounds a PR title+body in the commits unique to `head`
/// vs `base` (merge-base range) + the net diffstat, then calls the CLI. WRITES
/// NOTHING. Errors: `aiFailed` (empty range / no usable title / CLI failure) |
/// `git` (bad ref) | (`aiUnavailable` via the command-layer gate).
pub fn generate_pr_description(
    workdir: &Path, base: &str, head: &str, opts: RunOpts,
) -> Result<PrDescription, AppError>;
```

Pseudocode (reuses the shipped resolver + renderers + cap exactly like `ai_changelog`):

```
generate_pr_description(workdir, base, head, opts):
  repo = open_workdir_repo(workdir)
  (_hdr, commits, old_tree, new_tree) =
      ai_explain::resolve_digest_range(&repo, &AiDigestRange::BetweenRefs{ from:base, to:head })?   # bad ref => Git
  diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), build_diff_options(&[],false))
  apply_find_similar(&mut diff); headers = collect_headers(&diff)
  if commits.is_empty() && headers.is_empty():
      return Err(AiFailed("nothing to describe: {head} has no commits beyond {base}"))   # BEFORE any CLI call
  total = commits.len()
  lines = commits.iter().take(AI_SUMMARY_MAX_COMMITS)
             .map(|c| CommitLine{ short_oid:c.id()[..7], summary:c.summary, author:c.author.name })
  commit_count = lines.len()
  commits_section = render_commit_list(&lines) + if total>lines.len() { "(+{n} more commits)\n" }
  payload = cap_review_payload(
      "COMMITS (head since base):\n{commits_section}\nNET CHANGES (diffstat):\n{render_headers(&headers).text}")
  result = run_claude(workdir, PR_PROMPT, Some(&payload),
                      RunOpts{ system_prompt:Some(PR_SYSTEM_PROMPT), ..opts })?
  (title, body) = split_title_body(&result.text)
  if title.is_empty(): return Err(AiFailed("Claude returned no usable title"))
  return PrDescription{ title, body, base, head, commit_count, cost_usd:result.cost_usd }

# pure, unit-tested helper:
split_title_body(text) -> (title, body):
  t = text.trim()
  first = first non-empty line, trimmed; strip a leading "# " or "PR: " / "Title:" prefix defensively
  body = remainder after that line, one leading blank line skipped, trim_end
  return (first, body)   # single-line output => body=""
```

## 4b. Prompts (single-line consts — Windows `.cmd` argv constraint; assert with a `prompts_are_single_line` test)

- `PR_SYSTEM_PROMPT` (verbatim, one line): "You are drafting a pull-request title and description for
  a teammate reviewer from a list of commits and a net diffstat on standard input. Output whose FIRST
  line is a concise imperative PR title (<=72 chars, no trailing period, no 'PR:' prefix), then a
  blank line, then a Markdown body that explains WHY the change exists and WHAT it does at a high
  level: a one-paragraph summary, then a `## Changes` section with grouped bullets of the notable
  changes, then a `## Notes` section ONLY if something is risky/incomplete/needs reviewer attention.
  Prefer intent over a commit-by-commit list. Do NOT wrap the output in a code fence."
- `PR_PROMPT` (positional `-p`, one line): "Draft a pull-request title and description for the branch
  described on standard input."

Grouping is the model's job (why-not-what, mirrors P56 changelog; OQ-B1). Rust does no prefix parsing.

## 4c. Command triple (`src-tauri/src/commands/ai.rs`) + gate

Mirror `ai_summarize_range` / `ai_summarize_range_inner` EXACTLY:

```rust
#[tauri::command]
pub async fn ai_generate_pr_description(
    app: tauri::AppHandle, state: tauri::State<'_, AppState>,
    repo_id: String, base: String, head: String,
) -> Result<PrDescription, AppError> {
    let file = settings::settings_file(&app)?;
    ai_generate_pr_description_inner(state.inner(), &file, &repo_id, base, head).await
}
pub(crate) async fn ai_generate_pr_description_inner(
    state: &AppState, settings_file: &std::path::Path, repo_id: &str, base: String, head: String,
) -> Result<PrDescription, AppError> {
    let s = settings::load_from(settings_file);
    if !(s.ai_enabled && s.ai_consented) {
        return Err(AppError::AiUnavailable("AI features are disabled or not yet consented to".into()));
    }
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move ||
        ai_pr_description::generate_pr_description(&workdir, &base, &head, RunOpts::default()))
        .await.map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```

Register in `src-tauri/src/commands/mod.rs` and `src-tauri/src/lib.rs` `generate_handler!`; re-export
`PrDescription` from `commands/shared.rs`. READ-ONLY ⇒ NO `repo-changed`. No new `AppError` variant
(reuses `AiUnavailable`/`AiFailed`/`Git`).

## 4d. IPC surface (the ONE new command)

| Command (snake) | TS method | Wire request | Response |
|---|---|---|---|
| `ai_generate_pr_description` | `aiGeneratePrDescription(repoId, base, head)` | `{ repoId, base, head }` | `PrDescription` |

`src/ipc/types.ts`: add `PrDescription` + the method on `IpcApi` (doc: rejects `aiUnavailable |
aiFailed | git`). `src/ipc/tauri.ts`: thin `invoke<PrDescription>('ai_generate_pr_description',
{repoId, base, head})`. `src/ipc/index.ts`: re-export `PrDescription`.

```ts
export interface PrDescription {
  title: string; body: string; base: string; head: string; commitCount: number; costUsd: number | null;
}
```

## 4e. Frontend seam + form behavior

P62 baked `onGenerateDescription?` into `PrCreateForm` (button hidden while undefined). P64 defines it:

```ts
// PrCreateForm props (fill the P62 seam):
onGenerateDescription?: (base: string, head: string) => Promise<PrDescription>;
aiEligible?: boolean;   // aiEnabled && aiConsented && installed — mirror CommitBox
```

- `PrCreateForm`: show a "Generate with AI ✨" button when `onGenerateDescription !== undefined`;
  disabled when `!aiEligible || !base || !head || generating || submitting`; tooltip explains why when
  `!aiEligible` (mirror CommitBox). On click → `generating=true` → await → populate the `title` +
  `body` fields (editable; **REC: overwrite both**, user asked to generate — OQ-B2) → on reject show
  an error toast. NEVER calls create. The user still clicks "Create pull request".
- `PrPanel` (container): pass `onGenerateDescription={(base, head) => ipc.aiGeneratePrDescription(
  repoId, base, head)}` and `aiEligible` (same source the rest of the app uses).
- Mock (`src/ipc/mock/handlers/forge.ts`): add `aiGeneratePrDescription` — honor `?ai=off` ⇒ throw
  `{kind:'aiUnavailable'}`; a `#fail` sentinel in `head` ⇒ `{kind:'aiFailed'}`; else return a canned
  `PrDescription` (title + short Markdown body, echoing base/head). `satisfies Partial<IpcApi>`.
  Mock↔real parity: `aiGeneratePrDescription` in `tauri.ts` ⇔ a `forgeHandlers`/mock entry.

## 5. Command-count delta

**+1 command** (`ai_generate_pr_description`; Part B). **Part A adds 0** — new providers slot behind
P62's existing `forge_*` surface (the whole point of the abstraction). Absolute is approximate:
base 147 → +7 (P62) → +1 (P63 `forge_commit_status`) → **+1 (P64)**. RECOUNT `generate_handler!` in
`src-tauri/src/lib.rs` at implementation and update `TODO.md`.

## 6. Acceptance criteria

**AI-gate (orchestrator alone):**
- `cargo build` + `cargo clippy -D warnings` green (bonsai-forge, bonsai-core, src-tauri).
- `cargo test -p bonsai-forge` — for EACH shipped provider, driven by a FAKE `HttpTransport` with
  canned fixture JSON (NO network, cross-platform): extended `detect_table`; `list_prs`/`get_pr`/
  `create_pr`/`list_review_comments`/`combined_status` map into the neutral DTOs (state/draft/headSha/
  comment-kind split); `combined_status` rollup precedence per provider's CI vocabulary; auth-header
  assembly asserted on the captured `HttpRequest` (`PRIVATE-TOKEN` / `Bearer` / `Basic base64(:PAT)`);
  a redaction test proving the token is absent from any formatted request/error; wire-shape camelCase
  for the new `ForgeKind` arms + `project`.
- `cargo test -p bonsai-core` — Part B via the `BONSAI_CLAUDE_BIN` stub pattern (as `ai_changelog`):
  `prompts_are_single_line`; `pr_description_wire_shape_is_camel_case`; `split_title_body` cases
  (title+body, title-only, `# `/`PR:` strip, blank-line handling); `empty_range_fails_before_cli`
  (fake bin path ⇒ must be `AiFailed`, not `AiUnavailable`); end-to-end stub echo in a
  `tests/ai_pr_description_cli.rs`; a consent-gate test on `ai_generate_pr_description_inner`
  (disabled ⇒ `AiUnavailable`).
- `tsc` + `pnpm build` green; every `forge_*`/`aiGeneratePrDescription` in `tauri.ts` has a mock entry.
- Browser harness (`pnpm dev:mock`): `?forge=gitlab` (or `auth`) + AI consented (seed
  `bonsai.mockUiSettings` `aiConsented:true` in localStorage, reload) → open the create-PR form →
  "Generate with AI ✨" populates title+body from the mock → edit → Create submits (never auto-posts).
  `?forge=bitbucket`/`azure` render the neutral list/detail identically. Screenshot the populated
  create form as the single visual proof.

**USER CHECKPOINT (native, human — orchestrator must NOT self-pass):**
- Per SHIPPED provider (GitLab now; Bitbucket/Azure when they land), in `pnpm tauri dev`: paste a real
  PAT/token, confirm accepted, real MR/PR list renders, one opens with real body/comments, and
  creating a test PR/MR on a scratch remote succeeds + opens in the browser. Token in the OS keychain,
  ABSENT from settings.json/logs; sign-out removes it. Rate-limit/bad-token messages read sensibly.
- AI PR description against a real branch: Generate yields a sensible title + why-not-what body
  grounded in the actual commits; edit; Create uses the edited text.

## 7. Open questions (recommended default baked in; non-blocking)

- **OQ-A1 — Order & split.** REC: P64 = Part B (all providers) + GitLab; **order GitLab → Bitbucket →
  Azure DevOps**; Bitbucket/Azure follow as P64b/P64c. Rationale: GitLab is the least-divergent second
  provider (validates the abstraction cheaply); Azure is the most divergent (forces `project`, Basic
  auth, ref stripping). FLAG for orchestrator scheduling.
- **OQ-A2 — Coverage depth.** REC: full trait parity for every provider, but let
  `additions/deletions/changed_files` be `0` (and `mergeable`=`None` where not directly exposed) in v1
  rather than firing an extra heavy diff/diffstat call per PR; add it later. `list_review_comments` +
  `combined_status` ship for all.
- **OQ-A3 — `project` field.** REC: add `project: Option<String>` to `ForgeTarget` + `ForgeRepoContext`
  (additive, camelCase). Needed for Azure org/project/repo; `None` elsewhere.
- **OQ-A4 — Provider-aware token validation.** REC: add `fn viewer(&self)` to the trait; `forge_set_token`
  builds the provider with the candidate token and calls `viewer()`. Minimal additive extension (NOT a
  redesign); keeps provider identity endpoints + JSON inside each `dto.rs`. Alt: a `match kind` in
  `auth.rs` (leaks provider knowledge out of the provider module — rejected).
- **OQ-A5 — Bitbucket secret.** REC: paste a Bitbucket **access token** → `Bearer` (single-secret,
  preserves P62 keychain model). App-password (needs `user:app_password` → `Basic`) is a documented
  fallback that strains the single-secret invariant; defer unless a checkpoint needs it.
- **OQ-A6 — GitLab subgroups.** REC: `owner` = full namespace path (may contain `/`), `repo` = last
  segment; the API takes the URL-encoded full path. Relaxes P62's exactly-2-segments rule for GitLab.
- **OQ-B1 — Prompt style.** REC: why-not-what, model-driven grouping (`## Changes` / optional
  `## Notes`), reusing the P56 changelog philosophy. Alt: strict conventional-commit sections — heavier,
  deferred.
- **OQ-B2 — Fill vs overwrite.** REC: overwrite both title+body on generate (user explicitly asked;
  fields stay editable). Alt: fill-only-if-empty + a separate "Regenerate" — more UI, deferred.
