import { useCallback, useRef, useState } from 'react';
import type { PointerEvent as ReactPointerEvent } from 'react';
import type { ImageDiff, ImageSide } from '../ipc';

// P61b: presentational image-diff viewer. Three compare modes, all view-local
// state (the active `mode` is owned by DiffOverlay's switcher; opacity/divider
// are internal here). Sides arrive as base64 (D2); we build a `data:` URL for a
// plain <img>. Onion/Swipe need BOTH sides — when one is absent (add/delete) or
// over-cap they fall back to the labelled side-by-side panes.

export type ImageMode = 'sideBySide' | 'onion' | 'swipe';

/** Per-side cap for the too-large message (mirrors Rust MAX_IMAGE_BYTES = 8 MiB).
 *  The over-cap side arrives as `null` and carries no byteLen, so the exact size
 *  is unavailable — we cite the cap instead. */
const MAX_IMAGE_MB = 8;

function dataUrl(side: ImageSide): string {
  return `data:${side.mime};base64,${side.base64}`;
}

function sizeLabel(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export interface DiffImageViewProps {
  diff: ImageDiff;
  mode: ImageMode;
}

export function DiffImageView({ diff, mode }: DiffImageViewProps) {
  const oldUrl = diff.old !== null ? dataUrl(diff.old) : null;
  const newUrl = diff.new !== null ? dataUrl(diff.new) : null;
  // Onion / swipe only make sense with both sides; otherwise degrade to panes.
  if (mode !== 'sideBySide' && oldUrl !== null && newUrl !== null) {
    return mode === 'onion' ? (
      <OnionView oldUrl={oldUrl} newUrl={newUrl} />
    ) : (
      <SwipeView oldUrl={oldUrl} newUrl={newUrl} />
    );
  }
  return <SideBySide diff={diff} />;
}

// ---------- side-by-side (also the fallback when a side is absent) ----------

function SideBySide({ diff }: { diff: ImageDiff }) {
  return (
    <div className="img-diff-scroll">
      <div className="img-diff img-diff-sbs">
        <ImagePane title="Old" side={diff.old} tooLarge={diff.oldTooLarge} absentLabel="Added" />
        <ImagePane
          title="New"
          side={diff.new}
          tooLarge={diff.newTooLarge}
          absentLabel="Deleted"
        />
      </div>
    </div>
  );
}

function ImagePane({
  title,
  side,
  tooLarge,
  absentLabel,
}: {
  title: string;
  side: ImageSide | null;
  tooLarge: boolean;
  absentLabel: string;
}) {
  return (
    <figure className="img-diff-pane">
      <figcaption className="img-diff-caption">
        <span>{title}</span>
        {side !== null && <span className="img-diff-size mono">{sizeLabel(side.byteLen)}</span>}
      </figcaption>
      {side !== null ? (
        <div className="img-diff-frame">
          <img className="img-diff-img" src={dataUrl(side)} alt={`${title} version`} />
        </div>
      ) : (
        <div className="img-diff-missing">
          {tooLarge ? `Larger than ${MAX_IMAGE_MB} MB — too large to preview` : absentLabel}
        </div>
      )}
    </figure>
  );
}

// ---------- onion-skin ----------

function OnionView({ oldUrl, newUrl }: { oldUrl: string; newUrl: string }) {
  const [opacity, setOpacity] = useState(0.5);
  return (
    <div className="img-diff-scroll">
      <div className="img-diff img-diff-onion">
        <div className="img-diff-stage">
          <img className="img-onion-base" src={oldUrl} alt="Old version" draggable={false} />
          <img
            className="img-onion-top"
            src={newUrl}
            alt="New version"
            draggable={false}
            style={{ opacity }}
          />
        </div>
        <label className="img-diff-control">
          <span>Old</span>
          <input
            type="range"
            min={0}
            max={1}
            step={0.01}
            value={opacity}
            onChange={(e) => setOpacity(Number(e.target.value))}
            aria-label="Crossfade old to new"
          />
          <span>New</span>
        </label>
      </div>
    </div>
  );
}

// ---------- swipe ----------

function SwipeView({ oldUrl, newUrl }: { oldUrl: string; newUrl: string }) {
  const [divider, setDivider] = useState(0.5);
  const stageRef = useRef<HTMLDivElement>(null);
  const draggingRef = useRef(false);

  const updateFromClientX = useCallback((clientX: number) => {
    const el = stageRef.current;
    if (el === null) return;
    const rect = el.getBoundingClientRect();
    if (rect.width === 0) return;
    const frac = (clientX - rect.left) / rect.width;
    setDivider(Math.min(1, Math.max(0, frac)));
  }, []);

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    draggingRef.current = true;
    e.currentTarget.setPointerCapture(e.pointerId);
    updateFromClientX(e.clientX);
  };
  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (draggingRef.current) updateFromClientX(e.clientX);
  };
  const stopDragging = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!draggingRef.current) return;
    draggingRef.current = false;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
  };

  const pct = `${divider * 100}%`;
  return (
    <div className="img-diff-scroll">
      <div className="img-diff img-diff-swipe">
        <div
          className="img-diff-stage img-swipe-stage"
          ref={stageRef}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={stopDragging}
          onPointerLeave={stopDragging}
        >
          <img className="img-swipe-under" src={oldUrl} alt="Old version" draggable={false} />
          <img
            className="img-swipe-over"
            src={newUrl}
            alt="New version"
            draggable={false}
            // Keep only the left `divider` fraction of the NEW image (clip the
            // right side away), so left shows NEW and right reveals OLD beneath.
            style={{ clipPath: `inset(0 ${(1 - divider) * 100}% 0 0)` }}
          />
          <div className="img-swipe-divider" style={{ left: pct }} aria-hidden="true" />
        </div>
        <div className="img-diff-hint">Drag to compare — left: new · right: old</div>
      </div>
    </div>
  );
}
