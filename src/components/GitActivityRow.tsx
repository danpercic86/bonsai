/**
 * P87b §3.4/§3.5 — one git-activity run row: a collapsed summary line (glyph,
 * noun, status pill, duration, timestamp, chevron) that discloses per-hook
 * sub-rows, the output log (reusing the `.ai-log` surface) and a Copy button.
 *
 * PURE-ish: the only local state is the disclosure toggle and the transient
 * "Copied" flag; everything shown is derived from the run + `tick` by
 * `gitActivityFormat.ts`.
 */
import { useEffect, useState } from 'react';

import {
  GIT_ACTIVITY_LINE_MAX_CHARS,
} from './repoWorkspace/gitActivityLog';
import {
  categoryMeta,
  durationLabel,
  hookPill,
  objectsReadout,
  phaseLabel,
  statusPill,
  timeLabel,
  timeTitle,
} from './gitActivityFormat';
import type { GitActivityCategory } from '../ipc';
import type { GitActivityRun } from './repoWorkspace/useGitActivity';

export interface GitActivityRowProps {
  run: GitActivityRun;
  /** Live clock for the running row's elapsed. */
  tick: number;
}

/** The op word for the blocking-hook dialog note (§3.5-4 / §8). */
function opNoun(category: GitActivityCategory): string {
  if (category === 'push' || category === 'forcePush') return 'push';
  if (category === 'mergeCommit') return 'merge';
  return 'commit';
}

export function GitActivityRow({ run, tick }: GitActivityRowProps) {
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const id = window.setTimeout(() => setCopied(false), 1200);
    return () => window.clearTimeout(id);
  }, [copied]);

  const meta = categoryMeta(run.category);
  const pill = statusPill(run.status);
  const Glyph = meta.glyph;
  const running = run.status === 'running';
  // §3.4 sub-line (running only): the live phase / transfer readout.
  const subLabel = running ? (objectsReadout(run) ?? phaseLabel(run.category, run.phase)) : null;

  // §3.5-4: a failed run whose failure was a blocking hook (a failed hook record).
  const blockingHook =
    run.status === 'failed' && run.hooks.some((h) => !h.success);

  const combinedOutput = run.lines.map((l) => l.text).join('\n');
  const canCopy = run.lines.length > 0;

  const onCopy = () => {
    const p =
      navigator.clipboard?.writeText(combinedOutput) ??
      Promise.reject(new Error('Clipboard unavailable'));
    void p.then(() => setCopied(true)).catch(() => setCopied(false));
  };

  const toggle = () => setExpanded((o) => !o);

  return (
    <li className="git-run" data-status={run.status}>
      <div
        className="git-run-summary"
        data-run-row
        tabIndex={-1}
        onClick={toggle}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            toggle();
          }
        }}
      >
        <button
          type="button"
          className="file-chevron git-run-chevron"
          aria-expanded={expanded}
          aria-label={expanded ? 'Collapse run' : 'Expand run'}
          onClick={(e) => {
            e.stopPropagation();
            toggle();
          }}
        >
          <span aria-hidden="true">{expanded ? '▾' : '▸'}</span>
        </button>
        <span className="git-run-glyph" aria-hidden="true">
          <Glyph />
        </span>
        <span className="git-run-noun">{meta.noun}</span>
        {subLabel !== null && (
          <span className="git-run-subphase" title={phaseLabel(run.category, run.phase)}>
            {subLabel}
          </span>
        )}
        {run.linesDropped > 0 && (
          <span
            className="git-run-trimmed"
            title="Bonsai keeps the last 500 output lines per run"
          >
            {'⋯ trimmed'}
          </span>
        )}
        <span className="git-run-pill" data-status={pill.dataStatus}>
          <span className="git-run-pill-glyph" aria-hidden="true">
            {pill.glyph}
          </span>
          {pill.label}
        </span>
        <span className="git-run-duration">{durationLabel(run, tick)}</span>
        {!running && run.endedAt !== null && (
          <span className="git-run-time" title={timeTitle(run.endedAt)}>
            {timeLabel(run.endedAt)}
          </span>
        )}
      </div>

      {expanded && (
        <div className="git-run-detail">
          {run.hooks.length > 0 && (
            <ul className="git-run-hooks">
              {run.hooks.map((h) => {
                const hp = hookPill(h.code, h.success);
                return (
                  <li key={`${h.hook}:${h.at}`} className="git-run-hook">
                    <span className="git-run-hook-dot" aria-hidden="true">
                      {'•'}
                    </span>
                    <span className="git-run-hook-name">{h.hook}</span>
                    <span className="git-run-hook-pill" data-status={hp.dataStatus}>
                      <span className="git-run-pill-glyph" aria-hidden="true">
                        {hp.glyph}
                      </span>
                      {hp.label}
                    </span>
                  </li>
                );
              })}
            </ul>
          )}

          <div className="git-run-output">
            {canCopy && (
              <button
                type="button"
                className="btn-icon git-run-copy"
                aria-label="Copy output"
                title="Copy"
                onClick={onCopy}
              >
                {copied ? 'Copied' : 'Copy'}
              </button>
            )}
            <ol className="ai-log git-run-log" tabIndex={0} aria-label="Run output">
              {run.linesDropped > 0 && (
                <li
                  className="ai-log-dropped"
                  title="Bonsai keeps the last 500 output lines per run"
                >
                  {`↑ ${run.linesDropped.toLocaleString()} earlier lines trimmed`}
                </li>
              )}
              {run.lines.length === 0 && run.hooks.length === 0 && (
                <li className="git-log-empty">{running ? 'Working…' : 'No output.'}</li>
              )}
              {run.lines.map((line) => (
                <li key={line.seq} className="git-log-line" data-stream={line.stream}>
                  {line.stream === 'stderr' && <span className="sr-only">stderr: </span>}
                  {line.text}
                  {line.text.length === GIT_ACTIVITY_LINE_MAX_CHARS && (
                    <span
                      className="ai-log-trunc"
                      title="This line was cut off at 2,000 characters"
                    >
                      truncated
                    </span>
                  )}
                </li>
              ))}
            </ol>
          </div>

          {blockingHook && (
            <p className="git-run-note">
              {`This hook blocked the ${opNoun(run.category)}. The full output opened in a dialog.`}
            </p>
          )}
        </div>
      )}
    </li>
  );
}
