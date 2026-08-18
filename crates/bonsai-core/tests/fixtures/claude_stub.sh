#!/bin/sh
# P13 stub `claude` CLI used by src/ai/mod.rs and src/assets/generate.rs unit
# tests on macOS/Linux (POSIX twin of claude_stub.cmd, same protocol).
#
# Selected via the BONSAI_CLAUDE_BIN env var; behaviour chosen via
# BONSAI_STUB_MODE. `cat >/dev/null` drains stdin so the drain-and-poll writer
# completes normally even for payloads larger than the OS pipe buffer.
#
# Modes (BONSAI_STUB_MODE): success (default) | success_fence | error |
# nonzero | slow | version.
#
# P13 tester adversarial modes (all valid JSON envelopes, exit 0 unless noted):
#   success_markers - result body STILL contains <<<<<<< ======= >>>>>>> markers
#                     (proves resolve_conflict_text trusts the caller, like git add).
#   empty           - result is "" -> run_claude maps to AiFailed("no output").
#   whitespace      - result is whitespace-only -> AiFailed (trim() is empty).
#   success_crlf    - result body uses CRLF (\r\n) line endings verbatim.
#   check_model     - echoes MODEL_IS_SONNET only when argv contains
#                     "--model sonnet"; otherwise an is_error envelope. Proves
#                     RunOpts::default() spawns --model sonnet (DEFAULT_MODEL).
#
# P15 tester mode:
#   dump_stdin      - writes the RECEIVED stdin payload VERBATIM to the file
#                     named by $BONSAI_STUB_STDIN_DUMP, then emits the normal
#                     `success` envelope. Lets a test prove that the staged/diff
#                     lines actually reach the CLI's stdin (payload assembly).

# P68a streaming (NDJSON) modes — one `echo` per protocol line, a single
# `read -r` per turn (never `cat >/dev/null`: the streaming session holds stdin
# OPEN for a second turn, so draining to EOF would block forever):
# P68b adds one mode: stream_tools — reports TOOLS_READONLY / TOOLS_EMPTY
# depending on the `--tools` value in argv (the D10 allowlist assertion).
#   stream_success | stream_slow | stream_ask | stream_partial | stream_garbage
#   | stream_bulk | stream_stderr_fail | stream_hang_stdin. stream_slow and
#   stream_hang_stdin both TICK $BONSAI_STUB_MARKER (append one line ~every second)
#   while they live, so a test proves the killed child left nothing behind by deleting
#   that file after the run and finding it still absent. See claude_stub.cmd
#   for the per-mode description (including its Windows `set /p` line-length
#   warning); the two files must stay behaviourally identical.

