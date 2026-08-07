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

case "$BONSAI_STUB_MODE" in
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
