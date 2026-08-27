# Spec-007 UI Contract — Replay Mode (animated history playback)

**Inputs:** `docs/specs/007-graph-replay-mode/spec.md` + `plan.md` (authoritative mechanics),
`docs/contracts/ui-reference.md` §2/§3/§4.2/§10.2/§11, `src/graph/revealFlash.ts`,
`src/styles/search.css` (`.commit-search` bar idiom), `src/styles/settings-primitives.css`
(`.settings-segment`), `src/styles/graph-filter.css` (fab/chip geometry).

**New tokens: none.** Every surface below uses existing tokens (`--bg-1/-2/-3`, `--border`,
`--text-1/-2/-3`, `--accent`, `--graph-canvas-bg`, `--radius` where noted). The frontier-pulse
alphas are code constants in `src/graph/replay/` (revealFlash precedent), not CSS tokens.

> **Amended 2026-08-27 (post-implementation review):** §2.2 fab radius + chip offset corrected,
> fab visibility gate clarified; §5 long-content rule simplified to ellipsis-only; palette-over-
> replay behavior recorded in §2.1. Each amendment is marked inline.

---

## 1. AC5 ruling — recorded downscope (FLAG for orchestrator)

A frontier pulse is **not**, on an honest reading, "leaves/blossoms sprout" (spec AC5). Ruling:
**v1 satisfies AC5 in reduced form** — the revealFlash-derived frontier pulse (§4 below) reads as
sprouting *under* Bonsai's leaf/blossom dots because the pulse halo ring expands from the dot,
but there is no scale-in glyph growth. A bespoke sprout animation (glyph scale-in 0.4→1.0 over
~250ms per frontier row, Bonsai style only) is a **recorded follow-up**, not v1. **The
orchestrator must note this downscope against AC5 when closing the spec.** This is the right
call for v1: the pulse shares one painter across both graph styles, keeps `drawBonsai.ts`
untouched, and stays inside the single-rAF budget.

## 2. Entry points

### 2.1 Command palette (primary discovery)

- Action label: **"Replay history"** (no ellipsis — it acts immediately, no intermediate dialog).
  Group: existing graph/view group in `buildPaletteActions`.
- Disabled (rendered but non-invocable, standard palette disabled style) when the loaded layout
  has 0 rows. Disabled hint text: `No commits to replay`.
- **No global keyboard shortcut.** Recommendation, recorded: ~150 commands already; a secondary
  "wow" mode does not earn chrome-level key real estate. Palette + fab only.
- **Amended:** Ctrl/Cmd+K remains openable *over* an active replay — the palette is a true modal
  above the overlay (its own focus scope; replay's trap only sees keys that bubble to the
  overlay root). Invoking "Replay history" while replay is already open restarts the replay
  from a fresh snapshot; acceptable v1 behavior, recorded.

### 2.2 Graph-pane fab (visible discovery)

