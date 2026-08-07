# P56 — Local AI changelog / release-notes

A tag/ref range (`v1.2.0..v1.3.0`, or "since last tag") → **grouped, categorized release notes**
(Features / Fixes / …), generated **fully locally**. Walks the commit range Bonsai already computes
and reuses the shipped range resolver + payload renderers; the CLI only *writes the prose*. Read-only:
**WRITES NOTHING to git.** Output renders in `AiOutputPanel` (copyable / lightly editable, §9).

Obeys the Phase-2 shared conventions (`docs/contracts/phase2-ai-native-overview.md`): C1 grounding
(reuse `render_commit_list`/`render_headers`), C2 output in `AiOutputPanel`, C3 `ai_*` triple +
consent gate + camelCase + mock parity, C4 local-first, C5 tier seam preserved. Direct template:
`P28-what-changed-digest.md` (the `AiDigestRange`/`resolve_digest_range` idiom) and
`git/ai_summary.rs` (`summarize_range` range + payload shape).

References read (verified): `crates/bonsai-core/src/git/ai_summary.rs` (`summarize_range`, `AiSummary`,
`AI_SUMMARY_MAX_COMMITS`, the revwalk+merge-base+`render_headers` pipeline), `git/ai_explain.rs`
(`AiDigestRange`, `resolve_digest_range` — currently **private**; `format_commit_meta`,
`cap_review_payload`), `ai/payload.rs` (`render_commit_list`, `render_headers`), `commands/ai.rs`
(triple + gate), `src/components/AiOutputPanel.tsx`, `src/components/workspaceMenus.ts` (tag-pill
action set), `src/components/WhatChangedDialog.tsx` (the range-dialog precedent),
`src/ipc/mock/handlers/ai.ts`.

**Command delta: +1** (`ai_changelog`). Absolute count depends on P53–P55 landing order; orchestrator
renumbers `generate_handler!`. Sub-increments: **P56a** core+command+IPC+mock+tests · **P56b** UI.
Open questions in §11.

---

## 1. Decisions (with rationale)

**D1 — New module `git/ai_changelog.rs`, NOT more arms on `ai_explain`.** Release notes are
release-shaped (a tag-centric range + a *grouped-Markdown* prompt), distinct from the digest's
free-prose. A sibling module (like `ai_summary.rs`) keeps each file focused and < ~500 lines.

**D2 — Reuse the shipped range resolver (per the overview reuse map).** Promote
`ai_explain::resolve_digest_range` + `format_commit_meta` to `pub(crate)` and call them; changelog
maps its range → `AiDigestRange::BetweenRefs { from, to }`, getting back `(header, commits, old_tree,
new_tree)`. "Since last tag" resolves the previous tag FIRST, then delegates to the same path. (Alt:
a self-contained resolver mirroring `summarize_range` — OQ6.) No re-walk of raw objects; no re-shell.

