# P84 — Reveal in graph (single-click sidebar → scroll + flash) — UI contract

Owner: ui-designer. Status: draft. Sibling: architect P84 contract (data/IPC — the oid a ref
points at is already in `GraphLayout`/refs; no new backend needed for the in-layout case).

## 0. Summary of the interaction

Single-clicking a sidebar row (local branch / remote branch / tag / stash) **reveals** that ref's
target commit in the centre graph: scroll the canvas so the commit is on screen, **select** it
(right panel updates, persistent), and play a **transient flash** on that row so the eye lands on
it. **Double-click keeps its existing meaning** (checkout for local branches). This spec covers the
flash, the reveal↔selection relationship, a11y, and the not-in-loaded-history edge case only.

## 1. Placement & component decomposition

The flash is painted on the existing commit-graph `<canvas>` — no new DOM overlay. Reveal is a new
imperative entry point on the graph, mirroring the existing `selectedIndex`-driven scroll-into-view
(`GraphCanvas.tsx:518-530`, `viewport.ts:scrollRowIntoView`).

| File | Change | Why not an existing file |
|---|---|---|
| `src/graph/GraphCanvas.tsx` (container, already large) | Add **one** prop `revealFlash: { oid: string; nonce: number } \| null` and a small effect + a `flashStartRef`/rAF flash loop. No render-body growth. | Reveal must drive the same paint pipeline; a sibling component cannot reach the canvas ctx. Keep the addition to props + one effect + one draw call. |
| `src/graph/revealFlash.ts` **(new, ~60 lines)** | Pure flash math: `flashAlpha(elapsedMs, reducedMotion)` and `flashRingRadius(elapsedMs, baseRadius, reducedMotion)`. Plain in → plain out, no canvas import (mirrors `viewport.ts` / `headGuide`). | Keeps the animation curve unit-testable headless (rAF never fires in the harness) and off the container's line budget. |
| `src/graph/draw.ts` | Add `drawRevealFlash(ctx, node, x, y, alpha, ringRadius, theme, m)` — one row-bg overlay rect + one dot halo ring. Small, sits beside `drawHeadGuide`. | Same imperative draw layer as every other graph visual. |
| `src/components/RevealAnnouncer.tsx` **(new, ~25 lines)** | Always-mounted visually-hidden `role="status" aria-live="polite"` span; text set by the reveal handler. | Reuses the app's established split (§9 / §10 of ui-reference use exactly this pattern); the status region must be permanently mounted, not conditionally rendered, so SRs pick up the change. |
| `src/components/sidebar/rows.tsx` | Add `onReveal(target)` single-click handler to `BranchRow`, `RemoteRow`, `ConfiguredRemoteRow`'s tracking rows, `StashRow`, and tag rows (`TagsSection.tsx`). Double-click checkout is unchanged. | These are the small presentational row files; the handler is one line each. |

The container (`RepoWorkspace.tsx`) owns the wiring: sidebar `onReveal` → resolve oid → set
selection + bump `revealFlash.nonce` → set announcer text. `nonce` (monotonic counter) lets the
**same ref clicked twice** re-flash (a new oid alone would not change identity).

## 2. Flash treatment (canvas)

Reveal draws **two** coordinated marks on the target row, both in the accent family so they read as
one "found it" gesture and stay coherent with the persistent selection ring (which is already
`--accent`, ui-reference §4):

1. **Row-background pulse.** A full-width overlay rect over the row (`0 → vp.width`, height
   `m.rowHeight`), filled with `theme.accent` at an **animated `globalAlpha`** (peak → 0). It is
   drawn in a new **Pass 2.5**, i.e. *after* the selection/hover row backgrounds
   (`draw.ts:324-334`) and *before* edges, so it layers on top of the `--selection` fill the
   revealed+selected row already has. Restore `globalAlpha = 1` immediately after.
2. **Dot halo ring.** A stroked circle around the commit avatar, `theme.accent`, `lineWidth` 2px,
   radius growing from `m.avatarSelRingRadius + 1` to `m.avatarSelRingRadius + 6`, same animated
   alpha. Drawn in Pass 4 right after the existing selection ring (`draw.ts:391-397`) so it sits
   outside it. This is the primary attractor when the row background is busy with edges/pills.

