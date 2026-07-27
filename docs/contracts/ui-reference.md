# Bonsai — UI Reference Spec (all milestones)

Feel: GitButler-clean minimalism; GitKraken-style commit graph as the centerpiece. Dark theme is
the default. All values below are canonical — implement as CSS custom properties in
`src/styles.css` and reuse everywhere.

## 1. Layout geometry

```
+--------------------------------------------------------------+
| Header bar (40px): "Bonsai" · repo name + path · ⟳ refresh    |
+-----------+---------------------------------+----------------+
| Sidebar   | Commit graph (canvas)           | Right panel    |
| 240px     | flex 1, min 480px               | 380px          |
| branches  |                                 | status / diff /|
| remotes   |                                 | commit details |
| tags      |                                 |                |
+-----------+---------------------------------+----------------+
```

- Header: height 40px, `--bg-1` background, 1px bottom border `--border`. Left: app name (600
  weight). Center-left: repo folder name (text-1) + full path (text-3, 12px, truncated). Right:
  refresh icon button (32×32 hit area).
- Left sidebar: fixed 240px, `--bg-1`, 1px right border. Collapsible sections "Branches",
  "Remotes", "Tags" (section headers: 11px uppercase, letter-spacing 0.08em, text-3).
- Center: `--bg-0`, hosts the `<canvas>` graph; fills remaining width, min 480px.
- Right panel: fixed 380px, `--bg-1`, 1px left border. Content: commit details when a graph node
  is selected; working-dir status + staging otherwise.
- Pane resizing: not required before Polish; fixed widths are fine.

## 2. Theme tokens

CSS custom properties on `:root` (dark, default) and `[data-theme="light"]`.

| Token | Dark | Light | Use |
|---|---|---|---|
| `--bg-0` | `#16181d` | `#ffffff` | app/canvas background |
| `--bg-1` | `#1d2026` | `#f6f7f9` | panels, header |
| `--bg-2` | `#262a31` | `#eceef2` | hover, inputs, pill bg base |
| `--bg-3` | `#2f343d` | `#e2e5ea` | active/pressed |
| `--text-1` | `#e8eaed` | `#1c1f24` | primary text |
| `--text-2` | `#a8adb8` | `#4b515c` | secondary text |
| `--text-3` | `#6b7280` | `#8a919e` | muted/labels |
| `--border` | `#2c313a` | `#dcdfe5` | 1px pane/row borders |
| `--accent` | `#4f8cff` | `#2f6fe4` | primary buttons, links, focus ring |
| `--accent-text` | `#ffffff` | `#ffffff` | text on accent |
| `--selection` | `#2a3b57` | `#dbe7ff` | selected row background |
| `--danger` | `#e5534b` | `#d13438` | errors, destructive |
| `--success` | `#57ab5a` | `#1a7f37` | staged/added |
| `--warning` | `#d4a72c` | `#9a6700` | modified/dirty |

Focus: 2px `--accent` outline, offset 1px, keyboard only (`:focus-visible`).

## 3. Typography & spacing

- UI font: `"Segoe UI Variable", "Segoe UI", system-ui, -apple-system, sans-serif`.
- Mono (hashes, paths, diffs): `"Cascadia Code", "Cascadia Mono", Consolas, "JetBrains Mono", monospace`.
- Sizes: base 13px / line-height 1.45; secondary 12px; section labels 11px; header app name 14px;
  diff/mono 12px. Weights: 400 normal, 600 emphasis; never bolder.
- Spacing scale (margins/padding/gaps): 4 / 8 / 12 / 16 / 24 px only.
- Border radius: 6px (buttons, panels, inputs), 999px (pills).

## 4. Commit graph metrics (canvas)

- Row height: **28px**. Lane width (x-spacing between lane centers): **16px**. Left graph gutter:
  12px before lane 0.
- Commit dot: radius **4px**, filled with the lane color, 2px ring of `--bg-0` behind it (so edges
  passing under read cleanly). Selected commit: radius 5px + 1.5px `--accent` outer ring. HEAD
  commit dot: 1.5px `--text-1` outer ring.
- Edge stroke: **2px**, round caps, color = lane color of the edge's lane.
- Fork/merge curve: cubic bézier between (x1, y1) and (x2, y2) of adjacent rows with control
  points `(x1, y1 + rowHeight/2)` and `(x2, y2 − rowHeight/2)` — vertical tangents at both ends,
  GitKraken-style S-curve. Straight vertical segments elsewhere.
- Commit text column starts after `gutter + laneCount*laneWidth + 12px`: message (text-1,
  truncated), then author + relative date (text-3, 12px) as space allows.
- HiDPI: canvas backing store scaled by `devicePixelRatio`; all metrics above are CSS px.

## 5. Lane color palette (deterministic, both themes)

Assigned by `lane % 10`, computed in Rust with the layout; stable while scrolling by construction.

| # | Hex | | # | Hex |
|---|---|---|---|---|
| 0 | `#4f8cff` blue | | 5 | `#3ec6c0` teal |
| 1 | `#f2994a` orange | | 6 | `#e8c341` yellow |
| 2 | `#9b6dff` purple | | 7 | `#f26d9c` pink |
| 3 | `#43b97f` green | | 8 | `#7a86ff` indigo |
| 4 | `#e5534b` red | | 9 | `#8fbf4d` lime |

These hold ≥ 3:1 contrast against both `#16181d` and `#ffffff` at 2px strokes; do not lighten or
darken per theme.

## 6. Ref pills (beside commit message)

Shape: 999px radius, 11px text, 600 weight, padding 2px 8px, max-width 160px with ellipsis,
4px gap between pills. Rendered on the canvas in graph rows.

| Kind | Style |
|---|---|
| Local branch | bg = lane color at 18% alpha; text + 1px border = lane color |
| Current branch (HEAD attached) | solid lane-color bg, `--accent-text` text, prefix `⌂ ` |
| Remote branch | bg `--bg-2`, text `--text-2`, 1px `--border`; label `origin/name` |
| Tag | bg `#d4a72c` at 18% alpha, text + border `#d4a72c`, prefix `# ` |
| HEAD (detached) | solid `--danger` bg, white text, label `HEAD` |

## 7. File status colors (right panel, M1+)

Added/staged `--success`, modified `--warning`, deleted `--danger`, untracked `--text-3` italic,
renamed `--accent`. Letter badge (A/M/D/U/R) in mono 11px before the path.

## 8. Empty / loading / error states

- Empty (no repo): centered column — "Bonsai" 20px/600, tagline "A tidy Git client" (text-3),
  primary button "Open repository" (accent bg, 8px 16px padding).
- Empty panes (repo open, nothing selected): centered text-3 message, 13px (e.g. "Select a commit
  to see details").
- Unborn repo: empty graph pane message "No commits yet"; status panel remains usable.
- Loading: skeleton rows (bg-2 rounded bars, 1.2s pulse) for lists; graph shows nothing until
  layout arrives (no spinners over the canvas). Any operation > 300ms shows an indeterminate 2px
  accent bar under the header.
- Errors: inline banner at top of the affected pane — `--danger` at 12% alpha bg, `--danger` text,
  6px radius, dismissible. No modal error dialogs.
- Buttons: primary (accent), secondary (bg-2 + border), icon (transparent, bg-2 on hover); all
  32px tall, 6px radius.
