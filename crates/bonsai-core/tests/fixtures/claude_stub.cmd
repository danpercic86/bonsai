@echo off
REM P13 stub `claude` CLI used by src-tauri/src/ai/mod.rs unit tests.
REM Selected via the BONSAI_CLAUDE_BIN env var; behaviour chosen via BONSAI_STUB_MODE.
REM
REM Choice of a .cmd (no .sh twin, no Rust helper bin): the target/test machine is
REM Windows (win32) and Rust >= 1.77 routes `Command::new("<path>.cmd")` through
REM cmd.exe automatically, so a committed batch script needs ZERO Cargo.toml changes
REM (no [[bin]] target). Tests locate this file via env!("CARGO_MANIFEST_DIR").
REM
REM Modes (BONSAI_STUB_MODE): success (default) | success_fence | error | nonzero
REM | slow | version. `find /v "" >nul` drains stdin so the drain-and-poll writer
REM completes normally even for payloads larger than the OS pipe buffer.
REM
REM P13 tester adversarial modes (all valid JSON envelopes, exit 0 unless noted):
REM   success_markers - result body STILL contains <<<<<<< ======= >>>>>>> markers
REM                     (proves resolve_conflict_text trusts the caller, like git add).
REM   empty           - result is "" -> run_claude maps to AiFailed("no output").
REM   whitespace      - result is whitespace-only -> AiFailed (trim() is empty).
REM   success_crlf    - result body uses CRLF (\r\n) line endings verbatim.
REM   check_model     - echoes MODEL_IS_SONNET only when argv contains
REM                     "--model sonnet"; otherwise an is_error envelope. Proves
REM                     RunOpts::default() spawns --model sonnet (DEFAULT_MODEL).
REM
REM P15 tester mode:
REM   dump_stdin      - writes the RECEIVED stdin payload VERBATIM to the file
REM                     named by %BONSAI_STUB_STDIN_DUMP%, then emits the normal
REM                     `success` envelope. Lets a test prove that the staged/diff
REM                     lines actually reach the CLI's stdin (payload assembly).
REM                     Uses the ABSOLUTE System32 find.exe so it drains stdin
REM                     regardless of PATH ordering (a GNU `find` on PATH would
REM                     not).
REM P55a tester mode:
REM   emit_file       - drains stdin, then emits the JSON envelope file named by
REM                     %BONSAI_STUB_ENVELOPE% VERBATIM. Lets a test drive
REM                     plan_operation with an ARBITRARY model reply (each intent
REM                     / garbage) without batch-escaping JSON on the argv.
REM P68a streaming (NDJSON) modes — one `echo` per protocol line, `set /p` to read
REM ONE stdin turn (never a drain-to-EOF: the streaming session holds stdin OPEN
REM so a second turn is possible, so `find /v ""` would block forever here).
REM
REM P68b WARNING — `set /p` has a cmd.exe line-length ceiling (~1 KB of accepted
REM input) and does NOT consume the rest of an over-long line: the residue stays in
REM the pipe and the NEXT `set /p` (e.g. `stream_ask`'s reply read) would swallow
REM it instead of the reply. So this stub CANNOT exercise an interactive turn with a
REM bulk-sized (~400 KB) payload on Windows — P68a's ~90-byte turns are fine. A
REM bulk interactive round-trip needs a real CLI (USER CHECKPOINT) or a Rust helper
REM bin, not this script.
REM   stream_success - init, thinking heartbeat, assistant text, post_turn_summary, result.
REM   stream_slow    - init, then ~3 s of SILENCE (longer than a 2 s test idle
REM                    limit), then a result. While alive it APPENDS a line to
REM                    %BONSAI_STUB_MARKER% (when set) about once a second, so a test
REM                    proves the child died by deleting that file after the run and
REM                    finding it still absent a tick later — no timing assumption
REM                    about how long the kill itself took.
REM   stream_ask     - first result body is `BONSAI_NEEDS_INPUT: ...`; a SECOND
REM                    result (the real body) follows the second stdin line.
REM   stream_partial - init + assistant text, then exits WITHOUT a result.
REM   stream_garbage - a non-JSON line, an unknown `type`, then a valid result.
REM   stream_bulk    - a result body carrying two `===== BONSAI RESULT: ... =====`
REM                    blocks (the P68b bulk-split fixture).
REM   stream_stderr_fail - writes a usage-style error to STDERR and exits NON-ZERO
REM                    without ever touching stdout. Proves the child's real error
REM                    text survives the stdout-EOF/stderr race (P68a review S1).
REM   stream_hang_stdin - echoes one line, then sleeps ~20 s WITHOUT EVER READING
REM                    STDIN, so a payload larger than the pipe buffer leaves our
REM                    write blocked. Proves cancel still works while a write is in
REM                    flight (P68a review S2). Ticks %BONSAI_STUB_MARKER% once a
REM                    second while it hangs (same convention as stream_slow) so the
REM                    same test also proves the killed child left nothing behind.
setlocal
if /i "%BONSAI_STUB_MODE%"=="stream_success"     goto :stream_success
if /i "%BONSAI_STUB_MODE%"=="stream_slow"        goto :stream_slow
if /i "%BONSAI_STUB_MODE%"=="stream_stderr_fail" goto :stream_stderr_fail
if /i "%BONSAI_STUB_MODE%"=="stream_hang_stdin"  goto :stream_hang_stdin
if /i "%BONSAI_STUB_MODE%"=="stream_ask"         goto :stream_ask
if /i "%BONSAI_STUB_MODE%"=="stream_partial"     goto :stream_partial
if /i "%BONSAI_STUB_MODE%"=="stream_garbage"     goto :stream_garbage
if /i "%BONSAI_STUB_MODE%"=="stream_bulk"        goto :stream_bulk
if /i "%BONSAI_STUB_MODE%"=="emit_file"          goto :emit_file
if /i "%BONSAI_STUB_MODE%"=="dump_stdin"        goto :dump_stdin
if /i "%BONSAI_STUB_MODE%"=="version"          goto :version
if /i "%BONSAI_STUB_MODE%"=="nonzero"           goto :nonzero
if /i "%BONSAI_STUB_MODE%"=="slow"              goto :slow
if /i "%BONSAI_STUB_MODE%"=="error"             goto :error
if /i "%BONSAI_STUB_MODE%"=="success_fence"     goto :fence
if /i "%BONSAI_STUB_MODE%"=="success_markers"   goto :markers
if /i "%BONSAI_STUB_MODE%"=="empty"             goto :empty
if /i "%BONSAI_STUB_MODE%"=="whitespace"        goto :whitespace
if /i "%BONSAI_STUB_MODE%"=="success_crlf"      goto :crlf
if /i "%BONSAI_STUB_MODE%"=="check_model"       goto :check_model
goto :success

:version
echo 2.1.220
exit /b 0

:nonzero
echo something broke 1>&2
exit /b 1

:slow
ping -n 4 127.0.0.1 >nul
echo {"result":"late","is_error":false,"type":"result"}
exit /b 0

:error
find /v "" >nul
echo {"is_error":true,"result":"boom","type":"result"}
exit /b 0

:fence
find /v "" >nul
echo {"result":"```rust\nMERGED_FENCED\n```","is_error":false,"type":"result"}
exit /b 0

:success
find /v "" >nul
echo {"result":"MERGED_BODY_OK","is_error":false,"total_cost_usd":0.012,"session_id":"sess-abc","type":"result"}
exit /b 0

REM ---- P13 tester adversarial modes ----

:markers
REM `<`/`>` are batch redirection operators; rather than fight ^-escaping we
REM `type` a committed one-line JSON envelope whose `result` carries literal
REM conflict markers (with \n escapes inside the JSON string).
find /v "" >nul
type "%~dp0claude_envelope_markers.json"
exit /b 0

:empty
find /v "" >nul
echo {"result":"","is_error":false,"total_cost_usd":0.0,"type":"result"}
exit /b 0

:whitespace
find /v "" >nul
echo {"result":"   \n  \t","is_error":false,"total_cost_usd":0.0,"type":"result"}
exit /b 0

:crlf
find /v "" >nul
echo {"result":"L1\r\nL2\r\nL3\r\n","is_error":false,"total_cost_usd":0.02,"session_id":"sess-crlf","type":"result"}
exit /b 0

:check_model
find /v "" >nul
echo %* | findstr /C:"--model sonnet" >nul
if errorlevel 1 (echo {"is_error":true,"result":"model was not sonnet","type":"result"}) else (echo {"result":"MODEL_IS_SONNET","is_error":false,"type":"result"})
exit /b 0

REM ---- P15 tester mode ----

:dump_stdin
REM Capture stdin VERBATIM to the dump file (absolute find.exe so it drains
REM regardless of a GNU `find` earlier on PATH), then emit the success envelope.
%SystemRoot%\System32\find.exe /v "" > "%BONSAI_STUB_STDIN_DUMP%"
echo {"result":"MERGED_BODY_OK","is_error":false,"total_cost_usd":0.012,"session_id":"sess-abc","type":"result"}
exit /b 0

REM ---- P68a streaming (NDJSON) modes ----

:stream_success
set /p _turn=
echo {"type":"system","subtype":"init","session_id":"sess-stream","model":"sonnet","tools":["Read","Grep","Glob"]}
echo {"type":"system","subtype":"thinking_tokens","tokens":42}
echo {"type":"assistant","message":{"content":[{"type":"text","text":"MERGED_STREAM_BODY"}]}}
echo {"type":"system","subtype":"post_turn_summary","status_category":"review_ready","needs_action":false}
echo {"type":"result","subtype":"success","is_error":false,"result":"MERGED_STREAM_BODY","total_cost_usd":0.0238,"session_id":"sess-stream"}
exit /b 0

:stream_slow
set /p _turn=
echo {"type":"system","subtype":"init","session_id":"sess-slow","model":"sonnet","tools":[]}
REM ~3 s of stdout silence, TICKING the marker about once a second while alive (see
REM the header): a one-shot write after the sleep made the "nothing survived"
REM assertion race the kill path under load.
for /L %%t in (1,1,3) do (
  ping -n 2 127.0.0.1 >nul
  if defined BONSAI_STUB_MARKER echo tick>>"%BONSAI_STUB_MARKER%"
)
echo {"type":"result","subtype":"success","is_error":false,"result":"LATE_BODY","total_cost_usd":0.01,"session_id":"sess-slow"}
exit /b 0

:stream_stderr_fail
REM No stdout at all, no stdin read: the ONLY diagnosis is this stderr line.
echo STUB_USAGE_ERROR: unknown option --verbose 1>&2
exit /b 2

:stream_hang_stdin
REM Deliberately never reads stdin. `ping` is the batch sleep; taskkill /T (used by
REM kill_child_tree) reaps it with the cmd.exe parent. Ticks the marker once a
REM second (same convention as :stream_slow) so the cancel test can assert directly
REM that nothing survived, instead of arguing from the shared reap path.
echo {"type":"system","subtype":"init","session_id":"sess-hang","model":"sonnet","tools":[]}
for /L %%t in (1,1,20) do (
  ping -n 2 127.0.0.1 >nul
  if defined BONSAI_STUB_MARKER echo tick>>"%BONSAI_STUB_MARKER%"
)
exit /b 0

:stream_ask
set /p _turn=
echo {"type":"system","subtype":"init","session_id":"sess-ask","model":"sonnet","tools":["Read"]}
echo {"type":"result","subtype":"success","is_error":false,"result":"BONSAI_NEEDS_INPUT: which locale wins?","total_cost_usd":0.0238,"session_id":"sess-ask"}
set /p _reply=
echo {"type":"assistant","message":{"content":[{"type":"text","text":"ANSWERED_BODY"}]}}
echo {"type":"result","subtype":"success","is_error":false,"result":"ANSWERED_BODY","total_cost_usd":0.0263,"session_id":"sess-ask"}
exit /b 0

:stream_partial
set /p _turn=
echo {"type":"system","subtype":"init","session_id":"sess-partial","model":"sonnet","tools":[]}
echo {"type":"assistant","message":{"content":[{"type":"text","text":"HALF_A_BODY"}]}}
exit /b 0

:stream_garbage
set /p _turn=
echo this is not json at all
echo {"type":"brand_new_event","payload":"ignored"}
echo {"type":"result","subtype":"success","is_error":false,"result":"GARBAGE_TOLERATED","total_cost_usd":0.001,"session_id":"sess-garbage"}
exit /b 0

:stream_bulk
set /p _turn=
echo {"type":"system","subtype":"init","session_id":"sess-bulk","model":"sonnet","tools":["Read"]}
echo {"type":"result","subtype":"success","is_error":false,"result":"===== BONSAI RESULT: a/one.json =====\nONE_BODY\n===== BONSAI RESULT: b/two.json =====\nTWO_BODY","total_cost_usd":0.03,"session_id":"sess-bulk"}
exit /b 0

:emit_file
REM Drain stdin (absolute find.exe), then print the caller-provided envelope file
REM VERBATIM. The test builds the envelope with serde_json so the `result` string
REM (the model's intent JSON) is correctly escaped — no batch quoting headaches.
%SystemRoot%\System32\find.exe /v "" >nul
type "%BONSAI_STUB_ENVELOPE%"
exit /b 0
