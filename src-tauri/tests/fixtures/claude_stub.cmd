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
setlocal
if /i "%BONSAI_STUB_MODE%"=="version"       goto :version
if /i "%BONSAI_STUB_MODE%"=="nonzero"        goto :nonzero
if /i "%BONSAI_STUB_MODE%"=="slow"           goto :slow
if /i "%BONSAI_STUB_MODE%"=="error"          goto :error
if /i "%BONSAI_STUB_MODE%"=="success_fence"  goto :fence
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