**No new token.** The flash reuses `theme.accent` (already resolved into `Theme`, `colors.ts:73`)
with alpha compositing. See §8 for the token proposal that is deliberately *not* being added now.

### 2.1 Peak alpha (contrast / clash check)

- Row-bg pulse peak `globalAlpha`: **0.30 dark / 0.24 light**. Over the revealed row's `--selection`
  fill this is decorative (not a text-contrast surface — the commit message keeps its own `--text-1`
  on the composited bg, which stays ≥ 4.5:1 because accent at ≤0.30 shifts luminance only slightly).
  The pulse is a *motion/shape* cue, not the sole carrier — the halo ring and the announcement carry
  the meaning too, satisfying "never colour alone".
- Halo ring at peak alpha vs the row background: `--accent` (#4f8cff dark / #2f6fe4 light) at 2px is
  the same edge already cleared at **≥3:1** for the selection ring in ui-reference §4, so the reveal
  halo inherits that pass.

## 3. Duration, easing, pulse count

- **One** pulse (professional/calm; GitKraken-style single ping — not a strobe).
- **Total 900ms.** Alpha curve: fast rise to peak by **~90ms**, then **ease-out fade to 0 by 900ms**
  (`alpha = peak * (1 - easeOut(t'))` for `t'` over the 90→900ms tail; `easeOut(x)=1-(1-x)^2`).
- Halo radius grows linearly `+1 → +6px` over the full 900ms (expanding-ring feel), independent of
  the alpha fade.
- Runs on a **self-contained rAF loop in GraphCanvas** that calls `paintNow()` each frame until
  `elapsed ≥ 900ms`, then one final clear paint. It only touches transform/opacity-equivalent
  (globalAlpha) + a radius — no layout, no input blocking, and it composites over the normal draw so
  it never contends with scroll paints (a scroll during the flash simply repaints with the current
  alpha).

### 3.1 `prefers-reduced-motion`

No animation. The reveal still scrolls + selects, and instead of the pulse it draws a **static**
overlay at a lower steady alpha (**0.18 dark / 0.14 light**) plus a static halo ring at fixed radius
`m.avatarSelRingRadius + 3`, **held for 1200ms then cleared in a single step** (no interpolation,
two paints total: on and off). `flashAlpha`/`flashRingRadius` take a `reducedMotion` flag and return
these constants. The media query is read once via `window.matchMedia('(prefers-reduced-motion: reduce)')`
in the container and passed down (do not re-query per frame).

## 4. Coexistence with selection

**Recommended (spec this): reveal also selects.** Single-click sets `selectedIndex` to the target
row (right panel shows that commit) **and** fires the flash. Selection **persists**; the flash is
**transient** and leaves no residue — after 900ms/1200ms the row shows only the normal selected
styling (`--selection` bg + `--accent` selection ring). This matches the existing keyboard path
where arrow-selecting a commit already scrolls it into view and updates the panel; reveal is the
mouse-from-sidebar equivalent, plus the attention flash.

The existing selection scroll-into-view effect (`GraphCanvas.tsx:518-530`) already handles the
scroll when `selectedIndex` changes, so reveal-of-an-in-view target needs no extra scroll code — it
sets selection (→ scroll) and bumps the flash nonce (→ flash). One caveat: if the ref's oid is
**already** the selected row, `selectedIndex` does not change, so the scroll effect no-ops (correct —
it's already in view) but the flash **must still fire** because `nonce` changed. That is why the
flash is nonce-driven, not selection-driven.

## 5. Interaction & keyboard

- **Single-click** a branch/remote/tag/stash row → reveal (scroll + select + flash).
- **Double-click** a local branch → checkout (unchanged, `rows.tsx:67-70`). Guard against the flash
  fighting the checkout: double-click naturally produces a preceding single click, so the reveal +
  a same-row selection are harmless before checkout; no debounce needed.
- **Keyboard:** Enter stays **checkout** for local branches (existing muscle-memory contract,
  `rows.tsx:59`). See the flagged ambiguity in §10 — recommendation is to **not** hijack Enter and
  to add reveal to the command palette instead ("Reveal selected ref in graph") rather than a new
  row keystroke, keeping the sidebar's key model unchanged.
- Reveal has **no** confirmation and is non-destructive (pure navigation) — no ConfirmDialog.

## 6. Accessibility

- **Announcement** via the always-mounted `RevealAnnouncer` (`role="status" aria-live="polite"`,
  visually hidden through the existing `.sr-only` utility — `tokens-and-base.css:169`; do **not**
  inline the clip recipe). Set its text on every reveal (append a zero-width nonce or toggle a
  trailing space so identical consecutive reveals still announce):
  - In-layout: **`Revealed <ref> at commit <short-oid>`** — e.g. `Revealed origin/main at commit a1b2c3d`.
  - Not in loaded history: **`<ref> is not in the loaded history`** — e.g. `v1.2.0 is not in the loaded history`.
  - `<ref>` is the human label: local branch name, `origin/name` for remotes, the tag name, or
    `stash@{n}`. `<short-oid>` is the 7-char abbreviation.
- The flash itself is decorative (`aria-hidden` canvas) — meaning is carried by selection + the live
  region, never by the flash colour alone.
- Honour `prefers-reduced-motion` (§3.1).
- Hit target: sidebar rows already meet the ≥24px row floor (ui-reference §3.1); unchanged.

## 7. Edge cases & harness states

- **oid not in the current (truncated/paged) layout** → no scroll, no flash, no selection change.
  Announce `<ref> is not in the loaded history`. **Also** show a subtle toast (a toast system
  exists — `usePushToast`, `ToastContext.ts:11`; tones in `Toasts.tsx:7`): tone **`info`**, text
  **`"<ref>" isn't in the loaded history yet. Load more commits to reveal it.`**, dedupe `key`
  `reveal-miss` so rapid clicks don't stack. On the streamed/paged path the oid may still be
  arriving; do not auto-load — just inform.
- **Empty graph / unborn HEAD** → same as not-in-layout (announce + toast), no crash.
- **Detached HEAD row / worktree rows** are out of scope for reveal (they are not ref→commit
  navigations in the same sense); leave their click behaviour unchanged.

### Mock-IPC fixture states to add (`src/ipc/mock/`)
1. **In-layout hit** — a branch/tag whose oid is a visible node near the top and one far down
   (exercises scroll-into-view + flash).
2. **Already-selected target** — reveal a ref pointing at the currently selected commit (asserts
   nonce-driven re-flash without a selection change).
3. **Not-in-layout miss** — a tag pointing at an oid absent from the fixture layout (announce +
   info toast).
4. **Paged/truncated** — a `totalRows`-extended streamed layout where the target oid is beyond the
   loaded window (miss path on the streamed branch).
5. **Reduced-motion** — verify via emulated `prefers-reduced-motion` that the static-highlight
   branch of `flashAlpha` is used.

**USER CHECKPOINT (headless harness cannot judge):** the *feel* of the pulse — timing, whether the
ease-out fade reads as "one calm ping" vs a flicker, and that the expanding halo is noticeable but
not distracting over a dense graph — because rAF does not fire in the hidden Browser pane
(frame-timing is always a checkpoint here). The pure `flashAlpha`/`flashRingRadius` curves and the
announcement/toast text ARE harness-verifiable via unit tests + a single scripted paint.

## 8. New-token proposal (NOT applied — needs reconciliation)

A distinct `--reveal-flash` hue was considered and **rejected for v1**: reusing `--accent` with
alpha keeps the reveal in the same visual family as the selection ring (restraint over addition) and
introduces zero new tokens. If a future pass wants the flash to be distinguishable from selection at
a glance, add:

| Token | Dark | Light | Use |
|---|---|---|---|
| `--reveal-flash` | `#5ea0ff` | `#2f6fe4` | reveal row-pulse + dot-halo overlay (alpha-composited) |

**Coordination note (per task):** I could not confirm via a ListAgents tool whether session
`bonsai-9c` is concurrently editing `docs/contracts/ui-reference.md`. Per the standing "never run
two ui-designers on ui-reference.md concurrently" rule, I did **not** touch `ui-reference.md` in
this pass. Because the accepted design adds **no** token and **no** new reusable motion primitive
(it composites `--accent` and reuses the §9 reduced-motion + `.sr-only` patterns), `ui-reference.md`
needs **no** edit for P84. If the `--reveal-flash` token above is later adopted, it must be added to
ui-reference §2 (both themes) with a measured contrast note, by whichever session then owns the
file — reconcile at that point.
