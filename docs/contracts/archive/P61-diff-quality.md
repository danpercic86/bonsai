# P61 — Diff quality: word-level/intraline highlighting · image diff

Two enhancements on the shared diff renderer. Roadmap P61 (Phase 3 — Correctness & parity).
Both are backend-computed (Rust owns diff math) and rendered by React; both are **opt-in / toggleable**
per the clutter principle. Mostly harness-verifiable; the ONLY USER CHECKPOINT is final visual
fidelity (intraline colour legibility + real image rendering).

**Command count: 134 → 135** (`get_image_diff`; assumes P60 shipped first — else 129→130). Intraline
adds **no** command: it is an additive `intraline: bool` param on the three hunk-returning diff
commands + an optional `spans` field on `DiffLine`.

References read (verified): `crates/bonsai-core/src/git/diff.rs` (`LineKind`, `DiffLine{kind,oldNo,
newNo,content,noNewline}`, `Hunk`, `FileDiff{path,origPath,status,binary,tooLarge,hunks}`,
`normalize_content`, `workdir_file_diff` / `commit_file_diff` / `compare_head_file_diff`, `pathspecs`,
`commit_trees`, `head_endpoint`), `src-tauri/src/commands/diff.rs` (the three `*_file_diff` commands,
each already carrying `full_context: bool`), `src/components/DiffView.tsx` (`DiffViewProps`, the
`rows` flat list, `lineRow` §217-302 rendering `.diff-lineno`/`.diff-marker`/`.diff-content`, the
`highlight` HTML-string path via `dangerouslySetInnerHTML`), `src/components/DiffViewSplit.tsx` (the
per-cell `cell(side,line)` renderer, from P46), `src/ipc/types.ts` (`DiffLine`/`Hunk`/`FileDiff`),
`src/ipc/mock/handlers/diff.ts` (`getWorkdirFileDiff`/`getCommitFileDiff`), `src-tauri/capabilities/
default.json` (**no asset-protocol permission** — decisive for D2). House format:
`docs/contracts/{M4-diff,P46-diff-viewer-enhancements}.md`. No `similar`/`image` crate in
`crates/bonsai-core/Cargo.toml` (checked) → intraline is hand-rolled, no new dep.

---

## 0. Key decisions (with rationale)

**D1 — Intraline is a backend pass emitting char-offset `spans` on changed `DiffLine`s, gated by a new
`intraline: bool` request param** (parallel to the shipped `full_context: bool`). Off (default) → the
`FileDiff` wire shape is byte-identical to today (`spans` skipped when empty). On → the frontend
refetches the open slot (same refetch pattern the File/Diff/full-context toggle already uses) and each
changed line carries the sub-ranges that differ from its paired counterpart. Rejected: a separate
`compute_intraline(FileDiff)` command (would ship the whole diff up and back); folding is cheaper and
reuses all fetch plumbing + mock handlers.

**D2 — Image blobs travel as base64 over IPC (a command), NOT the Tauri asset protocol / a temp
file.** This is forced by the mandatory browser-harness invariant: `asset://` / `convertFileSrc` is a
native-only protocol that **cannot be served by `src/ipc/mock.ts` in a plain browser**, so an
asset-protocol design would leave image diffs unrenderable in the harness (the orchestrator's only
visual-verification path). Base64-over-IPC is uniform across mock and native: the command returns raw
bytes as base64 + a MIME type, the frontend builds a `data:` URL for a plain `<img>`. Secondary wins:
no new capability/scope to configure (`default.json` has no `core:asset:*` today), and no temp-file
lifecycle to manage/clean. Cost — base64 inflates payload ~33% and the fetch is one-shot; bounded by a
per-side byte cap (D3). Temp-file/asset protocol is explicitly rejected for v1.

**D3 — Per-side image cap `MAX_IMAGE_BYTES = 8 MiB` (raw).** Over cap → that side is `null` +
`*TooLarge: true` (the frontend shows "image too large to preview — N MB"). Bounds worst-case IPC
payload at ~11 MB base64 per side; tunable (OQ2).

**D4 — Image detection is extension-based, not content-sniffing.** Offer "View as image" when the
file path's extension ∈ `{png,jpg,jpeg,gif,webp,bmp,ico,avif}`. `FileDiff.binary` (already computed)
confirms there is no useful text diff, but the image switch keys off extension so it works for
added/deleted images too. **SVG stays a text diff** (it is diffable text; not in the image set) —
OQ3.

