# P68e — AI activity dock (UI contract)

Owner: ui-designer. Implementer: senior-dev. **Design-only document — no application code is
touched by this file.**

Parent contract: `docs/contracts/P68-ai-conflict-streaming.md` (§7 §E, D2/D5/D8/D14, §5.2
`useAiRuns`, §8.5 mock seams). Design system: `docs/contracts/ui-reference.md` (§1 layout, §2
tokens, §3 type/spacing, §8 states) — **updated in this same pass** with the dock's geometry
(new §9) and a contrast note on `--text-3` (§2).

This contract answers, in the user's own words, six questions at a glance:
**is something happening · how long · what is it doing · what will it cost · can I stop it · does
it need me?** Every geometry, copy and state decision below is traceable to one of those.

Sources read (verified, not guessed): `src/styles.css` (`.workspace-host` 1406-1412,
`.workspace-toolbar` 1414-1426, `.panes` 1456-1463, `.pane-divider` 1472-1481, `.pill` 437-458,
`.btn-primary/.btn-icon` 462-507, `.btn-secondary/.btn-danger` 5186-5222, `.toast*` 590-654,
`.header-progress` + `@keyframes header-progress-sweep` 1270-1291, `.section-label` 2110-2116,
`.status-section` 2261-2265, `.row-action` 2481-2509, `.rp-actions*` 2516-2564, the `--rp-*` cozy
block 2028-2052 and `[data-density='compact']` 2066-2085, `.hook-output*` 4521-4554,
`.dialog-input` 4494-4512, `.mono` 126-129), `src/components/PaneDivider.tsx` (90),
`src/components/AiOutputPanel.tsx` (140, D14 — untouched),
`src/components/StatusConflictsSection.tsx` (167), `src/components/OpBanner.tsx` (merge arm
131-160), `src/components/WorkspaceRightPanel.tsx:206` (`data-density` precedent),
`src/components/repoWorkspace/useWorkspaceKeyboard.ts` (212-300 — the shortcut host and its
`typing` guard), `src/App.tsx` (865-913 global shortcuts), `src/components/paletteActions.ts`
(`hint`/`group`/`keywords` shape), `src/components/RepoWorkspace.tsx:2240-2265` (the AI palette
entries), `src/ipc/mock/handlers/aiStream.ts` (33-35 seams, 88-108 emit/log/partial, 174-228 the
run script), `src/components/Toasts.tsx` (25).

---

## 0. Decisions this contract makes (U1–U14)

**U1 — The dock is a disclosure, not a pane.** Collapsed it is a single 30px status bar; expanded
it is that bar plus a resizable body. It renders `null` until the first run exists (parent
contract §7.1), so a user who never uses AI never pays a pixel.

**U2 — The collapsed bar is the primary deliverable.** The reported failure was "no feedback",
and the cheapest fix is a bar the user cannot miss: status word + subject + **live elapsed** +
cost + **the latest output line** + Cancel. Expanding is for detail, never for basic reassurance.

**U3 — No height animation.** Changing the dock height re-lays out `.panes`, which re-lays out
the graph canvas. A 120 ms height transition would force ~8 canvas relayouts of a 20k-row graph
per toggle. Collapse/expand/resize snap instantly; only opacity fades (≤150 ms) are allowed.

**U4 — Nothing in the log is announced to screen readers.** A streaming NDJSON log in an
`aria-live` region is hostile. One tiny visually-hidden `role="status"` region announces **status
transitions only** (§11).

**U5 — Enter sends the reply; Shift+Enter is a newline; Ctrl/Cmd+Enter also sends.** The reply is
a chat answer, the user is blocked, and Enter is the idiom they expect. Ctrl/Cmd+Enter is kept as
a superset so the parent contract's test ("submits on click and on Ctrl+Enter") still passes and
commit-box muscle memory works.

**U6 — Focus is never stolen.** `awaitingInput` may auto-**expand** the dock (once per run) but
only focuses the reply box when the user is demonstrably idle (§4.4). `Ctrl/Cmd+Shift+A` is the
deliberate way in, and it is bound **before** the `typing` guard so it works from the commit box.

**U7 — Partial output is quarantined.** It lives behind a closed disclosure, in muted dashed-border
mono, with a fixed sentence saying Bonsai will not apply it, and it has no Copy and no Apply
control (D2 scope, §11.5 of the parent contract).

**U8 — Colour never carries meaning alone.** Every status is a **word** (`Running`, `Needs you`,
`Ready`, `Failed`, `Cancelled`, `Stopping…`) with a glyph that matches the P68d conflict-row
glyphs (`✨ ? ✓ ⚠ ⊘`). Tints and hues are redundant reinforcement.

**U9 — `--text-3` is not used for any text in the dock.** Measured: `--text-3` on `--bg-1` is
**3.38:1** (dark) and **2.96:1** (light) — below AA for text. The dock's muted role is
`--text-2` (**7.9:1** dark on `--bg-0`, **6.4:1** dark on `--bg-2`, **4.9:1** light on
`--bg-0`), exposed as `--ai-dock-meta` so a future global fix can rebind it. See §12-F1.

**U10 — No new global chrome.** No toolbar button, no sidebar entry. Entry points are: the run
itself (the dock reveals), the P68d conflict-row affordances, two command-palette entries, and
one shortcut. Nothing in the header or sidebar is displaced.

**U11 — No terminal emulator.** The user asked "maybe show the terminal directly?". Answer: no.
A PTY means ANSI parsing, resize protocol, and an interactive shell surface that contradicts
D1 (Rust owns the subprocess) and D10 (read-only tools). The log **is** the terminal view — it is
the child's stdout/stderr, line for line, including `⚙ tool(arg)` and `stderr:` lines.

**U12 — The dock follows `panelDensity`.** One density setting for the whole app;
`data-density` on the dock root, `--ai-dock-*` swapped by one rule block (the P67c `--rp-*`
precedent).

**U13 — Cost shows `$—` while unknown, never a guess.** `costUsd` only lands on `turnEnd`/`done`,
so a long first turn has no cost to show. `$—` + a title explaining when it appears is honest;
extrapolating a number is not. See §12-B1.

**U14 — Six small files, not three.** The parent contract named three; the header row and the
awaiting-input block are each substantial and independently testable, and the pure formatters
belong outside React. Every file lands ≤ ~170 lines (§9).

---

## 1. Placement, geometry, anatomy

### 1.1 Placement

```
+-----------------------------------------------------------------------+
| Header bar (40px)                                                     |
+-----------------------------------------------------------------------+
| .workspace-toolbar (40px, flex: none)                                 |
+-----------------------------------------------------------------------+
|            |                                |                         |
| .sidebar   | .graph-pane (canvas)           | .right-panel            |
|            |          .panes  (flex: 1)     |                         |
|            |                                |                         |
+-----------------------------------------------------------------------+
| .ai-dock   (flex: none, full width, third child of .workspace-host)   |
+-----------------------------------------------------------------------+
```

`.workspace-host` is already `display:flex; flex-direction:column; min-height:0`
(`styles.css:1406`). The dock is its **third element child** (`flex: none`, `overflow: hidden`),
rendered in `RepoWorkspace.tsx` immediately after the `.panes` closing tag. Full width is
deliberate: P67b reclaimed ~115 px in the right panel and the dock must not eat it back.

### 1.2 Collapsed (the "is something happening?" answer)

