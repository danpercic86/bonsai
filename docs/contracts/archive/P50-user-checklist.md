# P50 — USER CHECKPOINT checklist (native-only)

These items require the native Tauri window or human perception and CANNOT be self-declared by the
orchestrator (the AI gate only proves argv/parsing correctness + mock-driven UI). Run via
`pnpm tauri dev` against a REAL repo (and one large repo, e.g. a clone of a big OSS project).

## Search
- [ ] `git` binary present on PATH (path/content search shells out); if absent, path/content search
      surfaces a clear error toast, message/author search still work (git2 revwalk).
- [ ] On a large repo (20k+ commits): message/author/`all` search feels instant (< ~150 ms).
- [ ] Content search (`-S` literal) on the large repo returns in reasonable time and does NOT freeze
      the UI; typing a new query supersedes the previous result (no stale flash).
- [ ] Content regex (`-G`) with a valid pattern returns matches; an invalid regex shows an error
      toast, not a crash.
- [ ] Matching commit dots are visibly highlighted in the graph; next/prev jumps scroll the graph to
      each match and select it; a match outside the loaded window shows the "not in current view" hint.
- [ ] Spot-check correctness: a few queries return the SAME commits as `git log --grep/--author/-S/-G`
      in a terminal on the same repo.
- [ ] Windows: no console window flashes for path/content search (CREATE_NO_WINDOW).

## Command palette
- [ ] Ctrl-K (Windows/Linux) / Cmd-K (macOS) opens the palette on the active tab; Esc closes it.
- [ ] Fuzzy typing filters actions; Enter runs the highlighted row; arrow keys move the highlight.
- [ ] Jump-to-branch / jump-to-tag scroll the graph to that ref; jump-to-commit by hex prefix works.
- [ ] A destructive-capable action reached from the palette still opens its confirm dialog (nothing
      destructive fires directly).
- [ ] Palette does not fight IME / dead-key composition when typing.

## List filtering
- [ ] Typing in the Branches / Remotes / Tags filter narrows that list live; clearing restores it.
- [ ] Empty-match state reads clearly ("No branches match …").
- [ ] Esc in a focused filter clears it without closing unrelated overlays; global shortcuts do not
      fire while typing in a filter.