**D5 — Intraline vs syntax-highlight on the same line: intraline wins per changed line.** highlight.js
returns one opaque HTML string per line; splitting it by char offset is impractical. So when intraline
is active, a **changed** line renders from `content` split by `spans` (plain text + emphasis spans, no
syntax colour on that line); **context** lines keep syntax highlighting. Simple, correct, legible.
Recommended over a fragile compose (OQ1).

**D6 — No new `AppError` variant.** Bad oid/path → `git`; unknown repo → `noRepo`; a non-image or
absent blob → `null` side (not an error).

---

## P61a — Word-level / intraline highlighting

### Module boundaries
- `crates/bonsai-core/src/git/diff.rs` — add optional `spans` to `DiffLine`; add a pure
  `intraline::annotate_hunk(&mut Hunk)` pass (put the token-diff in a sibling
  `crates/bonsai-core/src/git/intraline.rs` to keep `diff.rs` under the ~500-line limit); thread an
  `intraline: bool` through `workdir_file_diff` / `commit_file_diff` / `compare_head_file_diff` (call
  the pass after `collect_file_diff`).
- `src-tauri/src/commands/diff.rs` — add `intraline: bool` param to `get_workdir_file_diff`,
  `get_commit_file_diff`, `compare_with_head_file_diff` (+ `_inner`), forwarded to core.
- `src/ipc/{types.ts, tauri.ts}` — `spans?` on `DiffLine`; `intraline` arg on the three wrappers.
- `src/components/DiffView.tsx` + `DiffViewSplit.tsx` — render `spans`; `DiffOverlay` gains a
  **"Highlight changes"** toggle; `RepoWorkspace` threads the flag into the fetch (mirror
  `diffViewMode`/`full_context`).
- `src/ipc/mock/handlers/diff.ts` — honor `intraline` (compute spans in the mock too).

### Wire shape (Rust)
```rust
// diff.rs — DiffLine gains one optional field (default empty => wire-invisible when off):
#[serde(skip_serializing_if = "Vec::is_empty", default)]
pub spans: Vec<[u32; 2]>,
// Each [start, len] is a CHANGED sub-range within `content`, measured in Unicode
// SCALAR VALUES (chars / code points), not bytes and not UTF-16 units. Present only
// on `add`/`del` lines that were PAIRED with a counterpart; empty on context lines,
// on unpaired pure-add/pure-del blocks (the whole line reads as the change), and
// whenever `intraline=false`. Ranges are ascending and non-overlapping.
```
Frontend slices via `Array.from(content)` (code-point aware) — see rendering below. Note in the doc
comment: char offsets chosen over UTF-16 for a natural Rust implementation (`char_indices`); a
multibyte unit test guards it (OQ4 records the UTF-16 alternative).

### Algorithm (normative pseudocode)
`intraline::annotate_hunk(hunk)` — pair consecutive del/add runs exactly like P46 `pairSplitRows`,
then char-diff each paired row:
```
for each maximal run of consecutive changed lines in hunk.lines:
    dels = [lines with kind==Del];  adds = [lines with kind==Add]     // in order
    // pair index-by-index; only paired rows get spans (surplus side => no spans)
    for i in 0 .. min(len(dels), len(adds)):
        (old_spans, new_spans) = token_diff(dels[i].content, adds[i].content)
        dels[i].spans = old_spans        // ranges removed from OLD
        adds[i].spans = new_spans        // ranges added to NEW
    // context lines and surplus del/add lines keep spans = []

token_diff(a, b) -> (Vec<[u32;2]>, Vec<[u32;2]>):
    ta = tokenize(a); tb = tokenize(b)                 // Vec<Token{ text, char_start, char_len }>
    ops = lcs_diff(ta, tb)                             // Myers/LCS over token TEXT equality
    a_spans = merge_adjacent( ranges of tokens in `ta` marked Deleted )
    b_spans = merge_adjacent( ranges of tokens in `tb` marked Inserted )
    return (a_spans, b_spans)

tokenize(s):  // word-level: maximal runs of one class, so whole tokens flip, not chars
    split s (iterating chars, tracking code-point index) into maximal runs where each
    run is one of: [alphanumeric+`_`] | [whitespace] | [each other char = its own 1-char token]
    // classing punctuation individually keeps `foo(bar)` -> `foo`,`(`,`bar`,`)`
```
- `lcs_diff` is a standard LCS/Myers over `Token.text` equality (hand-rolled, no dep; inputs are
  single lines, so O(n·m) on tokens is fine). `merge_adjacent` coalesces touching changed-token ranges
  into `[char_start, total_char_len]`.
