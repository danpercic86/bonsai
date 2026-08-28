/**
 * P91 §5 — HARNESS-ONLY anomaly analyzer (mock mode, increment 5b).
 *
 * The AUTHORITATIVE detector is Rust `src-tauri/src/obs/anomaly.rs`, which runs on
 * the sink's writer thread over the unified record stream. This module is NOT a
 * port of it and MUST NEVER reach a production bundle: it lives under
 * `src/ipc/mock/` and is invoked only by the browser harness's in-memory ring
 * (`obsRing.ts`), where records never reach the Rust sink and so carry no
 * `anomaly` records. It exists solely so the P91 mock-mode AI-gate can assert
 * anomalies with no Tauri (§6 "so the browser harness can assert schema + anomalies
 * with no Tauri"; §12 gate row 5).
 *
 * SCOPE: exactly the three rules the gate names — `dup-ipc`, `slow-command`,
 * `slow-phase`. The other §5 rules are deliberately NOT implemented here; the Rust
 * detector owns the full set.
 *
 * Design: a retrospective, dump-time BATCH pass over the ring (no streaming state
 * to maintain). Windows key off each record's own `ts` (wall-clock ms), exactly as
 * the Rust module doc mandates — never arrival time. Deterministic given the ring.
 */
import type { LogRecord } from '../types';
import type { PhaseTiming, SpanPayload } from '../types/obs';
import type { AnomalySeverity } from '../../obs/types';

/** §5 `dup-ipc` window. Mirrors `anomaly/window.rs:W_DUP_IPC_MS`. */
const W_DUP_IPC_MS = 300;
/** §5.1 `slow-command` constants. Mirror `anomaly/slow.rs`. */
const MIN_SAMPLES = 20;
const HARD_MS = 10_000;
const SLOW_RATE_MS = 10_000;
const SLOW_PHASE_SHARE = 0.7;

/** §5.1 `SLOW_RULES`: `[cmd_prefix, floor_ms, k]`. Longest matching prefix wins;
 *  `''` is the default row. SOURCE OF TRUTH: `anomaly/slow.rs:SLOW_RULES`. */
const SLOW_RULES: ReadonlyArray<readonly [string, number, number]> = [
  ['', 150, 3.0],
  ['get_graph', 1200, 3.0],
  ['commit_create', 2000, 3.0],
];

function slowRuleFor(cmd: string): { floor: number; k: number } {
  let best: readonly [string, number, number] = SLOW_RULES[0];
  for (const rule of SLOW_RULES) {
    if (cmd.startsWith(rule[0]) && rule[0].length >= best[0].length) best = rule;
  }
  return { floor: best[1], k: best[2] };
}

/** Repo-mutating command prefixes (§5 "no intervening mutation"). SOURCE OF TRUTH:
 *  `anomaly.rs:is_mutation_cmd` — keep in sync. Prefix match. */
const MUTATION_PREFIXES = [
  'commit', 'stage', 'unstage', 'discard', 'checkout', 'create_branch',
  'delete_branch', 'create_tag', 'delete_tag', 'create_stash', 'apply_stash',
  'drop_stash', 'merge', 'commit_merge', 'abort_merge', 'rebase', 'cherrypick',
  'revert', 'reset', 'fetch', 'pull', 'push', 'force_push', 'clone_repo',
  'init_repo', 'add_remote', 'remove_remote', 'rename_remote', 'add_submodule',
  'deinit_submodule', 'init_submodule', 'add_worktree', 'bisect_',
  'force_refresh_tag', 'auto_sync_tags', 'delete_remote', 'apply_composed_commits',
  'apply_identity_profile',
];

function isMutationCmd(cmd: string): boolean {
  return MUTATION_PREFIXES.some((p) => cmd.startsWith(p));
}

/** Nearest-rank p95 over a sample list (mock stand-in for the Rust histogram;
 *  "§5.1 shape" is the threshold formula, not the bucket implementation). */
function p95(samples: number[]): number | undefined {
  if (samples.length === 0) return undefined;
  const sorted = [...samples].sort((a, b) => a - b);
  const idx = Math.min(sorted.length - 1, Math.ceil(0.95 * sorted.length) - 1);
  return sorted[Math.max(0, idx)];
}

function severityToLvl(s: AnomalySeverity): LogRecord['lvl'] {
  return s;
}

interface SpanInfo {
  trace?: string;
  seq: number;
  ms: number;
  phases: PhaseTiming[];
}

/**
 * Batch-analyzes the ring and returns the derived `anomaly` records to append to
 * the dumped stream. Does NOT mutate `records`; each anomaly's `seq` is assigned
 * above the ring's current max so `refs` still resolve to the real record seqs.
 */
