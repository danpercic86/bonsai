# P125 — Dependency tracker: smoke checklist

Spec: `docs/specs/009-dependency-tracker/spec.md` (AC1–AC13). Contract: `P125-dependency-tracker.md`,
AC→test table `P125-ecosystems.md` E4, UI `P125-ui.md` §9/§10.

## A. Automated scratch-repo smoke (AI gate, offline)

Test: `crates/bonsai-deps/tests/smoke_scratch.rs::smoke_scratch_repo_offline` (`#[ignore]`d; run
it alone, never together with the live `live_record.rs`):

```sh
TMP='D:\Data\Temp\P125-smoke\tmp' TEMP='D:\Data\Temp\P125-smoke\tmp' \
  cargo test -p bonsai-deps --test smoke_scratch -- --ignored --nocapture
```

It builds a git2 repo at `D:\Data\Temp\P125-smoke\repo` (override with `P125_SMOKE_DIR`) and runs
`bonsai_deps::run_check` over `FakeTransport::with_fixtures(tests/fixtures/remote)` and an empty
`FakeHostEnv`. No live network is used.

| File | Purpose |
|---|---|
| `package.json` + `pnpm-lock.yaml` | lodash 4.17.20 (public) + `@zzsentinel/scoped` (private) |
| `.npmrc` | `@zzsentinel:registry=https://npm.corp.invalid/` |
| `Cargo.toml` + `Cargo.lock` | serde, tokio, plus the vulnerable transitive smallvec 1.6.0 |
| `py/requirements.txt` | requests==2.19.1 |
| `go/go.mod` | gorilla/websocket v1.4.0 |
| `dotnet/App/App.csproj` | Newtonsoft.Json 12.0.3 |
| `android/build.gradle` | an unsupported ecosystem |
| `broken/package.json` | a truncated (malformed) manifest named `zz-smoke-broken` |

Result from 2026-09-29, HEAD `b7ee24b`: **PASS**. There were 31 recorded requests and 7 groups:

| Group | State | Rows |
|---|---|---|
| `Cargo.toml` (+`Cargo.lock`) | ok | tokio 1.38.0→1.53.1 minor (2 advisories); serde 1.0.200→1.0.229 patch; smallvec 1.6.0 **transitive** (RUSTSEC-2021-0003, GHSA-43w2-9j62-hq99) |
| `package.json` (+`pnpm-lock.yaml`) | ok | lodash 4.17.20→4.18.1 minor (5 advisories); `@zzsentinel/scoped` **private** (scopedRegistry, host `npm.corp.invalid`) |
| `android/build.gradle` | notTracked "Gradle" | — |
| `broken/package.json` | parseError invalidSyntax, line 2, col 0 | — |
| `dotnet/App/App.csproj` | noLockfile | Newtonsoft.Json 12.0.3→13.0.4 major (GHSA-5crp-9r3c-p9vr) |
| `go/go.mod` | ok | gorilla/websocket v1.4.0→v1.5.3 minor (GO-2020-0019 +3) |
| `py/requirements.txt` | noLockfile | requests 2.19.1→2.34.2 minor (10 advisories) |

The summary was major 1, minor 4, patch 1, vulnerable 6 (critical 1, high 4, low 1), private 1,
notTracked 1 and parseErrors 1. Privacy: `@zzsentinel/scoped`, `zzsentinel` and `zz-smoke-broken`
were **absent** from every recorded URL and body. Matching was case-insensitive, after
percent-decoding and JSON-decoding. The `lodash` positive control was present.

Observations for the orchestrator (not asserted as bugs):
- The `broken/package.json` group reports `lockfilePath: pnpm-lock.yaml`, which is the ROOT lock.
  This follows the contract's `lockfile_for` rule (same directory, then ancestors).
- The parse-error position of a JSON truncated at EOF reads `col 0`. serde_json's EOF column is
  passed through unchanged at `crates/bonsai-deps/src/adapter.rs:111`. The UI line would read
  "line 2, col 0". This is cosmetic.

## B. Browser harness (AI gate) — `pnpm dev` + `VITE_MOCK_IPC=1`

- [ ] `?deps=off`: the consent body and **Turn on**. No Check-now button.
- [ ] `?deps=loading`: skeleton rows and "Checking…".
- [ ] `?deps=populated`: patch, minor and major rows; a vulnerable direct row (critical) and a
      vulnerable transitive row (tagged `transitive`); local and private pills; Gradle `not tracked`;
      the `broken/go.mod` parse-error line; the "checked … ago" caption.
- [ ] `?deps=uptodate`, `?deps=stale` (notice + pill), `?deps=offline` (one notice, "3 hours ago"),
      `?deps=error` (failure block), `?deps=empty`, `?deps=mirror` (report-wide lead),
      `?deps=large` (200 rows, then Show all).
- [ ] AC10: on `?deps=stale`, press **Check now**. The old report must stay visible, and the rAF
      frame-timing log must show no sustained frames > 33 ms.
- [ ] Both themes. No toast or modal error appears in any mode.

## C. USER CHECKPOINT — native app (`pnpm tauri dev`)

Use a throwaway copy of the scratch repo, `D:\Data\Temp\P125-smoke\repo`, or any scratch repo.
Never use a real work repo for the disable/wipe steps.

1. **AC1 (fresh profile).** Open the repo. The toolbar Dependencies entry shows no chip, and its tooltip is the "off" title
   (`TOOLBAR_TITLE_OFF`). Opening it
   shows the consent text with the enable control. Optionally confirm with a proxy or firewall log
   that no request goes to `api.deps.dev` or `api.osv.dev`.
2. **Consent.** Settings → General → Dependency checks → switch on. The consent dialog appears.
   Cancel leaves it off. Confirm turns it on, and a check starts for the open repo.
3. **AC2 / AC4 / AC5.** When the check finishes, the groups match section A. The summary chips equal
   the list. smallvec is labelled `transitive` with RUSTSEC-2021-0003. Gradle reads "not tracked".
   The broken manifest shows its parse-error line, and the other groups are unaffected.
4. **AC6.** `@zzsentinel/scoped` reads "private — not checked", with no registry button.
5. **AC7.** Quit the app and relaunch. The last repo reopens, and the dialog shows the previous
   report at once, labelled "checked … ago", before the new check's result arrives.
6. **AC8.** Set the interval to 1 h in Settings. The dialog footer's "Next check in …" moves accordingly.
   **Check now** starts a check immediately, even right after one has finished.
7. **AC9.** Edit `package.json` (e.g. add a dependency) and save. Within a few seconds the report
   shows the stale notice and pill, and no check starts on its own.
8. **AC10.** While a check runs (Check now), scroll the commit graph, switch the status panel and
   open a diff. The UI stays responsive, and the old results stay until the new ones replace them.
9. **AC11.** In Task Manager or Process Explorer, no `npm`, `pnpm`, `cargo`, `pip`, `go` or `dotnet`
   child process of Bonsai appears during a check.
10. **AC12.** Settings → switch off. The result note appears, and the dialog returns to the consent
    state. `<app_data_dir>\deps-tracker` (`bonsai_deps::cache::CACHE_DIR`) no longer exists. No
    further requests are made.
11. **Repo switch.** With two repos open, switch tabs mid-check. The old check is cancelled, and the
    active repo is the one checked.
12. **Keyboard.** Ctrl+F focuses the filter. Esc closes layered popovers first. Shift+F10 opens the
    row menu. Focus returns to the toolbar button when the dialog closes.