- Guard cost: skip the pass for a line whose `content.chars().count()` exceeds `MAX_INTRALINE_CHARS =
  2000` (leave `spans=[]`) — avoids O(n·m) blowups on minified/one-line files.
- Purely-added or purely-deleted rows (no counterpart) → `spans=[]` (no emphasis; the add/del tint
  already conveys "all new/removed"), matching `git --word-diff` behaviour.

### Command change
```rust
// each of the three, mirroring the existing `full_context: bool` param placement:
pub async fn get_commit_file_diff(.., full_context: bool, intraline: bool) -> Result<FileDiff, AppError>;
// _inner forwards `intraline` into commit_file_diff(.., full_context, intraline)
```
Core signatures gain a trailing `intraline: bool`; when true, run `annotate_hunk` on each hunk after
`apply_find_similar`/`collect_file_diff`, before the `too_large` short-circuit is irrelevant (skip if
`binary || too_large`).

### TypeScript
```ts
export interface DiffLine {
  kind: LineKind; oldNo: number | null; newNo: number | null; content: string;
  noNewline?: boolean;
  /** P61a: changed sub-ranges within `content` as [startCodePoint, lenCodePoints];
   *  absent/empty => render plain. Slice via Array.from(content). */
  spans?: [number, number][];
}
// three wrappers gain a trailing `intraline: boolean` arg:
getCommitFileDiff(repoId, oid, path, origPath, fullContext, intraline): Promise<FileDiff>;
getWorkdirFileDiff(repoId, path, origPath, staged, fullContext, intraline): Promise<FileDiff>;
compareWithHeadFileDiff(repoId, oid, path, origPath, fullContext, intraline): Promise<FileDiff>;
```

### Rendering (DiffView + DiffViewSplit)
- New pure helper `src/utils/intralineSegments.ts` →
  `segmentLine(content: string, spans?: [number,number][]): { text: string; changed: boolean }[]`
  (splits `Array.from(content)` on the ranges; returns whole line as one unchanged segment when spans
  absent/empty). Unit-tested.
- In `lineRow` (`DiffView.tsx` §286-290): when the "Highlight changes" toggle is ON **and**
  `line.spans?.length` **and** `line.kind !== 'context'` → render the `.diff-content` from
  `segmentLine(...)`, wrapping `changed` segments in
  `<span class="diff-intra diff-intra-{add|del}">…`; otherwise keep the existing highlight-HTML /
  plain path unchanged (D5: highlighted context lines are unaffected).
- Same treatment inside `DiffViewSplit.tsx`'s `cell(side,line)` content span.
- CSS (`styles.css`, near `.diff-*`): `.diff-intra-add` (stronger add tint / underline) and
  `.diff-intra-del` (stronger del tint / strike-through-free background) — must stay legible over the
  row's `.diff-line-add`/`.diff-line-del` background AND the `.diff-line-selected` gradient. New
  tokens `--intra-add` / `--intra-del`.
- `DiffOverlay` toggle: a **"Highlight changes"** switch beside the Diff/File/Split group;
  `RepoWorkspace` holds `intraline: boolean` (+ ref), threads it into every diff fetch, and refetches
  the open slot when it flips (reuse the `full_context` refetch path).

### Mock
`getWorkdirFileDiff`/`getCommitFileDiff`/`compareWithHeadFileDiff` accept `intraline`; when true, run
the SAME `segmentLine`-friendly token diff **in TS** over the fixture's paired del/add lines (share a
tiny `mockTokenDiff` helper) and attach `spans`. Provide at least one fixture with a same-line edit
(e.g. `const x = 1;` → `const x = 42;`) so the harness shows emphasis only on `1`→`42`.