case "$BONSAI_STUB_MODE" in
  stream_success)
    IFS= read -r _turn
    echo '{"type":"system","subtype":"init","session_id":"sess-stream","model":"sonnet","tools":["Read","Grep","Glob"]}'
    echo '{"type":"system","subtype":"thinking_tokens","estimated_tokens":420,"estimated_tokens_delta":420}'
    echo '{"type":"assistant","message":{"content":[{"type":"text","text":"MERGED_STREAM_BODY"}]}}'
    echo '{"type":"system","subtype":"post_turn_summary","status_category":"review_ready","needs_action":false}'
    echo '{"type":"result","subtype":"success","is_error":false,"result":"MERGED_STREAM_BODY","total_cost_usd":0.0238,"session_id":"sess-stream"}'
    exit 0
    ;;
  stream_slow)
    IFS= read -r _turn
    echo '{"type":"system","subtype":"init","session_id":"sess-slow","model":"sonnet","tools":[]}'
    # ~3 s of stdout silence, TICKING the marker about once a second while alive
    # (see the header): a one-shot write after the sleep made the "nothing survived"
    # assertion race the kill path under load.
    t=0
    while [ "$t" -lt 3 ]; do
      sleep 1
      if [ -n "$BONSAI_STUB_MARKER" ]; then echo tick >> "$BONSAI_STUB_MARKER"; fi
      t=$((t + 1))
    done
    echo '{"type":"result","subtype":"success","is_error":false,"result":"LATE_BODY","total_cost_usd":0.01,"session_id":"sess-slow"}'
    exit 0
    ;;
  stream_stderr_fail)
    # No stdout at all, no stdin read: the ONLY diagnosis is this stderr line
    # (P68a review S1 — it must survive the stdout-EOF/stderr race).
    echo 'STUB_USAGE_ERROR: unknown option --verbose' 1>&2
    exit 2
    ;;
  stream_hang_stdin)
    # Never reads stdin, so a payload larger than the pipe buffer leaves the
    # session's write blocked (P68a review S2), and ticks the marker once a second so
    # the cancel test can assert directly that nothing survived. THIS SHELL must stay
    # the direct child (no `exec sleep`, unlike before): it is what writes the ticks,
    # and it is what kill_child_tree kills on POSIX. It holds the unread stdin the
    # whole time, so the session's write stays blocked; the 1 s `sleep` grandchild
    # that briefly outlives the kill releases the pipe within a tick.
    echo '{"type":"system","subtype":"init","session_id":"sess-hang","model":"sonnet","tools":[]}'
    t=0
    while [ "$t" -lt 20 ]; do
      sleep 1
      if [ -n "$BONSAI_STUB_MARKER" ]; then echo tick >> "$BONSAI_STUB_MARKER"; fi
      t=$((t + 1))
    done
    exit 0
    ;;
  stream_ask)
    IFS= read -r _turn
    echo '{"type":"system","subtype":"init","session_id":"sess-ask","model":"sonnet","tools":["Read"]}'
    echo '{"type":"result","subtype":"success","is_error":false,"result":"BONSAI_NEEDS_INPUT: which locale wins?","total_cost_usd":0.0238,"session_id":"sess-ask"}'
    IFS= read -r _reply
    echo '{"type":"assistant","message":{"content":[{"type":"text","text":"ANSWERED_BODY"}]}}'
    echo '{"type":"result","subtype":"success","is_error":false,"result":"ANSWERED_BODY","total_cost_usd":0.0263,"session_id":"sess-ask"}'
    exit 0
    ;;
  stream_partial)
    IFS= read -r _turn
    echo '{"type":"system","subtype":"init","session_id":"sess-partial","model":"sonnet","tools":[]}'
    echo '{"type":"assistant","message":{"content":[{"type":"text","text":"HALF_A_BODY"}]}}'
    exit 0
    ;;
  stream_garbage)
    IFS= read -r _turn
    echo 'this is not json at all'
    echo '{"type":"brand_new_event","payload":"ignored"}'
    echo '{"type":"result","subtype":"success","is_error":false,"result":"GARBAGE_TOLERATED","total_cost_usd":0.001,"session_id":"sess-garbage"}'
    exit 0
    ;;
  stream_tools)
    # P68b: report WHICH tool allowlist arrived, so a test can prove the read-only
    # default (D10) really reaches the CLI. Twin of the .cmd `:stream_tools` mode.
    IFS= read -r _turn
    echo '{"type":"system","subtype":"init","session_id":"sess-tools","model":"sonnet","tools":[]}'
    # Also on stderr, so a failing assertion can print the argv it actually saw.
    echo "ARGV: $*" 1>&2
    case " $* " in
      *"Read,Grep,Glob"*)
        echo '{"type":"result","subtype":"success","is_error":false,"result":"TOOLS_READONLY","total_cost_usd":0.001,"session_id":"sess-tools"}'
        ;;
      *)
        echo '{"type":"result","subtype":"success","is_error":false,"result":"TOOLS_EMPTY","total_cost_usd":0.001,"session_id":"sess-tools"}'
        ;;
    esac
    exit 0
    ;;
  stream_bulk)
    IFS= read -r _turn
    echo '{"type":"system","subtype":"init","session_id":"sess-bulk","model":"sonnet","tools":["Read"]}'
    # printf, NOT echo: POSIX sh (dash on Ubuntu, bash-as-sh on macOS) expands
    # `\n` inside `echo` into a real newline by default, splitting this single
    # NDJSON line in two and corrupting the JSON (see the other printf'd modes
    # above for the same reason). The literal `\n` here must reach the parser
    # verbatim as a JSON string escape, not become a shell-level newline.
    printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"result":"===== BONSAI RESULT: a/one.json =====\nONE_BODY\n===== BONSAI RESULT: b/two.json =====\nTWO_BODY","total_cost_usd":0.03,"session_id":"sess-bulk"}'
    exit 0
    ;;
  dump_stdin)
    cat > "$BONSAI_STUB_STDIN_DUMP"
    echo '{"result":"MERGED_BODY_OK","is_error":false,"total_cost_usd":0.012,"session_id":"sess-abc","type":"result"}'
    exit 0
    ;;
  emit_file)
    # P55a: drain stdin, then print the caller-provided envelope file VERBATIM so
    # a test can drive plan_operation with an ARBITRARY model reply (each intent /
    # garbage). The test builds the envelope with serde_json (correct escaping).
    cat > /dev/null
    cat "$BONSAI_STUB_ENVELOPE"
    exit 0
    ;;
  version)
    echo '2.1.220'
    exit 0
    ;;
  nonzero)
    echo 'something broke' 1>&2
    exit 1
    ;;
  slow)
    sleep 3
    echo '{"result":"late","is_error":false,"type":"result"}'
    exit 0
    ;;
  error)
    cat > /dev/null
    echo '{"is_error":true,"result":"boom","type":"result"}'
    exit 0
    ;;
  success_fence)
    cat > /dev/null
    printf '{"result":"```rust\\nMERGED_FENCED\\n```","is_error":false,"type":"result"}\n'
    exit 0
    ;;
  success_markers)
    cat > /dev/null
    cat "$(dirname "$0")/claude_envelope_markers.json"
    exit 0
    ;;
  empty)
    cat > /dev/null
    echo '{"result":"","is_error":false,"total_cost_usd":0.0,"type":"result"}'
    exit 0
    ;;
  whitespace)
    cat > /dev/null
    printf '{"result":"   \\n  \\t","is_error":false,"total_cost_usd":0.0,"type":"result"}\n'
    exit 0
    ;;
  success_crlf)
    cat > /dev/null
    printf '{"result":"L1\\r\\nL2\\r\\nL3\\r\\n","is_error":false,"total_cost_usd":0.02,"session_id":"sess-crlf","type":"result"}\n'
    exit 0
    ;;
  check_model)
    cat > /dev/null
    case " $* " in
      *' --model sonnet '*)
        echo '{"result":"MODEL_IS_SONNET","is_error":false,"type":"result"}'
        ;;
      *)
        echo '{"is_error":true,"result":"model was not sonnet","type":"result"}'
        ;;
    esac
    exit 0
    ;;
  *)
    cat > /dev/null
    echo '{"result":"MERGED_BODY_OK","is_error":false,"total_cost_usd":0.012,"session_id":"sess-abc","type":"result"}'
    exit 0
    ;;
esac
