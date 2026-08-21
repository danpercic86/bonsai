# P61 — Diff quality: native USER CHECKPOINT checklist

Milestone P61 (word-level/intraline highlighting P61a `c088999`; image diff P61b `68163b6`).
Contract: `docs/contracts/P61-diff-quality.md`.

This splits the P61 acceptance into what the orchestrator has already proved by AI gate versus
what only a human at the native window can confirm. **The orchestrator must never self-declare the
NATIVE section** — present the AI-gate evidence, then ask the user to run `pnpm tauri dev`.

## Harness limitation (why these are USER CHECKPOINTs)

The mandatory browser harness (`pnpm dev` + `VITE_MOCK_IPC=1`) runs headless: the Browser pane
composites the canvas/overlay at 0×0, so the *live* diff-overlay and image-view flows (which mount
over the canvas/selection) cannot be visually self-verified there. Logic, wire shapes, mock seams,
and the pure render helpers ARE machine-verified (see AI-GATE below); final visual fidelity and the
native compositing path are human-perception items below.

---

## AI-GATE PROVED (already green — listed for context, do NOT re-ask the user)

Backend (Rust unit + integration; `cargo test -p bonsai-core {intraline,image_diff}`):
- Intraline: 9/9 in-module — tokenize classes, `token_diff` (single/multi-token, shared prefix/
  suffix, pure-insert, pure-delete, identical→empty), multibyte/code-point offsets, and the
  `MAX_INTRALINE_CHARS` skip.
- D1 wire invariant: `intraline=false` serializes with NO `spans` key (byte-identical to pre-P61a)
  — asserted in `diff.rs` (`!json.contains("spans")`).
- Image: 14/14 in-module (real git2 repos) — all three contexts (workdir unstaged+staged, commit
  incl. root→old None, compare), add/delete/rename via `orig_path`, 8 MiB over-cap → `None` +
  `*TooLarge`, the full MIME map, all 7 RFC 4648 base64 vectors + a 0..=255 round-trip, camelCase
  wire shape + request deserialization, bad-oid → `git`, path validation.
- Image integration (`tests/image_diff_cli.rs`, 2/2, added P61 QA): staged image add on an unborn
  HEAD → old None; workdir rename resolves old via `orig_path`.

Frontend (vitest):
- `segmentLine` (`intralineSegments.test.ts`): span slicing incl. start/end/multiple spans,
  multibyte + emoji code-point boundaries, out-of-range/overlap clamping.
- `isImagePath` (`imagePaths.test.ts`, added P61 QA): full raster set, case-insensitivity, SVG
  excluded, basename-only, dotfile/no-extension, last-segment-only.

Harness (AI gate, mock IPC): the "Highlight changes" toggle appears and fires exactly one refetch;
the image mode switcher (Side-by-side / Onion / Swipe) appears for image paths; the mock's
`added.` / `deleted.` / `huge.` seams drive the Added / Deleted / too-large placeholders; console
clean; `tsc`/`build` clean; `generate_handler!` = 135; no file over ~500 lines.

---

## NATIVE — user must confirm in `pnpm tauri dev` (open a real repo)

### P61a — Word-level / intraline highlighting
- [ ] In the diff overlay, a **"Highlight changes"** toggle is present beside the Diff/File/Split
      group.
- [ ] Toggle **ON**: on a *modified* line, only the changed sub-range is emphasised (e.g. editing
      `const x = 1;` → `const x = 42;` emphasises just `1`→`42`, not the whole line).
- [ ] Both **add** and **del** paired rows show their emphasis, and it is **legible** — the intraline
      tint stands out over the row's add/del background **and** over the selected-row gradient — in
      **both light and dark themes**.
- [ ] **Context** lines keep their normal **syntax highlighting** in both toggle states (D5: only
      changed lines lose syntax colour to intraline).
- [ ] **Multibyte / emoji** lines highlight at correct **code-point** boundaries — a line with an
      accented char or emoji before the edit still boxes the right characters (no off-by-one from
      byte/UTF-16 counting).
- [ ] Toggle **OFF**: the diff is visually identical to pre-P61 (plain add/del rows, syntax
      highlight intact).
- [ ] The toggle works the same in the **split** (side-by-side) diff view as in the unified view.

### P61b — Image diff (base64-over-IPC)
Use a real repo containing a small **PNG or JPG** that is modified across commits, plus a workdir edit.
- [ ] **Working-dir overlay** (unstaged and staged): opening the image change shows the actual image
      (NOT "Binary file" / no text-diff fallback).
- [ ] **Commit file diff** (select a graph node → the file in DiffBrowser): shows the image for that
      commit vs its first parent; a **root-commit** add shows only the New side.
- [ ] **Compare-with-HEAD file diff**: shows the image for HEAD (old) vs the chosen commit (new).
- [ ] The **old/new pairing is correct** in each of the three contexts (the "before" image is the
      parent/HEAD/index side, the "after" image is the commit/workdir/index side).
- [ ] **Side-by-side** mode shows two labelled images (Old / New).
- [ ] **Onion-skin** mode: the opacity slider crossfades New over Old smoothly across its full range.
- [ ] **Swipe** mode: dragging the vertical divider clips/reveals the New image over the Old.
- [ ] **Add** shows only the New image (Old = "Added"); **Delete** shows only the Old image
      (New = "Deleted"); a **rename** maps old→new correctly (old side resolved from the pre-rename
      path).
- [ ] An image **larger than 8 MB** shows the "larger than 8 MB — too large to preview" placeholder
      on that side (not a broken image), with the other side still rendering if in range.
- [ ] An **`.svg`** file still opens as a **TEXT diff** (with intraline available), NOT the image
      viewer (D4/OQ3).
- [ ] Real image blobs render correctly (not corrupt/blank) — confirms the base64→`data:` URL path
      end-to-end on the native webview.

### Regression
- [ ] Text diffs for non-image files are unchanged; switching between an image file and a text file
      in the same overlay swaps between the image viewer and the text diff cleanly (no stale toolbar
      or leftover image).
