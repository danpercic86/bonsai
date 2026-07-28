# P2 — Post-v1 Follow-ups: Implementation Contract

Status: authoritative for P2. Scope: pane resizing, light-theme toggle, extended graph keyboard
nav, app icon/branding, code-signing investigation (docs only). Builds on `M0-scaffold.md` (IPC
conventions, AppError, spawn_blocking), `M2-graph.md` (GraphCanvas/Theme), `P1-polish.md`
(`settings.rs`/`settings.json`, shortcut handler §6.2, toast system, WIP-row render-offset
pattern), `ui-reference.md` (tokens/metrics — light palette already specified §2, unused until
now).

Invariants (unchanged): Rust owns Git logic + graph layout math; no lane/edge math in TS; IPC
carries compact precomputed data; commands=req/resp, events=signals; git2 under `spawn_blocking`;
`src/ipc/mock.ts` updated with EVERY IpcApi change so the browser harness keeps working.

---

## 1. Scope split (sub-increments)

| # | Item | Increment |
|---|---|---|
| 1 | Pane resizing (sidebar + right panel dividers, persisted) | **P2a** |
| 2 | Light-theme toggle | **P2b** |
| 3 | Extended graph keyboard nav (PageUp/PageDown/Home/End) | **P2c** |
| 4 | App icon + branding | **P2d** |
| 5 | Code signing — investigate/document only, no implementation | **P2d §5 (doc section, no code)** |

Each of P2a–P2c is a self-contained senior-dev pass (read §0 shared conventions + its own
section). P2d is small (icon pipeline + one config edit) and bundles the signing write-up since
neither touches app code together.

---

## 0. Shared conventions (read before any pass)

- Settings persistence: **`settings.json` via `src-tauri/src/settings.rs`** (established in P1),
  NOT `localStorage`. Rationale: `localStorage` is per-webview-origin state that a plain browser
  harness (`pnpm dev`) also has, but it is invisible to Rust, cannot be inspected/edited by the
  user or tests, and P1 already established `settings.json` as *the* durable app-state store
  (recents). Splitting persistence across two mechanisms for no reason is the kind of drift this
  project explicitly avoids. Both new settings fields below live in the SAME `Settings` struct/
  file as `recentRepos`.
- **Settings version bump: `SETTINGS_VERSION` stays `1`.** Both new fields are `#[serde(default)]`
  additively — old `settings.json` files with only `recentRepos` deserialize fine (serde `default`
  on the whole struct already makes missing fields fall back to their type defaults). No migration
  code needed; document this reasoning inline in `settings.rs` so a future genuine breaking change
  has a clear precedent for when a version bump IS required.
- No new Tauri commands for the pane widths / theme choice specifically — **reuse and extend** the
  existing `get_recent_repos`-style pattern by adding two small generic commands (§2) rather than
  one command per setting, so P3+ settings additions don't each need new IPC surface.

---

## 2. Shared IPC surface additions (`src/ipc/types.ts`, `src-tauri/src/settings.rs`, both passes touch this — land in P2a, consumed by P2b)

### 2.1 Rust — `settings.rs` additions

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice { Dark, Light }
impl Default for ThemeChoice { fn default() -> Self { ThemeChoice::Dark } }

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PaneWidths {
    pub sidebar: u32,      // px, clamped [180, 480] on read AND on save
    pub right_panel: u32,  // px, clamped [280, 640] on read AND on save
}
impl Default for PaneWidths {
    fn default() -> Self { PaneWidths { sidebar: 240, right_panel: 380 } } // ui-reference §1 defaults
}

// Settings struct (existing) gains, both #[serde(default)]:
//   pub theme: ThemeChoice,
//   pub pane_widths: PaneWidths,

