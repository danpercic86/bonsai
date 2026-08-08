import { useEffect, useRef, useState } from 'react';
import { ipc } from '../ipc';
import type {
  CreatePrInput,
  ForgeRepoContext,
  PrDetail,
  PrNavRequest,
  PrStateFilter,
  PrSummary,
  ReviewComment,
} from '../ipc';
import { usePushToast } from '../ToastContext';
import { errorMessage } from '../utils/errors';
import { SkeletonRows } from './CommitPanel';
import { ForgeConnect } from './ForgeConnect';
import { PrCreateForm } from './PrCreateForm';
import { PrDetailView } from './PrDetailView';
import { PrList } from './PrList';
import { PrReviewComments } from './PrReviewComments';

// P62c: right-pane PR panel CONTAINER (contract §8). Owns view state, the
// forge* IPC calls, and last-wins req-id guards (mirrors DiffImageCard). It is
// mounted only while the right-pane tab is 'prs', so mounting == "open the PR
// tab" and drives the bootstrap flow: repoContext → unsupported / connect /
// list; selecting a row loads detail + comments; create opens the new PR.

type View = 'loading' | 'error' | 'unsupported' | 'connect' | 'list' | 'detail' | 'create';

export interface PrPanelProps {
  repoId: string;
  /** Current branch name — seeds the create form's compare field. */
  defaultHead?: string | null;
  /** Base-branch hint for the create form (e.g. the upstream target). */
  defaultBase?: string | null;
  /** P63: external "open PR N" request (from a graph PR-badge click). A bumped
   *  `seq` re-opens even the same number. null ⇒ no pending navigation. */
  openToPr?: PrNavRequest | null;
}

