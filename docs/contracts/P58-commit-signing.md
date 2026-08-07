# P58 — Real commit signing + verification

Sign commits at creation time (SSH-first per Git ≥2.34 "everyone has an SSH key, no one has GPG",
**plus** GPG/OpenPGP), and verify signatures to **light** the verified/signed badge P51 stubbed —
on graph rows AND in the commit-details panel, plus a signature-status line in that panel. Signing
follows `commit.gpgsign` (with a per-commit override), using `user.signingkey` + `gpg.format`.

References read (current state, verified — not guessed):
`crates/bonsai-core/src/git/commit.rs` (`resolve_signature` L40, `create_commit` L75 →
`repo.commit(Some("HEAD"),…)` L133, `amend_commit` L154 → `head_commit.amend(…)` L205; CRLF-normalize
+ trim + guards),
`crates/bonsai-core/src/git/config.rs` (`apply_identity_profile` already writes `user.signingkey`;
`read_config`/`set_config` generic section.key path),
`crates/bonsai-core/src/git/search.rs` (`GitRunner`+`SpawnGitRunner` shell-out seam; never-prompt env,
`CREATE_NO_WINDOW`),
`crates/bonsai-core/src/git/maintenance.rs` (best-effort git shell-out precedent),
`crates/bonsai-core/src/git/remote.rs` (`credential_fill` L165: the capture-stdout, never-prompt,
`CREATE_NO_WINDOW` idiom),
`crates/bonsai-core/src/graph.rs` (`GraphNode` L51: `id`/`summary`/`author`/`ts`/`committer_ts`; NO
verification field — verification is separate/dynamic),
`src/graph/drawRowText.ts` (`drawBadgeStub` L69-83 — the faint unlit hollow glyph P58 lights; called at
L51 when `cols.sha` exists), `src/graph/rightColumns.ts` (badge slot geometry), `src/graph/metrics.ts`
(`badgeSlotWidth`/`badgeGap`), `src/graph/GraphCanvas.tsx` (`GraphCanvasProps` L44, `matchRows` threaded
via `propsRef` L237 into the `Interaction` L296 — the pattern P58 mirrors for `verifyStatus`),
`src/components/CommitPanel.tsx` (`CommitPanelProps` L55, renders `details.authorName/authorEmail/
authorTs` L123-130 — where the signature line goes), `src/components/CommitBox.tsx` (commit action —
where the sign toggle goes), `src-tauri/src/commands/staging.rs` (`commit` L51 / `commit_amend` L138 →
`create_commit`/`amend_commit`), `src-tauri/src/commands/merge.rs` (`commit_merge` L53 — another commit
site), `src-tauri/src/lib.rs` (`generate_handler!`; `search_commits` last, L134), `src/ipc/types.ts`
(`commit` L1291; `AppError` union). House pattern: P37-force-push-with-lease, P50-search-command-palette,
P51-graph-polish, P52 (`maintenance.rs`).

**Command count: +2** (`verify_commits`, `signing_status`) → confirm current tail in `generate_handler!`
and increment. `commit` / `commit_amend` gain a `sign` param (not new commands). **No new `AppError`
variant.** Open questions in §12.

---

## 0. Key decisions (with rationale)

**D1 — Signing mechanism: git2 assembles tree/identity/guards; the git binary produces the SIGNED
commit object via `git commit-tree -S`; the ref moves via `git update-ref`. RECOMMENDED.**
git2 0.21 has no "sign this commit" call — you must (approach **A**) build the content with
`Repository::commit_create_buffer`, sign the bytes YOURSELF (shell `ssh-keygen -Y sign -n git` for
`gpg.format=ssh`, `gpg --detach-sign -a` for openpgp), then embed via `Repository::commit_signed(content,
sig, Some("gpgsig"))` — which writes the object but does **not** move any ref. Approach **A** forces us to
reimplement git's `sign_buffer`: format dispatch, `gpg.ssh.program`/`gpg.program` overrides,
`ssh-keygen`'s file-based I/O + literal-`ssh-ed25519…` key handling, agent, armored-PEM wrapping — two
fiddly code paths, easy to get subtly wrong. Approach **B** (`git commit -S` for the whole thing) throws
away the M3 git2 path (index `write_tree`, CRLF-normalize, `NothingToCommit`/`EmptyMessage`, merge/bisect
guards, unborn handling) and re-maps every guard to git's stderr. **Approach C** keeps the M3 git2 path
intact and lets git do the ONE thing it does better than us — sign in BOTH formats, respecting all
config — as pure plumbing:
- git2: run all existing guards, `index.write_tree()` → `tree`, resolve `author`/`committer` via
  `resolve_signature` (unchanged), collect parent oids.
- `git commit-tree -S <tree> [-p <parent>…]`, message on **stdin**, identity via `GIT_AUTHOR_*` /
  `GIT_COMMITTER_*` env → git signs with `user.signingkey`+`gpg.format` and prints the new commit oid.