/// Clamps to the documented ranges; called by both load_from (defend against a
/// hand-edited file) and the setter commands (defend against a future UI bug).
pub fn clamp_pane_widths(w: PaneWidths) -> PaneWidths;
```

Wire format addition:
```json
{ "version": 1, "recentRepos": [...], "theme": "dark", "paneWidths": { "sidebar": 240, "rightPanel": 380 } }
```

`load_from` calls `clamp_pane_widths` on the deserialized value before returning (covers a
hand-edited or future-version file with out-of-range values).

### 2.2 Commands (`commands.rs`, register in `lib.rs`)

Prefer two generic-shaped commands over four single-field ones — smaller surface, same pattern
already used for recents (load → mutate → save → return):

```rust
#[tauri::command]
pub async fn get_ui_settings(app: tauri::AppHandle) -> Result<UiSettings, AppError>;
// spawn_blocking: load_from(settings_file(&app)?) -> UiSettings { theme, pane_widths }.
// Never rejects for a missing/corrupt file (same as get_recent_repos).

#[tauri::command]
pub async fn set_ui_settings(app: tauri::AppHandle, patch: UiSettingsPatch)
    -> Result<UiSettings, AppError>;
// spawn_blocking: load -> apply only the Some(..) fields in patch (clamp pane widths) ->
// save -> return the resulting UiSettings. Save failure -> AppError::Io (surfaced as a
// toast by the caller; NOT silently swallowed like the recents hook, because here the
// user just took an explicit action - toggling theme / finishing a drag - and silently
// losing it would be surprising).
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSettings { pub theme: ThemeChoice, pub pane_widths: PaneWidths }