Height: `var(--ai-dock-header-h)` = **30 px** cozy / **28 px** compact. One row, no body,
no resizer.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│▔▔▔▔▔▔▔▔▔▔▔  2px indeterminate accent sweep, only while a run is active  ▔▔▔▔▔│
│ ⌃  [✨ Running]  src/locales/de.json   ⚙ Read(src/i18n/index.ts)   1:07  $—  Cancel │
└──────────────────────────────────────────────────────────────────────────────┘
  ^chevron  ^status pill   ^subject (dir muted)   ^latest log line   ^elapsed ^cost ^danger
```

Order, left → right: collapse chevron · status pill · subject · **latest output line** (collapsed
only) · turn counter (only when `turn ≥ 2`) · elapsed · cost · `Review proposal` (ready single
run) · `Cancel` (active) · `✕` dismiss (terminal).

Collapsed variants:

| Dock state | Pill | Subject | Latest line | Elapsed | Right controls |
|---|---|---|---|---|---|
| 1 run, running | `✨ Running` | the path / `3 conflicts` | shown, ellipsized | ticking | `Cancel` |
| 1 run, cancel clicked | `✨ Stopping…` | same | shown (frozen source) | ticking | `Cancel` disabled |
| 1 run, awaiting input | `? Needs you` | same | hidden | ticking | `Answer` (accent) |
| 1 run, ready | `✓ Ready` | same | hidden | frozen | `Review proposal` · `✕` |
| 1 run, failed | `⚠ Failed` | same | hidden | frozen | `✕` |
| 1 run, cancelled | `⊘ Cancelled` | same | hidden | frozen | `✕` |
| >1 run | most urgent status (order: `awaitingInput` > `running` > `failed` > `ready` > `cancelled`) | `N AI runs` | latest line of the longest-running active run | longest active run, else most recent | `Cancel` only if exactly one active run, else none |

Aggregate cost when >1 run = the **sum** of the listed runs' `costUsd` (separate processes ⇒
summing is correct; A10 only forbids summing *within* one run).

### 1.3 Expanded — single run

Height: `var(--ai-dock-header-h)` + `height` prop (persisted, 120–600 px, default 180).

```
┌═ 4px resize grip (row-resize, keyboard-focusable) ═══════════════════════════┐
│ ⌄  [✨ Running]  src/locales/de.json          turn 2   1:07   $0.0238  Cancel │  30px header
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑ 1,204 earlier lines trimmed                                    (sticky)    │
│ session 8f21 · model sonnet · tools: Read, Grep, Glob                        │
│ ⚙ Grep(pattern: "plural")                                                    │
│ ⚙ Read(src/i18n/index.ts)                                                    │
│ The two sides disagree about the German plural form. Both files use ICU…     │  body,
│ stderr: warning: budget 80% consumed                                         │  scrolls
│                                                        ┌──────────────────┐  │
│                                                        │ ↓ Jump to latest │  │
│                                                        └──────────────────┘  │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 1.4 Expanded — bulk run (queue above the log)

```
┌═════════════════════════════════════════════════════════════════════════════┐
│ ⌄  [✨ Running]  3 conflicts                     turn 1   0:42   $—  Cancel │
├─────────────────────────────────────────────────────────────────────────────┤
│ ✓  src/locales/  de.json          Ready      [ Review ]                     │  queue,
│ …  src/locales/  fr.json          Working…                                  │  own
│ ⚠  packages/…/   messages.json    Failed  no result block returned  [Retry] │  scroll
├─────────────────────────────────────────────────────────────────────────────┤
│ batch 1/1: 3 files (18,204 B)                                               │  log
│ ⚙ Read(src/locales/de.json)                                                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.5 Expanded — awaiting input (the most consequential state)

```
┌═════════════════════════════════════════════════════════════════════════════┐
│ ⌄  [? Needs you]  src/locales/de.json           turn 1   2:15   $0.0238     │  no Cancel
│                                                                     Cancel  │  ← still present
├─────────────────────────────────────────────────────────────────────────────┤
│ … log, unchanged, still scrollable …                                        │
├─────────────────────────────────────────────────────────────────────────────┤ ← 1px --warning 40%
│ (?) Claude needs your answer                                                │  warning tint
│ Should the German plural form use "Einträge" or "Eintraege"?                 │  --text-1
│ ┌─────────────────────────────────────────────────────────────┐             │
│ │ Type your answer for Claude…                                │  [ Send ]   │  48px min
│ └─────────────────────────────────────────────────────────────┘             │
│ Enter sends · Shift+Enter for a new line                                    │  --text-2 11px
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.6 Multiple runs — the run strip

Shown only when `runs.length > 1`, between header and body, `role="tablist"`, height 24 px cozy /
22 px compact, `overflow-x: auto`, `gap: 4px`, `padding: 0 var(--ai-dock-pad-x)`.
Each chip: glyph + short label (basename for a path run, `3 conflicts` for bulk), 11px/600,
radius 6px, `--bg-2` idle / `--bg-3` + 1px `--accent` when `aria-selected`. Long labels
ellipsize at 120 px with `title` = full label.

### 1.7 Spacing and sizes (4/8/12/16/24 scale only)

| Property | Cozy | Compact |
|---|---|---|
| header height | 30px | 28px |
| header horizontal padding | 12px | 8px |
| header gap between items | 8px | 6px |
| control height (Cancel / Send / Review / Retry / dismiss / chevron) | 28px | **24px** (AA floor) |
| pill | 2px 8px, 11px/600, radius 999px | same |
| log font / line-height | 12px / 18px | 11px / 16px |
| log padding | 6px 12px | 4px 8px |
| queue row height | 26px | 24px |
| queue max-height | 40% of body, min 52px | same |
| ask block padding | 8px 12px | 6px 8px |
| reply textarea min / max height | 48px / 120px | 36px / 96px |
| resize grip | 4px visual, 8px pointer area | same |

---

## 2. The header row — exact copy

Element order and classes are part of the contract.

| Class | Content | Style | Notes |
|---|---|---|---|
| `.ai-dock-toggle` | `⌃` collapsed / `⌄` expanded | icon button, `--ai-dock-ctl-h` square | `aria-expanded`, `aria-controls="ai-dock-body"`, `aria-label="AI activity"` |
| `.ai-dock-status` | glyph + word (table below) | pill; `data-status` | glyph `aria-hidden` |
| `.ai-dock-subject` | `<span class="ai-dock-dir">src/locales/</span><span class="ai-dock-name">de.json</span>` or `3 conflicts` | mono 12px; dir `--ai-dock-meta` shrinks + ellipsizes, name `--text-1` `flex:none` | `title` = full path. Use `splitPath()` from `StatusFileRow.tsx` — do not re-implement |
| `.ai-dock-activity` | latest log line | mono 11px `--ai-dock-meta`, 1 line, ellipsis | **collapsed only**; hidden when `status !== 'running'` |
| `.ai-dock-turn` | `turn 2` | 11px `--ai-dock-meta` | only when `turn >= 2`; `title="Each reply from Claude is one turn"` |
| `.ai-dock-elapsed` | `1:07` | mono 12px `--ai-dock-meta`, `font-variant-numeric: tabular-nums` | `title="Elapsed"` while active, `title="Took 2:14"` when terminal |
| `.ai-dock-cost` | `$0.0238` or `$—` | mono 12px `--ai-dock-meta`, tabular | `$—` title: `Cost appears when Claude finishes a turn` |
| `.ai-dock-review` `.btn-primary` | `Review proposal` | primary | only `status === 'ready' && files.length === 0` |
| `.ai-dock-answer` `.btn-primary` | `Answer` | primary | **collapsed only**, `status === 'awaitingInput'`; expands + focuses the reply box |
| `.ai-dock-cancel` `.btn-danger` | `Cancel` → `Stopping…` | danger | present for `running`/`awaitingInput`; disabled once `cancelRequested` |
| `.ai-dock-dismiss` `.btn-icon` | `✕` | icon | terminal only; `aria-label="Dismiss this run"`, `title="Remove from the AI activity dock"` |