- `git update-ref -m "<reflog>" HEAD <newoid> [<oldoid>]` — git-exact HEAD/branch move + reflog for
  free (follows the symref; creates the branch on an unborn HEAD; the `<oldoid>` CAS prevents a race).

So C: **least new code, both formats free, exact `git commit -S` parity, unsigned path 100% unchanged.**
Consistent with the project already shelling git for credentials / search-pickaxe / commit-graph. (A is
the fallback if we ever want the object assembled fully in-process — §12 OQ1.)

**D2 — Verification: shell out to `git` for the authoritative verdict; git2 only detects presence.
RECOMMENDED.** Neither git2 nor libgit2 cryptographically verifies (no trust store, no gpg/ssh
invocation) — `Repository::extract_signature` only returns the embedded signature+payload (proves a
signature EXISTS, not that it's good). git verifies BOTH formats with one code path (gpg keyring for
openpgp, `gpg.ssh.allowedSignersFile` for ssh) and exposes the verdict via `--format=%G?`. So verify with
ONE subprocess for N oids:
`git log --no-walk=unsorted --format=%H%x1f%G?%x1f%GS%x1f%GK <oid1> <oid2> …` → one line per commit,
`%G?` forcing a per-commit signature check. Batched, bounded, cache-friendly.

**D3 — Sign-decision follows `commit.gpgsign` with a per-commit `sign: Option<bool>` override.
RECOMMENDED.** `None` ⇒ follow effective `commit.gpgsign` (git default false); `Some(true)` ⇒ force sign
(≡ `-S`); `Some(false)` ⇒ force unsigned (≡ `--no-gpg-sign`). Least-surprising (matches git) and lets the
commit box offer an explicit toggle. v1 UI defaults the toggle to the effective `commit.gpgsign` read via
`signing_status` (D6).

**D4 — Badge is lit from a per-oid verify map threaded like `matchRows`; requested only for VISIBLE rows
(virtualized), cached by oid. RECOMMENDED.** A commit oid is immutable, so its verdict is cache-stable
(invalidated only by a manual refresh — keyring/allowedSigners edits are rare). `GraphCanvas` gains
`verifyStatus?: ReadonlyMap<string, VerifyStatus>`; `drawRowText`/`drawBadgeStub` look up `node.id`:
absent ⇒ the P51 faint stub (not yet checked) is UNCHANGED; present ⇒ a lit glyph per status; `unsigned`
⇒ blank. New graph pref `showSignatureBadge` (default true) gates BOTH the request and the lit draw; when
off the P51 faint stub renders exactly as today (clutter principle — individually toggleable). Badge slot
GEOMETRY is P51's, unchanged (no layout churn — pure draw swap, as P51 §6.5 promised).

**D5 — No new `AppError` variant.** A signer failure (bad passphrase, gpg/ssh-keygen absent, agent
locked) → git's stderr → `AppError::Git`. Signing requested but `user.signingkey` unset for
`gpg.format=ssh` → `AppError::ConfigMissing` (naming the key, mirroring `resolve_signature`); openpgp with
no key falls back to git's committer-email key selection (git decides) — §12 OQ2. Verification never hard-
fails: an unresolvable status is `CannotCheck`, not an error.

**D6 — A small `signing_status` read command drives the commit-box indicator/toggle honestly.
RECOMMENDED.** The UI must show *whether* the next commit will be signed (and warn if the key is missing)
before the user commits. Deriving effective `commit.gpgsign`/`gpg.format`/`user.signingkey` from
`read_config` client-side means merging Global+Local — clunky. One tiny read that returns the backend's
own resolution is the single source of truth. (Alt: fold into an existing status payload — §12 OQ3.)

---

## 1. Module boundaries / files

**New**
- `crates/bonsai-core/src/git/exec.rs` — shared git shell-out seam richer than search's `GitRunner`:
  `GitExec` trait (`exec(args, cwd, stdin, env) -> Result<GitOutput, AppError>`; `GitOutput { success,
  code, stdout, stderr }`) + `SpawnGitExec` (never-prompt env, `CREATE_NO_WINDOW`, captures stdout+stderr
  +status). Used by signing (P58) and hooks (P59) — introduce here in whichever ships first (P58).
- `crates/bonsai-core/src/git/signing.rs` — signing + verification: `SigningConfig`/`SignFormat`,
  `resolve_signing`, `create_signed_commit`, `signing_status`, `verify_commits`, pure
  `build_verify_args`/`parse_verify_output`/`map_status_code`, SSH CLI oracle.
- `src-tauri/src/commands/signing.rs` — `verify_commits` + `signing_status` commands (+ `_inner`).
- `src/components/repoWorkspace/useCommitVerification.ts` — oid→status cache + debounced batch fetch
  keyed on the graph's visible range (mirrors `useCommitSearch`).