#[derive(Debug, Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSettingsPatch {
    pub theme: Option<ThemeChoice>,
    pub pane_widths: Option<PaneWidths>,
}
```

### 2.3 TypeScript (`src/ipc/types.ts`)

```ts
export type Theme = 'dark' | 'light';
export interface PaneWidths { sidebar: number; rightPanel: number; }
export interface UiSettings { theme: Theme; paneWidths: PaneWidths; }
export interface UiSettingsPatch { theme?: Theme; paneWidths?: PaneWidths; }
```

`IpcApi` gains:
```ts
getUiSettings(): Promise<UiSettings>;
setUiSettings(patch: UiSettingsPatch): Promise<UiSettings>;
```

Re-export from `src/ipc/index.ts`; add `src/ipc/tauri.ts` invoke wrappers.

### 2.4 Mock (`src/ipc/mock.ts`)

- Reuse the existing `bonsai.mockRecents` localStorage pattern: key `bonsai.mockUiSettings`,
  value `UiSettings` JSON, default `{ theme: 'dark', paneWidths: { sidebar: 240, rightPanel: 380 } }`.
- `getUiSettings()` → `delay(150)` → read (corrupt/missing → default). `setUiSettings(patch)` →
  merge, clamp pane widths client-side to the same ranges (mirror Rust clamp so harness behavior
  matches native), write, return. This is the ONE place the mock duplicates a Rust-side clamp —
  acceptable because it's a pure numeric guard, not git/layout logic.

### 2.5 Clamp ranges rationale
Sidebar `[180, 480]`: below 180 branch/tag names truncate unreadably; above 480 the graph pane
loses too much width at the 900px `minWidth` window floor (tauri.conf `minWidth: 900`). Right
panel `[280, 640]`: below 280 diff line-numbers + short hunks don't fit; above 640 the same
900px-window constraint applies. Both ranges leave ≥ 300px for the graph pane's own
`min-width: 480px` (ui-reference §1) only at the *default* window size (1280) — flag: at the
900px minimum window width, sidebar-max + panel-max + graph-min can exceed 900px. **Recommendation**:
additionally clamp live during drag against `window.innerWidth - otherPaneWidth - 480`
(graph-pane floor), i.e. the drag handler computes its own dynamic max independent of the
stored-value range; the stored range above is just the sane persisted bound. Document this in
code as the two are deliberately different checks (persisted sanity vs. live layout fit).

---

## 3. P2a — Pane resizing

### 3.1 New component `src/components/PaneDivider.tsx`

```ts
export interface PaneDividerProps {
  /** Which pane this divider resizes ('sidebar' grows right-to-left drag = left edge of
   *  sidebar's right border; 'right-panel' grows left-to-right drag = left edge of the
   *  panel's left border). */
  side: 'sidebar' | 'right-panel';
  onResize(deltaPx: number): void;       // called continuously during drag (mousemove)
  onResizeEnd(): void;                   // called once on mouseup/pointercancel — commit point
}
export function PaneDivider(props: PaneDividerProps): JSX.Element;
```

- Renders a 4px-wide invisible hit strip (`cursor: col-resize`) centered on the pane border
  (2px inside sidebar/right-panel content, 2px into the graph pane — matches the existing 1px
  `border` line visually, no new border rendered).
- Pointer-capture drag: `onPointerDown` → `setPointerCapture`; `onPointerMove` while captured
  computes `delta = e.clientX - lastX` and calls `onResize(delta)` (sidebar: positive delta =
  wider; right-panel: negative delta = wider, i.e. mirrored — component internally negates for
  `right-panel` so the caller's `onResize` always means "delta applied to that pane's width in
  its own growth direction"); `onPointerUp`/`onPointerCancel` → `onResizeEnd()` +
  `releasePointerCapture`. No React state inside the divider — purely an event relay to keep
  drag-frame cost in the parent's `useState` setter, not re-render the divider itself.
- `:focus-visible` + keyboard: `role="separator"` `aria-orientation="vertical"`
  `tabIndex={0}`; ArrowLeft/ArrowRight while focused nudge ±8px via the same `onResize`/
  `onResizeEnd` pair (accessibility; small win, cheap to add, uses the identical callback shape).

### 3.2 App state (`App.tsx`)

```ts
const [paneWidths, setPaneWidths] = useState<PaneWidths>({ sidebar: 240, rightPanel: 380 });
// Loaded once on mount from ipc.getUiSettings() (merged with the theme load, §4.2).
const saveTimerRef = useRef<number | null>(null);
const commitPaneWidths = useCallback((next: PaneWidths) => {
  // Debounced persist (300ms) so rapid successive small nudges (keyboard) don't spam IPC;
  // the drag path already only calls this once per onResizeEnd, but keyboard nudges are
  // per-keypress. Clears/resets the timer on each call.
  if (saveTimerRef.current !== null) window.clearTimeout(saveTimerRef.current);
  saveTimerRef.current = window.setTimeout(() => {
    void ipc.setUiSettings({ paneWidths: next }).catch((e) =>
      pushToast('error', `Could not save pane widths: ${errorMessage(e)}`));
  }, 300);
}, [pushToast]);
```

- Sidebar handler: `onResize(delta)` → `setPaneWidths(w => ({ ...w, sidebar: clampLive(w.sidebar
  + delta, 'sidebar') }))`; `onResizeEnd` → `commitPaneWidths(paneWidthsRef.current)` (a ref
  mirror kept in sync via `useEffect`, standard pattern to read latest state in a stable
  callback — avoids stale-closure by NOT putting `paneWidths` in `onResizeEnd`'s deps and
  re-creating the divider's callback every render).
- `clampLive(value, side)`: clamps to the persisted-range (§2.5) intersected with
  `window.innerWidth - graphMinWidth(480) - otherPaneCurrentWidth`, recomputed on every call
  (cheap; `window.innerWidth` read, not stored).
- Layout div widths: replace the hardcoded `240px`/`380px` in `styles.css` (`.sidebar { width:
  240px }` / `.right-panel { width: 380px }`) with **inline `style={{ width: paneWidths.sidebar
  }}`** on the two elements in `App.tsx` (CSS keeps everything else — border, background,
  padding — only `width` moves to inline style, per ui-reference §1 "fixed widths" note being
  superseded here).

### 3.3 Mount load (merges with §4.2 theme load — one `getUiSettings` call)

```ts
useEffect(() => {
  void (async () => {
    try {
      const s = await ipc.getUiSettings();
      setPaneWidths(s.paneWidths);
      applyTheme(s.theme); // §4.2
    } catch { /* keep defaults, non-fatal */ }
  })();
}, []);
```

### 3.4 P2a acceptance

`cargo test` green (new `settings.rs` tests below), `cargo clippy -D warnings`, `pnpm build`
green. Rust tests (in `settings.rs`):
1. `clamp_pane_widths_clamps_both_axes` — below-min and above-max on each field clamp to the
   documented bounds; in-range values pass through unchanged.
2. `ui_settings_roundtrip` — save/load a `Settings` with non-default `theme` + `pane_widths`.
3. `set_ui_settings_patch_is_partial` (test the command's inner logic, extracted as a pure fn if
   convenient, e.g. `apply_patch(&mut Settings, UiSettingsPatch)`) — patching only `theme` leaves
   `pane_widths` untouched and vice versa.

Harness: drag the sidebar/right-panel divider, widths change live and clamp at both ends; reload
the page → widths persisted (mock localStorage roundtrip); at a narrow viewport, dragging past
the graph's 480px floor stops early instead of squeezing the canvas below it.

---

## 4. P2b — Light-theme toggle

### 4.1 CSS (`src/styles.css`) — palette already defined

`[data-theme="light"]` block (lines ~40–55) already has the full light palette from
`ui-reference.md` §2 — **no new tokens needed**, only the toggle mechanism and applying the
attribute. Confirm `--selection` and `--accent-text` are present in the light block (grep shows
`--selection`/`--accent-text` only under `:root`; **add them to `[data-theme="light"]`** —
`--selection: #dbe7ff` (per ui-reference §2 table) and `--accent-text: #ffffff` (identical both
themes) — this is a gap in the current CSS, flagged here as an in-scope fix, not new design.