A third fab in the §4.2 right-edge cluster (z-index 5, same 30×30 fab recipe — **8px radius,
matching the existing search/filter fabs** [amended: earlier "circular" wording was wrong] —
same bg, border, hover, focus ring). Order right→left at the top edge: **search · filter ·
replay** (replay leftmost, `right: 90px`; it spans **[90, 120]px from the pane's right edge**).

- **Filter-chip clearance (amended, geometry verified):** the active filter chip anchors its
  right edge at `right:` and grows leftward, so clearing the replay fab requires the chip's
  right edge left of the fab's *left* edge plus the 8px gap: `right: 120 + 8 = 128px`. The
  contract's earlier "one fab slot + 8px" (→ 90) placed the chip's right edge on the fab's
  right edge and grew it *under* the fab — wrong. **`right: 128px` is canonical**, scoped to
  when the replay fab is present (`.graph-pane:has(.graph-replay-fab)`); the below-bar chip
  variant is unaffected (the cluster hides while the search/Ask-history bar is open).
- Glyph: `▶` (11px, `--text-2`; `--accent` on hover). `aria-label="Replay history"`,
  `title="Replay history"`.
- **Visibility gate (amended for precision):** when no layout is loaded at all
  (`graph === null` — no repo / still loading) the fab is **hidden with the rest of the
  cluster**, exactly like the search/filter fabs. When a layout is loaded but empty/unborn
  (0 rows), the fab stays **visible and `disabled`**: 40% opacity glyph,
  `title="No commits to replay"` — discoverability over tidiness.
- While replay is open the fab cluster is underneath the overlay and inert (overlay covers the
  pane); no state change needed on the fabs themselves.

## 3. ReplayMode overlay + transport bar

### 3.1 Placement & geometry (ASCII)

```
┌─ graph pane (overlay, absolute inset 0, z above fabs: z-index 6) ──────┐
│  replay canvas (fills remaining height; own scroller)                  │
│  bg: var(--graph-canvas-bg)  ← seamless with normal graph styles       │
│                                                                        │
│  [truncated-history indicator, if the layout is capped — unchanged     │
│   position, stays visible per plan decision 11]                        │
├─ transport bar (bottom, height 40px, density-invariant) ───────────────┤
│ ⏸  ────────●──────────────  1× 2× 4×   412 / 1,204 · Jun 2024    ✕   │
└────────────────────────────────────────────────────────────────────────┘
```

- Overlay: `position: absolute; inset: 0; z-index: 6` inside the **graph-pane host element**
  (the pane column root, not the canvas host) so it also covers the commit-search bar /
  Ask-history panel region when one is open at entry — those controls stay mounted but are
  visually covered and non-interactive, making the plan's "filter/reload controls are inert"
  hold by construction. The overlay sits above the §4.2 stack; the native scrollbar covered
  here belongs to the frozen working canvas — the overlay brings its own scroller, so the
  "never cover the scrollbar" rule is not violated in spirit: the visible scrollbar is the
  overlay's own.
- **Canvas metrics inherit density.** The replay canvas draws rows with the working graph's
  `metrics` prop (row height, lane width, dot radius per §4 / `panelDensity`), so cozy/compact
  affect the replayed rows automatically — no replay-specific metric values exist.
- Transport bar: bottom-anchored, **height 40px**, `background: var(--bg-1)`,
  `border-top: 1px solid var(--border)` (the `.commit-search` in-pane bar idiom, mirrored to the
  bottom edge). Padding `0 12px`, item gap `12px` (8px within clusters).
  **Density-invariant** — a mode overlay is outside the §3 density scopes, same precedent as the
  §10.2 toast column. Identical in cozy and compact.
- Files (per plan): `src/graph/replay/ReplayMode.tsx` (container),
  `src/graph/replay/ReplayTransport.tsx` (presentational bar),
  `src/styles/graph-replay.css` (all styles below).

### 3.2 Transport anatomy (left → right)

1. **Play/Pause button** — 28×28, radius 6px, transparent bg, `--text-1` glyph 13px
   (`▶` paused/finished, `⏸` playing — glyphs, not color, carry state). Hover `--bg-3`;
   `aria-label` toggles `Play` / `Pause`; `aria-pressed` NOT used (it's a toggle of labels, not
   a pressed-state button). Hidden entirely under reduced motion (§6).
2. **Scrubber** — flexible, min-width 160px. Track: 4px tall, radius 999px, `--bg-3`
   (dark ≥3:1 edge vs `--bg-1` via its 1px `--border` outline; light same recipe). Fill
   (elapsed portion): `--accent`. Thumb: 14px circle, `--accent` fill, 2px `--bg-1` inner
   border; hit target extends to the full 24px bar-row height (§3.1 hit floor). Implement as a
   custom div-slider (house pattern), `role="slider"`, `aria-label="Replay position"`,
   `aria-valuemin=0`, `aria-valuemax=totalMs`, `aria-valuenow=playheadMs`, and
   **`aria-valuetext="Commit 412 of 1,204 — Jun 2024"`** (position + month/year of the frontier
   commit, never raw ms).
3. **Speed selector** — the `.settings-segment` recipe verbatim (min-height 24px, radiogroup
   semantics), options `1×` `2×` `4×`, `aria-label="Playback speed"`. Hidden under reduced
   motion (§6). The radiogroup is **one Tab stop** (arrow keys move within it, native radio
   roving behavior); the overlay's focus trap must not surface each radio as its own stop.
4. **Progress label** — `--text-3` 11.5px, tabular-nums: `412 / 1,204 · Jun 2024`
   (revealed / total · frontier commit month-year). `aria-hidden` (the slider valuetext carries
   it); truncates with ellipsis first when the bar is narrow.
5. **Close button** — 24×24 (§3.1), `✕` glyph `--text-2`, hover `--bg-3` + `--text-1`,
   `aria-label="Exit replay"`, `title="Exit replay (Esc)"`.

All controls: standard focus ring — 2px `--accent`, 1px offset, `:focus-visible` only.

### 3.3 Keyboard & focus

- On entry, focus moves to the **Play/Pause button** (or the scrubber under reduced motion).
  Focus is trapped within the overlay (transport controls only; the canvas is not focusable —
  replay is presentational, no row selection). The overlay root carries `role="dialog"` with
  an accessible name and **`aria-modal="true"`**.
- **Space / K**: play ↔ pause. **← / →**: scrub by `max(1, n/500)` rows (pauses playback if
  playing — see §5 scrub rule; arrow keys on a focused speed radio keep native radiogroup
  navigation). **Shift+←/→**: 10× that step. **Home / End**: jump to start / full reveal.
  "Start" = playhead 0 = the oldest commit revealed (1 visible row, never a blank canvas —
  blank exists only pre-play). Ruled 2026-08-27 reconciling this line with the model convention.
  **Esc**: exit. Tab order: play → scrubber → speed (one stop) → close.
- On exit, focus returns to the **replay fab** (or, if entry was via palette and the fab is
  disabled/absent, to the graph canvas host — the standard palette focus-restore path).

### 3.4 Microcopy

- Palette action: `Replay history` · disabled hint `No commits to replay`.
- Empty-repo no-op notice (if invoked anyway): toast, info tone, `Nothing to replay — this
  repository has no commits yet.`
- Finished state label swap (§5): button `title="Replay again"`.
- Reduced-motion hint line (§6): `Drag the slider to move through history.`

## 4. Reveal & frontier-pulse visuals

- **Unrevealed rows are absent, not dimmed.** Bare `--graph-canvas-bg` above the frontier. One
  line of why: the plan's painter only paints `[c, n)` and never edits `draw.ts`; dimming would
  require painting everything and reads as "disabled graph," not "history not yet written."
- **Frontier pulse** (rows revealed this tick, both graph styles): reuse the revealFlash motion
  math by reference — row-tint + dot halo ring, peak alpha **0.30 dark / 0.24 light**, rise
  ~90ms then quadratic ease-out, total **600ms** (shorter than revealFlash's 900ms: pulses
  overlap at 2×/4× and must not smear), ring `+1 → +5px`. Painted by `replayPaint.ts` AFTER
  `drawGraph`, tint color = the row's lane color at the stated alpha (lane hues all clear 3:1;
  the pulse is decorative emphasis, never a meaning carrier — position on the scrubber carries
  the state).
