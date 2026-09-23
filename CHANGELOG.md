# Changelog

All notable changes to Bonsai are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.6.0] — 2026-09-18

> **⚠️ PREPARED, NOT YET PUBLISHED (noted 2026-09-23).** The version is bumped in `package.json` and
> `src-tauri/tauri.conf.json` and the release is verified and clear to publish, but **no `v1.6.0`
> tag exists** — `git tag --list` ends at `v1.5.0` (2026-08-26). The `[1.6.0]` compare link at the
> bottom of this file therefore 404s until the tag is pushed. The date above is the date the
> release was prepared. Status and the two remaining recommendations are tracked in `TODO.md` under
> `🚀 RELEASE v1.6.0`. **Delete this note when the tag lands.**

Five new ways to read the commit graph — a first-parent view, branch solo/hide, folded linear runs,
an overview rail with a minimap, and colouring by author — plus an animated Replay mode, a Bonsai
tree style for the graph itself, and a local-only diagnostics page in Settings. External tools are
now picked from what the machine actually has instead of typed in, and a long accessibility,
contrast and security round closes screen-reader, legibility and credential-handling gaps.

### Added

- **The commit graph can be decluttered.** **First-parent only** follows each commit's first parent,
  so side histories that were merged in collapse out of the view and linear history renders as a
  single lane. Right-clicking a branch, a remote-tracking branch or a tag — in the sidebar or on a
  ref pill in the graph — solos or hides it, restricting the graph to the ancestry you chose. A
  filtered graph carries a chip saying so, so missing commits are never mistaken for missing data,
  and the branch filters clear in one action. Both choices persist across restarts.
- **Long uneventful stretches of history fold away.** With **Fold linear runs** on, consecutive
  commits that neither branch nor merge and carry no ref collapse into a single `⋯ N commits` row
  you can expand in place and re-collapse. Merge commits, fork points, ref-carrying commits, HEAD
  and stash entries are never folded, so the structure you navigate by stays visible. Folding
  composes with the declutter filters above.
- **An overview rail orients you in a large history.** A thin rail along the graph's right edge
  carries one tick per search match at its proportional position in the whole history — click a
  tick to jump to and select that match — and an on-demand minimap of the entire graph with a
  viewport thumb you can click or drag. It is hidden by default and appears while search is open or
  on hover near the graph's right edge; **Always show overview rail** pins it open. Nothing is
  computed or drawn while it is hidden.
- **The graph can be coloured by author, and shows a commit's parents on hover.** Settings →
  Appearance → **Graph colors** switches edges and lanes between **Branch lanes** and **Author**,
  using the same hues as the commit avatars. Hovering or selecting a commit emphasises its direct
  parent edges and nodes and releases cleanly when the pointer moves away; the graph repaints only
  when the hovered or selected row changes.
- **Replay mode plays a repository's history back.** A **Replay history** control over the graph
  animates commits, edges and refs appearing from oldest to newest, with play/pause, a scrub bar you
  can drag to any point, and a speed selector; leaving replay restores the graph's scroll position
  and selection exactly as they were. Under `prefers-reduced-motion` nothing autoplays — the
  scrubber is manual — and no animation work is scheduled when paused, finished or exited. The
  advancing frontier pulses as new growth appears; the Bonsai style's bespoke leaf-sprouting
  animation is not in this release.
- **The commit graph has a Bonsai style.** Settings → Appearance → **Graph style** switches between
  **Standard** and **Bonsai**: edges that taper so the trunk reads thicker than the tips, a
  bark-and-foliage palette over a paper-and-soil backdrop, and a five-petal blossom behind the HEAD
  commit (a single bud behind the selected one), with its own light and dark palettes and a
  **Season** choice of **Living**, **Spring** or **Autumn**. It is a reskin, not a re-layout —
  topology, ordering, ref pills, selection and virtualized scrolling over 20k+ commits are
  unchanged — and the gentle sway is disabled under `prefers-reduced-motion`.
- **Settings → Developer records what the app does, on this computer only.** With **Dev mode** on,
  Bonsai writes a rolling local log at a detail level you choose, covering the commands sent to the
  Git engine with their timing and result, which parts of the window re-render and why, and
  commit-graph frame timing. Nothing is sent anywhere: the page carries a **What Bonsai records**
  disclosure, repository and branch names are redacted unless you opt in to **Include raw repository
  names**, commit messages and credentials are never written to disk in any mode, and home-directory
  paths are masked in an exported session — each export stamps whether masking was actually in
  effect, rather than assuming it. **Show in folder**, **Export session…** (written to a fixed
  location, which the page names) and **Delete all…** are alongside, the page reports when logging
  cannot write to disk instead of failing silently, and the confirmation for **Delete all…** names
  its full blast radius — the log files *and* the usage counts — including when the number of log
  files could not be read. The usage counts kept for local statistics cover a rolling 90 days.
- **A commit that carries several refs is now actionable.** Its context menu offers a branch picker,
  so Merge, Rebase and the other branch actions can be aimed at the branch you mean instead of
  whichever ref happened to come first, and the "+N" chip that hid the extra refs is clickable
  rather than hover-only.
- **The git-activity log names what each run was aimed at.** A row in the activity dock now carries
  the branch or remote-tracking ref the operation targeted — "Push origin/main" rather than just
  "Push" — so a log of several fetches, pushes and commits stays readable after the fact instead of
  reading as a list of verbs. Refs that arrive with control characters in them are stripped before
  display, and a name too long for the row is truncated rather than allowed to push the row wider.
  Each row now also tells a screen reader that it is actionable and whether it is expanded, and its
  chevron is no longer a separate tab stop — so tabbing past a long log costs one stop, not *N*.

### Changed

- **Terminal and editor are chosen from a list of what is actually installed, not typed in.** The two
  free-text command boxes in Settings → General are gone, and the page's subtitle no longer
  advertises a group it does not have. Bonsai scans your machine for the terminals and editors it
  knows about and offers what it found, with the resolved program path under each entry so two
  installs of the same tool can be told apart; a **Browse…** button opens a native file picker for
  anything not on the list. The first scan on a cold machine takes a couple of seconds and shows a
  placeholder while it runs, and a **Rescan** action picks up a tool installed since Bonsai started.
  Typing a program name is no longer possible at all, which is the point: the old boxes let a value
  that was never a real program become the thing Bonsai launched.

- **Two stash actions instead of three.** The commit-panel `⋯` menu now offers **Stash** — the whole
  working directory, staged, unstaged and brand-new files alike — and **Stash staged**. The old
  "Stash all" (which quietly left untracked files behind) and "Stash all + untracked" are gone.

