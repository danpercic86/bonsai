# P69 — USER CHECKPOINT checklist (settings redesign, P69a–P69l)

Everything below needs the native Tauri window or human perception, so the
orchestrator cannot pass it. The AI gate is green at `a13b729`: tsc 0 ·
vitest **1977 / 162 files** · e2e **156 passed / 1 skipped** · eslint 0 errors ·
ratchet exit 0 · **+0 IPC commands across the whole milestone** (see `TODO.md` P69).

This is a **batched** checklist: eleven increments shipped without a native run
between them, so items below span the whole redesign, not just the last one.

Run `pnpm tauri dev` and open a **real repository you do not mind editing** —
several items below write to its `.git/config`. A scratch clone is ideal.

> **Why so much of this is native-only.** The browser harness is headless:
> `requestAnimationFrame` never fires and screenshots fail outright, so nothing
> about scroll feel, motion, or real-display legibility can be checked there. And
> the mock IPC writes git config **to memory** — so every "did it really write to
> the repo" question below is unanswerable without the native window.

---

## 1. The headline feature: the header identity menu

- [ ] There is an identity control in the header showing the initials of whoever
      you commit as. **Is it discoverable?** If you had not been told it exists,
      would you have found it? (This replaced a section buried in Settings.)
- [ ] The initials are legible at the real size on your actual display. They are
      **10px inside a 22px circle** — the smallest text the app draws. If this is
      squinty, say so; it is a token change, not a redesign.
- [ ] The name/email it shows matches what git actually resolves. Compare with:

```bash
git config --get user.name && git config --get user.email
```

- [ ] Open a repo whose identity comes only from your **global** config (no local
      `user.*` block). The menu must still show it and say the source is global —
      git resolves local-over-global, and the old buried badge showed nothing at
      all in this, the ordinary case.

### The part that cannot be faked in the harness

- [ ] Click a different identity in the menu. Then, in that repo:

```bash
git config --local --list
```

- [ ] `user.name` and `user.email` really changed in the repo's own config file,
      and the header trigger updated to match.
- [ ] Make a commit and confirm the author on it is the identity you picked.
- [ ] Clicking the identity that is **already ticked** does nothing at all — no
      write, no toast, no new `user.*` block. (Deliberate ruling: it is a no-op
      regardless of whether the tick came from local or inherited global config.)

---

## 2. The two-pane Settings shell

- [ ] `Ctrl+,` (macOS `Cmd+,`) opens Settings. It should work even with the
      cursor in the commit message box.
- [ ] The shell reads well at your real window size — 880px wide, capped at
      `min(660px, 100vh − 64px)`. Try it with the window **small**; the rail is
      supposed to collapse to a horizontal strip below 720px.
- [ ] Both themes. Switch with the theme control and look again.
- [ ] `Esc` closes it — and if another overlay is stacked above it, `Esc` closes
      **that** first, not Settings.
- [ ] Every category's content fits its heading; nothing looks stranded in the
      wrong place.
- [ ] Arrow keys move the rail selection **without** activating it; `Enter` or
      `Space` activates. (Deliberate: auto-activation would fire a git-config read
      every time focus passed over Git config.)

---

## 3. Settings search (the newest increment, P69k)

- [ ] Settings opens with focus already in the search box; typing filters
      immediately with no perceptible lag.
- [ ] Search a setting by a word that is **not** in its visible label — e.g.
      `spend`, `fetch`, `identity`. It should still be found.
- [ ] A result is **live**: change a toggle or a number directly in the result
      list and it applies at once, without navigating anywhere first.
- [ ] The rail shows a per-category match count and the categories with hits are
      **emphasised** rather than the empty ones dimmed. **Look hard at this one** —
      hit and no-hit differ only by text colour plus the digit, and the design
      ruling on it is still open (see §7). Is the distinction actually visible?
- [ ] First `Esc` clears a non-empty query; a second `Esc` closes the dialog.
- [ ] A query that matches nothing says so, quotes what you typed, and offers
      **Clear search** that puts you back on a category.
- [ ] Known gap, no action needed — just confirm it matches this description:
      searching `graph` finds 5 settings but **highlights nothing**, because all
      five match on hidden keywords while their labels read "Row height", "Lane
      width", "Compact rows". The fix is specced but not implemented.