- **Bonsai variant:** identical pulse; under Bonsai the halo expands from the leaf/blossom dot
  and the warm backdrop, which is the v1 "sprout" reading (§1 downscope). No `drawBonsai.ts`
  changes.
- Cap concurrent pulses at ~24 rows/tick; beyond that (4× over dense history) draw the tint
  only, no rings — keeps the single-rAF paint bounded.

## 5. States

| State | Behavior / visuals |
|---|---|
| **Entering** | Overlay fades in 120ms opacity ease-out (none under reduced motion). Cutoff = n (blank canvas + transport). Autoplay starts immediately when `canPlay` (plan decision 4). |
| **Playing** | Play glyph → `⏸`. Auto-follow pins the frontier row at the viewport **top**; revealed history fills downward. User wheel/drag on the overlay scroller disengages follow (no visual chrome for the flag — pressing Play re-engages, matching plan decision 6). |
| **Paused + scrub** | Zero rAF scheduled. Dragging/keying the scrubber repaints once per change. **Scrubbing while playing pauses playback** (matches the spec's "scrubbing while paused" language; resuming is one Space press). |
| **Finished** | Playhead at end, full graph revealed, no frames scheduled. Play button shows `▶` with `aria-label="Replay again"` — pressing it resets to cutoff = n and plays. Close exits as always. |
| **Exit** | Overlay unmounts (no exit animation — instant, per spec "exits instantly"), `active=true` on the working canvas; scroll/selection untouched by construction. Focus restore per §3.3. |
| **Single-commit repo** | Entry works; one row reveals at t=0; scrubber spans it; controls all consistent (no special casing). |
| **Empty/unborn** | Entry disabled (§2); palette no-op notice if raced. |
| **Truncated (100k cap)** | The existing truncated indicator remains visible above the transport bar; replay covers loaded rows only. |
| **Theme switch / resize mid-replay** | Re-render at current cutoff (theme via `themeVersion` prop; resize via `useCanvasResizeObserver`), no state loss. |
| **Long content** | **Amended:** progress label truncates with ellipsis (flex `0 1 auto`, min-width 0) before the scrubber shrinks below its 160px floor; the label is `aria-hidden` and the slider valuetext always carries the full text. The earlier "<420px pane width: drop the label entirely" rule is withdrawn — it would need a container query for a marginal win; ellipsis-only is blessed. |

Both themes: every surface above is token-composed (`--bg-1` bar on `--graph-canvas-bg` canvas)
and needs no per-theme values beyond the pulse alphas already split dark/light. Contrast:
transport labels `--text-3` on `--bg-1` pass 4.5:1 in both themes (existing pairing, §2);
scrubber fill `--accent` vs track `--bg-3` clears 3:1 in both themes (existing accent-on-surface
pairing, as used by the graph rail viewport marker).

## 6. Reduced motion

`prefers-reduced-motion` ⇒ `canPlay` is false (plan decision 7). Transport shows **scrubber +
progress label + close only** — Play/Pause and the speed segment are **hidden, not disabled**
(a permanently disabled play button reads as broken). A hint line replaces them, `--text-3`
11.5px, left slot: `Drag the slider to move through history.` No entry fade, no pulses (static
single-paint reveal — the revealFlash reduced path precedent), zero rAF ever.

## 7. Harness fixtures & verification split

Mock-harness states to exercise (`VITE_MOCK_IPC=1`, fixture layouts unchanged — no new IPC):
normal multi-branch fixture; empty/unborn repo; single-commit; the large/100k-row fixture
(scrub granularity + label formatting); `emulateMedia({ reducedMotion: 'reduce' })`.

**Harness-verifiable (AI gate):** overlay mount/unmount + working-canvas freeze, exit restores
scroll/selection, scrub-while-paused updates the reveal count, slider aria semantics, reduced-
motion control set, the plan's no-new-rAF-schedules assertion, both themes via `resize_window`
colorScheme.

**USER CHECKPOINT (headless pauses rAF firing):** playback smoothness on a large repo,
auto-follow feel, pulse aesthetics in both graph styles, speed-change seamlessness.

## 8. Flags for orchestrator

1. **AC5 downscope** (§1) — pulse-only v1; bespoke Bonsai sprout is a recorded follow-up.
2. Scrub-during-play pauses playback (§5) — my recommendation, recorded here; trivial to flip.
3. No keyboard entry shortcut (§2.1) — palette + fab only; recorded recommendation.
4. `ui-reference.md` manual insertions (do not rewrite the file): add to §4.2 the replay fab as
   the third member of the fab cluster (leftmost) and the replay overlay at `z-index: 6`; add a
   one-line §4 note that replay reuses revealFlash motion at 600ms/+5px.
