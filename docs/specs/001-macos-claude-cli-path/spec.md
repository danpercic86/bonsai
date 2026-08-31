# GUI-launched Bonsai can't find the `claude` CLI on macOS/Linux

**Status:** done
**Created:** 2026-08-24

## Problem
When a user launches Bonsai by double-clicking the app, or via Spotlight/Dock, every AI feature
reports the CLI as unavailable ("Claude Code CLI not found on PATH") even though `claude` is
correctly installed and works fine from a terminal. The app is launched by the OS's app launcher
in this case, which hands the process a minimal `PATH` that does not include directories a user's
shell only adds via `.zshrc`/`.zprofile`/`.bashrc` (e.g. `~/.local/bin`, `/opt/homebrew/bin` on
Apple Silicon, nvm/volta shims, `~/.cargo/bin`, etc.). Bonsai currently only searches whatever
`PATH` the OS launcher gave it, so this is the common case, not an edge case — most users install
the CLI through a method that lands it in one of these shell-only locations. This makes every AI
feature (commit-message generation, conflict resolution, PR descriptions, etc.) silently
unusable for anyone who doesn't happen to start Bonsai from a terminal.

## Goals
- Bonsai finds an installed `claude` CLI regardless of how it was launched (double-click,
  Spotlight, Dock, or terminal), as long as it's discoverable from the user's normal login shell
  environment or another well-known install location.
- No behavior change for users where discovery already works today (terminal launches, explicit
  overrides used in tests).
- The fix applies to the platforms affected by this class of problem (macOS and Linux); Windows'
  existing PATH/PATHEXT handling is unaffected.

## Non-goals
- No new UI, settings, or user-facing configuration for pointing Bonsai at a custom `claude`
  location in this fix — that's a separate feature if it turns out to still be needed after this
  fix.
- Not changing what happens when `claude` truly isn't installed anywhere findable — the existing
  "not found" messaging and behavior for that case stays as is.
- Not addressing other CLIs Bonsai shells out to (e.g. `git`) — this spec is scoped to the
  `claude` CLI discovery path used by AI features.

## User-facing behavior
- A user who has `claude` installed and working in their terminal, then double-clicks Bonsai (or
  opens it from Spotlight/the Dock) and uses any AI feature, sees the feature work normally — no
  "Claude Code CLI not found on PATH" error, no extra setup step, no visible difference from
  launching via a terminal.
- If `claude` genuinely cannot be found anywhere Bonsai looks, the user still sees today's
  existing "Claude Code CLI not found on PATH" state (per `docs/contracts/ui-reference.md`'s
  existing AI-unavailable state) — this fix does not change that message or its styling, only
  when it is (correctly) shown.
- No added startup delay that's noticeable to the user; discovery work should not block app
  launch or make the UI feel slower than today.

## Acceptance criteria
1. Given `claude` is installed in a location only added to `PATH` by the user's shell startup
   files (not inherited by GUI-launched processes), when the user opens Bonsai by double-clicking
   the app (or via Spotlight/Dock) and triggers an AI feature, then the feature runs successfully
   instead of reporting the CLI as unavailable.
2. Given `claude` is installed and already discoverable via the process's inherited `PATH` (e.g.
   Bonsai launched from a terminal), when an AI feature runs, then behavior is unchanged from
   today.
3. Given `claude` is not installed or not discoverable anywhere Bonsai looks, when an AI feature
   is triggered, then the user sees the existing "Claude Code CLI not found on PATH" unavailable
   state, unchanged from today's behavior.
4. Given the app performed discovery once already in a session, when further AI features are
   used in that same session, then Bonsai does not repeat the (relatively slow) discovery work
   for every single AI call.
5. Given Windows, when Bonsai looks for `claude`, then its current PATHEXT-aware resolution
   behavior is unaffected by this fix.
6. Given the existing test-only override mechanism for pointing at a specific `claude` binary,
   when that override is set, then it continues to take precedence over any new discovery logic,
   unchanged from today.

## Edge cases & error states
- User's login shell itself is slow to start (heavy `.zshrc`), or the shell invocation used for
  discovery hangs or errors — discovery must not hang the app or an AI feature indefinitely; it
  should time out and fall back to today's "not found" outcome.
- User has multiple `claude` installations on their machine (e.g. one via npm global, one via a
  standalone installer) — Bonsai should pick a working one consistently rather than behaving
  differently between runs; exact precedence order is a `/plan` decision, not specified here.
- User's default login shell is unusual or misconfigured (e.g. `$SHELL` points somewhere broken)
  — discovery should fail gracefully into the existing "not found" state rather than crashing or
  showing a confusing error.
- Discovered path becomes stale mid-session (user uninstalls/moves `claude` while Bonsai is
  running) — out of scope for this fix; existing behavior (the next spawn attempt fails and
  surfaces as today's failure/unavailable state) is acceptable.

## Open questions
None — the technical direction (resolve against the login shell's environment, cached, with a
fallback) was already agreed with the user before this spec was written; no product-level
ambiguity remains for `/plan` to resolve.
