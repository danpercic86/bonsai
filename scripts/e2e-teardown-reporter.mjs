/**
 * P104 — the "4-worker e2e suite hangs after the last test".
 *
 * It is not a hang. Playwright prints NOTHING between the last test result and
 * the summary line, and on Windows that gap is Edge teardown, which degrades
 * badly when the machine is oversubscribed. Measured on this repo (21 tests,
 * 4 workers, 24 spinning CPU hogs on a 22-core box):
 *
 *   t+47s   last test result printed
 *   t+47s   <gracefully close start>   x4        (one Edge tree per worker)
 *   t+77s   <kill> +30s                x4        CDP `Browser.close` never
 *                                                answered; Playwright's close
 *                                                timeout is a hardcoded 30 s
 *                                                (DEFAULT_PLAYWRIGHT_TIMEOUT,
 *                                                no config lever)
 *   t+183s  taskkill returns           x4        `spawnSync('taskkill /T /F')`
 *                                                BLOCKS the worker for ~106 s
 *                                                under load, and still reports
 *                                                "PID <n> could not be
 *                                                terminated" for part of the
 *                                                Edge tree
 *   t+232s  <process did exit>         x4        Playwright awaits the browser
 *                                                process `close` event before
 *                                                the run may finish
 *   t+233s  "21 passed"                          summary, correct exit code
 *
 * 185 s of complete silence, then a correct result. On an IDLE box the same
 * teardown is 0.2 s per browser and the full suite finishes in ~2.6 min.
 *
 * So the only defect that is ours is that the phase is invisible: this reporter
 * announces it and ticks while it runs, so a slow teardown can never again be
 * mistaken for a hung suite (it cost two 10-minute timeouts and a wrong entry
 * on the board). It changes no timing and holds no handle: every timer is
 * unref'd, so the reporter itself can never keep the runner alive.
 *
 * Knobs: E2E_TEARDOWN_REPORTER=0 silences it; E2E_TEARDOWN_GRACE_MS overrides
 * the first-warning delay (default 15000).
 */

/** Silence until teardown has been this slow -- a healthy close is ~0.2s/browser. */
const DEFAULT_GRACE_MS = 15_000;
/** How often to re-announce once the phase has been named. */
const TICK_MS = 15_000;
/** Poll period; both thresholds are enforced against the clock, so one cheap
 *  interval drives them and they can never double-fire. */
const POLL_MS = 2_000;

const secs = (ms) => `${(ms / 1000).toFixed(1)}s`;

export default class TeardownReporter {
  #startedAt = 0;
  #armedAt = 0;
  #total = 0;
  #done = 0;
  #running = 0;
  #timer = null;
  #warned = false;
  #printedAt = 0;
  #workers = 0;
  /** Env is read per-instance, not at module load, so a test can construct one
   *  reporter with a short grace without reloading the module. */
  #graceMs = Number(process.env.E2E_TEARDOWN_GRACE_MS) || DEFAULT_GRACE_MS;
  #silent = process.env.E2E_TEARDOWN_REPORTER === '0';

  printsToStdio() {
    return true;
  }

  onBegin(config, suite) {
    this.#startedAt = Date.now();
    this.#total = suite.allTests().length;
    this.#workers = config.workers;
  }

  onTestBegin() {
    this.#running += 1;
    this.#disarm();
  }

  onTestEnd() {
    this.#running -= 1;
    this.#done += 1;
    // Arm only when nothing is executing AND every expected result is in, so a
    // retry or a long trailing test can never make us cry teardown early.
    if (this.#running === 0 && this.#done >= this.#total) this.#arm();
  }

  onEnd() {
    const teardown = this.#armedAt ? Date.now() - this.#armedAt : 0;
    this.#disarm();
    if (this.#silent) return;
    // Only worth a line if we already told the reader to expect a wait.
    if (this.#warned && teardown > 0) {
      console.log(`[e2e] teardown finished — ${secs(teardown)} after the last test.`);
    }
  }

  #arm() {
    this.#disarm();
    this.#armedAt = Date.now();
    this.#timer = setInterval(() => this.#tick(), POLL_MS);
    // Load-bearing: an un-unref'd interval here would itself hold the runner
    // open and turn this diagnostic into the very bug it reports.
    this.#timer.unref?.();
  }

  #disarm() {
    if (this.#timer) clearInterval(this.#timer);
    this.#timer = null;
    // Cleared too, so an interrupted run can never report a stale duration.
    this.#armedAt = 0;
  }

  #tick() {
    if (this.#silent || !this.#armedAt) return;
    const waited = Date.now() - this.#armedAt;
    if (waited < this.#graceMs) return;
    if (!this.#warned) {
      this.#warned = true;
      this.#printedAt = Date.now();
      console.log(
        `\n[e2e] all ${this.#total} results are in after ${secs(Date.now() - this.#startedAt)}; ` +
          `Playwright is closing up to ${this.#workers} browser(s) and the web server. ` +
          `Playwright prints nothing during this phase — it is teardown, not a hung test.`,
      );
      console.log(
        `[e2e] on Windows a loaded machine makes this slow: Edge misses the hardcoded 30s ` +
          `CDP close window, then a blocking taskkill runs. Re-run with DEBUG=pw:browser for ` +
          `per-process detail, or with --workers=1 (one browser to close). See TODO.md P104.`,
      );
      return;
    }
    if (Date.now() - this.#printedAt < TICK_MS) return;
    this.#printedAt = Date.now();
    console.log(`[e2e] still tearing down — ${secs(waited)} since the last test result.`);
  }
}
