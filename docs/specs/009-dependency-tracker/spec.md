# Dependency update tracker

**Status:** clarified — routed to the milestone loop (architect contract next)
**Created:** 2026-09-28
**Clarified:** 2026-09-28 (user accepted all proposed defaults; see "Decisions")

## Problem
A developer who opens a repository in Bonsai has no way to tell whether that repository's
dependencies are out of date or carry known vulnerabilities. They have to leave the app and run a
different tool for each ecosystem (npm, Cargo, pip, Go, NuGet, …). Many of those tools need the
toolchain installed, and some run code the repository controls. The result is that outdated or
vulnerable dependencies go unnoticed until something breaks or an external scanner flags them.

## Goals
- For the open repository, Bonsai shows which **direct** dependencies have a newer version
  available, and how big the jump is: **patch**, **minor** or **major**.
- Bonsai flags every dependency whose resolved version has a **known vulnerability**, with its
  severity and advisory identifier. That includes **transitive** (lockfile-only) dependencies.
- The check runs automatically **on a fixed interval**, **when a repo is opened**, and **on
  demand** through a refresh control, which follows the existing "watcher + manual refresh"
  convention.
- Coverage in the first release: **npm / pnpm / yarn, Cargo, PyPI (requirements / Poetry / uv),
  Go modules, NuGet**. Any ecosystem found in the repo that isn't supported is listed as **"not
  tracked"** rather than silently left out.
- Bonsai finds dependencies by **reading the repo's manifest and lockfile contents only**. It
  never runs the repository's own build or package-manager tooling.
- **Opt-in with informed consent:** checking sends dependency names and versions to public
  package-metadata and vulnerability services. Nothing leaves the machine until the user has
  turned the feature on, and they can turn it off at any time. Dependencies that look private are
  never sent.
- Results are cached, so re-opening a repo or restarting the app shows the last known report
  straight away, with its age, while a fresh check runs.
- A monorepo that contains several manifests (for example a JS frontend plus a Rust backend)
  gets one report covering all of them, grouped by manifest.

## Non-goals
- **Changing anything.** No version bumps, no lockfile edits, no install/update commands, and no
  PR or branch creation. The tracker is read-only.
- **Running project tooling** (`npm outdated`, `mvn`, `gradle`, `dotnet list`, etc.), including
  as an option, in this release.
- **Maven/Gradle, Composer, Pub**, and a Renovate-backed "any ecosystem" engine. These are named
  later phases, not part of this spec's acceptance criteria.
- **Private or authenticated registries**, and registry credentials of any kind.
- **Update listings for transitive dependencies.** Transitive entries are checked for
  vulnerabilities only.
- **Per-dependency ignore / snooze rules.** Deferred to a later phase.
- Licence compliance, dependency-graph visualisation, SBOM export.
- Notifications outside the app (OS notifications, email).
- Tracking several repos at once. This follows the locked "one repo open at a time" decision.

## Decisions (clarified 2026-09-28)
| # | Question | Decision |
|---|---|---|
| D1 | Consent scope | **One global switch** in Settings, applying to every repo. Off by default. |
| D2 | UI placement | **`ui-designer` proposes** the surface (toolbar entry vs right-panel section vs own view) in the UI contract. |
| D3 | Transitive dependencies | **Direct deps for updates; direct + transitive for vulnerabilities.** |
| D4 | Interval | **Default 12 h, user-configurable in Settings.** |
| D5 | Manifest/lockfile change | **Mark the report stale**; re-check at the next interval or on refresh, with no automatic re-check on change. |
| D6 | Opt-out | Turning the tracker off **deletes its cached results from disk**. |
| D7 | Private dependencies | **Never sent:** path/git/workspace-member dependencies and packages resolved from a non-public (scoped/custom) registry are excluded from outbound lookups and shown as "local" / "private — not checked". |
| D8 | Per-check cap | **No cap**; lookups are sent in batches. |
| D9 | Ignore / snooze | **Later phase**, not v1. |

## User-facing behavior
- **Off by default.** Until the user enables the tracker, the tracker surface shows a plain-words
  explanation of what gets sent (dependency names and versions, never private ones) and where it
  goes (public package-metadata and vulnerability services), plus an enable control. No request is
  made before that. Enabling or disabling is one global setting (D1). The same switch is in
  Settings, next to the check interval (D4).
- **Once on**, the user sees a compact summary for the open repo: counts of major / minor /
  patch updates and of vulnerable dependencies, plus when the last check ran. Vulnerable
  dependencies are the most prominent item. The summary opens a detailed list. `ui-designer`
  decides where the summary and list live (D2).
- **The detailed list** is grouped by manifest (e.g. `package.json`, `src-tauri/Cargo.toml`). Each
  row shows: dependency name, ecosystem, current version (resolved from the lockfile when there is
  one, otherwise the declared constraint), latest available version, update kind
  (patch/minor/major), and any vulnerability with its severity and advisory ID. Transitive
  dependencies appear only when they are vulnerable, and are labelled transitive (D3). Users can
  filter by "vulnerable only" and by update kind. Each row can open the package's registry page or
  the advisory page in the system browser.
