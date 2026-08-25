import { useCallback, useRef, useState } from 'react';
import { ipc } from '../../ipc';
import type {
  AiAnalysisMode,
  AiDiffTarget,
  AiDigestRange,
  ChangelogRange,
} from '../../ipc';
import { errorMessage } from '../../utils/errors';

/** P15b/P15c/P28/P53a/P56b/P57c: the shared read-only AI-output panel + every
 *  runner that fills it. RepoWorkspace owns the ipc.ai* calls; the panel itself
 *  is presentational. `null` => not shown. A single req-id guards against a stale
 *  response overwriting a newer request or a closed panel — every runner shares
 *  it so a slow/superseded reply is dropped (last-wins). Extracted verbatim from
 *  RepoWorkspace so the container stays a composition site. */
export function useAiPanel(repoId: string) {
  // P15b: explain/review output panel (read-only prose). RepoWorkspace owns the
  // ipc.aiAnalyzeDiff call + the panel's loading/error/result state; the panel is
  // presentational. `null` => not shown. A req-id guards against a stale response
  // overwriting a newer request or a closed panel.
  const [aiPanel, setAiPanel] = useState<{
    title: string;
    text: string | null;
    loading: boolean;
    error: string | null;
    costUsd: number | null;
    /** P56b: opt-in editable body — set only by runChangelog so the notes can be
     *  tweaked before copying. Every other runner omits it (read-only <pre>). */
    editable?: boolean;
  } | null>(null);
  const aiPanelReqId = useRef(0);
  const aiPanelOpenRef = useRef(false);
  aiPanelOpenRef.current = aiPanel !== null;

  // P15b: run an explain/review analysis of a diff target and show the prose in
  // the AiOutputPanel. Read-only — writes nothing. Guarded by a req-id so a slow
  // response can't clobber a newer request or a closed panel.
  const runAnalyze = useCallback(
    (target: AiDiffTarget, mode: AiAnalysisMode, title: string) => {
      const id = ++aiPanelReqId.current;
      setAiPanel({ title, text: null, loading: true, error: null, costUsd: null });
      ipc.aiAnalyzeDiff(repoId, target, mode).then(
        (res) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: res.text, loading: false, error: null, costUsd: res.costUsd });
        },
        (e: unknown) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: null, loading: false, error: errorMessage(e), costUsd: null });
        },
      );
    },
    [repoId],
  );

  // P57c: answer a natural-language history question grounded in the retrieved
  // commits' real diffs, rendering the prose in the shared AiOutputPanel. Shares
  // runAnalyze's last-wins req-id guard so a slow/superseded response can't
  // clobber a newer request or a closed panel. `runHistoryAnswer` is handed to
  // useHistorySearch as its `runAiAnswer` route.
  const runHistoryAnswer = useCallback(
    (question: string, topK: number) => {
      const title = `History: "${question}"`;
      const id = ++aiPanelReqId.current;
      setAiPanel({ title, text: null, loading: true, error: null, costUsd: null });
      ipc.aiSearchHistory(repoId, question, topK).then(
        (res) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: res.text, loading: false, error: null, costUsd: res.costUsd });
        },
        (e: unknown) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: null, loading: false, error: errorMessage(e), costUsd: null });
        },
      );
    },
    [repoId],
  );

  // P15c: summarize the commits/diff unique to `target` vs `base` and show the
  // prose in the AiOutputPanel. Read-only — writes nothing. Shares the same
  // req-id guard as runAnalyze so a slow response can't clobber a newer request
  // or a closed panel.
  const runSummarize = useCallback(
    (base: string, target: string) => {
      const title = `Summary: ${base} → ${target}`;
      const id = ++aiPanelReqId.current;
      setAiPanel({ title, text: null, loading: true, error: null, costUsd: null });
      ipc.aiSummarizeRange(repoId, base, target).then(
        (res) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: res.text, loading: false, error: null, costUsd: res.costUsd });
        },
        (e: unknown) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: null, loading: false, error: errorMessage(e), costUsd: null });
        },
      );
    },
    [repoId],
  );

  // P28 §7: digest "what changed" over a range and show the prose in the
  // AiOutputPanel. Read-only — writes nothing. Shares the same req-id guard as
  // runAnalyze so a slow response can't clobber a newer request or a closed
  // panel. `title` is range-derived, built by WhatChangedDialog.
  const runDigest = useCallback(
    (range: AiDigestRange, title: string) => {
      const id = ++aiPanelReqId.current;
      setAiPanel({ title, text: null, loading: true, error: null, costUsd: null });
      ipc.aiDigest(repoId, range).then(
        (res) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: res.text, loading: false, error: null, costUsd: res.costUsd });
        },
        (e: unknown) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: null, loading: false, error: errorMessage(e), costUsd: null });
        },
      );
    },
    [repoId],
  );

  // P56b §6: generate grouped release notes for a tag/ref range and show the
  // Markdown in the AiOutputPanel (editable). Read-only — writes nothing. Shares
  // the same req-id guard as runAnalyze so a slow response can't clobber a newer
  // request or a closed panel. The provisional `title` covers the loading state;
  // on success the header becomes `Release notes: <fromRef>..<toRef>` from the
  // RESOLVED range (e.g. the previous-tag name for sinceLastTag).
  const runChangelog = useCallback(
    (range: ChangelogRange, title: string) => {
      const id = ++aiPanelReqId.current;
      setAiPanel({ title, text: null, loading: true, error: null, costUsd: null, editable: true });
      ipc.aiChangelog(repoId, range).then(
        (res) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({
            title: `Release notes: ${res.fromRef}..${res.toRef}`,
            text: res.text,
            loading: false,
            error: null,
            costUsd: res.costUsd,
            editable: true,
          });
        },
        (e: unknown) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({
            title,
            text: null,
            loading: false,
            error: errorMessage(e),
            costUsd: null,
            editable: true,
          });
        },
      );
    },
    [repoId],
  );

  // P53a: blame-why — explain WHY a line exists and show the prose in the
  // AiOutputPanel. Read-only — writes nothing. Shares the same req-id guard as
  // runAnalyze so a slow response can't clobber a newer request or a closed
  // panel. `atOid` is the blamed version (null => HEAD in v1).
  const runExplainLine = useCallback(
    (path: string, lineNo: number, atOid: string | null, title: string) => {
      const id = ++aiPanelReqId.current;
      setAiPanel({ title, text: null, loading: true, error: null, costUsd: null });
      ipc.aiExplainLine(repoId, path, lineNo, atOid).then(
        (res) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: res.text, loading: false, error: null, costUsd: res.costUsd });
        },
        (e: unknown) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: null, loading: false, error: errorMessage(e), costUsd: null });
        },
      );
    },
    [repoId],
  );

  // P53a: BlameView "Why?" entry point — blame is always vs HEAD in v1, so
  // atOid is null. Title mirrors the mock/backend grounding label.
  const onBlameExplain = useCallback(
    (path: string, lineNo: number) => {
      runExplainLine(path, lineNo, null, `Why line ${lineNo} of ${path}`);
    },
    [runExplainLine],
  );

  const closeAiPanel = useCallback(() => {
    aiPanelReqId.current += 1;
    setAiPanel(null);
  }, []);

  return {
    aiPanel,
    aiPanelOpenRef,
    runAnalyze,
    runHistoryAnswer,
    runSummarize,
    runDigest,
    runChangelog,
    runExplainLine,
    onBlameExplain,
    closeAiPanel,
  };
}
