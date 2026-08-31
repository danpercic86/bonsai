@echo off
REM P70 test fixture, NOT product code: a stand-in `git` that RUNS FINE and says
REM NOTHING. Selected via BONSAI_GIT_BIN (see gitbin::GIT_BIN_ENV), exactly like
REM claude_stub.cmd is selected via BONSAI_CLAUDE_BIN.
REM
REM Why it exists: P70's whole point is that "git could not be LAUNCHED"
REM (FillOutcome::GitUnavailable -> AppError::GitNotFound, honest banner) and
REM "git ran and the credential helper had nothing" (FillOutcome::NoCredentials
REM -> the UNCHANGED pre-P70 auth message) are different outcomes. A nonexistent
REM path proves the first; THIS stub proves the second, because it is a real,
REM launchable program that exits 0 with empty stdout.
REM
REM Choice of a .cmd (no [[bin]] target): the code under test is `pub(crate)`, so
REM it can only be driven from a UNIT test inside the crate — and Cargo sets
REM CARGO_BIN_EXE_<name> only for integration tests/benches, so a [[bin]] stub is
REM not locatable from there. The committed-script twin (claude_stub.{cmd,sh},
REM found via env!("CARGO_MANIFEST_DIR")) is the pattern that works in both, and
REM Rust >= 1.77 routes Command::new("<path>.cmd") through cmd.exe automatically.
REM
REM BONSAI_GIT_STUB_MARKER (optional): append one line per invocation to that
REM file. This is the spawn counter that proves P70 §6.1 #18 -- with git
REM unresolvable the Helper credential rung must perform ZERO spawns, so the
REM ladder still reaches SshAgent. An absent marker file after a full
REM acquire_cred_with() run is the literal proof.
if defined BONSAI_GIT_STUB_MARKER echo spawned>>"%BONSAI_GIT_STUB_MARKER%"
REM Drain stdin (absolute find.exe, so a GNU `find` earlier on PATH cannot win)
REM so `git credential fill`'s `url=...` payload is consumed normally.
%SystemRoot%\System32\find.exe /v "" >nul
exit /b 0
