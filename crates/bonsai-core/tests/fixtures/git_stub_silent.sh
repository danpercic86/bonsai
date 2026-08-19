#!/bin/sh
# P70 test fixture, NOT product code: POSIX twin of git_stub_silent.cmd (same
# protocol, same guarantees) for macOS/Linux. A stand-in `git` that RUNS FINE
# and says NOTHING, selected via BONSAI_GIT_BIN (gitbin::GIT_BIN_ENV).
#
# See git_stub_silent.cmd for the full rationale. In short: a nonexistent path
# proves FillOutcome::GitUnavailable; this launchable-but-silent program proves
# FillOutcome::NoCredentials, and the two must not collapse into one message.
#
# BONSAI_GIT_STUB_MARKER (optional): append one line per invocation — the spawn
# counter behind P70 §6.1 #18 (the Helper rung must perform ZERO spawns when git
# is unresolvable, so the ladder still reaches SshAgent).
if [ -n "$BONSAI_GIT_STUB_MARKER" ]; then
  echo spawned >> "$BONSAI_GIT_STUB_MARKER"
fi
# Drain stdin so `git credential fill`'s `url=...` payload is consumed normally.
cat > /dev/null
exit 0
