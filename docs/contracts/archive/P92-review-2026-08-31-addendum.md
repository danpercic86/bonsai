# P92 design review — addendum (2026-08-31)

Companion to `P92-multi-ref-commit-ui.md` §8. Two items that must not be lost.

## A. `ui-reference.md` edits still owed (BLOCKING for the design system, not for the code)

`ui-reference.md` is ~950 lines and `ui-designer` has `Write` but no `Edit`, so a verbatim
full-file rewrite was judged too corruption-prone to attempt in this pass (the file has been
recovered from corruption once already — 2026-08-22). The two edits below are the complete,
final text; apply them with an editing tool in a dedicated pass.

**A.1 — §6.2, replace the bullet beginning "Context menus are now height-clamped, app-wide:"
with:**

> - **Context menus are height-clamped, app-wide** — and the clamp only works with its two
>   companion rules (P92 §8.1; the first shipped version had neither and was broken):
>   ```css
>   .context-menu { max-height: min(60vh, 480px); overflow-y: auto; overflow-x: hidden;
>                   overscroll-behavior: contain; }
>   .context-menu--sub { position: fixed; max-height: min(60vh, 480px); overflow-y: auto; }
>   ```
>   1. **The scroll-dismiss handler must ignore scrolls originating inside the menu root.**
>      `window.addEventListener('scroll', …, true)` receives the menu's own scroll box even though
>      `scroll` does not bubble, so an unguarded handler closes the menu the instant the user
>      wheels it. Guard with `rootRef.current.contains(e.target as Node)`, as the pointerdown
>      handler already does.
>   2. **Flyouts must escape the scroll box.** A scroll container clips absolutely-positioned
>      descendants, and `overflow-y: auto` computes `overflow-x` to `auto`. With
>      `.context-menu--sub` at `position: absolute` inside a `.context-menu-row`, an open flyout is
>      clipped, gives the parent a horizontal scrollbar, and makes the browser scroll the parent
>      sideways to reveal it. Position the flyout `fixed`, from the row's
>      `getBoundingClientRect()`, keeping the existing right-flip / bottom-raise clamping.
>      Because a fixed flyout no longer moves with the parent, **the parent closes its open flyout
>      when its own scroll box scrolls** (hover or ArrowRight reopens it).

**A.2 — §4.1, append:**

> - **Menu key / Shift+F10** on the focused graph scroller opens the **selected** row's context
>   menu, anchored at the ref band's left edge just under that row (clamped to the scroller's box).
>   This is the keyboard route to the P92 ref picker, and therefore to every ref on a multi-ref
>   commit. *Known gap:* arrow-key row selection is a **window-level** keydown, so a user can select
>   a row without the scroller holding focus and the Menu key then does nothing. The durable fix is
>   to focus the scroller whenever the window-level handler changes the graph selection (or to move
>   that nav onto the scroller's own handler). Tracked as a follow-up.
> - **Known defect (pre-P92, needs its own increment):** `aria-activedescendant="graph-row-{i}"`
>   points at an id that does not exist — the rows are canvas pixels — and `role="grid"` has no
>   `role="row"` children. A dangling IDREF is worse than none. Either render one visually-hidden
>   `role="row"` per *visible* row, or drop `role="grid"` + `aria-activedescendant` and let the live
>   region be the sole channel.

## B. Harness trap worth recording

The Browser pane reports `innerWidth/innerHeight = 0` while hidden, so **every `vh`/`vw` rule
evaluates to 0** and any geometry measurement is meaningless. Call `resize_window` with an explicit
size (1440×900) before measuring anything layout-dependent. Also: `setTimeout` is throttled to ~1s
in the hidden page, so a loop with many `await`s times the tool out — batch dispatches and await
sparingly. And never remove React-owned DOM nodes to "reset" a menu (it throws
`removeChild` on the next render); dismiss with Escape instead.