export function analyzeAnomalies(records: readonly LogRecord[]): LogRecord[] {
  const out: LogRecord[] = [];
  let nextSeq = records.reduce((m, r) => Math.max(m, r.seq), 0) + 1;

  const emit = (
    rule: string,
    severity: AnomalySeverity,
    detail: string,
    refs: number[],
    traces: string[],
    ts: number,
  ): void => {
    out.push({
      seq: nextSeq++,
      ts,
      mono: 0,
      src: 'rust',
      lvl: severityToLvl(severity),
      kind: 'anomaly',
      rule,
      severity,
      detail,
      refs,
      traces,
    } as unknown as LogRecord);
  };

  // Shared mutation timeline (wall-clock ts of mutation-cmd ipc.calls).
  const mutations: number[] = [];

  // dup-ipc state: recent ipc.call events + per-key last-fire debounce.
  const ipcCalls: Array<{ ts: number; seq: number; key: string }> = [];
  const dupLastFire = new Map<string, number>();

  // slow-command state: per-cmd sample list, per-cmd last-fire, retained spans.
  const baselines = new Map<string, number[]>();
  const slowLastFire = new Map<string, number>();
  const spans: SpanInfo[] = [];

  for (const rec of records) {
    const ts = rec.ts;
    const kind = rec.kind;

    if (kind === 'ipc.call') {
      // dup-ipc filters EXPLICITLY on kind === 'ipc.call' (§5 / decision 20),
      // NOT on argsHash-absence — a non-ipc.call record carrying an argsHash can
      // never reach this branch.
      const cmd = String(rec.cmd ?? '');
      const argsHash = String(rec.argsHash ?? '');
      if (isMutationCmd(cmd)) mutations.push(ts);
      const key = `${cmd}\u0000${argsHash}`;
      // Prune to the window, then find the most-recent prior same-key call.
      for (let i = ipcCalls.length - 1; i >= 0; i -= 1) {
        if (ipcCalls[i].ts < ts - W_DUP_IPC_MS) ipcCalls.splice(i, 1);
      }
      const prior = [...ipcCalls].reverse().find((e) => e.key === key);
      ipcCalls.push({ ts, seq: rec.seq, key });
      if (prior) {
        const mutationBetween = mutations.some((m) => m > prior.ts && m <= ts);
        const last = dupLastFire.get(key);
        const armed = last === undefined || ts - last >= W_DUP_IPC_MS;
        if (!mutationBetween && armed) {
          dupLastFire.set(key, ts);
          emit(
            'dup-ipc',
            'warn',
            `${cmd}: duplicate call within ${W_DUP_IPC_MS}ms, no intervening mutation`,
            [prior.seq, rec.seq],
            [],
            ts,
          );
        }
      }
    } else if (kind === 'span') {
      const span = rec as unknown as SpanPayload & { seq: number; trace?: string };
      spans.push({
        trace: span.trace,
        seq: rec.seq,
        ms: span.ms,
        phases: span.phases ?? [],
      });
    } else if (kind === 'ipc.result') {
      const cmd = String(rec.cmd ?? '');
      const ms = Number(rec.ms ?? 0);
      const trace = typeof rec.trace === 'string' ? rec.trace : undefined;
      const { floor, k } = slowRuleFor(cmd);
      const samples = baselines.get(cmd) ?? [];

      // Decide BEFORE observing this sample (compare-then-observe, §5.1).
      let fire: AnomalySeverity | undefined;
      let firedP95: number | undefined;
      if (ms > HARD_MS) {
        fire = 'error';
      } else if (samples.length >= MIN_SAMPLES) {
        const p = p95(samples);
        if (p !== undefined && ms > Math.max(floor, k * p)) {
          fire = 'warn';
          firedP95 = p;
        }
      }

      if (fire) {
        const last = slowLastFire.get(cmd);
        const rateOk = last === undefined || ts - last >= SLOW_RATE_MS;
        if (rateOk) {
          slowLastFire.set(cmd, ts);
          const detail =
            firedP95 !== undefined
              ? `${cmd}: ${ms.toFixed(0)}ms > max(floor ${floor.toFixed(0)}, ${k}×p95 ${firedP95}) over ${samples.length} samples`
              : `${cmd}: ${ms.toFixed(0)}ms exceeds hard cap ${HARD_MS.toFixed(0)}ms`;
          emit('slow-command', fire, detail, [rec.seq], trace ? [trace] : [], ts);
          detectSlowPhase(spans, rec.seq, trace, ts, emit);
        }
      }

      // observe AFTER comparing (§5.1).
      samples.push(Math.max(0, ms));
      baselines.set(cmd, samples);
    }
  }

  return out;
}

/** §5.1 `slow-phase`: when a slow-command fires, if the correlated span (same
 *  `trace`) has a phase ≥70 % of its `ms`, emit `slow-phase` referencing both the
 *  span and result seqs. Mirrors `anomaly/slow.rs:detect_slow_phase`. */
function detectSlowPhase(
  spans: SpanInfo[],
  resultSeq: number,
  trace: string | undefined,
  ts: number,
  emit: (
    rule: string,
    severity: AnomalySeverity,
    detail: string,
    refs: number[],
    traces: string[],
    ts: number,
  ) => void,
): void {
  if (!trace) return;
  const span = [...spans].reverse().find((s) => s.trace === trace && s.phases.length > 0);
  if (!span || span.ms <= 0) return;
  let top: PhaseTiming | undefined;
  for (const ph of span.phases) {
    if (!top || ph.ms > top.ms) top = ph;
  }
  if (!top || top.ms < SLOW_PHASE_SHARE * span.ms) return;
  const share = top.ms / span.ms;
  emit(
    'slow-phase',
    'info',
    `phase ${top.name} = ${top.ms.toFixed(0)}ms (${(share * 100).toFixed(0)}% of ${span.ms.toFixed(0)}ms)`,
    [span.seq, resultSeq],
    [trace],
    ts,
  );
}
