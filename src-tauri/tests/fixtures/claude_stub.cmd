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
setlocal
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