**D3 — Range = tags OR "since last tag".** `ChangelogRange::BetweenRefs { from, to }` (any
revparse-able refs — tags are the common case) and `ChangelogRange::SinceLastTag { target? }`
(`target` defaults to `HEAD`; resolves the most recent tag reachable from `target`, EXCLUDING
`target`'s own tip). This gives both the arbitrary-range and the one-click stories.

**D4 — Grouping = AI, guided by a conventional-commits HINT (OQ1).** Rust does NOT parse commit
subjects into groups (brittle; many repos aren't conventional). Instead the system prompt asks the
model to group by change type, USING conventional-commit prefixes (`feat`/`fix`/`perf`/`refactor`/
`docs`/`test`; `build`/`ci`/`chore` → Other) when present and inferring from the subject/diffstat
otherwise. Works on conventional AND non-conventional repos; keeps Rust simple.

**D5 — Fixed taxonomy (OQ2), empty groups omitted.** Headings, in order: **Features, Fixes,
Performance, Refactoring, Documentation, Tests, Other**. Output is Markdown: a one-sentence summary
line, then `### <Group>` sections with `- <description> (<short7>)` bullets. Merge commits / pure
version bumps may be skipped by the model.

**D6 — Dedicated result type `AiChangelog`.** Structurally near-`AiSummary`, but named `fromRef`/
`toRef` (echoes the RESOLVED refs — crucially the resolved previous-tag name for `SinceLastTag`, so
the panel header shows what "last tag" meant). Reusing `AiSummary` verbatim is the minimal-surface
alt (OQ4).

**D7 — Read-only; renders in `AiOutputPanel`.** No git mutation, no events, no channels (prose fits a
command; `ai_summary`/`ai_digest` precedent). "Editable" = a Copy affordance + an opt-in editable
textarea mode on `AiOutputPanel` (OQ5) — the notes are the one output users most want to tweak before
pasting into a GitHub release / `CHANGELOG.md`.

**D8 — No new `AppError` variant.** Empty range / no previous tag → `AiFailed(...)` BEFORE any CLI
call; bad ref → `Git`; gate → `AiUnavailable`; unknown repo → `NoRepo`.

---

## 2. Module boundaries

| File | Change | Increment |
|---|---|---|
| `crates/bonsai-core/src/git/ai_changelog.rs` (NEW) | `ChangelogRange`, `AiChangelog`, `MAX_CHANGELOG_COMMITS`, prompts, `resolve_last_tag`, `generate_changelog`, tests | P56a |
| `crates/bonsai-core/src/git/ai_explain.rs` (edit) | `resolve_digest_range` + `format_commit_meta` → `pub(crate)` (D2) | P56a |
| `crates/bonsai-core/src/git/mod.rs` (edit) | `pub mod ai_changelog;` | P56a |
| `src-tauri/src/commands/ai.rs` (edit) | `ai_changelog` + `_inner` (triple + gate) | P56a |
| `src-tauri/src/commands/shared.rs` (edit) | re-export `ChangelogRange`, `AiChangelog` | P56a |
| `src-tauri/src/lib.rs` (edit) | register `ai_changelog` | P56a |
| `src/ipc/types.ts` / `tauri.ts` (edit) | `ChangelogRange`, `AiChangelog`, `aiChangelog` | P56a |
| `src/ipc/mock/handlers/ai.ts` (edit) | `aiChangelog` handler | P56a |
| `src/components/ChangelogDialog.tsx` (NEW) | range picker (from/to/since-last-tag) | P56b |
| `src/components/workspaceMenus.ts` (edit) | tag-pill "Release notes since previous tag" | P56b |
| `src/components/RepoWorkspace.tsx` (edit) | `runChangelog` (mirrors `runDigest`); dialog + menu wiring | P56b |
| `src/components/AiOutputPanel.tsx` (edit) | opt-in Copy + `editable` textarea mode (OQ5) | P56b |
| `src/components/CommandPalette.tsx` (edit, optional) | "Release notes…" entry (OQ3) | P56b |

---

## 3. Rust types + core (`ai_changelog.rs`)

```rust
/// Which range to write release notes for. Command INPUT (Deserialize); TS mirror
/// is a discriminated union (§4). (P56)
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChangelogRange {
    /// Notes for commits in `to` but not `from` (merge-base range). Both accept any
    /// revparse-able ref/oid; tags (`v1.2.0`..`v1.3.0`) are the common case.
    BetweenRefs { from: String, to: String },
    /// Notes since the most recent tag reachable from `target` (default HEAD),
    /// EXCLUDING `target`'s own tip. `from` resolves to that previous tag.
    SinceLastTag {
        #[serde(default)]
        target: Option<String>,
    },
}

/// Grouped release notes (Markdown) + the RESOLVED range echoed for the UI header.
/// Serialize camelCase (mirrored in TS). (P56)
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChangelog {
    pub text: String,       // grouped Markdown release notes
    pub from_ref: String,   // resolved `from` (e.g. the previous-tag name for SinceLastTag)
    pub to_ref: String,     // resolved `to`
    pub commit_count: u32,  // commits listed (capped at MAX_CHANGELOG_COMMITS)
    pub cost_usd: Option<f64>,
}

/// Cap on commits listed in the payload (keeps the call bounded); beyond it a
/// "(+N more commits)" note (same idiom as AI_SUMMARY_MAX_COMMITS).
pub const MAX_CHANGELOG_COMMITS: usize = 300;

/// Blocking, READ-ONLY. Resolves `range` (reusing resolve_digest_range, D2),
/// gathers the commit list + net diffstat, renders the payload, and asks the CLI
/// for grouped Markdown notes. WRITES NOTHING. Errors: aiFailed (empty range / no
/// previous tag / CLI failure) | git (bad ref) | (aiUnavailable via the gate).
pub fn generate_changelog(
    workdir: &Path,
    range: ChangelogRange,
    opts: RunOpts,
) -> Result<AiChangelog, AppError>;

/// Most recent tag (annotated or lightweight) reachable from `target_oid`,
/// excluding a tag pointing AT `target_oid` itself; returns (tag_shorthand, oid).
/// `None` => no earlier tag (caller => AiFailed). Mirrors `git describe
/// --tags --abbrev=0 <target>^` semantics using git2 tag enumeration + merge-base
/// reachability + committer-time ordering.
fn resolve_last_tag(
    repo: &git2::Repository,
    target_oid: git2::Oid,
) -> Result<Option<(String, git2::Oid)>, AppError>;
```

`generate_changelog` flow:
1. `open_workdir_repo`; map `range` → `(from_ref, to_ref)`:
   - `BetweenRefs { from, to }` → as-is.
   - `SinceLastTag { target }` → `to_ref = target.unwrap_or("HEAD")`; revparse it → `to_oid`;
     `resolve_last_tag(to_oid)?` → `Some((tag, _))` ⇒ `from_ref = tag`; `None` ⇒
     `AiFailed("no earlier tag found before <to_ref>")` (no CLI call).
2. `resolve_digest_range(&repo, &AiDigestRange::BetweenRefs { from: from_ref, to: to_ref })?`
   → `(header_note, commits, old_tree, new_tree)`. Empty (no commits AND no diff content) ⇒
   `AiFailed("no changes between <from_ref> and <to_ref>")` (no CLI call).
3. Build payload: `render_commit_list(commits capped at MAX_CHANGELOG_COMMITS)` (+ "(+N more)" note)
   prefixed `COMMITS:` and `render_headers(old_tree→new_tree)` prefixed `NET CHANGES (diffstat):`;
   then `cap_review_payload` over the whole string. (Same shape as `ai_summary::summarize_range`.)
4. `run_claude(workdir, CHANGELOG_PROMPT, Some(&payload), RunOpts{ system_prompt:
   Some(CHANGELOG_SYSTEM_PROMPT), ..opts })`.
5. `AiChangelog { text, from_ref, to_ref, commit_count, cost_usd }`.

Prompt consts (single-line; `prompts_are_single_line` test):
- **`CHANGELOG_SYSTEM_PROMPT`** — normative content: *"You are writing release notes from a commit
  list and a diffstat on standard input. Produce concise Markdown release notes grouped by change
  type. Begin with one short summary sentence, then use these level-3 headings IN THIS ORDER, omitting
  any that would be empty: `### Features`, `### Fixes`, `### Performance`, `### Refactoring`,
  `### Documentation`, `### Tests`, `### Other`. Classify each commit by its conventional-commit prefix
  when present (feat→Features, fix→Fixes, perf→Performance, refactor→Refactoring, docs→Documentation,
  test→Tests; build/ci/chore→Other) and by its subject/diff otherwise. Under each heading write one
  bullet per notable change: a short human-readable description followed by the short hash in
  parentheses, e.g. `- Add SSH commit signing (a1b2c3d)`. Omit merge commits and pure version bumps.
  Output Markdown only — do NOT wrap the whole document in a code fence."*
- **`CHANGELOG_PROMPT`** = *"Write grouped release notes for the commits and diffstat on standard
  input."*

---

## 4. IPC surface

### 4.1 Command (`commands/ai.rs`) — consent-gated triple (verbatim `ai_summarize_range` shape)

```rust
#[tauri::command]
pub async fn ai_changelog(
    app: tauri::AppHandle, state: tauri::State<'_, AppState>,
    repo_id: String, range: ChangelogRange,
) -> Result<AiChangelog, AppError> {
    let file = settings::settings_file(&app)?;
    ai_changelog_inner(state.inner(), &file, &repo_id, range).await
}
// _inner: consent gate (AiUnavailable unless ai_enabled && ai_consented) BEFORE repo_path;
// then spawn_blocking(move || ai_changelog::generate_changelog(&workdir, range, RunOpts::default())).
// READ-ONLY => does NOT emit repo-changed.
```
Register in `lib.rs`; re-export `ChangelogRange`, `AiChangelog` in `commands/shared.rs`.

| Command | IPC method | Args | Returns | Error kinds |
|---|---|---|---|---|
| `ai_changelog` | `aiChangelog` | `repoId, range` | `AiChangelog` | `aiUnavailable \| aiFailed \| git \| noRepo` |

### 4.2 TypeScript (`src/ipc/types.ts`)

```ts
export type ChangelogRange =
  | { kind: 'betweenRefs'; from: string; to: string }
  | { kind: 'sinceLastTag'; target?: string | null };

/** Grouped Markdown release notes + the resolved range. Mirrors Rust AiChangelog. */
export interface AiChangelog {
  text: string;
  fromRef: string;
  toRef: string;
  commitCount: number;
  costUsd: number | null;
}
```
`IpcApi`:
```ts
/** Generate grouped Markdown release notes for a tag/ref range (or since the last
 *  tag). Read-only; WRITES NOTHING; does NOT emit repo-changed. Fully local.
 *  Rejects aiUnavailable | aiFailed (empty range / no earlier tag / CLI) | git
 *  (bad ref) | noRepo. */
aiChangelog(repoId: string, range: ChangelogRange): Promise<AiChangelog>;
```
`tauri.ts`: `aiChangelog: (repoId, range) => invoke('ai_changelog', { repoId, range })`.

---

## 5. Mock (`src/ipc/mock/handlers/ai.ts` — extend `aiHandlers`)

`aiChangelog(repoId, range)`: `await delay(800); requireRepo(repoId);` `?ai=off` → throw
`{ kind:'aiFailed', message:'Claude Code CLI not found on PATH' }`. Else canned grouped Markdown,
echoing the resolved range so the harness shows what was generated:
- `fromRef` = `range.kind==='betweenRefs' ? range.from : 'v1.2.0'`; `toRef` = `betweenRefs ? range.to
  : (range.target ?? 'HEAD')`.
- `text` = a fixed grouped sample, e.g.
  ```
  This release adds AI-native operations and hardens the graph.

  ### Features
  - Natural-language to safe git operation (a1b2c3d)
  - AI branch naming (e4f5a6b)

  ### Fixes
  - Debounce watcher event storms on Windows (c7d8e9f)

  ### Documentation
  - Phase-2 contracts (0a1b2c3)
  ```
- `commitCount: 4`, `costUsd: 0.012`.
Deterministic; no mock state; `mock.ts` already spreads `aiHandlers`.

---

## 6. Frontend (P56b)

- **One-click entry (primary):** in `workspaceMenus.ts`, add "Release notes since previous tag" to the
  **tag-pill** action set → `runChangelog({ kind:'sinceLastTag', target: tagName }, \`Release notes for
  ${tagName}\`)`. Gated on `aiEligible`.
- **General entry:** `ChangelogDialog.tsx` (NEW, presentational; models `WhatChangedDialog`): a
  radio pick — **Between refs** (`from`/`to` text inputs with a datalist of tag/branch names, `to`
  defaulting to the current branch/HEAD) vs **Since last tag** (optional `target`, default HEAD).
  Submit → `onSubmit(range, title)`. Opened from a "Release notes…" toolbar/palette action (OQ3).
- **`RepoWorkspace.tsx`** — `runChangelog(range, title)` mirrors `runDigest`: same `aiPanel` state +
  `aiPanelReqId` last-wins guard, `ipc.aiChangelog(repoId, range)`, panel title
  `Release notes: <fromRef>..<toRef>` (from the result). Errors → the panel's error banner.
- **`AiOutputPanel.tsx`** — render `text` (Markdown SOURCE, copy-friendly monospace) with a **Copy**
  button; add an OPT-IN `editable`/`onEdit` prop (a textarea variant) used ONLY by changelog so users
  can tweak before pasting (OQ5). All existing read-only callers pass nothing → unchanged.

---

## 7. Tests (AI gate)

Reuse the AI idioms (P28 §10, `ai_summary` tests): `claude_stub` via `CLAUDE_BIN_ENV`, scratch repos,
`prompts_are_single_line`, `*_wire_shape_is_camel_case`, deserialize locks; `TMP`/`TEMP=D:\Temp`;
never run `cargo test` + `clippy` concurrently.

1. `changelog_range_deserializes_each_variant` — exact TS JSON (`{"kind":"betweenRefs","from":"v1","to":"v2"}`,
   `{"kind":"sinceLastTag"}`, `{"kind":"sinceLastTag","target":"HEAD"}`).
2. `changelog_wire_shape_is_camel_case` (`text`/`fromRef`/`toRef`/`commitCount`/`costUsd`; `None`→`null`).
3. `resolve_last_tag_finds_previous` — fixture with tags `v1`@A, `v2`@C, `v3`@E along a chain →
   `resolve_last_tag(E)` = `v2` (excludes `v3`@E itself); `resolve_last_tag(A)` (no earlier tag) =
   `None`; a target between tags picks the nearest reachable earlier tag.
4. `between_refs_commit_set` — `main` A–B, `feature` +C–D off B → `generate_changelog(BetweenRefs{main,
   feature})` lists exactly [D,C] (via the stub echo / `render_commit_list`), `commitCount==2`.
5. `since_last_tag_maps_to_previous_tag` — `SinceLastTag{target:v3}` resolves `from_ref=="v2"`,
   `to_ref=="v3"`.
6. `empty_range_fails_before_cli` — `from==to` → `AiFailed`, fake bin panics if spawned.
7. `no_earlier_tag_fails_before_cli` — untagged repo → `SinceLastTag` → `AiFailed`.
8. `prompts_are_single_line`.
9. **Stub harness** — `generate_changelog(BetweenRefs{...})` returns the stub's canned text; assert the
   payload contains a `COMMITS:` block and a `NET CHANGES (diffstat):` section.
10. **CLI oracle** (`crates/bonsai-core/tests/`, degrade-skip if `git` absent) — commit set ==
    `git log --format=%h <from>..<to>` (membership + order) for both a tag range and `SinceLastTag`.

**Frontend:** `tsc` + `pnpm build` clean; harness (`VITE_MOCK_IPC=1`): tag-pill menu "Release notes
since previous tag" → grouped Markdown in `AiOutputPanel` with a working Copy; `ChangelogDialog`
between-refs submit → notes; `?ai=off` → error banner.

---

## 8. Sub-increments

- **P56a — core + command + IPC + mock.** `ai_changelog.rs` (§3) + tests §7 (1–9); `resolve_digest_range`/
  `format_commit_meta` → `pub(crate)`; `mod.rs`; `ai_changelog` command + `_inner`; `shared.rs`;
  `lib.rs`; `types.ts`/`tauri.ts`; `ai.ts` mock. CLI oracle §7.10 if `git` present.
  **Acceptance:** `cargo test -p bonsai-core ai_changelog` green (resolve-last-tag, empty/no-tag before
  CLI, wire/deserialize locks); build/clippy clean; `tsc`/`pnpm build` clean; console
  `aiChangelog('r',{kind:'sinceLastTag'})` resolves `{text,fromRef,toRef,commitCount,costUsd}`.
- **P56b — UI.** `ChangelogDialog.tsx`, tag-pill menu entry, `runChangelog`, `AiOutputPanel` Copy +
  opt-in editable mode, optional palette entry. **Acceptance:** harness §7 frontend bullet; no file
  over ~500 lines.

Orchestrator commits each approved sub-increment (`wip(P56a): …`).

---

## 9. Acceptance — AI gate vs USER CHECKPOINT

**AI gate:** §7 green; consent gate in `_inner`; `tsc`/`pnpm build` clean; harness screenshots of the
tag-pill entry + `ChangelogDialog` → grouped notes in `AiOutputPanel` (+ Copy), and the `?ai=off`
error path; command delta +1.

**USER CHECKPOINT** (`docs/contracts/P56-user-checklist.md`; real `claude` CLI + real repo with tags):
- Right-click a tag → "Release notes since previous tag" → sensible grouped Markdown citing real
  changes/hashes since the prior tag; Copy yields pasteable Markdown.
- A `v1.2.0..v1.3.0` between-refs range works; "since last tag" from HEAD works.
- Empty range and no-earlier-tag show clear messages; AI-disabled is gated with a clear message; no
  code leaves the device (local CLI only).

---

## 10. Error mapping (no `error.rs` change)

| Situation | Variant | TS kind |
|---|---|---|
| AI disabled / not consented / CLI missing | `AiUnavailable` (gate) | `aiUnavailable` |
| Bad ref / other git2 failure | `Git` | `git` |
| Empty range / no earlier tag for `SinceLastTag` | `AiFailed(...)` | `aiFailed` |
| CLI failed / timed out / empty | `AiFailed` | `aiFailed` |
| Unknown `repoId` | `NoRepo` | `noRepo` |

---

## 11. Open questions (flag to orchestrator)

- **OQ1 — grouping approach.** Recommend **AI grouping with a conventional-commits HINT** (D4): no
  brittle Rust parser, works on non-conventional repos. Alt: Rust parses conventional prefixes into
  deterministic groups, AI only polishes prose. Confirm.
- **OQ2 — taxonomy.** Recommend Features / Fixes / Performance / Refactoring / Documentation / Tests /
  Other (empty omitted). Confirm the set + order.
- **OQ3 — range-selection UX.** Recommend tag-pill "Release notes since previous tag" (one-click
  primary) + `ChangelogDialog` for arbitrary ranges + a palette "Release notes…" entry (P50 palette
  exists — cheap). Confirm which ship in v1 (recommend tag-menu + dialog; palette optional).
- **OQ4 — result type.** Recommend a dedicated `AiChangelog` (echoes resolved `fromRef`/`toRef` incl.
  the resolved previous-tag name). Alt: reuse `AiSummary` verbatim (minimal surface, but `base`/
  `target` naming is odd for notes). Confirm.
- **OQ5 — editable output.** Recommend adding an OPT-IN `editable` textarea mode + Copy to
  `AiOutputPanel` (release notes are the output users most want to tweak), backward-compatible for
  existing read-only callers. Alt: Copy-only, zero change to the shared panel. Confirm.
- **OQ6 — range-resolver reuse.** Recommend promoting `resolve_digest_range` to `pub(crate)` and
  reusing it (D2, per the overview reuse map). Alt: a self-contained resolver in `ai_changelog.rs`
  mirroring `summarize_range` (less coupling, small duplication). Confirm.
- **OQ7 — `SinceLastTag` semantics from a tag `T`.** Recommend `from` = the tag BEFORE `T`, `to` = `T`
  (so a tag-pill invocation reads "what shipped in `T`"). The default (`target` = HEAD) reads "since
  the latest tag." Both flow through the single `target` param. Confirm.
