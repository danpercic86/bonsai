// P61b: image-diff data for the open center-pane overlay slot when its path is
// an image (D4). Extracted verbatim from RepoWorkspace so the container only
// wires it. The overlay only ever serves workdir kinds (staged/unstaged/
// untracked) — commit/compare per-file diffs live in DiffBrowser — so the
// request is always a Workdir one.
import { useEffect, useRef, useState } from 'react';
import type { DiffOverlayMeta } from '../DiffOverlay';
import { ipc } from '../../ipc';
import type { ImageDiff, ImageDiffRequest, StatusSnapshot } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import { isImagePath } from '../../utils/imagePaths';

interface UseImageDiffDeps {
  repoId: string;
  overlayMeta: DiffOverlayMeta | null;
  /** Status-snapshot identity: a repo-changed refresh must re-read the image
   *  too (mirrors how the text slot refetches on status change). */
  status: StatusSnapshot | null;
}

export interface UseImageDiff {
  imageDiff: ImageDiff | null;
  imageDiffLoading: boolean;
  imageDiffError: string | null;
}

export function useImageDiff({ repoId, overlayMeta, status }: UseImageDiffDeps): UseImageDiff {
  // P61b: image-diff data for the open overlay slot when its path is an image
  // (D4). Fetched in parallel with the text slot (getWorkdirFileDiff still runs
  // and returns a cheap binary FileDiff); DiffOverlay renders DiffImageView from
  // this instead of the text diff. `imageDiffReqId` guards against races.
  const [imageDiff, setImageDiff] = useState<ImageDiff | null>(null);
  const [imageDiffLoading, setImageDiffLoading] = useState(false);
  const [imageDiffError, setImageDiffError] = useState<string | null>(null);
  const imageDiffReqId = useRef(0);
  // Last image path fetched: keep the previous image dimmed during a same-file
  // refresh, but clear it when a DIFFERENT image opens (no wrong image under the
  // new header).
  const imageDiffPathRef = useRef<string | null>(null);

  const overlayMetaRef = useRef(overlayMeta);
  overlayMetaRef.current = overlayMeta;

  // P61b: when the open overlay slot is an image (D4), fetch getImageDiff for
  // the current context and hand it to DiffOverlay. The overlay only ever serves
  // workdir kinds (staged/unstaged/untracked) — commit/compare per-file diffs
  // live in DiffBrowser — so the request is always a Workdir one. Depends on the
  // status snapshot identity so a repo-changed refresh re-reads the image too
  // (mirrors how the text slot refetches on status change). Non-image or
  // conflict/proposal slots clear the image state.
  useEffect(() => {
    const meta = overlayMetaRef.current;
    const isWorkdirKind =
      meta !== null &&
      (meta.kind === 'staged' || meta.kind === 'unstaged' || meta.kind === 'untracked');
    if (meta === null || !isWorkdirKind || !isImagePath(meta.path)) {
      imageDiffReqId.current += 1;
      imageDiffPathRef.current = null;
      setImageDiff(null);
      setImageDiffLoading(false);
      setImageDiffError(null);
      return;
    }
    const request: ImageDiffRequest = {
      kind: 'workdir',
      path: meta.path,
      origPath: meta.origPath,
      staged: meta.kind === 'staged',
    };
    const id = ++imageDiffReqId.current;
    // A different image than the one currently shown -> drop the stale preview.
    if (imageDiffPathRef.current !== meta.path) setImageDiff(null);
    imageDiffPathRef.current = meta.path;
    setImageDiffLoading(true);
    setImageDiffError(null);
    void ipc.getImageDiff(repoId, request).then(
      (d) => {
        if (id !== imageDiffReqId.current) return;
        setImageDiff(d);
        setImageDiffLoading(false);
      },
      (e) => {
        if (id !== imageDiffReqId.current) return;
        setImageDiff(null);
        setImageDiffError(errorMessage(e));
        setImageDiffLoading(false);
      },
    );
    // overlayMeta is read via ref; the primitive deps below capture every change
    // that matters (which file, which section) plus a status-driven refresh.
  }, [repoId, overlayMeta?.path, overlayMeta?.kind, overlayMeta?.origPath, status]);

  return { imageDiff, imageDiffLoading, imageDiffError };
}