- `src/ipc/mock/handlers/signing.ts` — mock `verifyCommits` + `signingStatus`.

**Edited**
- `crates/bonsai-core/src/git/commit.rs` — `create_commit`/`amend_commit` gain `sign: Option<bool>`;
  when signing, branch to `signing::create_signed_commit` instead of `repo.commit`/`Commit::amend`.
- `crates/bonsai-core/src/git/merge.rs` — `commit_merge` gains the same `sign` param (consistency; §12
  OQ4).
- `crates/bonsai-core/src/git/mod.rs` — `pub mod exec; pub mod signing;`.
- `src-tauri/src/commands/staging.rs` — `commit`/`commit_amend` gain `sign: Option<bool>`; thread through
  `_inner` to core.
- `src-tauri/src/commands/merge.rs` — `commit_merge` `sign` param (if included).
- `src-tauri/src/commands/mod.rs` — `mod signing; pub use signing::*;`.
- `src-tauri/src/lib.rs` — register `verify_commits`, `signing_status` (after `search_commits`).
- `src/ipc/types.ts` — `VerifyStatus`/`CommitVerification`/`VerifyResults`/`SignFormat`/`SigningStatus`;
  `GraphPrefs.showSignatureBadge`; `IpcApi.verifyCommits`/`signingStatus`; `commit`/`commitAmend` `sign`.
- `src/ipc/tauri.ts` — `verifyCommits`/`signingStatus` wrappers; `sign` passthrough.
- `src/ipc/mock.ts` — spread `signingHandlers`; `commit`/`commitAmend` accept+ignore `sign`.
- `src/graph/GraphCanvas.tsx` — `verifyStatus?` prop → `propsRef` → `Interaction`; new
  `onVisibleRangeChange(first, last)` callback fired after the paint's visible-window computation.
- `src/graph/draw.ts` — thread `verifyStatus` into the row loop / `drawRowText`.
- `src/graph/drawRowText.ts` — `drawBadgeStub` becomes state-aware: draw a lit glyph for a known
  `VerifyStatus`, else the existing faint stub (gated by `display.showSignatureBadge`).
- `src/graph/colors.ts` — badge state colors (`--badge-good`/`--badge-warn`/`--badge-unknown`; reuse
  existing success/danger/text3 where possible).
- `src/components/CommitPanel.tsx` — signature-status line (icon + status text + signer/key) from the
  verify map for the selected oid.
- `src/components/CommitBox.tsx` — a "Sign commit" toggle + a small "will sign (SSH/GPG)" / "signing key
  not set" indicator driven by `signing_status`.
- `src/components/SettingsGraphSection.tsx` — `showSignatureBadge` checkbox.
- `src/components/WorkspaceGraphPane.tsx`, `src/components/RepoWorkspace.tsx` — wire
  `useCommitVerification`; thread `verifyStatus` + `onVisibleRangeChange`; read `signing_status`.
- `src-tauri/src/settings.rs` + `src/ipc/mock/persistence.ts` — `GraphPrefs.show_signature_badge`
  (default true; clamp/Default/tolerant-parse + back-compat test, mirroring P51 fields).

---

## 2. Wire types

### 2.1 Rust (`crates/bonsai-core/src/git/signing.rs`)

```rust
/// `git log --format=%G?` verdict, one per commit. Authoritative for BOTH
/// ssh and openpgp (git owns the trust check).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VerifyStatus {
    Good,        // %G? G — valid signature from a trusted/known signer
    GoodUnknown, // U — valid signature, signer identity not established (e.g. ssh key not in allowedSigners)
    Bad,         // B — bad signature
    Expired,     // X — good signature that has expired
    ExpiredKey,  // Y — good signature made by an expired key
    Revoked,     // R — good signature made by a revoked key
    CannotCheck, // E — cannot check (missing key, no allowedSignersFile, gpg/ssh absent)
    Unsigned,    // N — no signature
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitVerification {
    pub oid: String,               // full 40-hex (echoes the request; the frontend keys its cache/badge by this)
    pub status: VerifyStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer: Option<String>,    // %GS — signer name/identity; None when unsigned/empty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,       // %GK — key id / fingerprint; None when unsigned/empty
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResults {
    /// One entry per RESOLVABLE requested oid, in request order. Oids git could
    /// not resolve are omitted (frontend keeps them "unchecked").
    pub verifications: Vec<CommitVerification>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignFormat { Ssh, Openpgp }

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SigningStatus {
    pub enabled: bool,               // effective commit.gpgsign
    pub format: Option<SignFormat>,  // gpg.format (ssh|openpgp); None when unset (git default = openpgp)
    pub has_key: bool,               // user.signingkey set + non-empty (after trim)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,         // user.signingkey for display (path or key id); None when unset
}

// Internal (not on the wire): resolved once per commit.
pub struct SigningConfig { pub sign: bool, pub format: SignFormat, pub key: Option<String> }
```

