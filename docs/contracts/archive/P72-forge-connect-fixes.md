# P72 — Forge connect fixes: Azure DevOps 401 + dead external links

Status: contract (design only). Board entry: `TODO.md` §"🐛 P72".
Companion UI contract (copy + states, owned by `ui-designer`): `docs/contracts/P72-ui.md`.
Prior contracts: `docs/contracts/phase4-forge-overview.md`, `docs/contracts/P64-forge-providers-ai-pr.md`,
`docs/contracts/P64-user-checklist.md`, `docs/contracts/P62-user-checklist.md`,
`docs/contracts/P49-external-integrations.md`.

Two independently committable increments:

| Increment | Scope | Commit message |
| --- | --- | --- |
| **A** | Azure DevOps `viewer()` validate-then-identify + `map_status` 203 + doc drift | `wip(P72): azure validate-then-identify` |
| **B** | `openUrl` IPC end-to-end (core → command → IPC → mock → both anchors) | `wip(P72): openUrl IPC for external links` |

A and B share no files. Either may land first; A is the blocking user bug.

---

## 0. Root cause (settled — do not re-diagnose)

**Bug 1 — Azure connect rejects a valid Code-scoped PAT with 401.** The Basic auth header is
correct (`crates/bonsai-forge/src/azure/rest.rs:36-46` → `Basic base64(":" + PAT)`, empty username;
`api-version=7.1` on every URL; the PAT never in a URL) and `detect.rs:83-137` parses the remote
correctly. The fault is the **validation endpoint**: `set_token` → `validate_token`
(`lib.rs:124-154`) → `AzureDevOpsProvider::viewer()` (`azure/mod.rs:116-127`) probes
`PROFILE_URL` on `app.vssps.visualstudio.com`, which is gated on the PAT's **User Profile (Read)**
(`vso.profile`) scope. The Connect panel asks for a **Code (Read & Write)** PAT
(`src/components/ForgeConnect.tsx:53-57`), which has no profile scope ⇒ Azure 401 ⇒
`AppError::AuthFailed("Azure DevOps rejected the credentials (401)")`, implying the token is bad.
Tests pass because the in-file `FakeTransport` (`azure/mod.rs:196-228`) answers 200 on the profile
URL. The contract encodes the same mismatch (`P64-forge-providers-ai-pr.md:147-158` vs
`P64-user-checklist.md:64-67`).

Scope facts, verified against Microsoft's official OAuth scope table: `vso.profile`
("User profile (read)") is what `_apis/profile/profiles/me` requires; `vso.code` covers
`GET _apis/git/repositories/{repo}`, and `vso.code_write` inherits it. So a repo-endpoint probe is
reachable by exactly the PAT the UI asks for.

**Bug 2 — "Create a token" / "Open in browser ↗" are dead in the native app.**
`ForgeConnect.tsx:98-106` and `PrDetailView.tsx:46-53` are plain `<a href target="_blank">` with no
`onClick` and no IPC. There is no opener/shell plugin (`src-tauri/Cargo.toml` registers dialog,
updater, process only), no `opener:*` permission (`src-tauri/capabilities/default.json:6-11`), and
no new-window/navigation handler in `src-tauri/src/lib.rs` — the webview silently drops the
request. It works in the browser harness, which is exactly why the AI gate missed it. Predicted and
deferred at `P62-user-checklist.md:39-43`. The P49 commands cannot be reused: they fs-precheck
`path.exists()` (`src-tauri/src/commands/external.rs:54-57`), which rejects a URL.

**Process finding (record in the milestone notes, no action).** Both defects live *exactly where the
test doubles are*: the Azure `FakeTransport` returns 200 from the profile URL, so no test ever asked
"what can a Code-only PAT reach?", and the browser harness is the one environment where
`target="_blank"` works. Neither is a coverage-**count** problem; both are fidelity-of-double
problems. The countermeasure is the per-URL routing test matrix in §3, not more tests elsewhere.

**Out of scope (note only, no fix here).** `dev.azure.com/{org}/_git/{repo}` shorthand where
`repo == project` returns `None` from `detect_azure` (`detect.rs:123`); it surfaces as *unsupported*,
not 401. Documented at `P64-user-checklist.md:59-63`; stays open.

---

## 1. Increment A — Azure validate-then-identify

### A0. Design in one line

`viewer()` becomes **validate (must succeed) → identify (best effort)**: validate with the *repo*
endpoint the app's real work already needs (Code scope), then make ONE best-effort attempt at the
existing profile endpoint for a display name, and never let that second half fail the connect.

**No `connectionData` rung.** An earlier draft added `_apis/connectionData` as a first identity
attempt; it is dropped, deliberately. Two reasons: (1) `connectionData` has **no documented scope**
in Microsoft's scope table and returns `authenticatedUser`, so it most plausibly needs `vso.profile`
too — i.e. it would reliably fail for exactly the Code-only PAT it was added to serve, costing a
guaranteed-failing round-trip on every connect; (2) `ForgeViewer.login` is **never rendered
anywhere** in the frontend (the only `src/` references outside tests are
`src/ipc/fixtures/forge.ts:41` and the `types.ts` declaration; `PrPanel.tsx:187` discards the
resolved viewer). A speculative second endpoint cannot be justified for a value nothing displays.