### Acceptance
1. `cargo test -p bonsai-core intraline` green: `tokenize` classes; `token_diff` on
   same-line-single-token, multi-token, prefix/suffix-shared, pure-insert, pure-delete, identical (→
   empty); **multibyte** case (accented char / emoji shifts offsets correctly — guards the code-point
   contract); `MAX_INTRALINE_CHARS` skip. Loose oracle: paired-line spans are consistent with
   `git diff --word-diff=porcelain` emphasis on a small fixture (documented as approximate — our
   pairing is per-row, git's is per-hunk). `intraline=false` → `FileDiff` serializes with **no**
   `spans` key (byte-identical to today).
2. `clippy` clean; the three commands compile with the new param; `generate_handler!` unchanged (no
   new command); `tsc`/`build` clean; no file over ~500 lines (token diff in `intraline.rs`, segments
   in `intralineSegments.ts`).
3. Harness: toggle **Highlight changes** ON → a modified line emphasises only the changed sub-range;
   OFF → plain; context lines keep syntax highlight in both states; toggle triggers exactly one
   refetch of the open slot.

---

## P61b — Image diff (side-by-side / onion-skin / swipe)

### Module boundaries
- `crates/bonsai-core/src/git/image_diff.rs` — **NEW**: `ImageSide`, `ImageDiff`,
  `ImageDiffRequest`, `get_image_diff`; blob resolution reuses `diff.rs` helpers (`commit_trees`,
  `head_endpoint`, `pathspecs`) + `open_workdir_repo`.
- `crates/bonsai-core/src/git/mod.rs` — `pub mod image_diff;`.
- `src-tauri/src/commands/diff.rs` — `get_image_diff` command + `_inner`.
- `src-tauri/src/lib.rs` — register (after `compare_with_head_file_diff`).
- `src/ipc/{types.ts, tauri.ts}` + `src/ipc/mock/handlers/diff.ts`.
- Frontend: `src/components/DiffImageView.tsx` (**NEW**, presentational, its own file) — the three
  compare modes; `DiffOverlay` shows an image-mode switcher (Side-by-side / Onion / Swipe) instead of
  Diff/File/Split when the open file is an image; `RepoWorkspace` fetches `ImageDiff` for image files.

### Wire shape (Rust)
```rust
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSide {
    /// Raw blob bytes, standard base64 (NO `data:` prefix — the frontend builds the URL).
    pub base64: String,
    /// MIME from the path extension, e.g. "image/png". Frontend uses it in the data URL.
    pub mime: String,
    /// Raw byte length pre-base64 (for the "N KB" label). Natural dimensions are read
    /// frontend-side from the <img> (no image-decoding dep in core).
    pub byte_len: u32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDiff {
    pub path: String,
    /// OLD side (parent/HEAD/index). None when added OR when the blob is missing.
    pub old: Option<ImageSide>,
    /// NEW side (commit/workdir/index). None when deleted OR missing.
    pub new: Option<ImageSide>,
    /// A present-but-oversized side is null with its flag true ("too large to preview").
    pub old_too_large: bool,
    pub new_too_large: bool,
}

/// Which pair to load — mirrors the three existing file-diff contexts so the
/// frontend constructs it exactly where it picks a `*_file_diff` command today.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ImageDiffRequest {
    /// old = index blob (staged==true → HEAD blob); new = workdir file (staged → index blob).
    Workdir { path: String, orig_path: Option<String>, staged: bool },
    /// old = first-parent tree blob; new = commit tree blob (root commit → old None).
    Commit { oid: String, path: String, orig_path: Option<String> },
    /// old = HEAD tree blob; new = to-commit tree blob.
    Compare { to_oid: String, path: String, orig_path: Option<String> },
}

/// Blocking. Resolves both sides of an image comparison as base64. Each side is
/// `None` when absent (add/delete) or missing; a side over MAX_IMAGE_BYTES is
/// `None` with `*_too_large=true`. Never returns raw libgit2 objects. Uses
/// `orig_path` for the OLD side on renames. Bad oid → `git`; unknown repo → `noRepo`.
pub fn get_image_diff(workdir: &Path, req: &ImageDiffRequest) -> Result<ImageDiff, AppError>;
```
Blob resolution helpers (reuse existing diff.rs internals): tree side → `tree.get_path(path)?.to_object(&repo)?.as_blob()` → `blob.content()`; workdir-new side → read the file bytes under `workdir`; index side → `repo.index()?.get_path(Path::new(path),0)` → `repo.find_blob(entry.id)`. MIME from extension (`png→image/png`, `jpg|jpeg→image/jpeg`, `gif`, `webp`, `bmp→image/bmp`, `ico→image/x-icon`, `avif→image/avif`). Encode with `base64` (already transitively available; else `base64` crate — flag if a dep is needed).

### Command
```rust
#[tauri::command]
pub async fn get_image_diff(
    state: tauri::State<'_, AppState>, repo_id: String, request: ImageDiffRequest,
) -> Result<ImageDiff, AppError>;
// _inner: repo_path → spawn_blocking(image_diff::get_image_diff). Read-only; no repo-changed.
```

### TypeScript
```ts
export interface ImageSide { base64: string; mime: string; byteLen: number; }
export interface ImageDiff {
  path: string; old: ImageSide | null; new: ImageSide | null;
  oldTooLarge: boolean; newTooLarge: boolean;
}
export type ImageDiffRequest =
  | { kind: 'workdir'; path: string; origPath: string | null; staged: boolean }
  | { kind: 'commit'; oid: string; path: string; origPath: string | null }
  | { kind: 'compare'; toOid: string; path: string; origPath: string | null };
/** Both sides of an image comparison as base64 (D2). Rejects git | noRepo. */
getImageDiff(repoId: string, request: ImageDiffRequest): Promise<ImageDiff>;
```
Frontend builds a URL per side: `` `data:${side.mime};base64,${side.base64}` ``.

### Frontend (`DiffImageView.tsx`)
Pure presentational; props `{ diff: ImageDiff; mode: 'sideBySide' | 'onion' | 'swipe' }`.
- **Side-by-side:** two labelled `<img>` (Old / New); a missing side shows "Added"/"Deleted"; an
  oversized side shows "N MB — too large to preview".
- **Onion-skin:** new over old, an opacity `<input type=range>` (0→1) crossfading.
- **Swipe:** old under new; a draggable vertical divider clips the new image (`clip-path`/overflow).
- `RepoWorkspace`: when the open diff's path extension ∈ the image set (D4), fetch `getImageDiff` with
  the request matching the current context and render `DiffImageView` instead of `DiffView`; the
  `DiffOverlay` toolbar shows the image-mode switcher (default `sideBySide`) instead of the
  Diff/File/Split group. Own small module; no state beyond the mode + opacity/divider (view-local).

### Mock (`src/ipc/mock/handlers/diff.ts`)
`getImageDiff(repoId, request)`: `requireRepo`; return canned tiny base64 PNGs — a solid **red** 2×2
for `old`, **green** for `new` (inline base64 constants), `mime:'image/png'`, plausible `byteLen`.
Seams: a path containing `added.` → `old:null`; `deleted.` → `new:null`; `huge.` →
`old:null, oldTooLarge:true`. This canned-bytes handler is exactly why D2 (base64-over-IPC) satisfies
the browser-harness invariant — an `asset://` design could not render here at all.

### Acceptance
1. `cargo test -p bonsai-core image_diff` green: each `ImageDiffRequest` variant resolves the correct
   pair (add → old None; delete → new None; rename uses `orig_path` for old); over-cap side →
   `None`+`*TooLarge`; MIME map; base64 round-trips to the original bytes. Bad oid → `git`. Scratch
   fixtures include a committed PNG modified across two commits + a workdir image edit.
2. `clippy` clean; `generate_handler!` = 135; `tsc`/`build` clean; `DiffImageView.tsx` under ~500
   lines; no `@tauri-apps/*` executed in the harness.
3. Harness (AI gate): open an image file → the mode switcher appears; side-by-side shows the two
   canned images; onion opacity slider crossfades; swipe divider clips; added/deleted/too-large
   fixtures show the right placeholders; console clean.

### USER CHECKPOINT (native, human perception — never self-declared)
- Real image blobs (e.g. a modified PNG/JPG in a real repo) render correctly in all three modes; the
  new/old pairing is right for commit, workdir, and compare contexts.
- Intraline emphasis colours are **legible** on both add and del rows and over a selection, in light
  and dark themes.

---

## Sub-increment order
`P61a` intraline → `P61b` image diff. Independent; intraline first (touches the shared text renderer;
smaller, fully harness-verifiable). Each is one senior-dev pass; commit after reviewer approval.

## Open questions (flag to orchestrator)
- **OQ1 — Intraline × syntax highlight per changed line.** Recommend mutually-exclusive (D5: intraline
  wins on changed lines; context keeps highlight). Confirm, or ask for a compose (highlight then
  overlay emphasis — significantly more work, marginal payoff).
- **OQ2 — `MAX_IMAGE_BYTES` = 8 MiB.** Bounds base64 payload; tune up (bigger previews) or down
  (lighter IPC)? Confirm 8 MiB.
- **OQ3 — SVG.** Recommend treating SVG as a text diff (diffable, benefits from intraline), NOT the
  image viewer. Confirm, or ask to also offer image rendering for SVG.
- **OQ4 — Span offset unit.** Recommend code-point (char) offsets + `Array.from` slicing (natural in
  Rust, guarded by a multibyte test). UTF-16-unit offsets would let JS slice `content` directly but
  are awkward to compute in Rust. Confirm code points.
- **OQ5 — `base64` dependency.** If no base64 encoder is already available to bonsai-core, P61b needs
  the `base64` crate (tiny, ubiquitous). Confirm adding it, or specify a hand-rolled encoder.