### 2.2 TypeScript (`src/ipc/types.ts`)

```ts
export type VerifyStatus =
  | 'good' | 'goodUnknown' | 'bad' | 'expired' | 'expiredKey' | 'revoked' | 'cannotCheck' | 'unsigned';
export interface CommitVerification { oid: string; status: VerifyStatus; signer?: string; key?: string; }
export interface VerifyResults { verifications: CommitVerification[]; }
export type SignFormat = 'ssh' | 'openpgp';
export interface SigningStatus { enabled: boolean; format: SignFormat | null; hasKey: boolean; key?: string; }
```

`IpcApi` gains (near `searchCommits`):
```ts
/** Verify signatures for a bounded set of commit oids (visible graph rows).
 *  Read-only; does NOT emit repo-changed. One git subprocess per call, capped
 *  at MAX_VERIFY_BATCH. Rejects git | noRepo. Unresolvable oids are omitted. */
verifyCommits(repoId: string, oids: string[]): Promise<VerifyResults>;
/** Effective signing config for the commit-box indicator/toggle. Read-only. Rejects noRepo | git. */
signingStatus(repoId: string): Promise<SigningStatus>;
```
`commit` / `commitAmend` change (additive optional):
```ts
/** sign: null/undefined => follow commit.gpgsign; true => force sign; false => force unsigned. */
commit(repoId: string, message: string, sign?: boolean | null): Promise<CommitResult>;
commitAmend(repoId: string, message: string, sign?: boolean | null): Promise<CommitResult>;
```
`tauri.ts`: `verifyCommits: (repoId, oids) => invoke('verify_commits', { repoId, oids })`;
`signingStatus: (repoId) => invoke('signing_status', { repoId })`;
`commit: (repoId, message, sign = null) => invoke('commit', { repoId, message, sign })` (same for amend).

`GraphPrefs` gains `showSignatureBadge: boolean` (default true) — rides the existing whole-struct
`graph` patch (P51 D1), so `UiSettings`/`UiSettingsPatch` are unchanged.

---

## 3. Backend core — `crates/bonsai-core/src/git/signing.rs`

```rust
pub const MAX_VERIFY_BATCH: usize = 512; // argv sanity; frontend sends only visible rows

/// Resolve whether/how to sign. `override_sign`: None => commit.gpgsign (default
/// false); Some(b) => b. format from gpg.format (default Openpgp); key from
/// user.signingkey (trimmed, non-empty).
pub fn resolve_signing(cfg: &git2::Config, override_sign: Option<bool>) -> SigningConfig;

/// Read-only status for the UI (opens repo at workdir). key = user.signingkey.
pub fn signing_status(workdir: &Path) -> Result<SigningStatus, AppError>;

/// Create a SIGNED commit object via `git commit-tree -S` and move HEAD via
/// `git update-ref` (D1). BLOCKING. Pre-checked by the caller (guards, tree,
/// identity already resolved). `amend`: message via stdin; parents = supplied;
/// GIT_AUTHOR_* preserves the (amend) author + date; GIT_COMMITTER_* from
/// `committer`. Errors: ConfigMissing (ssh + no key), Git (signer failure /
/// stale-old-oid update-ref race / non-utf8 oid).
pub fn create_signed_commit(
    exec: &dyn GitExec,
    workdir: &Path,
    tree: git2::Oid,
    parents: &[git2::Oid],
    author: &git2::Signature<'_>,
    committer: &git2::Signature<'_>,
    message: &str,      // already CRLF-normalized + trimmed + trailing '\n' (matches the git2 path)
    old_head: Option<git2::Oid>, // Some => update-ref CAS old value; None => unborn (create branch)
    reflog_summary: &str,        // "commit: <summary>" / "commit (amend): <summary>"
) -> Result<git2::Oid, AppError>;

/// Verify `oids` in ONE `git log --no-walk` subprocess. BLOCKING. Validates each
/// is 40-hex (drops non-hex). Empty => Ok(empty, no spawn). A wholesale git
/// failure degrades to CannotCheck for every requested oid (never Err), so a
/// missing gpg/ssh toolchain still renders sensibly.
pub fn verify_commits(exec: &dyn GitExec, workdir: &Path, oids: &[String])
    -> Result<VerifyResults, AppError>;

// ---- pure helpers (unit-tested) ----
fn build_verify_args(oids: &[String]) -> Vec<String>;
    // ["log","--no-walk=unsorted","--format=%H%x1f%G?%x1f%GS%x1f%GK", <oid>…]
fn parse_verify_output(stdout: &str) -> Vec<CommitVerification>;
    // per line: splitn(4,'\x1f') -> oid, code, signer, key; map_status_code(code); empty signer/key -> None
fn map_status_code(c: char) -> VerifyStatus;
    // G->Good U->GoodUnknown B->Bad X->Expired Y->ExpiredKey R->Revoked E->CannotCheck N|_->Unsigned
```

