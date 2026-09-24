# Tasks: P120 part b — dev-mode stability field signals

**Contract:** `docs/contracts/P120-stability-signals.md` (binding; it supersedes plan.md where
they differ)
**Status:** ready — starts once part a's task 3 (`src/obs/stability/*`) is reviewed

- [x] B1. `record.rs` split, contract §2.3. Behaviour-preserving move of the six small enums into
  `src-tauri/src/obs/record_enums.rs`, with `pub use` re-exports so no public path breaks. Delete
  `record.rs`'s entry from `scripts/file-size-baseline.json`. Test counts must be identical before
  and after (T-R12). — owner: refactorer
- [x] B2. Rust, contract §2.1 / §2.2 / §2.4 / §2.5:
  - `record_stability.rs`
  - the two `LogPayload` newtype variants
  - `record_kind.rs`: the arms plus `ALL_KINDS`
  - `anomaly.rs` dispatch, and `anomaly/stability.rs` with `unprompted-shift` (stateless) and
    `focus-steal` (pending ≤64, causes ≤256, drained at session end)
  - tests T-R1–T-R11, including `tests_kind_parity.rs` and `tests_anomaly_stability.rs`.

  — owner: senior-dev (after B1)
- [x] B3. Frontend, contract §3.1–§3.5 / §5:
  - `LogKind` gets the two kinds (`src/ipc/types/obs.ts:32`)
  - `UiPayload` members plus the `Exclude` list (`src/obs/types.ts:222`)
  - `src/obs/stability/devSink.ts` (coalescing)
  - `src/obs/stability/install.ts` (`startStability()` gated on `obsEnabled()` via
    `onObsConfigChange`, `flushAll` before unregistering, a `pagehide` flush), wired next to the
    first `configureObs` call, which extends part a's mock-only start
  - `src/ipc/mock/obsAnomalyStability.ts`, plus its one-line call from `obsAnomaly.ts`
  - shared fixture JSON in `src/ipc/mock/fixtures/`
  - tests T-F7–T-F11 and T-F13.

  — owner: senior-dev ([P] with B2; disjoint files)
- [ ] B4. Disclosure copy, contract §6: ui-designer writes the one-sentence wording and the
  static-vs-runtime choice (`layoutShiftSupported()`) to `docs/contracts/P120-ui.md`. Then
  senior-dev applies it to `src/components/settings/catalog/dev.ts:100` and
  `SettingsDevCaptureSection.tsx`. The `dev.capture-react` help text is unchanged. — owner:
  ui-designer, then senior-dev (fold the apply step into B3 if it's ready in time)
- [ ] B5. Review the B1–B4 diff. Check:
  - redaction independence (AC 13)
  - no `OBS_SCHEMA_VERSION` change
  - the bounds on `focus-steal`'s pending and causes buffers
  - no layout reads in the detectors (T-F13)
  - size discipline.

  Run ui-designer on B4 at the same time. — owner: reviewer + ui-designer
- [ ] B6. Tests: run T-R1–T-R12 and T-F1–T-F13, then a mock-harness run in which the part-a inject
  flags produce `unprompted-shift` / `focus-steal` anomalies in the mock ring. Report numbers only.
  — owner: tester
- [ ] B7. Integrate:
  - bare `pnpm gate`, with the output pasted
  - harness proof
  - commit `wip(spec-008): part b …`
  - then **ask the user** to run the P120 USER CHECKPOINT (`TODO.md` P120 entry). It is never
    self-declared.

  — owner: orchestrator

## Notes
- Pass each subagent the contract path plus its section numbers. Contract §0 (A1–A16) is binding.
- The files touched here are `src-tauri/src/obs/**`, `src/obs/types.ts`, `src/ipc/types/obs.ts`,
  `src/ipc/mock/obsAnomaly*.ts`, `src/obs/stability/{devSink,install}.ts` and the settings copy
  files. None of them overlap part a's tasks 5–6.
- No `security-auditor` path trigger applies (contract §8).
