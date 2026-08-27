/** GraphCanvas mount effect — ResizeObserver on the host + DPR-change handling
 *  (re-armed matchMedia listener, M2c §4.3). Moved VERBATIM out of
 *  GraphCanvas.tsx (spec-005 size offset — no behavior change): `resize()` also
 *  performs the initial paint, and unmount cancels any pending paint rAF. */
import { useEffect } from 'react';
import type { RefObject } from 'react';

export function useCanvasResizeObserver(
  hostRef: RefObject<HTMLDivElement | null>,
  resize: () => void,
  rafRef: RefObject<number>,
): void {
  useEffect(() => {
    const host = hostRef.current;
    if (host === null) return;
    resize();
    const ro = new ResizeObserver(() => resize());
    ro.observe(host);

    let mq: MediaQueryList | null = null;
    const onDprChange = (): void => {
      resize();
      arm();
    };
    const arm = (): void => {
      mq?.removeEventListener('change', onDprChange);
      mq = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
      mq.addEventListener('change', onDprChange);
    };
    arm();

    return () => {
      ro.disconnect();
      mq?.removeEventListener('change', onDprChange);
      if (rafRef.current !== 0) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = 0;
      }
    };
  }, [hostRef, resize, rafRef]);
}
