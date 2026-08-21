# P72 — Forge connect fixes — UI contract

Backend/IPC contract: `docs/contracts/P72-forge-connect-fixes.md` (architect). This file covers only
what the user sees. **Scope: copy, states, and link wiring. No new components, no new files, no new
tokens, no layout change.** Nothing in §1–§10 of `docs/contracts/ui-reference.md` changes.

Files touched (all existing):

| File | Change |
| --- | --- |
| `src/components/ForgeConnect.tsx` | Azure hint copy (§2); link rewired to `openUrl` (§3) |
| `src/components/PrDetailView.tsx` | "Open in browser" rewired to `openUrl` (§3) |
| `src/components/PrPanel.tsx` | owns the open-URL handler and passes it to both children (§3). **No other change** |
| `src/ipc/mock/handlers/external.ts` | `openUrl` mock (§5) |
| `src/styles.css` | **one line only** — see §3.2. Nothing else. |

---

## 1. Forge identity — no UI work in P72

**Decision (orchestrator, 2026-08-19): P72 ships no UI for forge identity.** The backend's
empty-login fallback is **unobservable in the UI today, and that is accepted as-is.** The correct
time to design a "Connected as …" affordance is when a real consumer of `ForgeViewer` is built —
not inside a bug-fix milestone. Neither user complaint ("Create a token does nothing", "Azure
rejects my valid PAT") asks for it, and a successful connect already signals success by the panel
switching to the PR list; adding chrome to make an internal fallback visible would spend the user's
surface area on our problem, not theirs.

Concretely, **do not** add in P72: a success toast, a "Connected as …" chip, a header badge, a
persistent identity row, an avatar, or any `viewer` read in `PrPanel`. `handleConnect` keeps
discarding the resolved viewer exactly as it does today.

### 1.1 Finding of fact — there are zero existing render sites

`ForgeViewer.login` / `ForgeViewer.avatarUrl` and `ForgeRepoContext.viewer` are **never rendered**.
Verified by grep across `src/`: the only non-type references are `src/ipc/types.ts`,
`src/ipc/tauri.ts`, `src/ipc/fixtures/forge.ts`, `src/ipc/mock/handlers/forge.ts` (which reuses
`FORGE_VIEWER.login` as a mock PR *author*), and `PrPanel.tsx:187` which discards the resolved value
(`.then(() => …)`). `PrPanel.tsx` never reads `ctx.viewer`; `PrDetailView` renders
`summary.author`, which comes from the PR, not the viewer.

**Consequence — and the evidence for the decision above:** an empty `login` cannot produce a
dangling "as", a stray separator, an empty gap, or a username-shaped placeholder, because it is not
displayed at all. There is no existing markup to defend and therefore no defect to fix here.

### 1.2 Pre-specified copy — NOT IMPLEMENTED IN P72

Recorded so that whenever an identity affordance is built, the empty-login case is already decided
and nobody re-derives it. **No file in P72 contains these strings.**

| `login` | Text |
| --- | --- |
| non-empty, after `trim()` | `Connected to dev.azure.com as dpercic` |
| `''` or whitespace-only | `Connected to dev.azure.com` |

Pattern: `` `Connected to ${host}` `` + `` ` as ${login}` `` only when `login.trim() !== ''`. Trim
before the emptiness test **and** before interpolation. Never a placeholder ("unknown", "—", "your
account"); never a trailing period; never `warning` styling or a `⚠` — a missing optional name is
not an error. `host` falls back to `'the forge'`, the same fallback `PrPanel.tsx:265` already passes
to `ForgeConnect`, so the string is never `Connected to undefined`.

### 1.3 No "why is there no name?" hint

Rejected, and it stays rejected independently of §1.2: the explanation is already delivered
**before** the user connects, by the Azure hint line in §2 ("Add User Profile (Read) to show your
account name"). That is the only place it belongs.

### 1.4 Avatar slot

`avatar_url` is `None` for Azure DevOps in **all** cases, and may be `null` for any provider. No
avatar is rendered anywhere today and none is added. Forward rule for any future site (record only
— do not implement now): a `null` `avatarUrl` renders the existing initials disc (the graph's
`avatarColor`/`initials` helpers in `src/graph/geometry.ts`), and an empty `login` renders **no**
avatar element at all — never a broken `<img>`, never an empty circle, never a "?" glyph.

---

## 2. `CONNECT_HINTS` copy (`ForgeConnect.tsx:36-63`)

Only the `azureDevOps.scopes` string changes. `url` and `placeholder` are untouched.

**Before:** `Use an Azure DevOps personal access token with Code (Read & Write).`

**After (exact string) — UNCHANGED from today:**

```
Use an Azure DevOps personal access token with Code (Read & Write).
```

**Orchestrator decision (2026-08-19) — reverses this section's original proposal.** The proposed
second sentence (`Add User Profile (Read) to show your account name.`) is **struck**. ui-designer
correctly flagged the honesty problem in its own review and recommended keeping the sentence anyway;
I am overruling that, because the objection it raised is the decisive one. Nothing in Bonsai renders
an account name for ANY provider (§1.1), so the sentence would advertise a capability that produces
no observable result — and per §1 we are deliberately not building one in P72. A hint that sends the
user back to the Azure portal to add a scope, for no visible effect, is worse than silence.

The happy consequence: **the existing Azure hint string is already correct once the backend fix
lands.** It promised that a Code (Read & Write) PAT is what you need, which was false only because
the validation probed a profile endpoint. Fixing the backend makes the copy true. So P72 changes no
copy in `CONNECT_HINTS` at all, and §2 is a no-op for senior-dev.

Revisit this section when a `ForgeViewer` consumer is built: at that point the second sentence
becomes both true and useful, and the pre-specified strings in §1.2 apply.

**Other providers reviewed — no edits proposed.** `gitHub`, `gitLab`, and `bitbucket` all follow
`Use a <token type> with <scope>.`, sentence case, one sentence, terminal period; the Azure line now
leads with the same clause. The `unknown` fallback (`…read and write access to pull requests.`) is
correctly generic and, per the comment at `:24-26`, never reaches this panel. Leave all four alone.

---

## 3. The rewired links

Two anchors, one behaviour. Both **remain `<a>` elements with a real `href`** — they are navigations,
not commands, and must keep link semantics (see §4).

| Site | Line | Anchor text | `href` |
| --- | --- | --- | --- |
| `ForgeConnect.tsx` | 98-106 | `Create a token` | `hint.url` |
| `PrDetailView.tsx` | 46-53 | `Open in browser ↗` | `summary.url` |

### 3.1 Markup deltas

- Add `onClick={(e) => { e.preventDefault(); onOpenUrl(url); }}`. `preventDefault()` runs
  unconditionally on a plain primary click.
- **Do not intercept modified or auxiliary clicks.** Guard: if `e.metaKey || e.ctrlKey ||
  e.shiftKey || e.altKey || e.button !== 0`, return **without** `preventDefault()`. In the browser
  harness that preserves ctrl/middle-click-to-new-tab; in the Tauri webview it is a no-op. This is
  the one place the JS-driven link must defer to the platform.
- Keep `target="_blank"`. It costs nothing, keeps harness behaviour identical for the modified-click
  path, and is what the accessible "opens externally" expectation is built on.
- `rel`: `ForgeConnect` gains `noopener` → `rel="noreferrer noopener"`, matching `PrDetailView`
  which already has both. Same order in both files.
- Both components stay presentational: the handler arrives as a prop.
  `ForgeConnectProps` gains `onOpenUrl(url: string): void`; `PrDetailViewProps` gains the same.
  `PrPanel` owns the single implementation and passes it to both (it already renders `ForgeConnect`
  at :263 and `PrDetailView` via `renderDetail()`). No component calls `ipc` directly.
- `PrDetailView`: wrap the glyph as `<span aria-hidden="true">↗</span>` so the accessible name is
  exactly `Open in browser` instead of a name ending in a decorative arrow character. Visual result
  is byte-identical.

### 3.2 States — must not regress

`.forge-connect-link` (`styles.css:6990-7006`) and `.pr-open-link` (`:6708`, on `.section-action`)
are **unchanged**, with one required addition:

- **Default.** `.forge-connect-link`: `var(--accent)`, no underline, `nowrap`.
  `.pr-open-link`: `.section-action` — 11px/500 `var(--text-3)`, transparent bg, 4px radius.
- **Hover.** `.forge-connect-link:hover` → underline appears (colour unchanged).
  `.pr-open-link` inherits `.section-action:hover:not(:disabled)` → `var(--text-1)`.
- **`:focus-visible`.** The global rule at `styles.css:127-130` — `2px solid var(--accent)`,
  `outline-offset: 1px`, `:focus-visible` only. Anchors with `href` are natively focusable, so this
  keeps working with no change. **Do not add `tabIndex`.**
- **Active/pressed.** No dedicated `:active` style today; add none. The toast (failure) or the
  opening window (success) is the feedback.
- **Cursor.** `.pr-open-link` gets `cursor: pointer` from `.section-action`. `.forge-connect-link`
  relies on the UA default for `a[href]` — which still applies, because the `href` is retained.
  **The one CSS line in this milestone:** add `cursor: pointer;` to `.forge-connect-link` explicitly
  so the affordance can never silently depend on the `href` surviving a future refactor. Nothing
  else in `styles.css` changes.
- **Disabled / loading.** Neither link has one, and neither gains one. The `Create a token` link
  stays live while `submitting` is true (the token input is what disables). No spinner: the IPC
  round-trip is a process spawn, typically <200ms, and a spinner on a link would be worse than
  nothing. If it fails, the toast says so.
- **Long content.** `ForgeConnect`'s label is a fixed 14-char string. `PrDetailView`'s is fixed too;
  the long value is `summary.url`, which is never displayed — recommended: `title={summary.url}` on
  that anchor so the destination is inspectable before clicking. Not required.
- **Both themes.** `--accent`, `--text-3`, `--text-1` are all already theme-swapped; no per-theme
  rule needed. **Both densities:** both links live in density-invariant surfaces and keep their
  current sizes.

### 3.3 Failure copy

Pattern: `pushToast('error', \`<prefix>: ${errorMessage(e)}\`)`, one prefix per site:

| Site | Toast |
| --- | --- |
| `ForgeConnect` link | `Could not open the token page: <message>` |
| `PrDetailView` link | `Could not open the pull request page: <message>` |

- Rendered example: `Could not open the token page: could not launch browser (explorer): The system
  cannot find the file specified.` — the backend text already names the tool and the reason, so the
  prefix names the **intent** instead, and no libgit2/OS text is invented or suppressed.
- **Why prefixed, unlike P49.** `RepoWorkspace.tsx:2353-2370` and `App.tsx:370-387` toast a bare
  `errorMessage(e)` because those actions are invoked from a context menu whose item text is still
  on screen. A link click has no such residue, and a toast opening with a lowercase "could not…"
  reads like a leaked log line. The closer precedent is `PrPanel`'s own
  `Could not connect: …` (:195) and `Could not open the pull request: …` (:221); these two strings
  extend that family. Note the deliberate distinction from :221 — that one is about *creating* a PR,
  so this one says "page".
- Success is silent apart from the browser window appearing — same as P49.
- The prefixes are provider-agnostic; no per-forge variants.

### 3.4 Keyboard

- **Enter** activates, unchanged: a focused `a[href]` dispatches a `click` on Enter, which the new
  handler receives. Nothing extra to implement.
- **Space must NOT activate.** These are links, not buttons; do not add a `keydown` handler. Space
  scrolls the panel, which is correct link behaviour and what a screen-reader user expects after
  hearing "link".
- Tab order unchanged: `ForgeConnect` = hint link → token input → Connect. `PrDetailView` =
  `← Pull requests` → `Open in browser` → body content.
- **Not added to the command palette.** Both are context-bound to a visible panel; a global
  "Open the token page" command would be meaningless without a repo/provider in view.

---

## 4. Accessibility

- **Empty login.** Nothing is rendered and nothing is announced (§1) — there is no name gap, no
  orphaned "as", and no live-region update to get wrong. This is the a11y argument *for* the §1
  decision: the cheapest accessible treatment of an absent optional value is not to render a slot
  for it.
- **JS-driven links keep link semantics.** No `role="button"`, no `tabIndex`, no `keydown` handler,
  `href` retained. Screen readers announce "link, Create a token" / "link, Open in browser" exactly
  as before; the change is invisible to AT, which is the goal.
- `aria-hidden` on the `↗` glyph (§3.1) removes a spoken junk character from the accessible name.
- **Hit targets.** `Open in browser ↗` sits in `.pr-detail-title-row` next to `← Pull requests` and
  keeps its current box; `Create a token` is inline text in a paragraph. Both are unchanged, both
  already ship — this pass must not shrink them.
- **Failure toasts** are announced by the existing toast live region; both strings are complete
  sentences that name the intent before the technical detail, so the first words heard are the
  meaningful ones.
- **Contrast (existing pairs, re-confirmed, no new tokens).** `--accent` on `--bg-1` and `--text-3`
  on `--bg-1` are the pairs in play, both already AA-audited in `ui-reference.md` §2 for both
  themes; nothing new is introduced. `--accent` link text is additionally never colour-alone — it
  underlines on hover and is inside a sentence that names the destination.
- **Colour is never the sole carrier** in any string here; all state is words.
- **Motion.** None added. No `prefers-reduced-motion` change.
- **Tokens.** No new token in either theme. `ui-reference.md` needs **no edit** for P72; this
  contract is linked from `docs/contracts/INDEX.md` instead.

---

## 5. Harness states (mock IPC, `VITE_MOCK_IPC=1`, port 1420)

With §1 struck, the only harness-observable new states are the **two link-failure toasts** and the
**reworded Azure hint**. No forge-mock change is needed: `src/ipc/mock/handlers/forge.ts` is
untouched (the previously-proposed mutable-`viewer` seam and `noname`/`longname` token sentinels are
withdrawn). The unauthenticated Connect panel is already driven by `e2e/11-forge.spec.ts:85-96`.

1. **Azure Connect panel** — `?forge=azureDevOps` → the new two-sentence hint line renders and wraps.
   Include a narrow-width check (right panel at minimum width) for the wrap asserted in §2, and both
   themes (the line uses only `--text-2`, so this is a read, not a fix).
2. **`openUrl` success** — `src/ipc/mock/handlers/external.ts` gains
   `openUrl(url: string): Promise<void>` using the existing `simulate`-style shape: `await delay(120)`
   then `console.info('[mock] open url: …')`. Clicking either link must produce that log and **no**
   toast, and must not navigate the harness page away.
3. **`openUrl` failure — `PrDetailView`.** Reuse the module's sentinel convention: a URL containing
   `#fail` rejects with
   `{ kind: 'externalToolFailed', message: 'Mock: could not launch browser for "<url>"' }`. Reach it
   by pointing a fixture PR's `summary.url` at `https://example.test/pr/1#fail`. This one must be
   harness-visible.
4. **`openUrl` failure — `ForgeConnect`.** Awkward to reach, because the URL comes from the static
   `CONNECT_HINTS` table. **Two routes, both pre-approved by the orchestrator — senior-dev picks
   without coming back to ask:**
   (a) *preferred* — append the `#fail` sentinel to `CONNECT_HINTS.unknown.url` (or add a
   harness-only URL-param override) so the toast can be triggered in the browser; or
   (b) prove that toast with a **component-level vitest** on `ForgeConnect` (reject the injected
   `onOpenUrl`, assert the exact string) and let the harness cover only the `PrDetailView` one.
   If (b) is chosen, note it in the increment report; it is not a USER CHECKPOINT gap, because the
   two sites share one handler and one code path.
5. **Long content** — a `hint.url` and a `summary.url` long enough to confirm the failure toast
   truncates by the toast's existing rules and does not reflow the panel.

**USER CHECKPOINT (cannot be AI-verified):** that a click actually opens the system browser in the
native Tauri window on Windows/macOS/Linux — the harness only proves the wiring and the toast. This
is the whole point of bug 2 and must be confirmed by the user: `Create a token` from the Azure
connect panel, and `Open in browser ↗` from a real PR.

---

## 6. Out of scope

Any forge-identity display — success toast, "Connected as …" chip, header badge, avatar (§1);
a Disconnect control; the 401/203 error-message wording (backend-owned,
`P72-forge-connect-fixes.md`); the `dev.azure.com/{org}/_git/{repo}` shorthand; any change to
`ui-reference.md`.
