# P63 — Forge signals on the graph: native USER CHECKPOINT checklist

The badge **rendering is on `<canvas>`**, which the headless browser harness cannot composite (0×0),
so the actual pixels + canvas click/hover are native-only. The AI gate already passed: `forgeBadges.ts`
+ `useForgeSignals` pure helpers unit-tested (`pnpm vitest run` **197**), `tsc` 0 errors, `pnpm build`
green, and a `pnpm dev:mock` pass confirmed the Settings toggles exist + default OFF, enabling them
persists to settings and runs the hook with a clean console, and `?forge=off` is silent (no error UI).
Command count after P63: **155**.

## Verify in `pnpm tauri dev` against a real connected GitHub repo (with open PRs + CI)

Prereq: complete the P62 checkpoint first (a real PAT connected).

1. **Toggles** — Settings → Graph → **Show PR badges** and **Show CI status** are present and OFF by
   default. Enabling each redraws the graph.
2. **PR badges** — on branch-tip rows whose branch has an OPEN PR, a PR badge shows the state
   (open = green, draft = grey outline) and `#number`. Branches without an open PR show none.
3. **CI status dot** — each relevant tip shows a status dot colored by the rollup: success (green
   check), failure/error (red ×), pending (dot), neutral (dash); **none ⇒ no dot** (not a grey blob).
4. **Unpushed / gone tip** — a LOCAL branch tip that isn't on the remote shows **no CI dot** and does
   NOT blank the other rows' badges (the per-sha 404 is omitted, not fatal — this was the P63a fix).
5. **Compact mode** — turning on Graph → compact hides both badges even when their toggles are on.
6. **Clutter** — with everything toggled on, confirm the row isn't unreadably busy; overflow past the
   ref-band collapses into the existing `+n` affordance (pill + its badges move as one unit).
7. **Click → PR** — clicking a PR badge opens the right pane's **Pull requests** tab to that PR's
   detail. Clicking the same badge again re-opens it (seq bump).
8. **Hover** — hovering a PR/CI badge shows a tooltip (PR title / CI summary).
9. **Refresh** — after a Fetch/Pull, and after refocusing the window (past the 60 s TTL), badges
   re-fetch and reflect new PR/CI state. Failures are silent (a transient forge outage never toasts
   or blocks the graph).
10. **No forge** — on a non-GitHub repo (or with the forge disconnected), enabling the toggles is a
    no-op with no errors (badges simply don't appear).
