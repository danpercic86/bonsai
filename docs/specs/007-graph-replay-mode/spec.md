# Replay Mode — Animated History Playback

**Status:** done (AI gate green 2026-08-27; USER CHECKPOINT verified 2026-08-27 by the user in `3a71951`; AC5 shipped in reduced form)
**Created:** 2026-08-26

## Problem

Bonsai renders history as a static structure; there's no way to *watch* a repository grow — a
delightful, shareable way to present a project's story (as Gource and similar tools popularized).
This is a secondary "wow" view, not a daily driver, but it differentiates and pairs naturally
with the Bonsai tree theme.

## Goals

- A self-contained **replay mode** over the current graph: commits (nodes/edges/refs) appear
  progressively over time, playing from the oldest visible commit to the newest.
- Transport controls: play/pause, a scrub bar (drag to any point in history), and a speed
  selector.
- Exiting replay returns to the normal graph exactly as it was (same scroll, same selection).
- Works with both graph styles; in the Bonsai style, growth reads organically (leaves/blossoms
  "sprout" as commits appear), reusing the theme's established motion language.
- Honors `prefers-reduced-motion`: no autoplaying motion — a manual scrubber only.
- No animation work is scheduled when paused, finished, or exited.

## Non-goals

- No export/recording (video/GIF) in v1.
- No backend/Rust changes; replay animates data the graph already has.
- No editing or Git operations while in replay — it is a read-only presentation mode.
- No change to the normal graph's rendering or performance when replay is not active.
- No per-file/tree visualization (Gource-style file spheres) — this replays the commit graph
  itself.

## User-facing behavior

- An entry point (per ui-designer — e.g. a command-palette action and/or a control near the
  graph) starts replay on the currently loaded view (respecting active filters).
- The graph clears to its starting state and commits appear in order over wall-clock time, edges
  and ref pills materializing with them; the view follows the growth.
- A transport bar offers play/pause, a scrubber mapping the full history, and speed steps.
  Scrubbing while paused shows the history state at that point.
- Escape (or a close control) exits instantly back to the normal graph with prior scroll and
  selection intact.
- With reduced motion set, entering replay shows the scrubber-driven static view — dragging
  reveals history up to that point; nothing auto-animates.

## Acceptance criteria

1. Given replay starts, then commits appear progressively in order; play/pause, scrub, and speed
   all function; the reveal state always matches the scrubber position.
2. Given the user exits replay (Escape or control), then the normal graph returns with prior
   selection and scroll position intact, and no replay artifacts remain.
3. Given `prefers-reduced-motion`, then no auto-motion occurs — the scrubber is the only way to
   advance, and it works.
4. Given replay is paused, finished, or exited, then no animation frames are scheduled (verified
   no idle rAF activity).
5. Replay works in standard and Bonsai styles; the Bonsai variant sprouts leaves/blossoms
   consistent with the theme's existing motion.
6. Entering/leaving replay does not mutate the working layout, filters, or selection state.
7. Playback feel and smoothness on a large repo are confirmed in the native app (USER
   CHECKPOINT).

## Edge cases & error states

- **Empty/unborn repo:** replay entry is unavailable or a no-op with a gentle notice.
- **Single-commit repo:** replay trivially shows the one commit; controls remain consistent.
- **Truncated graph (100k cap):** replay covers what's loaded; the truncated indicator remains
  honest.
- **Filter change is not possible mid-replay** (controls that would reload the graph are
  inert/hidden in replay); exiting first is required.
- **Window resize / theme switch during replay:** the replay view re-renders correctly at the
  current scrub position.
- **Very large histories:** scrub granularity stays usable (scrubbing maps to positions without
  per-commit stalls).

## Open questions

None — ordering (topological display order, timestamps for pacing) and entry-point placement are
design decisions for `/plan` and ui-designer.