### 4.2 Theme application (`App.tsx`)

```ts
/** Sets data-theme on <html> (not <body> — matches the :root/[data-theme] selector scope) and
 *  persists via setUiSettings. Also triggers a graph repaint (§4.3). */
function applyTheme(theme: Theme): void {
  document.documentElement.setAttribute('data-theme', theme === 'light' ? 'light' : 'dark');
  // 'dark' also sets the attribute explicitly (rather than removing it) so [data-theme="light"]
  // and a default :root both work identically regardless of prior state — simpler than
  // conditionally removing the attribute for dark.
}
const [theme, setTheme] = useState<Theme>('dark');
const toggleTheme = useCallback(() => {
  const next: Theme = theme === 'dark' ? 'light' : 'dark';
  setTheme(next);
  applyTheme(next);
  void ipc.setUiSettings({ theme: next }).catch((e) =>
    pushToast('error', `Could not save theme: ${errorMessage(e)}`));
}, [theme, pushToast]);
```

Called from mount load (§3.3, `applyTheme(s.theme)` + `setTheme(s.theme)`) and from the toggle
button. `applyTheme` runs synchronously before first paint is impractical to guarantee via
`useEffect` (one-frame FOUC of dark-then-light is acceptable per product scope — dark is default
and already correct pre-load; only returning *light* users see one frame of dark). **Flag/
recommendation**: if this flash is undesirable, an inline `<script>` in `index.html` reading
`localStorage['bonsai.mockUiSettings']`-equivalent is NOT available (native app has no
localStorage-first path — settings live in `settings.json`, unreadable from a pre-React
`<script>` without a sync Tauri bridge). Recommend accepting the one-frame flash; do not build a
sync-read workaround for this small a cosmetic gap.

### 4.3 Toggle UI location

Header bar, right side, before the refresh button (ui-reference §1 header = app name / repo
info / refresh only — this adds one icon button). New button `.theme-toggle` (32×32, icon-button
recipe from ui-reference §8): sun glyph (☀) when `theme === 'dark'` (click → switch to light),
moon glyph (☾) when `theme === 'light'` — i.e. icon shows the theme you'd SWITCH TO, matching
common convention. `title` = `"Switch to light theme"` / `"Switch to dark theme"`. No new
keyboard shortcut (all single-letter/Ctrl+letter slots are taken or reserved per P1 §6.1; not
important enough to justify a new binding — accessible via the button + Tab/Enter).