export function PrPanel({ repoId, defaultHead, defaultBase, openToPr }: PrPanelProps) {
  const pushToast = usePushToast();

  const [ctx, setCtx] = useState<ForgeRepoContext | null>(null);
  const [view, setView] = useState<View>('loading');
  const [bootstrapError, setBootstrapError] = useState<string | null>(null);

  const [prs, setPrs] = useState<PrSummary[]>([]);
  const [filter, setFilter] = useState<PrStateFilter>('open');
  const [listLoading, setListLoading] = useState(false);
  const [listError, setListError] = useState<string | null>(null);

  const [selectedNumber, setSelectedNumber] = useState<number | null>(null);
  const [detail, setDetail] = useState<PrDetail | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [comments, setComments] = useState<ReviewComment[]>([]);
  const [commentsLoading, setCommentsLoading] = useState(false);
  const [commentsError, setCommentsError] = useState<string | null>(null);

  const [connecting, setConnecting] = useState(false);
  const [connectError, setConnectError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);

  const [bootstrapTick, setBootstrapTick] = useState(0);
  const [listTick, setListTick] = useState(0);

  // Per-concern last-wins guards: only the newest in-flight request may write.
  const ctxReqRef = useRef(0);
  const listReqRef = useRef(0);
  const detailReqRef = useRef(0);

  const authed = ctx !== null && ctx.provider === 'gitHub' && ctx.authenticated;

  // Bootstrap: resolve the forge context on mount / repo change / after connect.
  useEffect(() => {
    const id = ++ctxReqRef.current;
    setView('loading');
    setBootstrapError(null);
    void ipc.forgeRepoContext(repoId).then(
      (c) => {
        if (id !== ctxReqRef.current) return;
        setCtx(c);
        if (c.provider === 'unknown') {
          setView('unsupported');
        } else if (!c.authenticated) {
          setView('connect');
        } else {
          setListLoading(true);
          setView('list');
        }
      },
      (e: unknown) => {
        if (id !== ctxReqRef.current) return;
        setCtx(null);
        setBootstrapError(errorMessage(e));
        setView('error');
      },
    );
  }, [repoId, bootstrapTick]);

  // List: (re)load whenever authenticated and the filter / refresh tick change.
  useEffect(() => {
    if (!authed) return;
    const id = ++listReqRef.current;
    setListLoading(true);
    setListError(null);
    void ipc.forgeListPrs(repoId, { state: filter, page: 1, perPage: 30 }).then(
      (page) => {
        if (id !== listReqRef.current) return;
        setPrs(page.items);
        setListLoading(false);
      },
      (e: unknown) => {
        if (id !== listReqRef.current) return;
        setPrs([]);
        setListError(errorMessage(e));
        setListLoading(false);
        pushToast('error', `Could not load pull requests: ${errorMessage(e)}`);
      },
    );
  }, [repoId, filter, listTick, authed, pushToast]);

  // P63: react to an external "open PR N" request (a graph PR-badge click). Runs
  // once per new `seq` while authenticated; unauthenticated ⇒ the bootstrap flow
  // already shows ForgeConnect, so we simply wait (a later connect re-fires this
  // via the `authed` dep). A per-seq guard prevents a ctx reload from re-opening.
  const lastNavSeqRef = useRef<number | null>(null);
  useEffect(() => {
    if (openToPr === null || openToPr === undefined || !authed) return;
    if (lastNavSeqRef.current === openToPr.seq) return;
    lastNavSeqRef.current = openToPr.seq;
    loadDetail(openToPr.number);
    // loadDetail is a stable component-scope fn; depend only on the request+auth.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [openToPr, authed]);

  function loadDetail(number: number) {
    const id = ++detailReqRef.current;
    setSelectedNumber(number);
    setView('detail');
    setDetail(null);
    setDetailError(null);
    setComments([]);
    setCommentsError(null);
    setCommentsLoading(true);
    void ipc.forgeGetPr(repoId, number).then(
      (d) => {
        if (id !== detailReqRef.current) return;
        setDetail(d);
      },
      (e: unknown) => {
        if (id !== detailReqRef.current) return;
        setDetailError(errorMessage(e));
        pushToast('error', `Could not load PR #${number}: ${errorMessage(e)}`);
      },
    );
    void ipc.forgeListReviewComments(repoId, number).then(
      (cs) => {
        if (id !== detailReqRef.current) return;
        setComments(cs);
        setCommentsLoading(false);
      },
      (e: unknown) => {
        if (id !== detailReqRef.current) return;
        setCommentsError(errorMessage(e));
        setCommentsLoading(false);
      },
    );
  }

  function handleConnect(token: string) {
    setConnecting(true);
    setConnectError(null);
    void ipc.forgeSetToken(repoId, token).then(
      () => {
        setConnecting(false);
        setBootstrapTick((t) => t + 1); // re-run context → authenticated → list
      },
      (e: unknown) => {
        setConnecting(false);
        setConnectError(errorMessage(e));
        pushToast('error', `Could not connect: ${errorMessage(e)}`);
      },
    );
  }

  function handleCreate(input: CreatePrInput) {
    setCreating(true);
    setCreateError(null);
    void ipc.forgeCreatePr(repoId, input).then(
      (d) => {
        setCreating(false);
        // Show the new PR's detail directly; a fresh PR has no comments yet.
        ++detailReqRef.current; // supersede any in-flight detail load
        setSelectedNumber(d.summary.number);
        setDetail(d);
        setDetailError(null);
        setComments([]);
        setCommentsLoading(false);
        setCommentsError(null);
        setView('detail');
        setListTick((t) => t + 1); // refresh the list behind the detail
        pushToast('success', `Opened PR #${d.summary.number} — ${d.summary.url}`);
      },
      (e: unknown) => {
        setCreating(false);
        setCreateError(errorMessage(e));
        pushToast('error', `Could not open the pull request: ${errorMessage(e)}`);
      },
    );
  }

  return <div className="pr-panel">{renderBody()}</div>;

  function renderBody() {
    switch (view) {
      case 'loading':
        return (
          <div className="pr-panel-loading">
            <SkeletonRows />
          </div>
        );
      case 'error':
        return (
          <div className="pr-panel-state">
            <div className="error-banner error-banner-dismissible pr-error" role="alert">
              <span className="error-banner-text">{bootstrapError ?? 'Forge unavailable'}</span>
              <button
                type="button"
                className="section-action"
                onClick={() => setBootstrapTick((t) => t + 1)}
              >
                Retry
              </button>
            </div>
          </div>
        );
      case 'unsupported':
        return (
          <div className="pr-panel-state">
            <p className="pane-empty">
              {ctx !== null
                ? `${ctx.host} isn't a supported forge yet — pull requests are unavailable for this repository.`
                : "This repository's origin isn't a supported forge."}
            </p>
          </div>
        );
      case 'connect':
        return (
          <ForgeConnect
            host={ctx?.host ?? 'the forge'}
            owner={ctx?.owner ?? ''}
            repo={ctx?.repo ?? ''}
            submitting={connecting}
            error={connectError}
            onSubmit={handleConnect}
          />
        );
      case 'create':
        return (
          <PrCreateForm
            defaultHead={defaultHead}
            defaultBase={defaultBase}
            submitting={creating}
            error={createError}
            onSubmit={handleCreate}
            onCancel={() => setView('list')}
          />
        );
      case 'detail':
        return renderDetail();
      case 'list':
      default:
        return (
          <PrList
            items={prs}
            selectedNumber={selectedNumber}
            loading={listLoading}
            error={listError}
            filter={filter}
            onChangeFilter={setFilter}
            onSelect={loadDetail}
            onRefresh={() => setListTick((t) => t + 1)}
            onCreate={() => {
              setCreateError(null);
              setView('create');
            }}
          />
        );
    }
  }

  function renderDetail() {
    if (detail !== null) {
      return (
        <PrDetailView detail={detail} onBack={() => setView('list')}>
          <PrReviewComments comments={comments} loading={commentsLoading} error={commentsError} />
        </PrDetailView>
      );
    }
    return (
      <div className="pr-detail pr-detail-shell">
        <div className="pr-detail-header pr-detail-title-row">
          <button type="button" className="section-action pr-back-button" onClick={() => setView('list')}>
            {'← Pull requests'}
          </button>
        </div>
        {detailError !== null ? (
          <div className="error-banner error-banner-dismissible pr-error" role="alert">
            <span className="error-banner-text">{detailError}</span>
          </div>
        ) : (
          <div className="pr-detail-loading">
            <SkeletonRows />
          </div>
        )}
      </div>
    );
  }
}