- **"Not tracked":** unsupported ecosystems in the repo appear as a single line per manifest, e.g.
  "`build.gradle` — Gradle is not tracked yet", so the report never looks complete when it isn't.
- **Private / local dependencies** are listed as "local" or "private — not checked" and are never
  sent out (D7).
- **While a check runs** there is a non-blocking in-progress indicator. The previous results stay
  visible, and the rest of Bonsai stays fully responsive.
- **Refresh** starts a check immediately, whatever the interval says.
- **Interval:** automatic checks repeat every 12 h by default, and the interval can be changed in
  Settings (D4). When a manifest or lockfile changes, the report is marked **stale** (visibly) but
  no check is started until the next interval or a refresh (D5).
- Styling, states (empty/loading/error/stale), and tokens follow `docs/contracts/ui-reference.md`
  in both themes. Severity colours reuse the existing status-badge/danger families instead of
  adding new hues.

## Acceptance criteria
1. Given the tracker has never been enabled, when a repo is opened, then no network request
   related to dependencies is made. The tracker surface shows the consent explanation and an
   enable control.
2. Given the tracker is enabled and a repo with a `package.json` + `pnpm-lock.yaml` and a
   `Cargo.toml` + `Cargo.lock` is opened, when the check completes, then both manifests appear as
   separate groups. Each direct dependency shows current version, latest version and update kind,
   and the summary counts match the list.
3. Given a dependency whose resolved version is 1.2.3 and whose latest stable version is 1.2.5 /
   1.4.0 / 2.0.0, then it is classified patch / minor / major respectively, using that ecosystem's
   own version rules (e.g. PEP 440 for Python). Pre-release, yanked or deprecated versions are
   never offered as "latest" unless the current version is itself a pre-release.
4. Given a direct or transitive dependency whose resolved version has a published advisory, then a
   row shows the advisory ID and severity (transitive rows labelled as such), and the dependency
   is counted in the summary's vulnerable total. Given a transitive dependency with no advisory,
   then it does not appear in the list.
5. Given a repo containing a manifest from an unsupported ecosystem, then that manifest is listed
   as "not tracked", and the rest of the report is unaffected.
6. Given a path, git, or workspace-member dependency, or a package resolved from a non-public
   registry, then its name is never included in any outbound request, and it is shown as "local" /
   "private — not checked".
7. Given a completed check, when the app is restarted and the same repo is reopened, then the
   previous report appears immediately with a "checked <time> ago" label, before any new network
   result arrives.
8. Given the tracker is enabled with the default settings, when 12 h elapse with the repo open,
   then a new check runs without user action. Given the user changes the interval in Settings,
   then the new interval applies. When the user presses refresh, a check starts immediately.
9. Given a completed report, when a manifest or lockfile in the repo changes, then the report is
   visibly marked stale and no check is started until the next interval or a refresh.
10. Given a check is running, then the graph, status panel, and all other interactions stay
    responsive, and the previous results stay on screen until the new ones replace them.
11. Given any repository, the tracker never runs a process belonging to the repository or its
    toolchain: no package manager, build tool, or script.
12. Given the user turns the tracker off, then no further dependency-related network requests are
    made, and all cached tracker results are deleted from disk.
13. Given the browser harness (`VITE_MOCK_IPC=1`), the tracker renders from fixture data covering:
    not-yet-enabled, loading, populated (with updates, vulnerable direct + transitive rows,
    private/local rows and "not tracked" rows), up to date, stale, offline, and error.

## Edge cases & error states
- **Offline / service unreachable / rate-limited:** keep showing the last cached report, marked
  stale with its age, plus a short, non-alarming message ("Couldn't reach the package service —
  showing results from 3 h ago"). Retry at the next interval or on refresh. Don't show a modal
  error.
- **A public-looking dependency the services don't know about** (typo, unpublished): show it as
  "unknown", not as up to date and not as an error for the whole report.
- **Manifest without a lockfile:** use the declared constraint as "current". Note on the group
  that resolved versions are unavailable, and do not claim vulnerability status for a version
  range.
- **Malformed or unreadable manifest/lockfile:** that group shows a parse-error line naming the
  file. Other groups are unaffected.
- **Very large repos / lockfiles** (thousands of transitive entries, many workspace members): no
  cap (D8). Lookups are sent in batches, the check must not freeze the UI, and the list must stay
  scrollable.
- **Unborn/empty repo or no manifests:** a quiet "No dependency manifests found" state, with no
  network request.
- **Repo switched mid-check:** the in-flight result for the old repo is discarded and never shown
  against the new one.
- **Dependency pinned to an exact version on purpose:** still reported (ignore/snooze is a later
  phase, D9).

## Open questions
- None blocking. D2 (placement) is delegated to `ui-designer`'s UI contract.
- **Scope routing:** this adds a new user-visible surface, a new outbound network capability, and
  a new IPC surface, so it is **milestone-sized**. It runs through the milestone loop: `architect`
  contract, then a `ui-designer` UI contract, then a `security-auditor` pass on the outbound-data
  and CSP/capability changes before implementation.