### 3.1 `create_signed_commit` — normative
```
content := commit-tree stdin = message (verbatim; commit-tree does NO cleanup)
args := ["commit-tree", tree.hex, "-S"]                 # bare -S: git resolves key+format
for p in parents: args += ["-p", p.hex]
env := [ GIT_AUTHOR_NAME=author.name, GIT_AUTHOR_EMAIL=author.email,
         GIT_COMMITTER_NAME=committer.name, GIT_COMMITTER_EMAIL=committer.email ]
if amend: env += GIT_AUTHOR_DATE = rfc2822(author.when())   # preserve original author date
out := exec.exec(&args, workdir, Some(content.as_bytes()), &env)?
if !out.success:
    if ssh && key.is_none(): Err(ConfigMissing("commit signing requires user.signingkey …"))
    else: Err(Git(stderr_tail(out)))                    # git already says "gpg failed to sign"
new_oid := parse 40-hex from out.stdout.trim()
# move HEAD (git-exact reflog + symref follow + unborn branch creation)
uargs := ["update-ref", "-m", reflog_summary, "HEAD", new_oid] + old_head.map(hex).into_iter()
exec.exec(&uargs, workdir, None, &[])? -> on !success Err(Git(...))   # CAS mismatch => someone moved HEAD
Ok(new_oid)
```
(Alt: git2 `Repository::reference()` for the ref move to avoid the 2nd spawn — §12 OQ5. `update-ref`
is git-exact and CAS-safe; recommended.)

---

## 4. `commit.rs` integration (unsigned path UNCHANGED)

`create_commit(workdir, message, sign: Option<bool>)`: run ALL existing guards + normalize + resolve
`sig` + `index.write_tree()` (as today). Then:
```
let signing = resolve_signing(&cfg, sign);
if !signing.sign {
    // exactly today: repo.commit(Some("HEAD"), &sig, &sig, &full, &tree, &parents)
} else {
    let parents_oids: Vec<Oid> = head.iter().map(|c| c.id()).collect();
    let old = head.as_ref().map(|c| c.id());   // None on unborn
    let oid = signing::create_signed_commit(&SpawnGitExec, workdir, tree_oid, &parents_oids,
                &sig, &sig, &full, old, &format!("commit: {summary}"))?;
    // read back branch/summary exactly as the unsigned tail does
}
```
`amend_commit(workdir, message, sign)`: same, but parents = `head_commit.parent_ids()`, author =
`head_commit.author()` (preserved), `old = Some(head_commit.id())`, reflog `commit (amend): {summary}`,
and the signed branch does NOT use `Commit::amend` (it uses `create_signed_commit` with the original
parents). `resolve_signature` (ConfigMissing) still runs BEFORE any spawn. `commit_merge` mirrors
`create_commit` with the merge parents.

> The `sign` seam is the **only** touch to signature-agnostic guards — a signer can't silently downgrade
> a merge/amend. `create_signed_commit` bypasses hooks by design; **P59 wraps hook execution around this
> whole flow** (its output is not part of the signed content).

---

## 5. Commands + registration — `src-tauri/src/commands/signing.rs`

House shape (mirror `search_commits`; read-only ⇒ no `repo-changed`):
```rust
#[tauri::command]
pub async fn verify_commits(state: State<'_, AppState>, repo_id: String, oids: Vec<String>)
    -> Result<VerifyResults, AppError> { verify_commits_inner(state.inner(), &repo_id, oids).await }
pub(crate) async fn verify_commits_inner(state:&AppState, repo_id:&str, oids:Vec<String>)
    -> Result<VerifyResults, AppError> {
    let workdir = repo_path(state, repo_id)?;
    spawn_blocking(move || signing::verify_commits(&SpawnGitExec, &workdir, &oids)).await …?
}

#[tauri::command]
pub async fn signing_status(state: State<'_, AppState>, repo_id: String)
    -> Result<SigningStatus, AppError> { … spawn_blocking(move || signing::signing_status(&workdir)) … }
```
`staging.rs`: `commit(_, repo_id, message, sign: Option<bool>)` → `create_commit(&path, &message, sign)`;
same for `commit_amend`. Register `verify_commits`, `signing_status` in `lib.rs` after `search_commits`.

---

## 6. Mock (`src/ipc/mock/handlers/signing.ts`)

`signingHandlers satisfies Partial<IpcApi>`:
- `verifyCommits(repoId, oids)`: `await delay(80)`; map each oid to a **deterministic** `VerifyStatus`
  from its first hex nibble (`0-9` → mix of `good`/`goodUnknown`/`unsigned`, `a` → `bad`, `b` → `expired`,
  `c` → `cannotCheck`, rest → `unsigned`) so the harness shows every badge state; `signer`/`key` = canned
  strings for signed statuses. Document: UI-plumbing only (fixtures carry no real signatures). Respect
  `MAX_VERIFY_BATCH`. Omit any oid not in the current layout (mirrors "unresolvable").