### 4.4 Canvas graph theme mechanism (`src/graph/colors.ts`, `GraphCanvas.tsx`)

Current state: `themeRef.current ??= resolveTheme(canvas)` — resolved once, cached forever
(M2 §3.2 comment says "callers cache the result, never per frame" — still true, but "once per
mount" must become "once per mount **and once per theme change**").

- `GraphCanvas` gains a prop **or** reads a module-level theme-version signal — recommend a
  prop for explicitness and to keep the graph module IPC/state-free:
  ```ts
  // GraphCanvasProps gains:
  themeVersion: number;   // App increments a counter on every applyTheme() call
  ```
- Effect: `useEffect(() => { themeRef.current = resolveTheme(canvas); requestPaint(); },
  [themeVersion])` — replaces the `??=` mount-only assignment (mount still works: initial
  `themeVersion` starts at 0, effect runs once on mount as normal). `requestPaint()` is the
  existing paint-scheduling function already used by resize/data-change paths — reuse it, do not
  add a second repaint mechanism.
- App: `const [themeVersion, setThemeVersion] = useState(0);` `applyTheme` (or the caller)
  increments it: fold into `toggleTheme`/mount-load as
  `setThemeVersion(v => v + 1)` alongside `applyTheme(next)`.
- Lane palette stays IDENTICAL across themes (ui-reference §5: "do not lighten or darken per
  theme" — the 10 hex values are theme-invariant already; only `--bg-0`/`--text-*`/etc. differ).
  `resolveTheme` needs no change to its own logic, only to WHEN it's called.

### 4.5 P2b acceptance

`pnpm build` green. Harness: toggle button flips `data-theme`, all panels + canvas graph
background/text/pills recolor immediately (no stale dark commit-dot rings on light bg — verify
the "2px `--bg-0` ring behind dots" ui-reference §4 rule actually flips to white, since that ring
existing at all is a dark-bg-only affordance that must still read correctly on white); reload →
theme persisted; lane colors are pixel-identical across a toggle (only chrome recolors); every
existing screenshot-based harness check (P1 §11) still passes with `data-theme` absent (dark
default unaffected).

---

## 5. P2c — Extended graph keyboard navigation

Builds directly on P1 §6.2 (global handler) and §6.3 (Arrow selection + follow-scroll), applying
the SAME guard order (typing guard → dialogOpen/switcherOpen → enablement) — no new guard
architecture.

### 5.1 New bindings (append to the P1 §6.1 table; same conflict-analysis discipline)

| Binding | Action | Conflict analysis | Handled in |
|---|---|---|---|
| `PageDown` | Move selection **+visibleRows** (clamped to last row) | Page scroll when nothing is focused in-page; only fires per the existing typing/dialog guards, same as Arrow keys — preventDefault only when it actually fires | App |
| `PageUp` | Move selection **−visibleRows** (clamped to row 0) | same | App |
| `Home` | Select row 0 (topmost commit / WIP row is NOT a selectable target — see §5.3) | Scrolls page to top when unguarded; same preventDefault discipline | App |
| `End` | Select the last row (`graph.nodes.length - 1`) | Scrolls to page bottom when unguarded | App |
| `Enter` | **No new semantic.** Selection already updates the right panel live on ArrowUp/Down/PageUp/PageDown/Home/End (P1 pattern: `selectedIndex` change alone drives the commit-details fetch) — an extra "confirm" step would be redundant and inconsistent with mouse-click selection (single click selects immediately, no double-click/enter step). Rejected explicitly; do not implement. | — |

Only active when `selectedIndex !== null` (same rule as P1 Arrow keys — Home/End/PageUp/PageDown
with no prior selection do nothing, consistent with §12.7 of P1's "minimal arrow nav" precedent).
**Flag/decision**: this means Home/End can't be used to select the very first commit from a
cold state (no selection yet) — accepted for consistency; if the orchestrator wants "select-first-
on-Home-from-null" that's a one-line relaxation (`selectedIndex ?? 0` as the base) but changes the
"arrows only move an existing selection" precedent from P1 §12.7, so calling it out rather than
silently expanding scope.

### 5.2 `visibleRows` — needs a new value surfaced from GraphCanvas to App

`GraphCanvasProps` (or a ref-exposed imperative handle) gains a way for App to know how many
rows fit in the viewport, since PageUp/PageDown deltas depend on it (this is display-derived,
NOT layout math — no Rust involvement, purely `Math.floor(viewportHeightPx / ROW_HEIGHT)`):

```ts
// GraphCanvas exposes via forwardRef + useImperativeHandle (new — first imperative use in this
// component; alternative considered: lift viewport height into App via a resize-observed prop,
// rejected because App doesn't own the canvas's DOM size and would need its own ResizeObserver
// duplicate of the one already inside GraphCanvas).
export interface GraphCanvasHandle { getVisibleRowCount(): number; }
export const GraphCanvas = forwardRef<GraphCanvasHandle, GraphCanvasProps>((props, ref) => {
  useImperativeHandle(ref, () => ({
    getVisibleRowCount: () => Math.max(1, Math.floor(viewportHeightRef.current / METRICS.rowHeight)),
  }));
  // ... existing body; viewportHeightRef already tracked internally by the resize effect —
  // confirm/expose it as a ref if not already named exactly that.
});
```

App: `const graphRef = useRef<GraphCanvasHandle>(null);` PageDown handler:
`const n = graphRef.current?.getVisibleRowCount() ?? 10; setSelectedIndex(i => i === null ? null
: Math.min(i + n, graph.nodes.length - 1));` (PageUp mirrors with `Math.max(0, i - n)`).

### 5.3 WIP-row interaction (P1 §9)

Home/PageUp clamp to row **0 of the Rust layout** (the topmost real commit), never the WIP row —
consistent with P1 §9.4 ("never selectable"). No change to the WIP row's own click-to-deselect
behavior; keyboard nav simply never targets it. `End`/`PageDown` are unaffected by the WIP offset
(they only touch `graph.nodes.length`, the layout array, not screen rows) — the follow-scroll
effect already accounts for `wipOffset` when scrolling the target row into view (P1 §9.3's
`rowY` formula), so no new offset math is needed here.

### 5.4 `ShortcutOverlay.tsx` update

Add the four new rows to the static binding table content (P1 §6.4 component) — same file,
presentational content only, no interface change.

### 5.5 P2c acceptance

`pnpm build` green. Harness: with a commit selected, PageDown jumps ~one screenful down and the
graph auto-scrolls to keep it in view (follow-scroll reused verbatim); Home/End jump to row
0/last with correct scroll; verify against the 20k fixture (`?fixture=20k`) that End does not
freeze the tab (must not walk/allocate anything beyond `graph.nodes.length - 1`, pure index
arithmetic); overlay lists the four new bindings; Home/PageUp never select/highlight the WIP row
row.

---

## 6. P2d — App icon + branding, and code-signing (docs only)

### 6.1 Icon pipeline

1. **Source asset required from the user/orchestrator**: one 1024×1024 PNG, transparent
   background, RGBA. **Not generated by senior-dev** — flag as an external input needed before
   this pass can run. Suggested mark (guidance only, not a spec): a simple single-color bonsai
   silhouette — a short trunk with 3–4 rounded canopy lobes, inspired by the `--accent` blue
   (`#4f8cff`) on a transparent/dark-neutral disc, flat/geometric style (no gradients/shadows) so
   it downsamples cleanly to 16×16 favicon and taskbar sizes. Deliver as
   `docs/assets/bonsai-icon-source.png` (docs-tree, not app code — orchestrator or user supplies
   it; senior-dev's job starts once it exists).
2. Pipeline command (Tauri v2 CLI, already a dependency): from repo root,
   `pnpm tauri icon path/to/bonsai-icon-source.png` — generates the full icon set (`.ico`, `.icns`,
   PNG sizes) into `src-tauri/icons/`, overwriting the current default Tauri icons.
3. **`tauri.conf.json` touchpoints**: `bundle.icon` array already points at
   `["icons/icon.ico"]` — no path change needed (the generator writes to the same
   `src-tauri/icons/` directory the existing config already references); verify after generation
   that `icon.ico` (Windows bundle icon) and the window's runtime icon
   (`src-tauri/icons/32x32.png`/`128x128.png` etc., used for the taskbar/window titlebar via
   Tauri's default resolution) both regenerated — no explicit per-window `icon` override exists
   today in `tauri.conf.json`'s `windows[0]`, so none needs adding; Tauri v2 uses the bundle set
   for the window icon by default on Windows.
4. Add a favicon for the browser harness: `pnpm tauri icon` also emits a web-suitable PNG;
   manually copy/reference the 32×32 or 64×64 output as `public/favicon.ico`
   (or reuse `src-tauri/icons/icon.ico` directly) and add/update the `<link rel="icon">` in
   `index.html` if one isn't already present — purely cosmetic for the browser harness, not a
   product requirement, but cheap and avoids the default Vite leaf icon in screenshots.

### 6.2 P2d acceptance (icon)

`pnpm tauri build` produces an installer/exe with the new icon (visual check — USER CHECKPOINT,
see below); `pnpm build` unaffected (icon generation is a one-off CLI step, not part of the build
pipeline); browser harness tab shows the new favicon.

### 6.3 Code signing — investigation only (no implementation this milestone)

Out of scope to implement: requires a **user-provided code-signing certificate**, which cannot be
fabricated or obtained autonomously. This section documents what it would take, for the
orchestrator to schedule as a future milestone once a certificate is available.

**What's needed:**
- A code-signing certificate for Windows (`.pfx`/`.p12` + password, or a hardware-token/cloud-HSM
  cert e.g. Azure Trusted Signing / DigiCert KeyLocker for EV certs — required by SmartScreen
  reputation on newer Windows builds for unsigned-publisher warnings to disappear quickly).
  Options ranked by typical friction: (a) Azure Trusted Signing (cloud, per-signature billing, no
  hardware token, Microsoft-recommended path for 2025+) — **recommended** if the user wants the
  lowest-friction setup; (b) traditional OV certificate (`.pfx`) from a CA (DigiCert, Sectigo,
  SSL.com) — cheaper but still triggers SmartScreen warnings until reputation builds; (c) EV
  certificate (hardware token/HSM) — best SmartScreen behavior immediately, highest cost/friction.
- **Tauri config touchpoints** (once a cert exists): `tauri.conf.json` →
  `bundle.windows.certificateThumbprint` (or `bundle.windows.signCommand` for a custom
  signtool invocation, needed for cloud/HSM-based signing like Azure Trusted Signing which
  doesn't use a local `.pfx`), `bundle.windows.digestAlgorithm` (`sha256`), and a
  `bundle.windows.timestampUrl` (e.g. `http://timestamp.digicert.com`) so signatures remain valid
  after cert expiry. Local `.pfx` signing additionally needs the password supplied via the
  `TAURI_SIGNING_PRIVATE_KEY`-style env var pattern Tauri uses (exact var name is for the
  updater-signing feature, distinct from Authenticode signing — verify against the Tauri v2 docs
  at implementation time since this is the part most likely to have shifted).
- **CI/local build implication**: signing must happen on the machine running `pnpm tauri build`
  (or via a signing step Tauri shells out to); a cloud-HSM path (Azure Trusted Signing) avoids
  storing the private key on the build machine at all, which is the safer default recommendation
  for a small team.
- **Not implemented now**: no `tauri.conf.json` edit, no CI step, no cert acquisition. Revisit
  when the user has chosen and obtained a certificate; at that point this doc section becomes the
  seed of a proper P3 contract.

### 6.4 P2d USER CHECKPOINT

1. Native `pnpm tauri build` installer + resulting installed app show the new bonsai icon in the
   taskbar, window titlebar, and Start Menu entry.
2. User confirms the code-signing section's cert-option recommendation (or picks an alternative)
   before any future signing work is scheduled.

---

## 7. Overall P2 acceptance

AI gate (orchestrator verifies, no user needed):
- `cargo test` green incl. new `settings.rs` tests (§3.4); `cargo clippy -- -D warnings`;
  `pnpm build` green after every sub-increment.
- No changes to `graph.rs`/`compute_graph`/its tests — P2c is pure view-layer index arithmetic,
  same invariant as the P1 WIP row.
- Harness (VITE_MOCK_IPC=1) screenshots/checks per §3.4, §4.5, §5.5: divider drag + persisted
  widths; light-theme toggle recoloring chrome AND canvas with unchanged lane hues; PageUp/
  PageDown/Home/End selection + scroll on both the default and 20k fixtures; ShortcutOverlay shows
  all bindings (old + new).
- `src/ipc/mock.ts` compiles and implements `getUiSettings`/`setUiSettings` — confirmed by the
  harness tests above actually exercising persistence via localStorage round-trips.

USER CHECKPOINT (native `pnpm tauri dev` / `pnpm tauri build` — never self-declared):
1. Drag both dividers in the native window; resize feels smooth (no lag), widths persist across
   an app relaunch.
2. Toggle light theme in the native window; every pane and the commit graph read cleanly in both
   themes; theme persists across relaunch.
3. PageUp/PageDown/Home/End navigate the graph naturally in the native app on a real repo.
4. New app icon appears in the taskbar/titlebar/installer after `pnpm tauri build`.
5. User reviews and picks a code-signing certificate option from §6.3 (no native testing needed
   for this item — it's a decision, not a build artifact).

---

## 8. Ambiguities resolved here (flag to orchestrator if disagreed)

1. **`settings.json` over `localStorage`** for pane widths + theme — consistent with the P1
   precedent and the stated instruction; localStorage would fragment app-state persistence
   across two stores for no benefit.
2. **Two generic `get_ui_settings`/`set_ui_settings` commands** rather than four single-purpose
   ones (`get_pane_widths`, `set_theme`, ...) — smaller IPC surface, room for P3 settings
   additions without new commands each time. If the orchestrator prefers strict single-purpose
   commands matching the recents precedent exactly, that's a mechanical split of §2.2 into four
   commands with no other design impact.
3. **Persisted clamp range vs. live-drag clamp are two different checks** (§2.5) — the stored
   range is a sane absolute bound; the live drag additionally respects the current window size
   and the graph pane's 480px floor. Both are needed; conflating them into one clamp would either
   let a resize squeeze the graph pane on a small window (if only the stored range is enforced)
   or make the persisted default itself window-size-dependent (if only the live check is kept).
4. **One frame of dark-theme flash possible for returning light-theme users** (§4.2) — no
   pre-React synchronous settings read exists; judged not worth building a sync bridge for.
   Flagged explicitly rather than silently accepted.
5. **`Enter` gets no new graph semantic** (§5.1) — selection already updates the right panel
   live; an extra confirm step would be incons12istent with mouse click-to-select. Explicitly
   rejected rather than left ambiguous.
6. **Home/End/PageUp/PageDown are no-ops when nothing is selected** — mirrors the P1 §12.7
   "arrows only move an existing selection" precedent; a "Home selects row 0 from cold state"
   variant is a one-line change if the orchestrator wants different behavior, called out rather
   than silently decided.
7. **`GraphCanvasHandle` via `forwardRef`/`useImperativeHandle`** is the first imperative escape
   hatch in this component — justified because `visibleRowCount` is transient DOM-measured state
   App has no other way to learn without duplicating a ResizeObserver; every other prop on
   `GraphCanvas` stays declarative.
8. **Icon source asset is an explicit external dependency** — this contract cannot specify exact
   pixels, only a design brief; P2d cannot start until the 1024×1024 PNG exists at
   `docs/assets/bonsai-icon-source.png`.
9. **Code signing is documentation-only in P2** — flagged per the task's explicit instruction;
   Azure Trusted Signing recommended as the lowest-friction option but the user makes the final
   call before any future implementation milestone.