- **Tag synchronisation rides the background auto-fetch.** The sidebar's local-only / remote-only /
  diverged tag marks are refreshed as part of the existing five-minute auto-fetch cycle as well as on
  an explicit fetch. No new network trigger was added, and opening a repository still makes no
  network call of its own.

- **A pull request's file diffs open in the center overlay** instead of the narrow right panel, so a
  changed file gets the full width of the window to be read in.

- **The commit graph is usable with a screen reader.** It previously announced itself as a grid
  while containing no rows — the rows are drawn on canvas and have never existed as DOM. It is now
  one labelled, focusable region, and each settled selection is spoken once: the commit summary,
  author, relative date, "Row *n* of *N*", and the refs on that row. A visually-hidden hint says
  that the arrow keys move between commits and that the Menu key (or `Shift`-`F10`) opens the
  actions for the selected one.

- **Arrow-key navigation now moves focus into the graph**, so the keyboard actions menu for a commit
  is reachable that way instead of only after tabbing to it. Arrow keys pressed inside the
  git-activity log no longer also move the graph's selection behind it.

- **Dimmed text is legible throughout the app.** Every use of the dimmest text colour was
  enumerated — 124 of them — and 93 moved a step brighter wherever the text is something you have to
  read in order to act: timestamps, section labels, directory and branch names, option hints,
  counts, and the reflog's abbreviated object ids. Disabled controls, placeholders and purely
  decorative glyphs stay dim deliberately. Separately, ten toolbar and tab labels — the diff
  overlay's toggles, the right-panel tabs, the tab close button and the partial-staging gutter
  buttons — were below the contrast floor at rest and were raised.

- **Coloured text is legible on every surface it appears on.** Four further audits enumerated every
  place a hue — accent blue, danger red, success green, warning amber — is used as *text* or as a
  status letter rather than as a fill, and recorded a verdict for each one: 38 instances of hue text
  sitting on its own tint, 62 sitting on a neutral background, the eight status-badge letters, and
  the accent-as-text and white-ink-on-red cases. Anything you have to **read** moved onto a matching
  stronger ink; glyphs and decoration that only have to be **seen** were deliberately left as they
  were, so the app has not lost its colour. Two of the findings were plain bugs rather than contrast
  misses: one rule painted a colour token that is defined nowhere and had therefore always fallen
  back to a hardcoded literal, and another served one declaration to both an icon and a text
  selector, so only half of it was ever compliant.

- **The selected row is no longer flooded with the accent colour.** Selected rows, active list
  options and segmented-control segments now use a quieter selection fill with a leading accent bar,
  because white text on the accent fill fell below the contrast floor in dark mode. Primary buttons
  keep the loud accent fill and take dark ink instead, and their hover deepens in light mode rather
  than washing out.

- **Dependency refresh (maintenance).** Frontend majors: ESLint 9 → 10, Vite 7 → 8,
  TypeScript 5.9 → 6.0, jsdom 26 → 30, `@testing-library/jest-dom` 6 → 7, and
  `@vitejs/plugin-react` 5 → 6. Rust: `criterion` 0.5 → 0.8, `rand` 0.9 → 0.10,
  `reqwest` 0.12 → 0.13, `rmcp` 3.0 → 3.1, plus a full lockfile refresh. CI action pins moved to
  their current majors, including `tauri-action` v0 → v1.
- **Forge HTTPS now trusts the operating system's certificate store** rather than a root set
  bundled into the binary. This comes with `reqwest` 0.13, which replaced the bundled-roots
  feature with the platform verifier. In practice this is more compatible, not less — forge
  requests should now work behind a corporate TLS-inspecting proxy whose root is installed in the
  OS store. The trade-off: a stripped-down Linux environment with no system CA bundle at all can
  no longer fall back to roots baked into the binary. The TLS stack stays pure-Rust (rustls with
  the ring provider) and no OpenSSL is linked on any platform, as before.
- `keyring` stays on 3.x. Version 4 restructures onto `keyring-core` with renamed per-backend
  features and explicit credential-store registration, which changes how Bonsai selects each
  platform's native store — that is its own increment, not a dependency bump.

### Fixed

- **"Open in editor" now works on a standard VS Code install on Windows.** Bonsai resolved the bare
  name `code` against your `PATH` and took the first match, which on Windows is the extension-less
  shell script VS Code ships beside `code.cmd` — a file Windows cannot execute, so the action failed
  with "%1 is not a valid Win32 application" and, because the next rung looks for VS Code Insiders,
  the whole attempt gave up. Program lookup now prefers a real executable extension and only falls
  back to the bare name, and it ignores empty and relative `PATH` entries so a stray file named
  `code` or `git` inside the repository you have open can never be run as the program.

- **The commit graph no longer jumps back to the working directory while you are reading a commit.**
  A background refresh re-streamed the graph and re-picked the selection by row position, so a
  fetch, a watcher event or a branch switch could silently move you off the commit you had selected.
  The selection is now anchored to the commit id and survives the re-stream.
- **A branch switch no longer redraws the whole graph.** A checkout touches hundreds of files at
  once, and every one of them was waking the file watcher into a full re-stream. Watcher activity is
  now classified by which part of the repository changed, so a burst of working-directory writes
  refreshes the status list without rebuilding the history behind it.
- **A file marked for copying said "unchecked" when its box was ticked.** In the worktree copy
  dialog, a row whose preview could not be computed showed a red "unchecked" chip — inches from a
  checkbox that was, in fact, checked. It now reads "unknown" in a neutral colour, and both the
  chip and the row explain themselves to a screen reader. Red is reserved for "this will destroy
  something".
- **Settings tells you how an action turned out, in the page rather than behind it.** Every message
  Settings raised was drawn *behind* Settings' own overlay — measured, not inferred: a hit test at a
  message's centre returned the overlay, so its dismiss button could not be clicked at all. All
  fifteen places that report an outcome now do it where it can be read: as a note in the row that
  owns the action, present from the moment the page mounts so a screen reader announces every
  outcome — exporting twice used to be silent, because a live region does not re-fire on an
  identical string. A failed settings save keeps a toast for when Settings is
  closed and shows a banner when it is open, and the banner persists as long as the failure does
  instead of appearing once and vanishing. The remove-account failure stays in its dialog, the one
  place where a row note would have sat behind a second overlay.