**There is no UI work for identity in P72.** The empty-login case has no render site and no toast;
`P72-ui.md` covers only the Azure scope-hint copy and the link states. No mock change to
`src/ipc/mock/handlers/forge.ts` is required.

### A1. Files touched

| File | Change |
| --- | --- |
| `crates/bonsai-forge/src/azure/rest.rs` | +`repository_url`, `map_status` 203 arm, reworded 401 arm, header-byte test |
| `crates/bonsai-forge/src/azure/dto.rs` | +`parse_repo_probe`, +`AzRepoProbe` wire struct |
| `crates/bonsai-forge/src/azure/mod.rs` | `viewer()` body only (signature unchanged); tests updated/added |
| `crates/bonsai-forge/src/lib.rs` | doc drift only (`:11-12` Bearer claim, `:129`/`:139` "GET /user") |
| `crates/bonsai-forge/src/http.rs` | doc drift only (`:49` Bearer claim) |
| `docs/contracts/phase4-forge-overview.md` | doc drift (`:67-68`) |
| `docs/contracts/P64-forge-providers-ai-pr.md` | Azure `viewer` row (`:147-158`) |
| `docs/contracts/P64-user-checklist.md` | mark the 203 gap (`:69-74`) closed; keep `:59-63` open |
| `src/components/ForgeConnect.tsx` | Azure scope-hint copy **only if** `P72-ui.md` specifies a change (copy is ui-designer's) |

No change to `ForgeProvider`, to `viewer()`'s signature, to `ForgeViewer`, to the IPC surface, to the
mock forge handlers, or to GitHub / GitLab / Bitbucket. No new dependency.

**File-size check.** `azure/mod.rs` is **~513 lines** today (already over the ~500 soft limit) and
this increment adds tests. **Required:** move the `#[cfg(test)] mod tests` block into
`crates/bonsai-forge/src/azure/mod_tests.rs` and attach it exactly as `bonsai-core/src/external.rs`
does, in the same commit:

```rust
#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
```

That leaves `azure/mod.rs` ≈190 lines of logic. `azure/rest.rs` is 365 lines → ≈410 after this
change; no split needed. `azure/dto.rs` gains ~25 lines; check it against the limit and, if it
crosses, move *its* test module out the same way rather than growing the file.

### A2. `azure/rest.rs` — new URL builder

Add next to the existing builders. `pub`, carries `api-version=7.1`, never carries a token.

```rust
/// The repository object itself — the SCOPE-VALIDATION probe for `viewer()`.
/// Reaching it requires exactly the Code scope every other Azure call already
/// needs (`vso.code`, inherited by `vso.code_write`), so a Code-only PAT
/// validates; a 404 additionally means the org/project/repo triple is wrong.
pub fn repository_url(org: &str, project: &str, repo: &str) -> String;
```

Exact strings the senior-dev's tests must assert as literals:

```text
repository_url("org", "proj", "repo")
  == "https://dev.azure.com/org/proj/_apis/git/repositories/repo?api-version=7.1"

profile_url()  // unchanged
  == "https://app.vssps.visualstudio.com/_apis/profile/profiles/me?api-version=7.1"
```

`repository_url` must be built from the existing private `repo_base()`
(`= format!("{repo_base}?{API_VERSION}")`) so it cannot drift from the other repo endpoints.

### A3. `azure/rest.rs` — `map_status` changes

Signature unchanged (`pub fn map_status(resp: &HttpResponse) -> Option<AppError>`). Ordering matters:
the **203 arm must be checked before the `(200..300)` success early-return**.

```rust
// New, FIRST — before the 2xx success check.
if s == 203 { return Some(AppError::AuthFailed(<203 message>)); }
```

Exact message strings (assert these substrings, not the whole string, in tests):

| Status | `AppError` | Message |
| --- | --- | --- |
| 203 | `AuthFailed` | `"Azure DevOps did not accept the personal access token (HTTP 203 sign-in page) — it is invalid or expired; create a new PAT with Code (Read & Write)"` |
| 401 | `AuthFailed` | `"Azure DevOps rejected the personal access token (401) — it is invalid or expired, or it lacks Code (Read & Write) for this repository"` |
| 403 | `AuthFailed` | unchanged |
| 404 | `ForgeApi` | unchanged here (`"not found"`); the coords-naming message is added by `viewer()` — see A4 |
| 429 / 3xx / other | unchanged | unchanged |

Rationale for 203: Azure answers `203 Non-Authoritative Information` plus an HTML sign-in page for
an invalid/expired PAT. Today that lands in the 2xx branch, so the HTML reaches `dto::from_json` and
surfaces as `ForgeApi("malformed Azure DevOps response: …")` — the known gap at
`P64-user-checklist.md:69-74`. No message may contain the token, and `map_status` never reads
`resp.body`, so nothing can echo the HTML page.

### A4. `azure/mod.rs` — `viewer()` behaviour

Signature and trait impl unchanged:

```rust
fn viewer(&self) -> Result<ForgeViewer, AppError>;
```

Ordered behaviour:

```text
1. let (org, project, repo) = self.coords()?;          // ForgeUnsupported (see A5)
2. let token = self.require_token()?;                  // ForgeAuthRequired, no request yet
3. VALIDATE (must succeed):
     resp = rest::get(http, &rest::repository_url(org, project, repo), Some(token))?
       - 401/203 -> AuthFailed straight from map_status (A3), nothing cached, nothing stored
       - 404     -> REPLACE map_status's ForgeApi("not found") with the coords-naming message
                    below (matching on AppError::ForgeApi whose message == "not found" is
                    fragile; instead map the error with `.map_err(coords_hint)` where
                    `coords_hint` rewrites ONLY `ForgeApi` and leaves every other variant
                    untouched)
     dto::parse_repo_probe(&resp.body)?                // 200-with-HTML cannot read as success
4. IDENTIFY (best effort — MUST NEVER fail the connect); exactly ONE attempt:
     match rest::get(http, &rest::profile_url(), Some(token))
              .and_then(|r| dto::parse_viewer(&r.body)) {
       Ok(v)  => v,
       Err(_) => ForgeViewer { login: String::new(), avatar_url: None },
     }
5. if !self.target.host.is_empty() && !viewer.login.is_empty() {
       auth::cache_viewer(&self.target.host, viewer.clone());
   }
6. Ok(viewer)
```

Rules the implementation must honour:

- **Step 4 swallows errors, step 3 does not.** No `?` anywhere inside step 4. A rate-limit or
  network error during identify is *also* swallowed — it must not turn a successful validation into
  a failed connect. (Accepted trade-off: a 429 mid-identify yields an empty login instead of a
  rate-limit toast. That is strictly better than blocking a valid PAT, and nothing renders the login
  today.)
- **Never cache an empty-login viewer** (step 5 guard). `auth::cached_viewer` feeding
  `repo_context().viewer` must not be able to serve `login: ""` as if it were a resolved identity.
  Kept for when a consumer of `login` does appear.
- **No token, no `Authorization` value, and no response body** may appear in any error message
  produced here.
- `avatar_url` stays `None` for Azure in every path (unchanged from P64).

The 404 rewrite message (exact):

```text
"Azure DevOps could not find the repository {org}/{project}/{repo} — check the organization,
 project, and repository names, or whether the PAT's organization matches"
```

(one line in code; interpolate the three coords, nothing else — never the token, never the URL).

### A5. Error-taxonomy delta (explicit — this is the only behaviour change outside the happy path)

`viewer()` previously called `require_supported()` and deliberately did **not** need coords
(`azure/mod.rs:117-118`), because the profile endpoint is org-agnostic. Validating against the repo
endpoint means coords are now mandatory. Consequences:

| Target | Before | After |
| --- | --- | --- |
| non-Azure `kind` | `ForgeUnsupported` (via `require_supported`) | `ForgeUnsupported` (via `coords` → `require_supported`) — **unchanged** |
| Azure `kind`, `project == None` | proceeded, hit the profile endpoint | `ForgeUnsupported("Azure DevOps requires an org/project/repo remote")` — **new** |
| Azure, `project` present, no token | `ForgeAuthRequired` | `ForgeAuthRequired` — **unchanged** (no request issued) |

Existing tests: `viewer_requires_token` (`azure/mod.rs:288-291`) still passes unchanged
(`azure_target()` carries `project: Some("proj")`, and the error precedes any request; its unused
route needle should be updated to the repo URL for honesty).
`unsupported_host_rejects_data_calls_but_gives_context` (`:489-512`) does not call `viewer()` — leave
it, and optionally add `assert!(matches!(p.viewer(), Err(AppError::ForgeUnsupported(_))))` to pin the
new row above. Two tests MUST be rewritten:
`viewer_maps_display_name_and_basic_auth_on_vssps_host` (`:255-285`) and the `viewer()` half of
`error_status_maps_to_app_error` (`:484-485`) — see §3.1.

`validate_token` / `set_token` (`lib.rs:124-154`) are untouched in code: they already store nothing
unless `viewer()` returns `Ok`. Only their doc comments change (A7).

### A6. `azure/dto.rs` — new parser

```rust
/// `GET .../_apis/git/repositories/{repo}` ⇒ `Ok(())` iff the payload really is
/// the repository object (non-empty `id`). Guards the case where Azure answers
/// 200 with an HTML/redirect body: JSON parse failure OR a missing/empty `id`
/// ⇒ `AppError::ForgeApi`, never a silent success.
pub fn parse_repo_probe(body: &str) -> Result<(), AppError>;
```

Wire struct (private, `#[serde(rename_all = "camelCase")]`, all fields `#[serde(default)]`, mirroring
the existing `AzProfile` at `dto.rs:297-304`):

```rust
struct AzRepoProbe { id: Option<String>, name: Option<String> }
```

It goes through the existing `from_json` helper (`dto.rs:30-33`) so a malformed body keeps the
current `ForgeApi("malformed Azure DevOps response: …")` shape. Message for the empty case:
`ForgeApi("Azure DevOps did not return a repository object")`.

`parse_viewer` (`dto.rs:322-328`) is unchanged and remains the profile-endpoint parser — it is now
the only identity parser.

### A7. Documentation drift (all stale "Bearer only" claims — required, they are wrong today)

| Location | Current (wrong) | Required correction |
| --- | --- | --- |
| `crates/bonsai-forge/src/lib.rs:11-12` | "reaches the wire ONLY as an `Authorization: Bearer` header" | state the per-provider truth: GitHub/Bitbucket `Authorization: Bearer`, Azure DevOps `Authorization: Basic base64(":" + PAT)`, GitLab `PRIVATE-TOKEN`. Keep the invariants that hold: keychain-only storage, never a URL, never logged. |
| `crates/bonsai-forge/src/http.rs:49` | "Headers carry the `Authorization: Bearer <token>` value ONLY when authenticated" | "Headers carry the provider's auth header (`Authorization: Bearer`, `Authorization: Basic`, or `PRIVATE-TOKEN`) ONLY when authenticated; the redaction seam elides its value." |
| `docs/contracts/phase4-forge-overview.md:67-68` | same Bearer-only claim in F3 | same per-provider correction |
| `crates/bonsai-forge/src/lib.rs:129` and `:139` | "`viewer()` performs the single identity (`GET /user`) validation call" | provider-specific: GitHub `GET /user`, GitLab `GET /user`, Bitbucket `GET /user`, **Azure DevOps: validate on the repository endpoint, then ONE best-effort profile call** (name P72). |
| `docs/contracts/P64-forge-providers-ai-pr.md:147-158` | Azure `viewer` row = profile endpoint | rewrite the row to validate-then-identify per §A4, and note that a Code-only PAT is sufficient — resolving the §0 contract contradiction with `P64-user-checklist.md:64-67`. |
| `docs/contracts/P64-user-checklist.md:69-74` | 203 gap listed as open | mark **closed by P72** (§A3) with the new message. Leave `:59-63` (the `_git` shorthand) OPEN. |

No doc may claim the PAT is redacted "because base64" — base64 is not secrecy; the redaction seam in
`http.rs` is what keeps it off logs. Keep the existing wording where it already says that.

---

## 2. Increment B — `openUrl` IPC

Design constraint (from `docs/contracts/P49-external-integrations.md:21-22`, D1): **hand-rolled
per-OS spawn. No new Tauri plugin. No new capability grant.** `src-tauri/capabilities/default.json`
is NOT edited in this increment.

### B1. Files touched

| File | Change |
| --- | --- |
| `crates/bonsai-core/src/external.rs` | +`validate_web_url`, +`url_ladder`, +`open_url` |
| `crates/bonsai-core/src/external_tests.rs` | + the §3.2 cases |
| `src-tauri/src/commands/external.rs` | +`open_url` command (separate from `launch_inner`) |
| `src-tauri/src/lib.rs` | register `commands::open_url` in the `invoke_handler` list (after `commands::open_in_editor`, ~`:252`) |
| `src/ipc/types.ts` | +`openUrl` on `IpcApi` (beside the P49 triple, `:2450-2458`) |
| `src/ipc/tauri.ts` | +`openUrl` (beside `openInEditor`, `:856-858`) |
| `src/ipc/mock/handlers/external.ts` | +`openUrl` |
| `src/components/ForgeConnect.tsx` | `onOpenTokenPage` prop + `onClick`; add `noopener` to `rel` |
| `src/components/PrDetailView.tsx` | `onOpenUrl` prop + `onClick` |
| `src/components/PrPanel.tsx` | supply both callbacks (it already owns `ipc` + `usePushToast`) |
| `src/components/__tests__/…` | ForgeConnect vitest (§3.3) |
| `e2e/11-forge.spec.ts` | extend (§3.4) |

`crates/bonsai-core/src/external.rs` is 353 lines; +≈70 keeps it under the limit. Match its
doc-comment discipline (every public item documents *why*, and every safety decision is named).

### B2. `bonsai-core::external` — new public API

```rust
/// Accept ONLY a plain web URL, so a launcher can never be handed a protocol
/// the OS would resolve to something else. Pure: no fs, no spawn.
///
/// Accepts: a `http://` or `https://` scheme, matched CASE-INSENSITIVELY, with a
/// non-empty host component.
/// Rejects: every other scheme (`file:`, `javascript:`, `data:`, `ms-msdt:`,
/// `vscode:`), a UNC/`\\server\share` path, a scheme with no host
/// (`https://`, `http:///x`), an empty/whitespace-only string, and any input
/// whose first character is `-` (so the URL can never be parsed as a FLAG by the
/// launcher program).
///
/// Load-bearing, not decorative: `PrDetailView`'s URL comes from a forge API
/// response, i.e. from outside the app.
pub fn validate_web_url(url: &str) -> Result<(), AppError>;

/// Ordered browser-launch candidates for `url`. Pure — takes an explicit
/// [`TargetOs`], never `cfg!`, so every branch runs in unit tests on one host.
/// The caller MUST have validated `url` first.
pub fn url_ladder(os: TargetOs, url: &str) -> Vec<LaunchSpec>;

/// Validate `url`, then launch the first candidate that works.
pub fn open_url(runner: &dyn CommandRunner, os: TargetOs, url: &str) -> Result<(), AppError>;
```

**`validate_web_url` semantics.**

1. Operate on the raw string; do NOT trim and then accept (a URL is not allowed to carry
   leading/trailing whitespace).
2. Reject if `url.is_empty()` or `url.starts_with('-')`.
3. Reject unless the lowercased string starts with `"http://"` or `"https://"` (this alone rejects
   `file:`, `javascript:`, `data:`, UNC `\\…`, and a bare `example.com`).
4. Host = the substring after the scheme up to the first `/`, `?`, or `#`. Reject if empty, if it
   contains a space, or if it contains a `\`.
5. Otherwise `Ok(())`.

No URL crate is added; this is a deliberate allow-list on a string, matching the crate's
hand-rolled-over-dependency house style (base64, percent-encoding).

**Error variant + message rule.** Rejections return `AppError::ExternalToolFailed`, chosen so the
frontend's existing `externalToolFailed` → error-toast path and the mock's error shape need no new
`ErrorKind` on the TS side. **The message must never echo the untrusted URL.** Emit a bounded,
category-only message:

| Rejection | Message |
| --- | --- |
| non-http(s) scheme | `"refused to open a link that is not http or https"` |
| empty / leading `-` | `"refused to open a malformed link"` |
| empty or invalid host | `"refused to open a link with no host"` |

Rationale to record in the doc comment: a forge-supplied URL can be arbitrarily long and can contain
markup or lookalike text; echoing it into a toast turns a rejected link into a UI-spoofing surface.
The *rejected* URL is therefore never rendered. (A launch **failure** from `launch_first` keeps its
existing wording and names only the program, never the URL.)

**`url_ladder` table.** `cwd: PathBuf::from(".")` for every entry — no repo path is involved, and
this keeps `LaunchSpec` and its existing equality tests untouched. (Confirmed choice: an alternative
would be to make `cwd` an `Option<PathBuf>`, which would churn every P49 ladder test for no
behavioural gain. `"."` is always a valid working directory for the app process.)

| OS | # | program | args | `hide_console` | `wait_for_exit` |
| --- | --- | --- | --- | --- | --- |
| Windows | 1 | `explorer` | `[url]` | `true` | `false` |
| Windows | 2 | `rundll32` | `["url.dll,FileProtocolHandler", url]` | `true` | `false` |
| macOS | 1 | `open` | `[url]` | `false` | `true` (via `open_spec`) |
| Linux | 1 | `xdg-open` | `[url]` | `true` | `false` |

- Windows `wait_for_exit: false` is mandatory: `explorer` habitually exits non-zero *after* a
  successful hand-off — identical reasoning to `reveal_spec` (`external.rs:265-268`). Waiting would
  report a bogus failure and pointlessly advance the ladder.
- macOS uses `open_spec` and therefore `wait_for_exit: true`, per the documented `open` rule at
  `external.rs:78-95`: `open` always spawns fine and reports "Unable to find application" only via
  its exit code.
- **`cmd /c start` is explicitly rejected.** `start` is a `cmd.exe` builtin, so using it means
  handing a string to a shell — the exact thing P49 D2 forbids. `cmd` would then apply its own
  parsing to `&`, `^`, `%VAR%` and its `start` builtin treats the first quoted token as a window
  *title*. `explorer` and `rundll32` take the URL as a single argv token with no shell in the
  pipeline.

`open_url` is a two-liner: `validate_web_url(url)?;` then
`launch_first(runner, &url_ladder(os, url), "browser")`. The `what` label is exactly `"browser"`, so
a total failure reads `"could not launch browser (rundll32): …"`.

### B3. Tauri command

In `src-tauri/src/commands/external.rs` — a **separate** command, deliberately not folded into
`launch_inner`, because it needs no `AppHandle`, reads no settings template, and must **skip the
`path.exists()` precheck** that would reject every URL:

```rust
/// Open `url` in the user's default browser (P72). Web URLs only —
/// `bonsai_core::external::validate_web_url` rejects anything else BEFORE a
/// process is spawned. No `AppHandle`, no settings, and deliberately NO
/// `path.exists()` precheck (that is what makes `launch_inner` unusable here).
/// Rejects `externalToolFailed` (invalid URL, or no launcher succeeded).
#[tauri::command]
pub async fn open_url(url: String) -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        external::open_url(&SpawnRunner, TargetOs::host(), &url)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```

`spawn_blocking` is required: `Command::spawn`/`status` is blocking, and the macOS rung waits on
`open`'s exit. Register as `commands::open_url` in the `invoke_handler!` list in
`src-tauri/src/lib.rs` (the list at `:228-252`; place it right after `commands::open_in_editor`).
**No `capabilities/default.json` change** — this is a Bonsai command, not a plugin permission.
Like `reveal_in_file_manager`, `open_url` takes no `AppHandle`, so it is directly testable with
`tauri::async_runtime::block_on` (see `src-tauri/src/commands/tests_ai_consent_gate.rs:32`) without
the tauri `test` feature.

### B4. IPC surface

`src/ipc/types.ts`, on `IpcApi`, beside the P49 triple:

```ts
/** P72: open `url` in the user's default browser. Web URLs only — a non-http(s)
 *  scheme, a hostless URL, or a leading `-` is refused before anything spawns.
 *  Rejects AppError('externalToolFailed'). */
openUrl(url: string): Promise<void>;
```

`src/ipc/tauri.ts`:

```ts
openUrl(url: string): Promise<void> {
  return invoke<void>('open_url', { url });
},
```

`src/ipc/mock/handlers/external.ts` — reuse the existing `simulate`/`FAIL_SENTINEL` machinery so a
URL containing `#fail` still drives the error-toast path, and additionally keep the harness's
currently-useful behaviour of actually opening a tab on success:

```ts
async openUrl(url: string): Promise<void> {
  await simulate(url, 'browser');           // #fail ⇒ externalToolFailed AppError
  window.open(url, '_blank', 'noopener,noreferrer');
},
```

Order matters: `simulate` first, so the failure path never opens a tab. The mock does **not**
replicate `validate_web_url` (that is Rust's job and the harness has no launcher); if a future
harness case needs it, extend `FAIL_SENTINEL`-style triggers rather than duplicating the rule.
This keeps the whole surface implementable under `VITE_MOCK_IPC=1`.

### B5. Frontend wiring

Keep the `<a href>` element at both sites — it preserves link semantics, the focus ring, keyboard
activation, and the existing styling; only the *navigation* is intercepted.

`src/components/ForgeConnect.tsx`:

```ts
export interface ForgeConnectProps {
  // …existing…
  /** P72: route "Create a token" through the openUrl IPC — a bare
   *  target="_blank" is a silent no-op in the native webview. */
  onOpenTokenPage(url: string): void;
}
```

The anchor becomes `rel="noreferrer noopener"` (adds the missing `noopener`) plus
`onClick={(e) => { e.preventDefault(); onOpenTokenPage(hint.url); }}`. `ForgeConnect` stays
presentational: it takes no `ipc` and performs no error handling.

`src/components/PrDetailView.tsx`:

```ts
export interface PrDetailViewProps {
  // …existing…
  /** P72: route "Open in browser ↗" through the openUrl IPC. The URL comes from
   *  the forge API response, which is why the Rust side validates it. */
  onOpenUrl(url: string): void;
}
```

Anchor keeps `href={summary.url}` and `rel="noreferrer noopener"`, gains
`onClick={(e) => { e.preventDefault(); onOpenUrl(summary.url); }}`.

`src/components/PrPanel.tsx` (already imports `ipc` and calls `usePushToast()` at `:52`) defines one
callback and passes it to both children — `ForgeConnect` at `:263-272`, `PrDetailView` at `:320`:

```ts
const openExternalUrl = useCallback(
  (url: string) => {
    void ipc.openUrl(url).catch((e) => pushToast('error', errorMessage(e)));
  },
  [pushToast],
);
```

This is the established pattern from `src/App.tsx:370-386` and
`src/components/RepoWorkspace.tsx:2355-2369` — reuse it verbatim rather than inventing new error
handling. Both props are **required** (not optional) so a future call site cannot silently
regress to a dead link.

`P72-ui.md` owns all copy and the focus/disabled/error/pending visuals for these links, plus the
Azure scope hint. It specifies no identity/"Connected" rendering — there is none in P72.

---

## 3. Test matrix

### 3.1 Azure (Rust, `crates/bonsai-forge/src/azure/mod_tests.rs` unless noted)

`FakeTransport` already routes by URL-substring (`azure/mod.rs:213-228`), so per-URL routing needs no
new harness — pass multiple routes. Useful needles: `"/_apis/git/repositories/repo?"` (repo probe)
and `"/profile/profiles/me"`. **Note:** the repo-probe needle must include the `?` (or assert on the
full URL) so it cannot also match `/pullrequests` URLs, which share `repo_base`.

| # | Routes | Expected |
| --- | --- | --- |
| a | repo 200 (`{"id":"r1","name":"repo"}`) + profile 200 (`{"displayName":"Ada Lovelace"}`) | `Ok`, `login == "Ada Lovelace"`, `avatar_url == None`; spy: exactly 2 requests, request 1 URL == the `repository_url(...)` literal, request 2 == the `profile_url()` literal |
| b | repo 200 + profile 200 with `{}` (no display name, no email) | `Ok`, `login == ""` (`parse_viewer`'s existing `unwrap_or_default`); nothing cached |
| c | **repo 200 + profile 401** | **`Ok`**, `login == ""`, `avatar_url == None` — THE regression test for the user's bug (a Code-only PAT). Also assert `auth::cached_viewer(host)` was NOT populated with an empty login |
| d | repo 401 | `Err(AuthFailed)`, message contains `"Code (Read & Write)"`; spy shows exactly **1** request (identify is never attempted); nothing cached |
| e | repo 404 | `Err(ForgeApi)` whose message contains `"org"`, `"proj"`, and `"repo"`; NOT the bare `"not found"` |
| f | repo 203 (body = an HTML sign-in page) | `Err(AuthFailed)`; message must NOT contain `"malformed"`; assert the HTML body text does not appear in the message |
| g | repo 200 with an HTML body (not JSON) | `Err(ForgeApi)` from `parse_repo_probe` — a 200 + HTML cannot read as success |
| h | repo 200 + profile 429 / transport error | `Ok` with empty login (identify swallows everything, §A4) |
| i | no token, any routes | `Err(ForgeAuthRequired)`, **zero** requests recorded |
| j | Azure target with `project: None` | `Err(ForgeUnsupported)`, zero requests (the §A5 new row) |
| k | non-Azure `kind` | `Err(ForgeUnsupported)`, zero requests |
| l | Basic-auth pin (any successful case) | request 1 header `Authorization` starts with `"Basic "`, does not contain the plaintext PAT, and is not `Bearer`/`PRIVATE-TOKEN` |

In `crates/bonsai-forge/src/azure/rest.rs` tests:

| # | Assertion |
| --- | --- |
| m | `repository_url("org","proj","repo")` equals the literal string in §A2 and contains `api-version=7.1`; `profile_url()` unchanged |
| n | **Literal header bytes:** `base_headers(Some("pat"))` contains exactly `("Authorization".to_string(), "Basic OnBhdA==".to_string())`. The current test at `rest.rs:245-364` computes `expected` with the same `base64_encode` that is under test — self-referential; a bug in `base64_encode` would pass. Keep the RFC vectors test and **add** this hard-coded pin. |
| o | `map_status(&resp(203, …))` ⇒ `Some(AuthFailed(_))` (must NOT be `None`); 401 message mentions both invalid/expired **and** the Code scope; 200/201 still `None` |

Also update: `viewer_maps_display_name_and_basic_auth_on_vssps_host` → becomes case (a);
`error_status_maps_to_app_error`'s `viewer()` half → becomes case (d). `crates/bonsai-forge/src/lib.rs`'s
`validate_token_*` tests: if any drives an Azure target, add the repo route.

### 3.2 `openUrl` core (Rust, `crates/bonsai-core/src/external_tests.rs`)

`validate_web_url` accept/reject table:

| Input | Result |
| --- | --- |
| `https://github.com/settings/tokens` | accept |
| `http://localhost:3000/x?y=1#z` | accept |
| `HTTPS://EXAMPLE.COM/a` | accept (scheme match is case-insensitive) |
| `https://dev.azure.com/org/proj/_git/repo/pullrequest/7` | accept |
| `javascript:alert(1)` | reject |
| `file:///C:/Windows/System32/calc.exe` | reject |
| `data:text/html,<h1>x` | reject |
| `ms-msdt:/id` | reject |
| `\\server\share\x` | reject |
| `example.com` | reject (no scheme) |
| `https://` | reject (no host) |
| `http:///path` | reject (no host) |
| `-https://x.com` | reject (leading `-`) |
| `--url=https://x.com` | reject (leading `-`) |
| `""` / `"   "` | reject |
| `https://ex ample.com/x` | reject (space in host) |
| `https://ex\ample.com` | reject (`\` in host) |

Plus: every rejection message must **not** contain the input URL (assert
`!msg.contains(<the distinctive part of the input>)`), and every rejection is
`AppError::ExternalToolFailed`.

`url_ladder` argv, per OS, asserted as whole `LaunchSpec` values:

| # | Assertion |
| --- | --- |
| p | `url_ladder(Windows, u)` has len 2: `("explorer", [u], hide_console: true, wait_for_exit: false)` then `("rundll32", ["url.dll,FileProtocolHandler", u], true, false)`; both `cwd == PathBuf::from(".")` |
| q | `url_ladder(MacOs, u)` == `[("open", [u], hide_console: false, wait_for_exit: true)]` |
| r | `url_ladder(Linux, u)` == `[("xdg-open", [u], hide_console: true, wait_for_exit: false)]` |
| s | For every OS: the URL occupies exactly ONE argv token, and no spec's `program` or `args` contains `cmd`, `start`, `/c`, or `powershell` |

Ladder behaviour with the existing fake runner:

| # | Assertion |
| --- | --- |
| t | Windows, runner fails rung 1 and succeeds rung 2 ⇒ `Ok`, both attempted in order |
| u | Windows, runner fails both ⇒ `Err(ExternalToolFailed)` naming the LAST program (`rundll32`); message contains no URL |
| v | `open_url` with `javascript:alert(1)` ⇒ `Err`, and the fake runner recorded **zero** attempts (validation precedes every spawn) |

### 3.3 Frontend (vitest)

| # | Test |
| --- | --- |
| w | `ForgeConnect`: clicking "Create a token" calls `onOpenTokenPage` with the provider's hint URL exactly once, and the click's `defaultPrevented` is `true` (no navigation) |
| x | `ForgeConnect`: the anchor's `rel` contains both `noreferrer` and `noopener`; `href` still equals the hint URL (link semantics preserved) |
| y | `ForgeConnect`: `provider: 'unknown'` (empty hint URL) renders no anchor and never calls the callback |
| z | `PrDetailView`: clicking "Open in browser ↗" calls `onOpenUrl(summary.url)` once with `preventDefault` applied |
| aa | `PrPanel`: a rejected `ipc.openUrl` (mock `#fail`) surfaces exactly one error toast and leaves the view unchanged |

### 3.4 e2e (`e2e/11-forge.spec.ts`, extending the block at `:85-96`)

| # | Step |
| --- | --- |
| bb | In the connect state, click "Create a token" and assert the mock `openUrl` was invoked (e.g. stub `window.open` and assert the recorded URL, or assert the `[mock] open browser: …` console line) and that the SPA did not navigate away (the connect form is still mounted) |
| cc | Drive the failure path with a `#fail` URL and assert the error toast appears |

Full-gate commands (sequential — never concurrent cargo runs; set `TMP`/`TEMP` to `D:\Temp` on
Windows): `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `tsc`,
`pnpm vitest run`, `pnpm lint`, `pnpm lint:size`, `pnpm e2e`.

---

## 4. Acceptance criteria (the 7 from `TODO.md` §P72)

| # | Criterion | Verification |
| --- | --- | --- |
| 1 | An Azure PAT with only Code (Read & Write) connects successfully; nothing is stored on failure | **AI**: §3.1 cases c + d (c is the regression test; d asserts one request and no cache). **USER CHECKPOINT** for the real org |
| 2 | An invalid/expired Azure PAT yields a clear auth error, never "malformed response" | **AI**: §3.1 d + f + §3.2 o. **USER CHECKPOINT** with a deliberately bad PAT |
| 3 | Connected-as shows the display name when the PAT allows it, a plain "Connected" otherwise | **AI**: §3.1 a (name resolved) + c (empty login) pin the DATA. **Note:** `ForgeViewer.login` has no render site in the frontend today, so there is nothing to display and no UI work in P72 — this criterion is satisfied at the data layer only. Restate it that way in `TODO.md` |
| 4 | Clicking "Create a token" / "Open in browser ↗" opens the system browser in the native app; a launch failure raises an error toast | **USER CHECKPOINT (blocking)** — no native webview here. AI covers the wiring (§3.3 w–aa, §3.4 bb–cc) and the argv (§3.2 p–u), not the OS hand-off |
| 5 | `validate_web_url` rejects non-http(s) schemes, hostless URLs, and leading `-` | **AI**: §3.2 table (fully verifiable) |
| 6 | Full gate green: clippy `-D warnings`, `cargo test --workspace`, tsc, vitest, lint, e2e | **AI** |
| 7 | Contract/doc drift corrected (`P64-*`, `phase4-forge-overview.md:67-68`, `bonsai-forge/src/lib.rs:11-12`, `http.rs:49`) | **AI**: §A7 checklist, all six rows |

**USER CHECKPOINT script** (native only): connect the existing Code-scoped PAT in `pnpm tauri dev`
→ expect success; try a deliberately bad PAT → expect a clear auth error with no "malformed";
click "Create a token" in the connect state and "Open in browser ↗" on a PR → expect the system
browser.

---

## 5. Invariants re-affirmed

- Rust owns all forge logic and every launch decision; React only renders and routes clicks through
  IPC. No URL-scheme policy, no ladder, and no provider knowledge in TypeScript.
- IPC stays request/response commands; `openUrl` carries one small string and returns `void`. No new
  event and no channel.
- Blocking work (`git2`, HTTP, `Command::spawn`/`status`) runs under `spawn_blocking` at the command
  layer.
- The PAT never reaches a log, a `{:?}`, a URL, or an error message. `map_status` never reads
  `resp.body`; the 404/203 messages interpolate only coords and the status.
- A rejected URL is never echoed back into a user-facing message (§B2).
- Every new IPC surface is implemented in `src/ipc/mock/handlers/external.ts`, so `VITE_MOCK_IPC=1`
  keeps working in a plain browser. No change to `src/ipc/mock/handlers/forge.ts`.
- Connect costs at most 2 HTTP round-trips (validate + one best-effort identify), down from the
  3 an earlier draft would have spent.
- File-size discipline: `azure/mod.rs` tests move to `azure/mod_tests.rs` in Increment A (§A1).

## 6. Flagged for the orchestrator

1. **Identify-step errors are swallowed wholesale** (§A4). A 429 or network blip during the
   best-effort profile call yields an empty login rather than a rate-limit toast.
   Recommendation: accept — the alternative re-introduces the exact class of bug being fixed
   (a non-Code-scope condition failing a valid PAT), and nothing renders `login` today.
2. **`viewer()` now requires org/project/repo** (§A5). An Azure target with `project: None` changes
   from a profile-endpoint attempt to `ForgeUnsupported`. Recommendation: accept — detection always
   supplies `project` for a recognized Azure remote, and the out-of-scope `_git` shorthand bug
   already surfaces as *unsupported*, so this is consistent rather than a new user-visible path.
3. **Error variant for a rejected URL** = `AppError::ExternalToolFailed` (§B2), reusing the existing
   `externalToolFailed` TS `ErrorKind` so no frontend error taxonomy changes. Alternative: a new
   `InvalidInput` variant, which would touch the TS error union and every exhaustive match.
   Recommendation: as specified.
4. **`LaunchSpec.cwd = "."` for URL launches** (§B2). Alternative: make `cwd` optional, which churns
   every P49 ladder equality test for no behavioural gain. Recommendation: as specified.
5. **Windows ladder has no third rung.** If both `explorer` and `rundll32` fail on a locked-down
   machine, the user gets an error toast and no browser. A `cmd /c start` rung is deliberately NOT
   added (shell). If field reports show this, the next candidate to evaluate is `powershell
   -NoProfile Start-Process <url>` — still not a shell command *string*, but it does add a
   PowerShell dependency, so it is out of scope here.
6. **The mock does not enforce `validate_web_url`** (§B4), so the browser harness will "open" a URL
   the native app would refuse. Acceptance criterion 5 is proven by the Rust table instead.
   Recommendation: accept; duplicating the rule in TS would create a second source of truth.
7. **Acceptance criterion 3 is unachievable as literally worded** — there is no render site for
   `ForgeViewer.login` (verified: only `src/ipc/fixtures/forge.ts:41` and the `types.ts` declaration
   reference it outside tests; `PrPanel.tsx:187` discards the viewer). The contract satisfies it at
   the data layer and adds no UI. Recommendation: reword the criterion in `TODO.md` to
   "`viewer()` returns the display name when the PAT allows it, an empty login otherwise" and open a
   separate item if a connected-as indicator is ever wanted.