- `signingStatus(repoId)`: default `{ enabled:false, format:null, hasKey:false }`; a `?sign=ssh` /
  `?sign=gpg` query flips `enabled:true` + the format + `hasKey:true` + a canned `key`, so the commit-box
  indicator/toggle + "will sign" copy are harness-testable (mirrors P37's `?remote=` seam).
- `commit`/`commitAmend` in `mock.ts`: accept the `sign` arg and ignore its crypto effect (mock cannot
  sign); still return a normal `CommitResult`. Import + spread `signingHandlers`.

---

## 7. Frontend — light the badge + panel line + sign toggle

### 7.1 `useCommitVerification.ts` (mirrors `useCommitSearch`)
```ts
export function useCommitVerification(deps: {
  repoId: string;
  graphDataRef: { current: GraphLayout | null };
  enabled: boolean;                 // graphPrefs.showSignatureBadge
  pushToast(kind: ToastKind, msg: string): void;
}): {
  verifyStatus: ReadonlyMap<string, VerifyStatus>;   // oid -> status (cache)
  detailsFor(oid: string): CommitVerification | undefined; // for CommitPanel (signer/key)
  onVisibleRangeChange(first: number, last: number): void; // from GraphCanvas
  refresh(): void;                  // manual re-verify (drop cache)
};
```
Behavior: on `onVisibleRangeChange`, map rows→oids, collect the **uncached** ones, debounce ~150 ms,
`verifyCommits` in batches ≤ `MAX_VERIFY_BATCH`, merge results into the cache (last-wins `reqId` guard).
`enabled=false` ⇒ no fetch, empty map (P51 faint stub shows). `refresh()` clears the cache + re-requests
the current window (wired to the existing Refresh action; also call after a successful `commit` so the new
HEAD verifies).

### 7.2 Graph draw (`GraphCanvas.tsx` / `draw.ts` / `drawRowText.ts`)
- `GraphCanvasProps.verifyStatus?: ReadonlyMap<string, VerifyStatus>` + `onVisibleRangeChange?(first,
  last: number): void`. Thread `verifyStatus` via `propsRef` into the row loop (like `matchSet`); call
  `onVisibleRangeChange` once per paint after the visible window is computed (guard against redundant
  fires — only when first/last changed).
- `drawBadgeStub(ctx, cx, cy, theme, status?, showBadge)`: when `showBadge` and `status` is a signed
  state, draw a **lit** glyph — filled check/shield in `--badge-good` for `good`; hollow neutral in
  `--badge-unknown` for `goodUnknown`/`cannotCheck`; warning triangle in `--badge-warn` for
  `bad`/`expired`/`expiredKey`/`revoked`; NOTHING for `unsigned`. Else (no status yet, or `showBadge`
  false) draw the existing faint stub. Slot geometry unchanged (P51 §6.5).

### 7.3 `CommitPanel.tsx` — signature-status line
Below the author/date rows, when `detailsFor(details.oid)` exists and status ≠ `unsigned`, render a
`commit-signature` line: a status icon (same palette as the badge) + text
(`Good signature` / `Signed, unverified signer` / `BAD signature` / `Expired` / `Cannot verify`) + the
`signer` and short `key` when present. `unsigned` ⇒ render nothing (no clutter). The panel reuses the
same verify map (single source), so no extra IPC for the selected commit.

### 7.4 `CommitBox.tsx` — sign toggle + indicator
Read `signingStatus(repoId)` once on mount/repo-change (RepoWorkspace provides it). Render a small
"Sign commit" checkbox whose default = `signingStatus.enabled`, plus an inline hint:
- enabled + hasKey ⇒ "Commits will be signed ({SSH|GPG})".
- toggle on but `!hasKey` ⇒ a warning "No signing key set — set user.signingkey" (links to Settings).
Pass `sign` = the checkbox value to `commit`/`commitAmend` (send `null` when the checkbox equals the
config default, else the explicit bool, to keep "follow config" semantics — or always send the explicit
bool; §12 OQ6). On a `configMissing` error, toast the message (already actionable).

### 7.5 Settings + prefs
`SettingsGraphSection.tsx`: a **Signature badge** checkbox → `graph.showSignatureBadge` (whole-struct
patch, like P51). `settings.rs`/`persistence.ts`: add `show_signature_badge`/`showSignatureBadge` default
true (clamp passthrough + tolerant parse + a back-compat test that a legacy `graph` object without the key
loads with it defaulted true).

---

## 8. CLI-oracle test plan (`#[cfg(test)]` in `signing.rs`)

**SSH signing is fully AI-gate-testable, hermetic, offline** — an ephemeral ed25519 key with an EMPTY
passphrase needs no agent, and `ssh-keygen -Y verify` against a local `allowed_signers` file needs no
external trust. GPG needs a throwaway gpg home + generated key (slow/flaky in CI) ⇒ GPG is the USER
CHECKPOINT (optionally a `have_gpg()`-guarded test). Guard all with `have_git()`; SSH tests add
`have_ssh_keygen()`. `TMP`/`TEMP=D:\Temp` on Windows (MEMORY). Fixtures use `git2::Time` for determinism.

- **Pure units (no git):** `build_verify_args` exact vec (incl. many oids, non-hex dropped);
  `parse_verify_output` splits US records, maps every `%G?` code, empty signer/key → None;
  `map_status_code` full table; empty `oids` ⇒ empty, no spawn (fake `GitExec` that panics if called).
- **SSH sign oracle (ephemeral key, hermetic):** init repo; `ssh-keygen -t ed25519 -N "" -f key`; set
  `gpg.format=ssh`, `user.signingkey=<key>`, `commit.gpgsign` per case; stage a file; `create_commit(_,
  msg, Some(true))`:
  - HEAD moved to the new commit; `git verify-commit <oid>` exits 0; `git log --format=%G? -1` ∈ {`G`,`U`}
    (`G` when an `allowed_signers` file naming the key is configured; assert accordingly);
  - `verify_commits` returns `Good`/`GoodUnknown` with a `signer`; an UNSIGNED fixture commit → `Unsigned`;
  - author/committer identity matches `resolve_signature`; `git reflog -1` shows the `commit:` entry;
  - amend preserves the original author (and author date) and re-signs.
- **Config gates:** `sign=None` + `commit.gpgsign` unset ⇒ unsigned (byte-identical to the pre-P58 path —
  assert the object has NO `gpgsig` header via `git cat-file`); `commit.gpgsign=true` ⇒ signed;
  `sign=Some(false)` overrides `gpgsign=true` ⇒ unsigned; ssh + no `user.signingkey` + sign ⇒
  `ConfigMissing` (naming the key), NO commit created.
- **Verify degrade:** a repo with a signed commit but verification impossible (no `allowed_signers`) ⇒
  `GoodUnknown` or `CannotCheck` (never Err); a bogus/unknown oid is omitted from results.
- **`signing_status`:** unset ⇒ `{enabled:false, format:None, has_key:false}`; with `gpg.format=ssh` +
  key + `commit.gpgsign=true` ⇒ `{enabled:true, format:Some(Ssh), has_key:true, key:…}`.

Wire-shape tests: `VerifyStatus`/`CommitVerification`/`SigningStatus` serialize to the exact camelCase in
§2 (guards the TS mirror).

---

## 9. Sub-increment split

### P58a — Signing backend + config + commands + IPC + mock + SSH oracle
`exec.rs` (`GitExec`+`SpawnGitExec`); `signing.rs` (`resolve_signing`, `create_signed_commit`,
`signing_status`, `SigningConfig`/`SignFormat`/`SigningStatus`); `commit.rs`/`merge.rs` `sign` param +
signed branch; `commands/signing.rs` (`signing_status`) + `staging.rs`/`merge.rs` `sign`; `lib.rs`;
`types.ts`/`tauri.ts`/`mock.ts` (`signingStatus`, `sign`); SSH + config-gate oracle.
**Acceptance:** SSH sign oracle green (signed commit verifies via `git verify-commit`; unsigned path
byte-identical); ConfigMissing on ssh+no-key; `cargo clippy -D warnings` + `tsc`/`build` clean; harness:
`?sign=ssh` → `signingStatus` returns enabled/ssh; `commit(_, 'x', true)` resolves.

### P58b — Verification backend + command + IPC + mock + oracle
`signing.rs` (`verify_commits`, `build_verify_args`/`parse_verify_output`/`map_status_code`,
`MAX_VERIFY_BATCH`); `commands/signing.rs` (`verify_commits`); `lib.rs`; `types.ts`/`tauri.ts`
(`verifyCommits`); `mock/handlers/signing.ts` (`verifyCommits`).
**Acceptance:** verify oracle green (Good/GoodUnknown/Unsigned/degrade); pure units green; harness:
`verifyCommits('r', [<oids>])` returns a per-oid status map; `#`/non-hex handled.

### P58c — Frontend: light the badge + panel line + sign toggle + pref
`useCommitVerification.ts`; `GraphCanvas` `verifyStatus`+`onVisibleRangeChange`; `draw.ts`/`drawRowText.ts`
lit badge; `colors.ts`; `CommitPanel` signature line; `CommitBox` toggle+indicator; `SettingsGraphSection`
+ `settings.rs`/`persistence.ts` `showSignatureBadge`; `WorkspaceGraphPane`/`RepoWorkspace` wiring.
**Acceptance:** `pnpm build` clean; no file over ~500 lines. Harness (mock statuses): visible rows show
lit badges (green check / warn / neutral) with off-screen rows staying faint until scrolled in;
`showSignatureBadge=false` → faint stub only, no verify requests (assert no `verifyCommits` call in
console); selecting a signed commit shows the signature line with signer/key; the commit box shows "will
sign (SSH)" under `?sign=ssh`. Screenshot for final proof.

---

## 10. Acceptance criteria

### AI gate (orchestrator verifies)
- All §8 oracle + pure tests green (`cargo test -p bonsai-core signing`); `clippy`/`tsc`/`build` clean;
  `generate_handler!` lists the two new commands.
- Unsigned commits are byte-identical to pre-P58 (no `gpgsig` header) — regression-proof.
- Harness: badge lights per mock status, virtualized (only visible rows requested), toggle hides it +
  stops requests; commit-box indicator + sign toggle reflect `signingStatus`; panel signature line
  renders; `verifyCommits`/`signingStatus` mockable in a plain browser.

### USER CHECKPOINT (native; requires the user's real key material — cannot be AI-judged)
1. **SSH:** with `gpg.format=ssh` + `user.signingkey` (their real key/agent), `commit.gpgsign=true` (or
   the box ticked) → a commit that `git log --show-signature` reports as signed; GitHub/GitLab shows
   "Verified" after push; Bonsai's badge is green and the panel line reads correctly.
2. **GPG:** with `gpg.format=openpgp` + a real GPG key (passphrase/agent prompt happens in their
   environment) → a signed commit; badge + panel correct. (GPG passphrase UX is environment-owned; Bonsai
   never prompts — a locked agent surfaces as a clear `git` error toast.)
3. **Verification of others' commits:** open a repo with a mix of verified/unverified/unsigned commits
   (their `allowed_signers`/keyring) → badges match `git log --show-signature`.
4. **Missing key:** signing on with no `user.signingkey` (ssh) → a clear, actionable error, no commit.
5. Windows + macOS + Linux: signer/agent invocation works without a flashing console window.

---

## 11. Invariants held
Rust owns ALL git + signing logic (git2 for assembly, git binary for sign/verify — both `&Path`-only,
CLI-testable); React only renders badges/status from a precomputed map. IPC = compact request/response
(`verify_commits` batched over visible rows — never per-commit round-trips; `signing_status` a tiny read).
git2/git shell-outs run in `spawn_blocking`. Badge is virtualized (visible rows only) + individually
toggleable (clutter principle). Signing NEVER prompts (never-prompt env; a locked agent fails fast to a
`git` error). No new `AppError`. Mock covers every new surface (`VITE_MOCK_IPC=1`).

---

## 12. Open questions (flag to orchestrator; recommendation in bold)

- **OQ1 — Sign mechanism C vs A.** **Recommend C (`git commit-tree -S` + `update-ref`)** — least code,
  both formats free, exact parity, unsigned path untouched. A (`commit_create_buffer`+`commit_signed`,
  self-implemented ssh/gpg `sign_buffer`) only if we must assemble the object fully in-process (no strong
  reason). Confirm C.
- **OQ2 — OpenPGP with no `user.signingkey`.** **Recommend: let git select by committer email** (git's
  own default) rather than erroring; only ssh requires the key. Alternative: require the key for BOTH
  (simpler, more deterministic). Confirm.
- **OQ3 — `signing_status` command vs derive.** **Recommend the dedicated command** (single source, honest
  indicator). Alternative: extend an existing status payload / derive from `read_config` client-side.
- **OQ4 — Sign merge commits (`commit_merge`).** **Recommend YES** (git signs merges under `commit.gpgsign`;
  same `create_signed_commit` call). Minor extra scope; confirm include in P58a or defer.
- **OQ5 — Ref move: `git update-ref` vs git2 `Repository::reference()`.** **Recommend `update-ref`**
  (git-exact reflog + symref-follow + unborn-branch + CAS old-oid, no git2 ref subtleties) at the cost of
  one extra spawn. git2 avoids the spawn but must replicate HEAD/reflog behavior. Confirm.
- **OQ6 — `sign` param encoding from the UI.** **Recommend: send the explicit bool** the box shows
  (simplest, unambiguous) rather than `null`-when-equals-config. Either works; confirm.
- **OQ7 — Badge glyph set + colors.** **Recommend:** green filled check = `good`; neutral hollow =
  `goodUnknown`/`cannotCheck`; amber/red warning = `bad`/`expired`/`expiredKey`/`revoked`; nothing =
  `unsigned`. Confirm the exact iconography with the user at the USER CHECKPOINT (perception).
- **OQ8 — Verify cache invalidation.** **Recommend:** cache by oid for the session; drop on manual Refresh
  and after a new commit. A keyring/allowedSigners edit mid-session won't reflect until Refresh — document.
  Confirm acceptable.