---

## 4. Git config is visibly per-repository — and really writes

- [ ] The Git config category is unmistakably scoped to the open repo, with a
      **Local / Global** switch, and you can tell at a glance which file you are
      about to edit.
- [ ] With **no** repo open it shows a real empty state offering to open one —
      not a dead form and not a bare sentence.
- [ ] Edit a curated key (say `core.autocrlf`) at **Local**, then verify on disk:

```bash
git config --local --get core.autocrlf
```

- [ ] Add a custom key under **Advanced**, confirm it lands in `.git/config`, then
      remove it and confirm it is gone.
- [ ] Switch the scope to **Global** and confirm edits go to your **user** config,
      not the repo's. (Both levels are in-memory in the harness — this is the only
      place the distinction is real.)
- [ ] Open a repo with **no** identity set and try to commit. The failure should
      offer a route into Settings that lands on the identity fields with the name
      field focused.
- [ ] Then, with that deep link still fresh, **type in the search box.** Focus must
      stay in the search box. (A defect found in review: focus used to jump into
      the `user.name` field mid-typing, and that field commits on blur — so the
      tail of a search query could be written into the repo's committer name. It is
      fixed and e2e-covered, but this is the one place it is real.)

---

## 5. The two defect fixes that only a restart can prove

- [ ] Type a slider's own **minimum** value into its number field — e.g. Graph →
      Row height, minimum 24: type `24`. It must end up 24, not 240. (Before P69c
      the field clamped mid-keystroke and its own minimum was unreachable.)
- [ ] Change any setting and **immediately quit the app** — within a second, before
      the debounce would normally flush. Relaunch: the change must have survived.
- [ ] Known limitation, not a bug to report: the teardown flush is dispatch-only,
      so a **hard kill** (Task Manager / `kill -9`) can still lose the last change.
      A normal quit is what is being tested here.

---

## 6. Nothing went missing

The redesign moved ~59 controls out of eleven flat sections into seven
categories, and moved identity out of Settings entirely.

- [ ] Go through the categories and confirm **every setting you actually use** is
      still there and still does what it did. The taxonomy in
      `docs/contracts/P69-settings-ui.md` is the reference if you want to check
      exhaustively.
- [ ] Anything you had to hunt for is a finding worth reporting — the point of the
      redesign was that you should not have to.

---

## 7. Judgement calls that are yours, not defects

These shipped a particular way and are waiting on your word:

- [ ] **A3 — the AI gate note.** `Turn on "Enable AI features" above to change
      these.` still ships verbatim; `ui-designer` prefers `These take effect once
      AI features are on.` (it states the dependency without re-naming the consent
      switch that P68g deliberately hardened). Left frozen precisely because it is
      consent-adjacent. Your call.
- [ ] **Is the rail's hit emphasis strong enough?** This is the one item where the
      harness genuinely cannot substitute for your eyes. It ships `--text-1`, and
      `ui-designer` **upheld** that in P69l: the count is a number you read, so
      4.5:1 applies, and `--accent` fails it in both themes (worst on a *selected*
      item, where it measures ~3.5–3.7:1 — worse than the dimming it replaced).
      So the answer is deliberately quiet: the digit (`0` vs `N`) plus a
      whole-item luminance lift.
      If that lift is too subtle on your display, the sanctioned fix is to render
      **nothing** instead of `0` on zero-count items, making presence-vs-absence
      the carrier. **Not** a colour change — accent-on-selection is now prohibited
      house-wide. Just say "too subtle" and I will land the presence version.
- [ ] **A1 — identity CRUD placement.** Switching lives in the header menu;
      creating, renaming and deleting identities stayed in Settings. Confirm that
      split matches how you actually work.
- [ ] **A4 — update-check egress.** Whether the About category should state that
      checking for updates contacts a remote server.

---

## 8. Platform-conditional (cannot be checked from Windows)

- [ ] **macOS and Linux only:** focus rings. The shell uses CSS `:has()` with an
      `@supports not selector(:has(*))` fallback, because the inputs are
      `opacity: 0` — on an engine that does not parse `:has()` the result would be
      **no visible focus indicator at all**, not a degraded one. Tab through the
      Settings controls on WebKit (macOS) and `webkit2gtk` (Linux) and confirm
      every focused control shows a ring.