- **When the open repository goes away, everything on top of it closes.** If the repository Bonsai
  had open was deleted or moved out from under it, the workspace emptied but its overlays and
  dialogs stayed on screen: the reflog, diff, composer and command-palette overlays, and 39 armed
  repo-scoped dialog flags — several of them gating a destructive operation (reset, force discard,
  force push, delete branch) and one that would have dispatched a Git operation on the next click,
  including an AI run holding a stale snapshot of conflict paths. All of them are cleared on the same
  teardown now, and anything that was waiting on a dialog's answer is settled rather than dropped —
  a pending commit-and-push is cancelled instead of silently taking its success path and clearing
  the message you had typed.
- **An image changed by a pull request shows the image comparison, not a "Binary file" placeholder.**
  PR-mode diffs now ask for the old and new blobs by commit id, so the side-by-side / onion-skin /
  swipe card works there exactly as it does for commit and working-directory diffs.
- **Brand-new files could be destroyed by a branch switch.** A dirty-tree switch auto-stashes with
  untracked files included and pops the stash on the far side. libgit2's untracked-restore phase can
  silently fail to write those files back — the target branch tracks that path (a binary is replaced
  by the branch's version, a text file gets conflict markers) or a directory now occupies it — while
  recording no index conflict. Bonsai read that as a clean apply and **dropped the stash**, deleting
  the only remaining copy. Every apply/pop now verifies each carried untracked blob against the
  worktree and keeps the stash unless all of them landed byte-identically, reporting the paths it
  could not restore. Regression tests cover the binary, file-vs-directory, and text-collision cases.
- **A failed re-apply after a branch switch no longer looks like data loss.** If popping the
  auto-stash fails once the switch has already happened, the operation now succeeds with a
  structured "not applied" outcome that names `stash@{0}`, instead of surfacing a bare error next to
  a working directory that suddenly looks empty.
- **Two borders in the merge editor now actually render.** The split-label underline and the
  OURS/THEIRS divider referred to a colour token that does not exist, so they had been invisible
  since they were written.
- **Settings no longer ignores your first keystroke after you close the identity menu.** Dismissing
  the menu with `Esc` left a stale keyboard handler in place for exactly one more keypress, so the
  next `Ctrl`/`Cmd`-`,` did nothing and you had to press it again. Only reproducible in a release
  build, which is why it looked like a flaky test for so long.
- **macOS: the folder-access prompt should stop coming back.** The release `.app` was only
  linker-ad-hoc-signed — no sealed resources and an unstable identifier — so macOS had nothing to
  anchor a permission grant to and re-prompted after you clicked Allow. The bundle is now properly
  ad-hoc signed, starting with this release. Gatekeeper's "unidentified developer"
  warning is unchanged; that needs a Developer ID and notarization.
- **Two different file statuses no longer show the same letter.** The badge beside a changed file was
  produced by six independently drifted tables, three of which gave "added" and "untracked" the same
  `A` — and the badge carried no accessible name at all, so the distinction was unavailable to a
  screen reader as well as ambiguous on screen. All eight places that render one now come from a
  single component, and each badge announces itself: Added, Modified, Deleted, Renamed, Type changed,
  Conflicted, Untracked, or Status unknown.
- **A branch name too long for its ref pill no longer gets cut off silently.** An over-wide ref was
  hard-clipped, which makes a truncated name look like a complete one, and the full text was only
  available by hovering. Long refs now stay on one line and ellipsize: the remote prefix gives way
  first, and the branch name itself is shortened only as a last resort.

### Security

- **Removing a forge account is no longer reported as done while the credential remains.** Removing
  an account discarded the result of deleting its token from the OS keychain and returned success
  regardless, so a refused delete left a live credential in the keychain with no record naming it —
  unreachable from the app, because every sweep in Bonsai iterates its own records. Deleting the
  token is now the operation. If it fails, nothing else runs — no settings write, no record drop —
  and the account stays listed so the removal can be retried; a key that was already absent counts
  as success, so that retry is real rather than inferred. Two further paths wrote a token and then
  swallowed the record write, which orphans a credential the same way — one of them reachable from
  the PR and Checks panels. Their rollback now reads the stored state *before* the write and deletes
  only a key that no other record already named, so re-adding an account cannot destroy the working
  credential it just refreshed. A leftover legacy key that refuses to delete is reported rather than
  blocking the removal, which would otherwise strand an account that is listed, disconnected and
  unremovable. The copy that reports all this describes **state, not the act**: it says the
  credential "is no longer in the OS keychain", never that it "was removed", because the
  already-absent case makes the act-verb false on exactly the retry the message recommends.
- **A credential-deleting command nothing could reach is gone from the IPC surface.** A per-host
  sign-out command stayed fully plumbed — registered, typed, bound, mocked — long after the
  interface stopped calling it, and it carried the orphaning defect above in a worse form: it
  dropped every record on a host even when some of the keychain deletes had failed, erasing the way
  back to the tokens that were left behind. Dormant privileged surface is itself the finding, so the
  command was removed rather than kept fixed and unused, and two callerless credential helpers went
  with it.
- **A cloned repository could no longer aim Bonsai's external tools at a path of its choosing.**
  A submodule's location comes from `.gitmodules`, which is authored by whoever wrote the repository
  you cloned — and Bonsai was joining that value onto the superproject's directory without checking
  the result stayed inside it. A rooted path (`C:/…`, `/…`) or a UNC share (`\\host\share`) replaces
  the base entirely rather than extending it, so "Open in terminal", "Open in editor" and "Show in
  folder" on such a row would have run against an attacker-chosen location — and merely testing
  whether a UNC path exists makes Windows dial that host and offer your credentials. Submodule paths
  are now validated to stay within the superproject before any absolute path is produced.
- **A submodule Bonsai refuses to resolve is still shown, just without actions.** Hiding a submodule
  that Git itself reports would be its own kind of wrong, so the row still lists with its name and
  status; it simply carries no absolute path, which means **"Open in new tab" is disabled and every
  external-tool item is absent from its menu**. No raw path is ever used as a fallback.
- **The content-security policy now also pins `form-action`, `base-uri` and `object-src`.** Script
  sources were already restricted to the application itself, with no inline script and no `eval`.
  These three directives close the remaining ways a compromised renderer could aim a form submission
  somewhere else, rewrite how relative URLs resolve, or embed a plugin object.
- **Staging a file could follow a symlinked directory out of the repository.** Full-file staging
  validated paths only lexically — it never checked that the resolved location stayed inside the
  working tree, unlike partial staging, discard and conflict resolution, which all did. A repository
  containing a symlinked ancestor could therefore have an out-of-repository file read into its
  object database by a single stage. Every path is now resolved against the working directory before
  the index is touched, and a batch containing one escape stages nothing at all; a symlink you stage
  directly is still stored as a link. This affected staging from the app as well as through the MCP
  server, and it was not reachable on a stock Windows setup, where Git materialises symlinks as
  ordinary files.
- **The MCP server's write tools no longer inherit the user interface's assumptions.** Staging
  through a model is now limited to paths the repository status actually reports, so an ignored file
  such as `.env` can no longer be staged and committed by an agent that simply named it — the tool
  description had promised this, and nothing enforced it. Commit-producing tools in a repository
  with runnable git hooks are refused rather than running those hooks behind a disclosure a headless
  server cannot show. A test now snapshots every tool description, because those descriptions are
  the contract a model reads before it invokes a destructive operation.
- **The program Bonsai launches can only come from detection or a native file dialog.** There is no
  free-text terminal or editor command: the stored setting is either empty or one of a fixed catalog
  of tool ids, and a command typed into an older version migrates to a catalog entry or to nothing —
  the migration can manufacture neither a path nor a custom id. The one setting that does hold a
  filesystem path is written only by a native file dialog the backend opened itself; in Rust it is a
  distinct type with a single, deliberately grep-able constructor, so a string arriving from the
  renderer no longer even type-checks into a tool scan or a launch. Validating the *shape* of a
  program string cannot make it safe — an audit measured what such checks buy and it was neither
  "not arbitrary execution" nor "at most one argument" — so there is no free-text entry to validate.
  Control characters and bidi overrides are refused in a picked path, since in a filename they exist
  only to make a picker row read as something other than what launches. Two residuals are named
  rather than papered over: a configured tool still runs with Bonsai's privileges, and the built-in
  "open in terminal" rungs still start in the repository directory, because the alternative —
  building a command line out of a repository-authored path — would be the worse hazard.

- **Two dependencies with known defects were replaced before this release shipped.** The TLS library
  Bonsai uses to reach forges and to download its own updates accepted TLS 1.3 handshake messages
  that arrived at the wrong encryption level. The handshake transcript stays authenticated, so this
  was not a way for a network attacker to alter or complete a connection; the effect is that a peer
  could send in plaintext what the protocol requires to be encrypted without the connection being
  refused (RUSTSEC-2026-0285). Separately, the native bindings behind the SSH transport were pinned
  to a release their authors had since withdrawn. Both now sit on fixed versions.

### Performance

- **A background refresh that finds nothing now changes nothing.** A six-minute session was measured
  at 26,334 sidebar branch-row renders across 43 refresh rounds; the file watcher was not at fault
  (10,280 filesystem events correctly debounced to 43 fires) — the amplification was all downstream.
  Refresh rounds now keep the previous value whenever what they read is structurally equal, so a
  watcher round that finds nothing new commits no state at all, and the sidebar's filtered lists,
  rows and menu handlers are memoized behind comparators instead of being rebuilt every round. One
  ref change went from 1,012 renders to 8, and a branch row from 1,000 renders across 500 instances
  to 2 across 1. A background refresh also no longer greys out the refresh button: progress belongs
  to something you did, not to filesystem-event timing.

## [1.5.0] — 2026-08-26

Accurate pull-request diffs computed locally, a per-branch CI checks view, and a dedicated forge
column in the graph — plus a large git-action performance round and an internal file-size cleanup.

> **There is no 1.4.0 release.** This work was first cut as `1.4.0` (commit `5a0bf11`), but the
> release workflow had to be reworked mid-cut (`4abab3d`, `713dc6d`) and GitHub releases are
> immutable, so the same content was re-cut and published as `1.5.0` (`40949e1`). No `v1.4.0` tag
> exists; `v1.5.0` is the shipped artifact.

### Added

- **Pull-request changed-files list with inline per-file diffs**, computed **locally** from the
  base…head merge base rather than from forge-reported stats. This gives correct line counts on
  every forge — including Azure DevOps, which previously reported +0/−0 — and works for GitHub,
  GitLab, Bitbucket, and Azure DevOps. Binary and rename cases are handled.
- **Per-branch "Checks" tab** in the right panel showing the branch's CI status, refreshed on
  fetch/pull/push.

### Changed

- **PR and CI badges moved into a dedicated forge column** in the commit graph, keeping the graph
  lanes clean while surfacing forge state alongside each commit.

### Fixed

- Make the selected diff-file row clearly visible.
- Pull-request panel: correct empty-state header counts, dim stale results while refetching, and
  fall back to the Azure TFS API path where needed.

### Performance

- **Git-action performance round two.** Refresh rounds now reuse a single repository handle
  (thread-local handle cache, composite mutations open the repo once), cutting per-refresh-round
  repository opens from roughly nine-to-eleven down to about one. The graph-layout cache is capped
  at 50k nodes.

## [1.3.0] — 2026-08-24

Checkout from the graph, a live git-activity log, a complete lucide-react icon system, and a
keyboard-navigable sidebar — plus a batch of performance, accessibility, and security hardening.

### Added

- **Check out a commit into detached HEAD** directly from the graph and ref pills, and
  fast-forward a local branch when checking out its remote-tracking counterpart.
- **Git-activity log dock** — a live stream of git phases and progress, backed by a streaming
  execution seam with bounded line-event emission.
- **Sidebar single-click reveals the ref in the graph**, with automatic tag synchronisation on
  fetch.
- **Keyboard-navigable sidebar** (`role=tree` with roving tabindex) and improved graph a11y
  (grid semantics, light-mode lane palette, higher-contrast ref pills).
  > **The graph's "grid semantics" have since been removed — see [1.6.0].** They shipped in
  > 1.3.0 exactly as described here (`role="grid"` + `aria-rowcount` on the graph scroller,
  > `src/graph/GraphCanvas.tsx` at tag `v1.5.0`), but a `grid` with no `role="row"` children is
  > malformed, and the graph's rows are canvas pixels that never exist as DOM. The graph is now a
  > labelled focusable group whose selection is announced through a live region.
  > `role="grid"`, `aria-rowcount`, `role="row"` and `aria-rowindex` are **forbidden** on the graph
  > by `docs/contracts/ui-reference.md` §4.1 (revised 2026-08-31), and the 2026-08-22 design review
  > that recommended them is archived as **SUPERSEDED — do not implement**. The light-mode lane
  > palette and the higher-contrast ref pills from this release are unaffected.
- **First-time per-repo git-hook execution disclosure** and an AI conflict novel-content gate.

### Changed

- **Migrated all chrome icons to lucide-react**, replacing the previous inline-SVG/glyph set
  across the header, toolbar, file rows, onboarding, sidebar, and AI dialogs/menus.

### Fixed

- Resolve the `claude` CLI via a login-shell PATH fallback on macOS/Linux.
- Don't dismiss the clone dialog on an accidental backdrop click.
- Dialog focus management and modal semantics.
- Guard raw-fs git paths against symlinked-ancestor escape; validate MCP scope and reject
  control characters in credential URLs.

### Performance

- Per-repo layout cache (redecorate instead of re-walking the graph), scoped reason-aware refresh
  rounds, and elimination of double-refreshes on branch/fetch/push.

## [1.2.0] — 2026-08-22

Pull request review and management from inside Bonsai — merge and close/decline PRs across all four
supported forges — plus multi-account forge sign-in, tag synchronisation, and color-coded commit
identities.

### Added

**Pull requests**
- **Merge and close/decline a pull request from the PR panel.** The PR detail view now has a Merge
  action with a method picker (merge / squash / rebase, filtered to what the forge actually
  supports) and a Close/Decline/Abandon action that uses the right verb for each forge. Works for
  **GitHub, GitLab, Bitbucket, and Azure DevOps.** A PR that the forge reports as not mergeable
  fails with a clear message — nothing is forced and no conflict is auto-resolved.

**Forge accounts**
- **Sign in to more than one forge account.** Connect several accounts per host, switch the active
  one from the header, and re-authenticate an expired token in place. A new Accounts category in
  Settings lists every connected account.
- **Fine-grained-token guidance and branch pickers in the Open-PR form.** The form spells out
  exactly which token scopes it needs and offers source/target branch dropdowns instead of
  free-text.

**Sidebar**
- **Tag synchronisation.** The sidebar marks tags that are local-only, remote-only, or diverged from
  the remote, with per-tag resolve actions and a rollup summary — the same push/pull clarity
  branches already had.

**Identity**
- **Color-coded commit identities.** Assign a color to each Git identity so the header control shows
  at a glance which one your commits will carry.

### Changed
- **Commit panel overhaul.** A visible per-file stage toggle, a safer discard flow, empty-state
  hints when there is nothing to commit, and a more compact footer that collapses the commit
  modifiers into a menu — with accessibility and wording passes throughout.

### Fixed
- After you stage a file in the tree view, selection now advances to the visually next file instead
  of jumping elsewhere.
- Redundant refreshes are coalesced and the file watcher no longer reacts to its own writes, so
  Git operations no longer trigger a storm of rescans.
- Stash operations now verify they are acting on the stash you targeted, guarding against a
  wrong-target overwrite if the stash list changed underfoot.
- Removing or de-initialising a submodule with uncommitted changes now requires an explicit force,
  matching the rest of Bonsai's destructive-action guards.
- On Windows, a racy-Git phantom "modified" status no longer flickers in the working-directory list.
- The command palette's highlighted row resets correctly when the visible results change.
- An AI helper's stderr drain no longer overruns its intended time budget.

## [1.1.0] — 2026-08-20

A rebuilt Settings surface, fixes from the second full-project audit (2026-08-18), and a batch of
fixes for problems found in real use after 1.0.0 — Git discovery after an auto-update, forge
connect, submodules, and accessibility.

### Added

**Settings**
- **Settings is a two-pane window now.** Categories on the left, the settings you asked for on the
  right — instead of one narrow column with eleven sections stacked in it that you had to scroll
  through to find anything. `Ctrl`/`Cmd`-`,` opens it.
- **Search across every setting.** Type in the search box at the top and Settings shows the matching
  rows from every category at once, with the matched words highlighted and the category each row
  came from named next to it. You can find a setting by what it does, not only by its exact label.

### Changed
- **Who you commit as now lives in the header, not in Settings.** The header carries an identity
  control showing your initials: open it to see the name and email your commits will carry, and
  click a saved identity to switch to it. It reads what Git actually resolves — a repository's own `user.*`
  overrides your global config — which the old badge buried in Settings got wrong, showing nothing
  at all for the ordinary case of a global-only identity.
- **Every setting looks like what it is.** Real toggle switches instead of bare checkboxes, and
  segmented controls instead of buttons labelled with the state they were already in. Rows carry
  help text under the label, and any row you have moved away from its default grows a small reset
  arrow — so you can see at a glance what you have changed, and put back just that one thing.
- **Git config is visibly per-repository.** The Git config category now says which repository it is
  editing and has an explicit Local / Global switch, so you can no longer change a global setting
  while believing you changed a local one. With no repository open it says so plainly instead of
  showing an empty form.
- **Fifteen small sidebar controls are now easier to hit.** Section toggles, the per-section add
  buttons, and a handful of other tiny controls were enlarged to a comfortable 24px target — the
  box grows, the icon inside it does not, so the sidebar's spacing and rhythm are unchanged.
- **Windows now ships as a single NSIS installer.** The MSI is gone. The auto-updater points at the
  NSIS build, which relaunches Bonsai with your own environment (see below), so the MSI's
  environment problem cannot recur.

### Fixed
- **A slider's own minimum value can be typed again.** In a field whose minimum was 24, typing `24`
  produced `240`: the first keystroke was snapped up to the minimum before the second one arrived.
  Typed values are now left alone while you type and only checked when you finish.
- **Settings changed just before you quit now survive the restart.** A setting toggled in the last
  moments before closing Bonsai could be lost; pending changes are flushed on the way out.
- **Accessibility across Settings** — visible focus rings on switches and segmented controls,
  keyboard navigation through the category rail, small controls enlarged to a 24px target, and
  contrast fixes in both themes. Several controls that shared one name with a neighbour (two rows
  both reading "Limit", two both reading "Interval") now say which is which, so a screen reader
  can tell them apart.
- **A corrupt repository no longer hangs the app.** Operations that hit a truncated or corrupt
  loose object — status, the commit graph, streaming graph loading, and the background history
  index — now time out and return a clear error instead of freezing forever. This closes the
  known limitation shipped in 1.0.0; committing in such a repository is deliberately not cut off
  by the timeout, so a slow-but-valid commit can never be aborted halfway.
- **A Git hook that fails to start is now reported.** A `pre-commit` or `post-commit` hook that
  could not be launched used to look exactly like "no hook installed"; commit results now carry a
  visible warning, and the background history index reports commits it had to skip.
- **Forge requests no longer follow redirects**, and response bodies are read with a size cap —
  so a misbehaving or malicious server can neither bounce an access token through a redirect nor
  stall the app with an unbounded response.
- **Closing a repository tab now cancels its running AI conflict resolutions**, so a Claude run
  can no longer keep running (and spending) against a tab that no longer exists.
- A graph stream that delivers a malformed batch now surfaces as an error instead of silently
  freezing the graph, and the bulk-AI confirmation dialog now blocks the workspace shortcuts
  behind it like every other modal.
- **Git is now found even when it is installed just for you.** If Git is on your user PATH but not
  the machine PATH — and Bonsai was launched from somewhere without Git on PATH — Bonsai now finds
  it anyway, so history search and credential helpers work. And when Git genuinely cannot be found,
  Bonsai says "git not found" in one place instead of misreporting it as an authentication failure
  and repeating the wrong message.
- **After an auto-update, your environment comes back.** The updated app used to relaunch with the
  installer's stripped-down environment, which could break Git, your SSH agent, proxies, and the
  editor / terminal / file-manager integrations until you restarted it by hand. It now launches
  with your normal environment, the same as opening it from the Start menu.
- **An Azure DevOps token scoped only "Code (Read & Write)" now connects.** Bonsai used to check
  the token against an endpoint that needs an extra profile scope, so the exact token the connect
  screen tells you to create was rejected with a misleading error. It now validates against the
  code endpoint it actually uses. An invalid or expired token now gives a clear authentication
  error instead of "malformed response", and the "Create a token" and "Open in browser" links
  actually open your browser.
- **A submodule with orphaned Git data reconnects instead of failing.** A submodule whose working
  folder was empty but whose Git data still lived under `.git/modules` used to fail to update with
  "attempt to reinitialize"; Bonsai now reuses that data and restores the files. Initializing a
  submodule now also checks out its files, so the badge and the success message agree.
- **Toast messages are readable in both themes, and colour is no longer the only signal.** Toast
  text was below the WCAG contrast bar; it now meets it in light and dark. Each tone also carries a
  small shape glyph (error, info, success, warning), so the kind of message is clear without relying
  on colour alone.

## [1.0.0] — 2026-08-18

The first public release. Everything below landed after `0.3.0`: search and a command palette,
external-tool launching, a much richer commit graph, seven local-AI assists, real commit signing,
your own Git hooks, word-level and image diffs, pull requests from four forges (beta), and a
pre-release hardening campaign that rewrote large parts of the test suite and fixed 48 bugs.

(`v0.3.1` is tagged at the same commit as `v0.3.0` — it carried a version bump and no user-visible change, so it has no entry of its own.)

### Added

**Finding things**
- **Search across history** — by commit message, author, or path, and by file *content* (literal
  or regex pickaxe). Matches are highlighted on the commit graph with jump-to-next/previous, so
  you never lose your place. `Ctrl`/`Cmd`-F.
- **Command palette** (`Ctrl`/`Cmd`-K) — fuzzy-launch any action, jump to a branch, tag, or
  commit, or start a search, without hunting through menus. Destructive actions still go through
  their usual confirmation.
- **Type-to-filter boxes** on the branches, remotes, and tags lists in the sidebar, for repos with
  hundreds of refs.

**Working with your other tools**
- **Open in terminal, reveal in file manager, open in editor** — for the repository, a worktree,
  or a submodule, from the context menus, the tab menu, and the toolbar. The terminal command is a
  per-OS template that is auto-detected and editable in Settings; paths with spaces are handled,
  and a failed launch tells you so instead of doing nothing.

**Commit graph**
- **Row detail you can turn on and off individually** — short SHA, author, dates (with the full
  timestamp on hover), a choice of author or committer date, an ahead/behind chip on branch tips,
  and a signature badge. All under Settings → Graph.
- **Compact mode** for denser rows, for people who want more history on screen than decoration.
- **Progressive graph loading.** Large histories now stream into the view in batches instead of
  arriving as one large payload, with lane colors stable from the first batch, so you can start
  reading and scrolling before the walk finishes.
- **Faster on large repositories.** Bonsai now writes Git's `commit-graph` file when a repository
  is opened and after fetching; Git and Bonsai both read it automatically. The repository-health
  scan got roughly five times faster.

**AI assists** (all run through the Claude Code CLI on your machine, under your own subscription)
- **"Why did this line change?"** in blame — an explanation of a single line's history rather than
  just who touched it last.
- **"Explain this commit"** from any graph node, grounded in the commit's full message so the
  answer is about *why*, not a restatement of the diff.
- **Suggested branch names** from your current changes, offered in the create-branch dialog.
- **Commit composer** — turns a sprawling working tree into a proposed series of logical commits,
  each with its own message. You reassign, edit, merge, and drop groups before anything happens;
  applying is an all-or-nothing staged sequence, and your files on disk are never modified.
- **Ask Bonsai to…** — describe what you want in plain English and get back a structured,
  previewed, confirm-gated operation. The model may only pick and fill in one of ten known
  operations; it never produces a command line, and the planning step cannot change your
  repository at all.
- **Release notes** — grouped, categorized Markdown for a tag or ref range, or "since the last
  tag", ready to copy or edit.
- **Semantic history search** — ask a question about the project's history and get a prose answer
  grounded in the real diffs, with the commits it drew on ranked and clickable. The index is built
  locally, incrementally, and outside your `.git` directory.

**Trust: signing and hooks**
- **Commit signing with SSH or GPG** — honours `commit.gpgsign` with a per-commit override and a
  "will sign" indicator in the commit box. Annotated tags honour `tag.gpgSign` too. If signing is
  requested but no key is configured, you get a clear error instead of a silently unsigned commit.
- **Signature verification** — verified / unverified / unsigned badges on graph rows and a
  signature line in commit details, with the signer and key.
- **Your Git hooks now run.** `pre-commit`, `commit-msg`, `post-commit`, and `pre-push` execute
  around commit, amend, merge, and push. A hook that blocks shows its output in a dialog and stops
  the operation — never a silent success — with "Commit anyway (skip hooks)" as the explicit
  escape hatch, a per-commit skip checkbox, and a per-repository "Run git hooks" toggle.

**Everyday Git parity**
- **Rename a branch**, including the one you have checked out, preserving its upstream and reflog.
- **Non-fast-forward pull** — instead of just refusing, Bonsai now offers Merge or Rebase and runs
  the one you pick, behind a confirmation.
- **One-click Undo** — reads the reflog, tells you what it would undo, how it would do it, and how
  destructive that is, then does it on confirmation. It refuses to run a destructive undo over a
  dirty worktree.
- **Submodule add, deinit, and remove**, on top of the existing list / init / update / sync.
- **Cherry-pick from branch and tag pills**, not just the commit row, with an editable commit
  message and an automatic stash of a dirty worktree. Revert gained the same auto-stash handling.

**Diffs**
- **Word-level (intraline) highlighting** — within a changed line, only the parts that actually
  changed are emphasized. Toggleable per diff view.
- **Image diffs** — old and new side by side, as an onion-skin fade, or under a swipe divider.
  SVG keeps its text diff, where it is more useful.

**Pull requests (beta)**
- **Pull requests without leaving Bonsai** — connect a GitHub, GitLab, Bitbucket Cloud, or Azure
  DevOps repository with a personal access token (stored in your OS keychain) to list, read, and
  create PRs from a right-panel tab: labels, mergeability, changed files, and review comments
  inline.
- **PR and CI badges on the graph** — branch-tip pills show the pull-request state and the check
  rollup for that commit; clicking a badge opens the PR. Off by default, toggleable in Settings.
- **AI-drafted PR descriptions** from the commits in the range, filled into the create form for
  you to edit — never submitted automatically.

**Interface**
- **Grouped context menus** — the rebase and reset variants collapse into one row each with a
  hover submenu; clicking the parent runs the sensible default. Distinct icons per action, and
  destructive entries are marked in red.
- **New Worktree dialog** — a searchable branch picker that scales past a hundred branches, a
  wider card with full paths, and per-category select-all for the files you copy across.

**AI merge-conflict resolution**
- **Live AI activity dock.** AI conflict resolution now streams its progress into a collapsible
  full-width dock at the bottom of the window: the model's output as it arrives, the tools it
  calls, elapsed time, and the run's cost. The dock is resizable and remembers its height and
  collapsed state.
- **Cancel.** An AI conflict run can be stopped at any time. Everything logged before the cancel
  stays on screen, and nothing is written or staged.
- **No hard time limit by default.** A conflict run is no longer cut off after 90 seconds. It is
  bounded instead by an idle watchdog (no output for 5 minutes), with an optional absolute cap you
  can enable yourself.
- **Answer Claude mid-run.** If the model needs a decision it cannot make alone, the run pauses,
  the question appears in the dock with a reply box, and your typed answer continues the same run
  through to a proposal. A run waiting on you is never timed out.
- **Per-file AI state.** Each conflicted file now tracks its own run and its own result, so a
  proposal is no longer lost when you click another file while a run is in flight — and a run on
  one file no longer disables the AI button on every other file.
- **"Resolve all with AI".** Available from the Conflicts section header and from the merge
  banner when there are at least two text conflicts. Claude sees the conflicts together, so a
  change split across several files can be resolved coherently. Each file gets its own outcome:
  a file the model fails on does not invalidate the rest.
- **Read-only repository access for AI conflict runs.** Claude may now use `Read`, `Grep` and
  `Glob` — and nothing else. It cannot write files, stage anything, or run commands. Being able
  to read the surrounding code is what lets it match your project's conventions, and it is the
  real fix for conflict runs that used to fail after 90 seconds with nothing to show: the run had
  previously been started with no tools at all, leaving the model blind to the repository.
- **Reads are fenced to the repository.** An attempt to read outside the repository folder is
  refused, and the refusal is shown as a line in the AI activity dock.
- **Eight AI run settings** under Settings → AI: repository access (Read-only or none), live log
  on/off, partial-message streaming, idle timeout, optional absolute time cap, maximum
  interactive turns, optional spend limit, and the bulk payload size cap. There is no
  write/edit/shell option, by design.
- **A HEAD guideline that stays put.** The dashed line connecting the working-directory row to
  the checked-out commit no longer vanishes after a few rows of scrolling, and when the
  checked-out commit is off-screen an edge marker shows which way it lies.
- **Cozy / Compact panel density** (Settings → Appearance) for the right panel, independent of
  the commit graph's compact rows.

### Changed
- The right panel's fixed controls were reorganised and tightened, giving about 115px — roughly
  five more file rows in the cozy default — back to the changes tree. Compact density frees a
  further ~30px. "Stash all" moved into an overflow menu; all three stash scopes are still there,
  and the sidebar keeps its one-click stash.
- **Force-push-with-lease is now atomic.** It runs Git's own
  `--force-with-lease` / `--force-if-includes` instead of checking the remote and then pushing, so
  a commit that lands in the gap between those two steps can no longer be overwritten.
- **Working-directory status matches `git status` exactly** for a tracked file deleted and
  recreated as an untracked copy: two rows (an unstaged delete plus an untracked file) rather than
  one misleading rename row. Staged renames are unaffected.
- The embedded MCP server now returns a compact summary in its text block instead of a second copy
  of the whole payload; the full data is still in the structured content every client reads.
- Toasts are opaque instead of ~88% see-through over the commit graph, so they stay readable.

### Fixed
- The AI consent dialog described what happens inaccurately on two counts. It now states that
  Claude may read other files in the repository and that whatever it reads is sent to Anthropic
  with the request, that its tools are read-only and out-of-repository reads are refused, and
  that Bonsai writes to your files only when you apply a result — with the one exception spelled
  out below.
- **Stashing "staged only" could destroy work.** If a file had a staged deletion but had been
  rewritten on disk, the new content was lost. It is now folded into the stash.
- **Stash and auto-stash operations could target the wrong stash.** They are now addressed by
  commit id, so a stash list that changes between what you saw and what you confirmed can no
  longer apply or drop the wrong entry — and an auto-stash that vanished mid-operation is reported
  instead of quietly applying a stranger's.
- **Stale-branch cleanup could delete your default branch** when the base was given in another
  form (`refs/heads/main`, a commit id, a tag). The base is now protected by resolved identity, and
  tips are re-checked at delete time.
- **A saved HTTPS credential could be reused for the wrong account** on the same host. Cache keys
  are now path-scoped, and a credential the server rejects is evicted immediately instead of
  lingering for its full lifetime.
- **A corrupt bisect or interactive-rebase state file no longer wedges the app.** Reset and abort
  now clear the state, leave HEAD alone, and explain what happened, instead of failing forever with
  every mutation blocked.
- **Aborting a rebase no longer overwrites untracked files** that the abort's reset would have
  clobbered — the same guard the other sequencers already had.
- **Mutations are blocked during an active bisect** (commit, amend, reset, stash, merge, rebase,
  cherry-pick, revert), including in a detached-HEAD bisect where Git itself reports a clean state.
- **"Discard all" is all-or-nothing again** — a directory in the selection no longer left some
  untracked siblings deleted and others not. The confirmation now lists the untracked files that
  will be permanently deleted.
- Plain "Rebase X onto Y" is now behind a confirmation, matching reset and force-push. The
  delete-branch dialog no longer claims a branch is "fully merged" when it is not.
- Merge-conflict handling got six fixes: conflicts sort above staged and changed files, the first
  conflicted file opens automatically once per conflict episode (without re-opening one you just
  closed), conflict rows lost their misleading expand chevron, the conflict editor is
  syntax-highlighted in both themes, and its header no longer shows an irrelevant File/Diff/Split
  toggle or a duplicated path.
- The operation banner lets its actions wrap instead of squeezing its own text down to nothing.
- The MCP server no longer leaves a dead server showing as "enabled" after a failed restart.

### Notes
- **`Resolve automatically` writes without a review step.** Under Settings → AI assistance, the
  `Resolve automatically` autonomy mode writes Claude's marker-free results to your files and
  stages them with no review; only results that still contain conflict markers open as proposals.
  Both autonomy modes now state their consequence next to the choice itself. `Propose & review`
  (the default) never writes until you apply a result.
- **A bulk resolve can cost more than one Claude run.** Bonsai splits the conflicts into as many
  sequential runs as the payload size requires, all against your Claude quota; `Cancel all` stops
  the remaining ones. No file is ever silently truncated — a single file too large to send is
  reported as failed for that file alone.
- AI conflict features still run entirely through the Claude Code CLI installed on your machine,
  under your own Claude subscription. Nothing goes to Bonsai servers. The same is true of every
  other AI feature in this release.
- **The forge / pull-request features ship as beta.** PR listing for GitHub, GitLab, Bitbucket,
  and Azure DevOps, the PR/CI badges on the graph, and AI-drafted PR descriptions have not yet
  been verified against real access tokens for every provider, so expect rough edges there.
  Everything else is release-ready.
- **Progressive loading is not instant first paint on a huge repository.** The topological ordering
  Bonsai uses walks the whole reachable history before it can hand over the first row (about 0.7 s
  for 40k commits, 1.4 s for 120k, 2.3 s for 200k on a warm release build). What streaming buys you
  is a stable, progressively filling graph and no giant single transfer — not a first row in
  milliseconds. A faster ordering is planned for a later release.
- **AI-composed commits deliberately bypass your hooks.** A `pre-commit` hook that re-stages files
  would corrupt the composer's carefully partitioned plan, so composer commits run with hooks off.
  Every other commit path runs them.
- **Known limitation.** A repository with a truncated or corrupt loose commit object can hang the
  app; the underlying library spins on it and there is no bounded way to detect it first. It is
  only reachable through on-disk `.git` corruption. A fix is planned.
- **Quality.** A pre-release hardening campaign audited every public function in the Rust core, the
  command layer, the MCP servers, and the frontend, fixed 48 bugs (the most serious of which are
  listed above), and grew the suite to a full-workspace gate of unit, integration, component,
  end-to-end, property-based, and corrupt-input tests run on Windows, macOS, and Linux.
- Installers still ship **unsigned**. See the README for the one-time Windows SmartScreen /
  macOS Gatekeeper approval steps. OS code signing is planned for a later release
  (`docs/code-signing.md`).
- **Verification in progress.** Everything above has passed the automated suite and the browser
  harness. Native-window confirmation passes are still outstanding for the pull-request features,
  the graph badges, progressive loading, and the AI conflict dock — the parts needing real tokens,
  a real Claude CLI run, or human eyes on the canvas. See `docs/contracts/P62-user-checklist.md`,
  `P63-user-checklist.md`, `P64-user-checklist.md`, `P65-user-checklist.md`,
  `P67-user-checklist.md`, and `P68-user-checklist.md`.

## [0.3.0] — 2026-08-05

An interactive side-by-side diff view, plus continued release-readiness hardening.

### Added
- Interactive side-by-side (split) diff view with synchronized horizontal scrolling,
  copyable selection, and auto-advance to the next file after staging.
- MIT `LICENSE` and end-user `README.md`.
- Top-level and per-pane React error boundaries (commit graph, diff view, conflict editor)
  so a render error is contained to a pane instead of white-screening the whole app.

### Changed
- Release builds no longer emit internal self-test/perf logs to the console.
- A Content-Security-Policy is now enforced for the app webview.
- The release workflow publishes releases directly (not as drafts) so the auto-updater's
  `latest.json` resolves via the `releases/latest` URL.

### Fixed
- The macOS `universal-apple-darwin` release build now installs the extra Rust targets onto
  the toolchain pinned by `rust-toolchain.toml` (via `actions-rust-lang/setup-rust-toolchain`),
  fixing the "target x86_64-apple-darwin is not installed" build failure.

### Notes
- **Verification in progress.** A large set of already-built features (force-push-with-lease,
  reflog viewer, bisect, Git config editing, first-run onboarding, per-line discard, named
  worktrees, background auto-fetch, the AI what-changed digest, and the repo-health
  dashboard) has passed automated tests and the browser harness but is still completing a
  formal native-window verification pass ahead of a future 1.0.0 tag.
- Installers ship **unsigned**. See the README for the one-time Windows SmartScreen /
  macOS Gatekeeper approval steps. OS code signing is planned for a later release.

## [0.2.0] — 2026-08-05

The MVP and first productization phase. Highlights:

### Added
- Rich commit graph rendered on canvas — multi-colored branch lanes, curved
  fork/merge edges, ref pills (branches, `origin/*` remotes, tags, HEAD), virtualized for
  large histories (20k+ commits).
- Three-pane workspace: branches / remotes / tags sidebar, commit graph, and a status +
  diff + commit-details panel.
- Working-directory status with file-level staging/unstaging and commit.
- Diffs for both working-directory changes and any selected commit (vs. its first parent).
- Branch management: list, create, checkout, delete.
- Remotes: fetch, fast-forward-only pull, and push, with credential handling.
- Merge, rebase, and stash workflows with conflict resolution.
- Multiple repositories open in tabs; the last repo reopens on launch.
- Tauri v2 auto-update scaffolding (behind Bonsai IPC) and a first-run onboarding overlay.
- An embedded MCP server exposing structured Git data (graph, diffs, conflicts) to AI tools.

[Unreleased]: https://github.com/danpercic86/bonsai/compare/v1.6.0...HEAD
[1.6.0]: https://github.com/danpercic86/bonsai/compare/v1.5.0...v1.6.0
[1.5.0]: https://github.com/danpercic86/bonsai/compare/v1.3.0...v1.5.0
[1.3.0]: https://github.com/danpercic86/bonsai/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/danpercic86/bonsai/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/danpercic86/bonsai/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/danpercic86/bonsai/compare/v0.3.1...v1.0.0
[0.3.0]: https://github.com/danpercic86/bonsai/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/danpercic86/bonsai/releases/tag/v0.2.0