Status pill strings (**locked copy**):

| store status | pill glyph + label | `data-status` | hue (tint 14% / border 40% / glyph 100%) |
|---|---|---|---|
| `running` | `✨ Running` | `running` | `--accent` |
| `running` + `cancelRequested` | `✨ Stopping…` | `stopping` | `--ai-dock-meta` (no hue — it is ending) |
| `awaitingInput` | `? Needs you` | `awaiting` | `--warning` |
| `ready` | `✓ Ready` | `ready` | `--success` |
| `failed` | `⚠ Failed` | `failed` | `--danger` |
| `cancelled` | `⊘ Cancelled` | `cancelled` | `--ai-dock-meta` |

Pill label colour is **always `--text-1`** (12.1:1 light / 9.1:1 dark over the tint) — the hue
lives in the border and the glyph, so no label ever falls under AA. The glyph sits at ≥3:1
(graphics bar) in both themes; it is `aria-hidden` and duplicated by the word (U8).

**Elapsed timer after a run ends:** it **freezes at the terminal value** and never resets or
disappears; the `title` switches to `Took m:ss`. It is driven by `Date.now() - startedAt` while
active and by `endedAt - startedAt` once terminal (the store's `tick` interval is already cleared
when no run is active — D5, so the frozen value costs nothing).

**Progress cue:** `.ai-dock-progress`, a 2px strip on the dock's top edge, reusing
`@keyframes header-progress-sweep` (`styles.css:1287`). Rendered only while ≥1 run is
`running`/`awaitingInput`. `aria-hidden`. Under `prefers-reduced-motion: reduce` the sweep is
replaced by a static full-width 2px `--accent` bar at 60% opacity (§7).

---

## 3. The log body

Element: `<ol class="ai-log" id="ai-dock-log" tabindex="0" aria-label="AI output">`, one
`<li class="ai-log-line" data-kind="…">` per retained line. `overflow-y: auto; overflow-x: hidden;
white-space: pre-wrap; overflow-wrap: anywhere; tab-size: 4;` — mono, `var(--ai-dock-log-font)` /
`var(--ai-dock-log-line-h)`, background `var(--ai-dock-log-bg)` = `--bg-0` (the app's
content-surface convention: chrome is `--bg-1`, content is `--bg-0`, in both themes).

Wrapping, not horizontal scrolling: an assistant paragraph can be 2000 chars, and
`.hook-output-body`'s `white-space: pre` + 2-axis scroll is right for a lint dump but wrong for
prose. Tool/stderr lines wrap identically.

**Line kinds.** `data-kind` drives colour; the glyph/prefix carries the meaning (U8):

| kind | matched by | colour | redundant cue |
|---|---|---|---|
| `text` | default | `--text-1` | — |
| `tool` | text starts with `⚙ ` | `--ai-dock-meta`, the `⚙` glyph in `--ai-dock-tool` (`--accent`) | the `⚙` glyph |
| `stderr` | starts with `stderr: ` | `--danger` | the literal `stderr: ` prefix |
| `meta` | starts with `» `, `session `, `batch `, `rate limit: `, `summary: `, `system/` | `--ai-dock-meta`, 11px | the `»`/word prefix |

**Where the kind comes from — data request (§12-A1).** `AiRunLogLine` currently has
`{ seq, text }`. Preferred: `useAiRuns` (P68d, not yet landed) sets
`kind: 'text' | 'tool' | 'stderr' | 'meta'` **once at ingest**, so the dock stays presentational.
Fallback if P68d has already landed unchanged: a pure `classifyLogLine(text)` in
`src/components/aiDockFormat.ts`, unit-tested there. Either way the classification rule is the
table above and lives in exactly one place.

**2000-char truncation.** Rust's `truncate_text` yields **exactly** `MAX_EVENT_TEXT` chars and
appends `…`. The log renders a chip after any line whose `text.length === 2000`:
`<span class="ai-log-trunc">truncated</span>` — 11px/600, `--ai-dock-meta`, `--bg-2`, radius
999px, padding 0 6px, `title="This line was cut off at 2,000 characters"`. Export
`AI_EVENT_TEXT_MAX = 2000` from `aiDockFormat.ts` with a comment naming
`bonsai_core::ai::stream::MAX_EVENT_TEXT` as its mirror.

**500-line cap.** When `logDropped > 0`, the first `<li>` is `.ai-log-dropped`, `position:
sticky; top: 0; background: var(--ai-dock-log-bg);` so it stays visible while scrolling:
`↑ 1,204 earlier lines trimmed`, `title="Bonsai keeps the last 500 lines of AI output"`. Number
formatted with `toLocaleString()`.

**Autoscroll + escape.** Stick-to-bottom by default. `onScroll`: if
`scrollHeight - scrollTop - clientHeight > 24` → unstick; if `<= 4` → re-stick (a 24/4 hysteresis
band so a 1-line growth spurt cannot re-stick you). While unstuck and new lines arrive, show
`.ai-log-jump`: `position: absolute; right: 12px; bottom: 8px;` pill button, 24px tall, 11px,
`--bg-2` + 1px `--border`, label `↓ Jump to latest`, `aria-label="Jump to latest AI output"`.
Click → scroll to bottom + re-stick. Fades in at 100 ms opacity (no transform). It disappears
the moment stickiness returns. The button requires `.ai-dock-body { position: relative; }`.

**Empty states inside the log:**

| Condition | `.ai-log-empty` copy |
|---|---|
| `status === 'running'`, `log.length === 0` | `Starting Claude…` |
| `streamLogEnabled === false` | `Live output is off — turn on "Stream AI output" in Settings to see it here.` |
| terminal run with `log.length === 0` | `No output was captured.` |

The `streamLogEnabled` prop is required: without it, a user who turned `aiStreamLog` off sees an
empty log and concludes the feature is broken again — the exact bug class P68 exists to remove.

**Partial output (cancel / fail).** Rendered below the log, inside `AiActivityLog.tsx`, only when
`partialText !== null`:

```
▸ Unfinished output (not usable)                       ← <button aria-expanded>
  Stopped before Claude finished. This text is incomplete — Bonsai will not apply it.
  ┌ dashed --border ─────────────────────────────────────────────────────┐
  │ <pre class="ai-dock-partial-body">  mono 11px --ai-dock-meta, --bg-2 │
  └──────────────────────────────────────────────────────────────────────┘
```

Closed by default. No Copy button, no Apply button, no editable textarea — the deliberate
difference from `AiOutputPanel`'s editable card. Max-height 120px with its own scroll.

---

## 4. The awaiting-input affordance

File: `src/components/AiActivityAsk.tsx`. Rendered inside the body, **below** the log, `flex:
none`, only when `status === 'awaitingInput'`.

### 4.1 Visual distinction

- Container `.ai-dock-ask`: `border-top: 1px solid color-mix(in srgb, var(--warning) 40%,
  var(--border)); background: color-mix(in srgb, var(--warning) 14%, var(--bg-1));` padding
  `var(--ai-dock-ask-pad)`, `display: flex; flex-direction: column; gap: 8px`.
- `.ai-dock-ask-glyph`: 16px circle, `background: var(--warning); color: var(--bg-0);` content
  `?`, `aria-hidden`. **Contrast checked:** `--bg-0` on `--warning` = 6.4:1 dark, 4.8:1 light.
- `.ai-dock-ask-label`: `Claude needs your answer` — 11px/600, uppercase, letter-spacing 0.08em,
  **`--text-1`** (not `--warning`: `--warning` as small text is only 3.5:1 in light theme —
  §12-F2).
