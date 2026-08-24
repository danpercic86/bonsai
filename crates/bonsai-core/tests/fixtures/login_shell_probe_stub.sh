#!/bin/sh
# Stub standing in for the user's `$SHELL` in `bin_resolve::probe_login_shell_path`
# tests (spec 001, docs/specs/001-macos-claude-cli-path/). `probe_login_shell_path`
# spawns `Command::new($SHELL).arg("-ilc").arg("echo $PATH")`, so when a test points
# `$SHELL` at THIS script, it is invoked exactly the same way: argv is `-ilc`
# `echo $PATH`, which this stub ignores (a real shell would have already consumed
# those sourcing its rc files; all that matters here is what lands on stdout).
#
# Modes (BONSAI_PROBE_MODE):
#   normal (default) - prints a startup-banner-style line FIRST (mimicking a
#                       shell plugin/motd/nvm init message that writes to stdout
#                       before the real command's output), then a final
#                       PATH-shaped line -- so a test can assert
#                       probe_login_shell_path takes the LAST non-empty line,
#                       not the first.
#   hang              - sleeps far longer than SHELL_PROBE_TIMEOUT (2s) and never
#                       emits a PATH line, so a test can assert the probe times
#                       out and kills this process rather than hanging the suite.
#
# BONSAI_PROBE_FAKE_PATH, if set, is emitted verbatim as the final PATH line;
# otherwise a hardcoded placeholder is used. Lets a test point the "login shell's
# PATH" at an arbitrary temp dir.

case "$BONSAI_PROBE_MODE" in
  hang)
    sleep 30
    exit 0
    ;;
  *)
    echo "Welcome to Fake Shell 1.0 -- login banner"
    echo "${BONSAI_PROBE_FAKE_PATH:-/fake/bin/one:/fake/bin/two}"
    exit 0
    ;;
esac
