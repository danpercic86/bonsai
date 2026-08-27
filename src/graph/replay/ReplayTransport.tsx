// Spec-007 UI contract §3.2–§3.4: the presentational transport bar. 40px,
// bottom-anchored, density-invariant (mode overlay — outside the §3 density
// scopes). Reduced motion (§6): play/pause + speed are HIDDEN (not disabled)
// and a hint line takes the left slot. Keyboard is handled by the overlay root
// (ReplayMode) — events bubble through it — so the div-slider carries only the
// slider ARIA semantics and pointer scrubbing here.
import { useRef } from 'react';
import type { ReplaySpeed } from './replayModel';
import { REPLAY_SPEEDS } from './replayModel';

export interface ReplayTransportProps {
  status: 'paused' | 'playing' | 'finished';
  reducedMotion: boolean;
  /** Revealed / total rows; drive the progress label + slider valuetext. */
  revealed: number;
  total: number;
  playheadMs: number;
  totalMs: number;
  /** Frontier commit month-year ("Jun 2024"); null while nothing is revealed. */
  frontierLabel: string | null;
  speed: ReplaySpeed;
  onTogglePlay(): void;
  onScrub(fraction: number): void;
  onSetSpeed(sp: ReplaySpeed): void;
  onExit(): void;
  /** Focus anchors for entry focus (§3.3). */
  playRef: React.RefObject<HTMLButtonElement | null>;
  sliderRef: React.RefObject<HTMLDivElement | null>;
}

function fmt(n: number): string {
  return n.toLocaleString();
}

export function ReplayTransport(p: ReplayTransportProps) {
  const trackRef = useRef<HTMLDivElement>(null);
  const draggingRef = useRef(false);

  const scrubAt = (clientX: number): void => {
    const track = trackRef.current;
    if (track === null) return;
    const rect = track.getBoundingClientRect();
    if (rect.width <= 0) return;
    p.onScrub((clientX - rect.left) / rect.width);
  };

  const fraction = p.totalMs > 0 ? Math.min(1, p.playheadMs / p.totalMs) : 0;
  const progress = `${fmt(p.revealed)} / ${fmt(p.total)}${
    p.frontierLabel !== null ? ` · ${p.frontierLabel}` : ''
  }`;
  const valueText = `Commit ${fmt(p.revealed)} of ${fmt(p.total)}${
    p.frontierLabel !== null ? ` — ${p.frontierLabel}` : ''
  }`;
  const playing = p.status === 'playing';
  const playLabel = playing ? 'Pause' : p.status === 'finished' ? 'Replay again' : 'Play';

  return (
    <div className="graph-replay-transport">
      {p.reducedMotion ? (
        <span className="graph-replay-hint">Drag the slider to move through history.</span>
      ) : (
        <button
          ref={p.playRef}
          type="button"
          className="graph-replay-play"
          aria-label={playLabel}
          title={playLabel}
          onClick={p.onTogglePlay}
        >
          {playing ? '⏸' : '▶'}
        </button>
      )}
      <div
        ref={p.sliderRef}
        className="graph-replay-slider"
        role="slider"
        tabIndex={0}
        aria-label="Replay position"
        aria-valuemin={0}
        aria-valuemax={Math.round(p.totalMs)}
        aria-valuenow={Math.round(p.playheadMs)}
        aria-valuetext={valueText}
        onPointerDown={(e) => {
          draggingRef.current = true;
          e.currentTarget.setPointerCapture(e.pointerId);
          scrubAt(e.clientX);
        }}
        onPointerMove={(e) => {
          if (draggingRef.current) scrubAt(e.clientX);
        }}
        onPointerUp={() => {
          draggingRef.current = false;
        }}
        onPointerCancel={() => {
          draggingRef.current = false;
        }}
      >
        <div ref={trackRef} className="graph-replay-track">
          <div className="graph-replay-fill" style={{ width: `${fraction * 100}%` }} />
          <div className="graph-replay-thumb" style={{ left: `${fraction * 100}%` }} />
        </div>
      </div>
      {!p.reducedMotion && (
        <div className="settings-segmented graph-replay-speed" role="radiogroup" aria-label="Playback speed">
          {REPLAY_SPEEDS.map((sp) => (
            <label key={sp} className={`settings-segment${p.speed === sp ? ' is-selected' : ''}`}>
              <input
                type="radio"
                className="settings-segment-input"
                name="replay-speed"
                checked={p.speed === sp}
                onChange={() => p.onSetSpeed(sp)}
              />
              {sp}×
            </label>
          ))}
        </div>
      )}
      <span className="graph-replay-progress" aria-hidden="true">
        {progress}
      </span>
      <button
        type="button"
        className="graph-replay-close"
        aria-label="Exit replay"
        title="Exit replay (Esc)"
        onClick={p.onExit}
      >
        ✕
      </button>
    </div>
  );
}