- `.ai-dock-ask-question`: the question verbatim, UI font 13px `--text-1`, `white-space:
  pre-wrap`, `max-height: 96px; overflow: auto`, user-selectable.
- When collapsed and any run is `awaitingInput`, the collapsed bar gets
  `data-attention="true"` → the same 14% `--warning` background, so the bar reads as "needs you"
  without expanding. No blinking, ever.

### 4.2 Reply control

`<textarea class="ai-dock-ask-input">` — UI font 13px (prose, not code), `--bg-2`, 1px
`--border`, radius 6px, padding 6px 8px, min/max height per §1.7, `resize: none`, autogrow up to
max. Placeholder: `Type your answer for Claude…`. `aria-label="Your answer to Claude"`,
`aria-describedby` → `.ai-dock-ask-hint`.
`.ai-dock-send` `.btn-primary`, height `var(--ai-dock-ctl-h)`, padding 0 12px, label `Send`.
`.ai-dock-ask-hint`: `Enter sends · Shift+Enter for a new line`, 11px `--ai-dock-meta`.

### 4.3 Keyboard

| Key | Behaviour |
|---|---|
| `Enter` | send if `draft.trim() !== ''`; otherwise no-op (no error flash) |
| `Shift+Enter` | newline |
| `Ctrl/Cmd+Enter` | send (same guard) — muscle memory from the commit box |
| `Esc` | blur the textarea back to the log; **never** collapses the dock and **never** cancels the run (Esc must not be able to destroy a blocked run's context) |
| `Tab` | textarea → `Send` → out of the dock |

On send: `onReply(key, draft)`, clear the draft, disable the textarea and `Send`
(`Send` label becomes `Sending…`) until `status` leaves `awaitingInput`. The store already
appends `» answered (n bytes)` to the log — the UI adds no synthetic line.

### 4.4 Arrival behaviour (U6, the delicate part)

When a run transitions into `awaitingInput`:
1. If the dock is collapsed → expand it, **once per run** (guard on a
   `autoExpandedForRef: Set<runKey>` in `RepoWorkspace`), and select that run in the strip.
2. Announce via the polite region: `Claude needs your answer about src/locales/de.json`.
3. `useAiRuns` pushes an `info` toast: `Claude has a question — open the AI activity dock to
   answer.`
4. Focus the reply textarea **only if** `document.activeElement` is `null`, `document.body`, or
   already inside `.ai-dock`. If the user is in *any* input, textarea, select, or contenteditable
   (commit message, search, a dialog), focus stays put.
5. The row affordance in `StatusConflictsSection` shows `?` (P68d) — clicking it expands the dock
   **and** focuses the reply box (an explicit user action, so focusing is correct there).

`Ctrl/Cmd+Shift+A` — "AI activity": expand the dock; if any run is `awaitingInput`, select it and
focus the reply box; else focus the log. Registered in
`src/components/repoWorkspace/useWorkspaceKeyboard.ts` **before** the `typing` guard (the
`Ctrl+F` / `Ctrl+K` precedent at 236-250) so a user mid-commit-message can reach it. Add the row
`Ctrl/Cmd+Shift+A — AI activity dock` to `src/components/ShortcutOverlay.tsx` beside the other
`Ctrl+Shift` entries.

---

## 5. Bulk runs — the per-file queue

File: `src/components/AiRunQueue.tsx`. `<ul class="ai-run-queue">` above the log, `flex: none`,
`max-height: 40%` of the body with its own `overflow-y: auto`, rendered only when
`files.length > 0`.

Row `<li class="ai-run-queue-row" data-status="…">`, height `var(--ai-dock-queue-row-h)`,
`display: flex; align-items: center; gap: 8px; padding: 0 var(--ai-dock-pad-x)`:

| Column | Content | Style |
|---|---|---|
| glyph | `…` pending · `✓` ready · `⚠` failed | `flex: none`, 14px, hue per status, `aria-hidden` |
| path | `splitPath()` → muted dir + `--text-1` name | mono 12px, dir shrinks/ellipsizes, `title` = full path |
| status word | `Working…` · `Ready` · `Failed` | 11px `--ai-dock-meta`, `flex: none` |
| reason | `failed` only: the `reason` string | 11px `--danger`, 1 line, ellipsis, `title` = full reason, `flex: 1 1 auto; min-width: 0` |
| action | `Review` (ready) · `Retry` (failed) · nothing (pending) | `.btn-secondary`, height `var(--ai-dock-ctl-h)`, padding 0 10px |

Hover: row background `--bg-2`. `:focus-visible` on the action button: 2px `--accent`, offset 1px.
Rows are **not** clickable as a whole (the button is the target) — avoids an ambiguous hit area
next to the Review button.

Copy: `Review` → `aria-label="Review AI proposal for <path>"`, `title="Open the proposal in the
center pane"`. `Retry` → `aria-label="Retry AI resolution for <path>"`.

### 5.1 Discoverability of a finished proposal (a hard requirement)

The reported bug was "Propose & review does nothing" because the proposal opens in the **center**
pane while the button is in the right panel. Four redundant paths, in increasing effort:

1. **Toast** (P68d): single → `AI proposal ready for <path> — opened for review`; bulk →
   `AI proposals ready for <n> files — review them from the AI activity dock`.
2. **Row affordance** (P68d): `✓ review` on the conflict row.
3. **Dock, single run**: `Review proposal` button in the **header** (this is why
   `AiActivityRun.paths` is needed — §12-A2). Plus the one-line hint
   `.ai-dock-hint`: `Proposal is open in the center pane.` shown for 1 run / `status === 'ready'`
   / `files.length === 0`, 11px `--ai-dock-meta`, in the body above the log.
4. **Dock, bulk run**: the queue's per-row `Review` buttons — the only place a user can reach
   proposal #2 and #3 at all.

---

## 6. Cancel semantics in the UI

**On click, before any IPC resolves:**
- `Cancel` becomes `disabled`, label `Stopping…` (`aria-disabled` not needed — it is truly
  disabled), `aria-label="Stopping the AI run"`.
- The pill becomes `✨ Stopping…`, `data-status="stopping"`, hue drops to `--ai-dock-meta`.
- Elapsed keeps ticking (it is still running — pretending otherwise would be a lie).
- The progress sweep keeps running.
- The log gets **no synthetic line**. The dock never invents output.
- Announce: `Stopping the AI run for src/locales/de.json`.

**On the `cancelled` terminal event:** pill `⊘ Cancelled`, elapsed freezes, `Cancel` is replaced
by `✕`, and the partial disclosure (§3) appears if `partialText !== null`. Announce:
`AI run cancelled. Nothing was changed.`

A cancel issued before `runId` arrives is queued by the store (D8) — the UI is identical, because
it renders `cancelRequested`, not the IPC state.

Bulk `Cancel all` (from the conflicts header / OpBanner, P68f) targets the same run key, so the
dock's own `Cancel` and that button are the same operation and cannot disagree.

**No confirmation dialog for Cancel.** It stops work in progress and destroys nothing on disk
(D4: the AI never writes), and a modal in front of a run the user is trying to stop would be
hostile. The destructive-confirmation rule applies to *applying* a proposal, which is the
existing `resolve_conflict_text` path and out of scope here.

---

## 7. Motion

| Element | Motion | Reduced-motion |
|---|---|---|
| collapse / expand / resize | **none** (instant) — U3 | n/a |
| `.ai-dock-progress` | 1.1s `header-progress-sweep` (existing keyframes) | static full-width 2px `--accent` at 60% opacity |
| `.ai-dock-ask` on arrival | `opacity 0 → 1`, 120ms ease-out | no fade (instant) |
| `.ai-log-jump` | `opacity`, 100ms ease-out | no fade |
| `.ai-dock-status` hue change | `background-color`/`border-color` 120ms ease-out | none |
| queue row status change | none | n/a |

All animations are opacity/colour only; no transform, no layout property. One
`@media (prefers-reduced-motion: reduce)` block at the end of the dock's CSS section holds all
four overrides. Note: the app currently has **no** reduced-motion block at all (`header-progress`
and `skeleton-pulse` are ungated) — the dock adds the first one; retro-fitting the other two is
out of scope (§12-F3).

---

## 8. Resize and persistence

- `.ai-dock-resizer` on the dock's **top** edge: `height: 4px; cursor: row-resize; flex: none;
  background: transparent;` with `::after { content:''; position:absolute; inset:-2px 0; }` for an
  8px pointer band (matching the `.pane-divider` 4px hit-strip idiom).
- Hover: `background: color-mix(in srgb, var(--accent) 40%, transparent)`.
  `:focus-visible`: `outline: 2px solid var(--accent); outline-offset: -1px` (the
  `.pane-divider:focus-visible` rule, `styles.css:1478`).
- `role="separator" aria-orientation="horizontal" tabindex="0"
  aria-label="Resize AI activity dock" aria-valuenow={height} aria-valuemin={120}
  aria-valuemax={600}`.
- Keyboard: `ArrowUp` +8px, `ArrowDown` −8px (grows upward = larger dock), each committing
  immediately. `Home` → 120, `End` → the effective max.
- **Double-click resets to 180px** (`title="Drag to resize · double-click to reset"`).
- Clamp: `Math.min(600, Math.max(120, Math.round(window.innerHeight * 0.6)))` as the effective
  max, evaluated at pointer-down — the dock can never swallow the graph on a short window.
- Commit on `pointerup`/`pointercancel` only (one settings write per drag), via
  `onResizeHeight(next)`; `App.tsx` debounces the `setUiSettings` patch as it already does for
  pane widths.
- **Collapsed:** the resizer is not rendered and not focusable (nothing to resize). The height is
  remembered and restored on expand.
- Persisted: `aiDockHeight` (120–600, default 180) and `aiDockCollapsed` (default false) — both
  already specified in the parent contract §8.3. Nothing else persists: the selected run, the
  log, and the scroll position are session state and die with the run.

**Implementation route — extend `PaneDivider` (recommended).** Add `side: 'ai-dock'` to
`PaneDividerProps`, switch the axis (`clientY`, `ArrowUp`/`ArrowDown`,
`aria-orientation="horizontal"`) behind that value, and keep the existing normalize/capture
logic. Cost: ~20 lines in a 90-line file, one drag idiom in the app. Alternative if the reviewer
prefers zero churn in a shared component: a new `AiDockResizer.tsx` (~60) — but then two
drag implementations must be kept in sync, which is exactly what the house style avoids.

---

## 9. Component split, props, line budgets

| File | New/edit | Budget | Responsibility | Props (sketch) |
|---|---|---|---|---|
| `src/components/AiActivityPanel.tsx` | NEW | ~150 | Dock shell: root `<section>`, `data-density`, progress strip, resizer wiring, run strip (tablist + roving tabindex), body composition, announce region. No local state except the run-strip roving index. | `AiActivityPanelProps` (§9.1) |
| `src/components/AiActivityHeader.tsx` | NEW | ~130 | The one header row: chevron, pill, subject, activity line, turn, elapsed, cost, Review/Answer/Cancel/dismiss. Pure. | `{ run, collapsed, aggregate, onToggleCollapsed, onCancel, onDismiss, onReview, onAnswer }` |
| `src/components/AiActivityLog.tsx` | NEW | ~170 | Log list, kind styling, truncation chip, dropped note, stick-to-bottom + jump button, log empty states, **and** the partial-output disclosure. Owns `stickRef`/`scrollRef` + the disclosure's open flag. | `{ log, logDropped, status, partialText, streamLogEnabled, hint }` |
| `src/components/AiRunQueue.tsx` | NEW | ~110 | Per-file rows for a bulk run. Pure. | `{ files, onReviewFile, onRetryFile }` |
| `src/components/AiActivityAsk.tsx` | NEW | ~110 | Question + reply textarea + Send; owns the draft and all keyboard handling; exposes an imperative `focus()` via `forwardRef`. | `{ question, sending, onReply }` |
| `src/components/aiDockFormat.ts` | NEW | ~60 | Pure: `formatElapsed`, `formatCost`, `classifyLogLine` (fallback path), `pillFor(status, cancelRequested)`, `AI_EVENT_TEXT_MAX`, `AI_DOCK_HEIGHT_DEFAULT`. | — |
| `src/components/aiDockFormat.test.ts` | NEW | ~90 | Unit tests for the above. | — |
| `src/components/AiActivityPanel.test.tsx` | NEW | ~220 | The §13 vitest list. | — |
| `src/components/PaneDivider.tsx` | EDIT | 90 → ~110 | `side: 'ai-dock'` horizontal axis (§8). | — |
| `src/components/RepoWorkspace.tsx` | EDIT | +~16 | Mount the dock as `.workspace-host`'s third child; `activeRunKey` state; `autoExpandedForRef`; the two palette entries; the `Ctrl/Cmd+Shift+A` handler wiring. | — |
| `src/components/repoWorkspace/useWorkspaceKeyboard.ts` | EDIT | +~12 | `Ctrl/Cmd+Shift+A` before the `typing` guard. | — |
| `src/components/ShortcutOverlay.tsx` | EDIT | +~3 | One row. | — |
| `src/App.tsx` | EDIT | +~10 | `aiDockHeight` / `aiDockCollapsed` state, load, patch, prop pass (parent contract §2c). | — |
| `src/styles.css` | EDIT | **+~230** | One new `/* ---------- P68e: AI activity dock ---------- */` section at the end. (The parent contract budgeted ~130; density overrides, the reduced-motion block and the light/dark-safe tints cost the rest.) | — |
| `src/components/AiOutputPanel.tsx` | **UNTOUCHED** | 140 | D14. | — |

### 9.1 `AiActivityPanelProps` — deltas from the parent contract §7.2

Keep the parent's shape; these fields are **additive** and each has a named reason:

```ts
export interface AiActivityFile {
  path: string;
  status: 'pending' | 'ready' | 'failed';
  error: string | null;          // rendered as the queue's `reason` column
}

export interface AiActivityRun {
  key: string; label: string; status: AiRunStatus;
  elapsedMs: number; costUsd: number | null;
  question: string | null; error: string | null; partialText: string | null;
  log: AiRunLogLine[]; logDropped: number;
  files: AiActivityFile[];

  // ---- additive (§12-A2) ----
  /** Requested paths. Needed by the single-run header `Review proposal` button,
   *  which has no queue row to click. Already in the store as `paths`. */
  paths: string[];
  /** Store field `cancelRequested`. Drives the immediate `Stopping…` feedback
   *  (§6) — without it the UI cannot react before the terminal event. */
  cancelRequested: boolean;
  /** Last seen `AiRunEvent.turn`. Header turn counter. Requires the store to
   *  retain it (§12-A3). */
  turn: number;
}

export interface AiActivityPanelProps {
  runs: AiActivityRun[];                  // newest first; [] => render null
  activeKey: string | null;
  onSelectRun(key: string): void;
  collapsed: boolean;
  onToggleCollapsed(next: boolean): void;
  height: number;                         // 120..600
  onResizeHeight(next: number): void;
  onCancel(key: string): void;
  onReply(key: string, text: string): void;
  onDismiss(key: string): void;
  onReviewFile(key: string, path: string): void;

  // ---- additive ----
  /** `onRetryFile(key, path)` — starts a fresh single run for one failed file of
   *  a bulk run. The right-panel row also offers retry, but a user reading the
   *  dock must not have to go hunting. */
  onRetryFile(key: string, path: string): void;
  /** `panelDensity` → `data-density` on the root (U12). */
  density: PanelDensity;
  /** `UiSettings.aiStreamLog`. Without it, a user who turned streaming off sees
   *  an empty log and concludes it is broken again (§3). */
  streamLogEnabled: boolean;
  /** Store `atCapacity`; renders `3 of 3 running` in the run strip. */
  atCapacity: boolean;
}
```

---

## 10. Tokens — `--ai-dock-*`

Declared on `.ai-dock`, consumed everywhere as `var(--x, <fallback>)` (the P67b `--rp-*`
discipline). **No token below introduces a raw colour**: every colour is an alias of an existing
themed custom property or a `color-mix` over one, so light and dark work from one rule set and
no `[data-theme='light']` block is needed. Geometry tokens are theme-independent by nature.

```css
.ai-dock {
  /* geometry — cozy */
  --ai-dock-header-h: 30px;
  --ai-dock-pad-x: 12px;
  --ai-dock-gap: 8px;
  --ai-dock-ctl-h: 28px;          /* AA floor is 24px; never go below it */
  --ai-dock-log-font: 12px;
  --ai-dock-log-line-h: 18px;
  --ai-dock-log-pad: 6px 12px;
  --ai-dock-queue-row-h: 26px;
  --ai-dock-strip-h: 24px;
  --ai-dock-ask-pad: 8px 12px;
  --ai-dock-ask-min: 48px;
  --ai-dock-ask-max: 120px;
  --ai-dock-label-font: 12px;

  /* colour aliases — theme-safe by construction */
  --ai-dock-bg: var(--bg-1);          /* chrome, matches .workspace-toolbar */
  --ai-dock-log-bg: var(--bg-0);      /* content surface, matches .graph-pane */
  --ai-dock-meta: var(--text-2);      /* the muted role; NOT --text-3 (U9) */
  --ai-dock-tool: var(--accent);      /* the ⚙ glyph */
  --ai-dock-attention: var(--warning);/* awaiting-input tint + border + glyph */
}

.ai-dock[data-density='compact'] {
  --ai-dock-header-h: 28px;
  --ai-dock-pad-x: 8px;
  --ai-dock-gap: 6px;
  --ai-dock-ctl-h: 24px;
  --ai-dock-log-font: 11px;
  --ai-dock-log-line-h: 16px;
  --ai-dock-log-pad: 4px 8px;
  --ai-dock-queue-row-h: 24px;
  --ai-dock-strip-h: 22px;
  --ai-dock-ask-pad: 6px 8px;
  --ai-dock-ask-min: 36px;
  --ai-dock-ask-max: 96px;
  --ai-dock-label-font: 11px;
}
```

Status tints are written inline in the rules (not as tokens — one per status would be five tokens
used once each):

```css
.ai-dock-status[data-status='running']   { --h: var(--accent);  }
.ai-dock-status[data-status='awaiting']  { --h: var(--warning); }
.ai-dock-status[data-status='ready']     { --h: var(--success); }
.ai-dock-status[data-status='failed']    { --h: var(--danger);  }
.ai-dock-status[data-status='stopping'],
.ai-dock-status[data-status='cancelled'] { --h: var(--ai-dock-meta, var(--text-2)); }
.ai-dock-status {
  background: color-mix(in srgb, var(--h) 14%, var(--ai-dock-bg, var(--bg-1)));
  border: 1px solid color-mix(in srgb, var(--h) 40%, var(--ai-dock-bg, var(--bg-1)));
  color: var(--text-1);
}
.ai-dock-status-glyph { color: var(--h); }
```

**No new `:root` / `[data-theme='light']` token is introduced by this contract.**

---

## 11. Accessibility

| Element | Role / name |
|---|---|
| `.ai-dock` | `<section role="region" aria-label="AI activity">` |
| `.ai-dock-toggle` | `<button aria-expanded aria-controls="ai-dock-body" aria-label="AI activity">` |
| `.ai-dock-runs` | `role="tablist" aria-label="AI runs"`; chips `role="tab" aria-selected` + `id`; roving `tabindex` (0 on selected, −1 elsewhere); `ArrowLeft/ArrowRight/Home/End` move selection |
| `.ai-dock-body` | `id="ai-dock-body" role="tabpanel" aria-labelledby="<selected chip id>"` (only when >1 run; a single run needs no tab semantics) |
| `.ai-log` | `<ol tabindex="0" aria-label="AI output">` — **no `role="log"`, no `aria-live`** |
| `.ai-run-queue` | `<ul aria-label="Files in this AI run">` |
| `.ai-dock-ask` | `<div role="group" aria-label="Claude needs your answer">` |
| `.ai-dock-ask-input` | `aria-label="Your answer to Claude"`, `aria-describedby=".ai-dock-ask-hint"` |
| `.ai-dock-announce` | `<p role="status" aria-live="polite" aria-atomic="true">`, visually hidden |
| icon-only buttons | `✕` → `aria-label="Dismiss this run"`; chevron → see above; all glyphs `aria-hidden` |

**What is announced** (the whole list — one sentence each, replacing the previous):
`AI run started for src/locales/de.json` · `Claude needs your answer about src/locales/de.json` ·
`Stopping the AI run for src/locales/de.json` · `AI proposal ready for src/locales/de.json` ·
`AI proposals ready for 2 of 3 files` · `AI run failed: <error>` ·
`AI run cancelled. Nothing was changed.`
**What is never announced:** log lines, the elapsed timer, cost updates, turn changes, queue-row
transitions, scroll position. Rationale: at CLI output speed a live log makes a screen reader
unusable; a blind user gets the six state transitions and can read the log on demand by focusing
it (it is in the tab order).

`.ai-dock-announce` uses the standard visually-hidden recipe (`position:absolute; width:1px;
height:1px; overflow:hidden; clip-path:inset(50%); white-space:nowrap;`) declared locally — the
app has no `.sr-only` utility yet (§12-F4).

**Focus order** (DOM order = focus order): resizer → chevron → `Review`/`Answer` → `Cancel` →
`✕` → run chips (roving) → queue action buttons → log → reply textarea → `Send`.
No focus trap: the dock is not a dialog and must never trap. Nothing auto-focuses except §4.4's
guarded case. When a run is dismissed, focus moves to the chevron (never to `document.body`).

**Hit targets:** every button is ≥24px in both densities (`--ai-dock-ctl-h` floor). The 4px
resizer is a resize handle with an 8px pointer band plus full keyboard control — the same
compromise as the shipped `.pane-divider`.

**Focus rings:** `:focus-visible` only, `outline: 2px solid var(--accent); outline-offset: 1px`
(the resizer uses `-1px` so the ring stays inside the 4px strip).

**Contrast, measured** (dark / light):

| Pair | Dark | Light |
|---|---|---|
| `--text-1` on `--bg-0` (log lines) | 13.6:1 | 15.8:1 |
| `--text-1` on 14% warning tint over `--bg-1` (question) | 9.1:1 | 12.1:1 |
| `--text-2` on `--bg-0` (meta, elapsed, cost) | 7.9:1 | 4.9:1 |
| `--text-2` on `--bg-2` (partial body, chips) | 6.4:1 | 4.6:1 |
| `--danger` on `--bg-0` (`stderr:` lines, reasons) | 4.6:1 | 5.1:1 |
| `--bg-0` on `--warning` (ask glyph) | 6.4:1 | 4.8:1 |
| status glyph hue on its own 14% tint (graphics, ≥3:1) | 5.4–8.1:1 | 3.5–5.0:1 |
| `--accent` focus ring on `--bg-1` (graphics, ≥3:1) | 4.6:1 | 4.4:1 |

`--text-3` appears **nowhere** in the dock (U9).

---

## 12. Data / contract deltas, and flagged issues

### A — Additive requests to P68d's store (`useAiRuns.ts`, not yet landed)

Each is a field the store already computes or trivially can; none is a new backend field.

- **A1 — `AiRunLogLine.kind: 'text' | 'tool' | 'stderr' | 'meta'`,** classified once at ingest.
  *Why:* the dock otherwise has to sniff `⚙ ` / `stderr: ` prefixes at render time, which puts
  wire-format knowledge in a presentational component. **Recommendation: add it.** Fallback
  (`classifyLogLine` in `aiDockFormat.ts`) is specified so P68e is not blocked either way.
- **A2 — `AiActivityRun.paths: string[]` and `cancelRequested: boolean`.** `paths` is needed for
  the single-run `Review proposal` button (no queue row exists for a 1-path run);
  `cancelRequested` is needed for the immediate `Stopping…` feedback, which is the whole point of
  §6. Both already exist on `AiRunState`.
- **A3 — `AiRunState.turn: number`** (last seen `AiRunEvent.turn`). The task requires a turn
  counter in the header and the store currently drops the field. One assignment in the
  `turnEnd`/`awaitingInput` arms.
- **A4 — prune terminal runs.** Recommended container behaviour (not the dock's job): drop a
  terminal run whose paths are no longer in the conflicts list, and cap retained terminal runs at
  6 (oldest first). Without it the dock accumulates stale chips across a long merge. **Flagged
  for the orchestrator** — it is a store policy decision, not a visual one.

### B — Genuinely blocked / data-limited (flagged, not invented around)

- **B1 — Cost during the first turn is unknowable.** `costUsd` only exists on `turnEnd`/`done`,
  so a 4-minute single-turn run shows `$—` the whole way. The user accepted "no spend cap"
  *because* cost is visible, so this is a real gap in the safety story. Options: (a) ship `$—`
  with the explanatory title (**this contract's choice**); (b) ask the backend to pass
  `--include-partial-messages` and emit an interim cost — unverified line shape (spike §1.8), so
  not recommended now; (c) show a local estimate — rejected, an invented number is worse than
  none. **Flagged for the user's awareness**, not blocking.
- **B2 — "What is it doing" is only as good as the log.** There is no structured "current
  activity" field; `.ai-dock-activity` shows the latest log line, which during a thinking phase
  may be a stale `⚙ Read(...)` from 30 s ago while `thinking_tokens` heartbeats are silently
  swallowed (A4). Accepted: the elapsed timer and the progress sweep carry liveness; the log
  carries content. Emitting heartbeats as log lines was correctly rejected by A4 and this
  contract does not reopen it.
- **B3 — No re-attach after a window reload** (§11.2 of the parent contract). If the frontend
  reloads mid-run, the dock is empty while a child may still be alive until the exit hook. The
  dock cannot paper over this. **Recommend** a follow-up `ai_active_runs` listing; out of P68e.

### C — Deliberate deviations from the parent contract §7.2 (all supersets)

- Six files instead of three (U14, §9) — the named three all exist with the same class names.
- `Enter` sends in addition to `Ctrl/Cmd+Enter` (U5) — the parent's test still passes.
- `onRetryFile`, `density`, `streamLogEnabled`, `atCapacity` props added (§9.1).
- The parent's `.ai-dock-question` becomes `.ai-dock-ask-question` inside `.ai-dock-ask`; every
  other class name from §7.2 (`.ai-dock-header`, `.ai-dock-status`, `.ai-dock-elapsed`,
  `.ai-dock-cost`, `.ai-dock-runs`, `.ai-dock-reply` → `.ai-dock-ask-input`, `.ai-dock-resizer`,
  `.ai-log`, `.ai-log-line`, `.ai-log-dropped`, `.ai-run-queue`) is kept verbatim.

### D — Rejection cases (all three, with exact copy)

| Case | Where it shows | Copy |
|---|---|---|
| AI unavailable (`?ai=off`, CLI missing) | ✨AI button disabled + `title`; error toast if a call is still made; **no dock entry** | title: `Enable AI features in Settings to use this` (existing, unchanged) · toast: `Claude Code CLI not found — install it, or turn AI features off in Settings.` |
| Consent not granted | same shape | toast: `AI features are off. Turn them on in Settings → AI.` |
| Concurrency cap | pre-flight: row button disabled + `title`; if the command still rejects: error toast; the run strip shows a counter | title: `3 AI runs already in progress — cancel one to start another.` · toast: `Too many AI runs in progress (3 of 3 allowed) — cancel one and try again.` · strip: `3 of 3 running` |

Run-level failure inside the dock: an `.error-banner` (existing class, `role="alert"`,
non-dismissible) at the top of the body with the Rust message verbatim, followed by
`.ai-dock-error-next`: `Nothing was changed. You can retry, or resolve this file by hand.`
No raw libgit2/CLI text is ever synthesised by the UI — `AppError::message()` is already
user-facing; the dock adds only that next-step sentence.

### E — Command palette + shortcut

Two entries, appended after the existing `ai.ask` / `ai.changelog` pair in
`RepoWorkspace.tsx:2245-2264`, both gated on `runs.length > 0` (never advertise an empty dock):

```
{ id: 'ai.activity', title: 'AI activity',        hint: 'Ctrl+Shift+A', group: 'action',
  keywords: 'ai dock log output run streaming cancel claude progress' }
{ id: 'ai.answer',   title: 'Answer Claude…',     hint: '✨',            group: 'action',
  keywords: 'ai question reply input awaiting blocked claude' }   // only while awaitingInput
```

### F — Pre-existing defects observed while designing (NOT fixed here)

- **F1 — `--text-3` fails AA as text**: 3.38:1 on `--bg-1` (dark), 2.96:1 on `--bg-1` (light).
  It is used app-wide for metadata (ui-reference §3 even prescribes it). Fix is a token change
  (e.g. dark `#7e8797`, light `#767d8a`) verified across every consumer — a milestone of its own.
  Recorded in `ui-reference.md` §2 in this pass.
- **F2 — `.toast-warning` fails AA in light theme**: `--warning` `#9a6700` on its own 14% tint is
  3.47:1 (`styles.css:623-627`). Same class of fix as F1.
- **F3 — no `prefers-reduced-motion` block exists**: `header-progress-sweep` (1287) and
  `skeleton-pulse` (5274) animate unconditionally. The dock adds the app's first block; the other
  two should follow.
- **F4 — no `.sr-only` utility**: each announce region re-declares the recipe. Worth promoting to
  a global utility next time one is needed.

---

## 13. Acceptance criteria

### 13.1 AI gate — vitest (`AiActivityPanel.test.tsx`, `aiDockFormat.test.ts`)

1. `runs: []` → the component renders `null` (no DOM node at all).
2. Header per status: the pill text is exactly `Running` / `Needs you` / `Ready` / `Failed` /
   `Cancelled`, and `Stopping…` when `cancelRequested && status === 'running'`;
   `data-status` matches the §2 table.
3. `Cancel` is present for `running`/`awaitingInput` only, fires `onCancel(key)` once, and is
   `disabled` when `cancelRequested`.
4. `✕` is present for terminal statuses only and fires `onDismiss(key)`.
5. Elapsed: `formatElapsed` known-answer table — `0` → `0:00`, `7_400` → `0:07`,
   `725_000` → `12:05`, `3_723_000` → `1:02:03`. A terminal run's rendered elapsed does not change
   when the `elapsedMs` prop stays constant across re-renders.
6. Cost: `null` → `$—` with the documented `title`; `0.0238` → `$0.0238`; `1.2` → `$1.20`.
7. The reply form renders **only** for `awaitingInput`; submits on click, on `Enter`, and on
   `Ctrl+Enter`; `Shift+Enter` does **not** submit; an all-whitespace draft cannot submit.
8. Collapsed: the body (`#ai-dock-body`) is absent, the header is present, `.ai-dock-activity`
   shows the last log line, and `aria-expanded="false"` on the toggle.
9. `logDropped > 0` renders `.ai-log-dropped` with `↑ 1,204 earlier lines trimmed`.
10. A log line of exactly 2000 chars renders `.ai-log-trunc`; 1999 chars does not.
11. `data-kind` classification: `⚙ Read(x)` → `tool`, `stderr: boom` → `stderr`,
    `» answered (12 bytes)` → `meta`, `Hello` → `text`.
12. `streamLogEnabled: false` with an empty log renders the "Live output is off" line.
13. `files.length > 0` renders `AiRunQueue` with one row per file; `Review` is enabled only for
    `ready` and calls `onReviewFile(key, path)`; `Retry` only for `failed` and calls
    `onRetryFile(key, path)`; the `reason` text is present with a `title`.
14. Single ready run renders the header `Review proposal` button and calls
    `onReviewFile(key, paths[0])`; a bulk run does **not** render it.
15. `partialText !== null` renders the disclosure **closed**, containing the exact sentence
    `Stopped before Claude finished. This text is incomplete — Bonsai will not apply it.`, and
    contains **no** button with an accessible name matching `/copy|apply|stage/i`.
16. `runs.length > 1` renders `role="tablist"` with one `role="tab"` per run, exactly one
    `aria-selected="true"`, and `ArrowRight` moves the selection.
17. `atCapacity` renders `3 of 3 running` in the strip.
18. `.ai-log` has no `aria-live` attribute and no `role="log"`; `.ai-dock-announce` has
    `role="status"` and `aria-live="polite"`.
19. `density: 'compact'` sets `data-density="compact"` on the root.
20. Focus: rendering an `awaitingInput` run does **not** move focus when an unrelated `<input>`
    is focused (jsdom-verifiable); it *does* focus the textarea when `document.body` is active.

### 13.2 AI gate — harness (`pnpm dev:mock`, DOM + computed CSS only)

Machine-verifiable via `read_page` / `get_page_text` / one batched `javascript_tool`:

1. `.ai-dock` is `.workspace-host`'s **third** element child and its computed `flex-grow` is `0`.
2. `.panes` `getBoundingClientRect().height` shrinks by exactly the dock's rendered height when
   the dock appears, and returns when it is dismissed (⇒ nothing overlaps; the canvas re-lays
   out).
3. `?op=merge&aiSlow` + click ✨AI: `.ai-dock-status` text becomes `Running`, `.ai-dock-elapsed`
   text changes between two reads ≥1.2 s apart, `.ai-log-line` count grows.
4. Cancel: `.ai-dock-cancel` becomes `disabled` with text `Stopping…` immediately, and the
   `.ai-log-line` count present before the click is still ≥ that number afterwards (D2 — partial
   output survives).
5. `?op=merge&aiAsk`: `.ai-dock-ask` exists with the question text; typing + `Enter` (dispatched)
   completes the run and the pill becomes `Ready`.
6. `?op=merge&aiFail` over 3 files: `.ai-run-queue-row[data-status='ready']` count is 2 and
   `[data-status='failed']` is 1, with the reason string present.
7. `?ai=off`: no `.ai-dock` node is ever created.
8. Resize: set `aiDockHeight` via a drag-equivalent `onResizeHeight`, reload, and the computed
   height matches; `aiDockCollapsed` likewise.
9. Both themes and both densities: toggle `data-theme` / `panelDensity` and assert the computed
   `--ai-dock-log-font` / `color` values change as specified; no computed `color` in the dock
   resolves to the `--text-3` value.
10. `tsc` + `pnpm build` clean; console clean.

**Harness limitation, stated plainly:** `pnpm dev:mock` runs headless — the Browser pane
composites at 0×0, `document.visibilityState === 'hidden'`, `requestAnimationFrame` is paused,
and `computer{screenshot}` **fails outright**. Therefore **everything about appearance is
native-only**: whether the log reads as live, whether autoscroll feels right, whether 180px is a
comfortable default, whether the `Needs you` tint actually catches the eye, whether the sweep bar
is distracting. DOM structure, computed CSS values, text content and state transitions are fully
machine-verifiable and are covered above.

### 13.3 New mock fixtures needed (`src/ipc/mock/handlers/aiStream.ts`)

Existing seams `?aiSlow`, `?aiAsk`, `?aiFail`, `?ai=off` cover most states. Two gaps:

- **`?aiFlood`** — emit 620 log lines at ~5 ms intervals, where line 1 is exactly 2000 chars and
  lines 40/80 are `stderr: …`. Exercises: the 500-line cap + `logDropped`, the truncation chip,
  the jump-to-latest affordance, and D5's flush batching under load. **Without this, three of the
  log's states have no harness coverage.**
- **one deep/long conflict path** in the merge fixture, e.g.
  `packages/app/src/features/settings/i18n/locales/de-DE/really-long-file-name.messages.json`,
  so header/queue/row truncation is provable rather than assumed.

Both are mock-layer edits inside P68e's scope for senior-dev; they add no IPC surface.

### 13.4 USER CHECKPOINT — native only (`pnpm tauri dev`, real `claude` CLI)

Add to `docs/contracts/P68-user-checklist.md`:

1. A real run past 90 s: the elapsed timer reads correctly, the log visibly streams, `Cancel`
   stops it within ~1 s, and the pre-cancel output is still on screen.
2. A real mid-run question: the dock auto-expands, the tint is noticeable from across the window,
   and the typed answer completes the resolve.
3. **Focus is not stolen:** start a run, type a commit message, let the question arrive — the
   caret stays in the commit box, and `Ctrl/Cmd+Shift+A` jumps to the reply box.
4. Bulk over a real multi-file conflict: the queue shows per-file progress and each `Review`
   opens the right proposal in the center pane.
5. Drag-resize feel, double-click reset, and that the graph canvas re-renders crisply at the new
   height (no blurry devicePixelRatio artefacts).
6. Light theme and `compact` density, both by eye.
7. OS reduced-motion enabled: the sweep bar is static and nothing else animates.
8. A screen-reader pass (NVDA/VoiceOver): the six announcements fire, the log is **silent**, and
   the reply box is reachable by keyboard alone.

---

## 14. Deliberately not built (restraint)

- No toolbar or sidebar entry for the dock (U10).
- No height animation (U3).
- No terminal emulator / PTY (U11).
- No per-run desktop notification or sound.
- No cost chart, no token counters, no model picker in the dock — Settings owns configuration.
- No "apply all proposals" button in the dock: staging N AI results in one click from the surface
  that also shows unfinished output is exactly the confusion D2/D4 exist to prevent. Applying
  stays a per-file, reviewed action.
- No dock adoption by the other six AI runners (OQ5, deferred) — but the props are keyed by an
  opaque run key, so adoption needs no redesign (D14).
