import type { BranchesSnapshot, GraphLayout } from '../ipc';

/** P50c: command-palette entry taxonomy. `action` = an app/repo command,
 *  `branch`/`tag`/`commit` = a navigation jump (reveal in the graph), `search` =
 *  the dynamic "search commits for …" row. */
export type PaletteGroup = 'action' | 'branch' | 'tag' | 'commit' | 'search';

/** One palette row. `run` is a pre-bound thunk over an EXISTING handler — the
 *  palette never issues raw git IPC, so a mutating command (if ever added) must
 *  route through its confirm dialog here, exactly as the toolbar/menu does. */
export interface PaletteAction {
  id: string;
  title: string;
  /** Right-aligned muted text (shortcut hint or short-oid). */
  hint?: string;
  group: PaletteGroup;
  /** Extra text folded into the fuzzy match (never shown). */
  keywords?: string;
  /** Greyed + non-runnable (mirrors the toolbar's disabled state). */
  disabled?: boolean;
  run(): void;
}

/** Deps assembled by RepoWorkspace (repo-scoped handlers + snapshots) merged with
 *  the App-level `appCommands`. Pure w.r.t. these — no component state read here. */
export interface BuildPaletteDeps {
  // Gate flags (mirror the toolbar/menu disabled predicates).
  mutating: boolean;
  refreshing: boolean;
  statusLoading: boolean;
  graphLoading: boolean;
  opActive: boolean;
  canPullPush: boolean;
  hasHeadBranch: boolean;
  // Repo-scoped handlers (each reuses the SAME handler as its toolbar/menu entry).
  onFetch(): void;
  onPull(): void;
  onPush(): void;
  onRefresh(): void;
  onNewBranch(): void;
  onNewWorktree(): void;
  onOpenSearch(): void;
  /** P57c: open the "Ask history" overlay (semantic search + AI answer). */
  onOpenHistory(): void;
  // Navigation source data + the shared reveal path.
  branches: BranchesSnapshot | null;
  graph: GraphLayout | null;
  revealCommitByOid(oid: string): void;
  // App-level commands threaded down from App (toggle theme/lists, Settings, …).
  appCommands: PaletteAction[];
}

function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

/** Tag name → tip oid, harvested from the loaded graph (tags carry no oid on the
 *  wire). A tag absent from the (possibly truncated) graph resolves to undefined;
 *  its jump then reveals nothing and the shared reveal path toasts "not in view". */
function tagOidMap(graph: GraphLayout | null): Map<string, string> {
  const map = new Map<string, string>();
  if (graph === null) return map;
  for (const n of graph.nodes) {
    if (n.refs === undefined) continue;
    for (const r of n.refs) {
      if (r.kind === 'tag' && !map.has(r.name)) map.set(r.name, n.id);
    }
  }
  return map;
}

/** Assemble the full registry: repo actions + App actions, then branch / tag jump
 *  rows built from the loaded snapshots. Commit jump is NOT enumerated here (it is
 *  the dynamic hex-prefix row in CommandPalette). No destructive command is ever
 *  produced (§6.1 safety) — only Fetch/Pull/Push/Refresh + dialog-openers + jumps. */
export function buildPaletteActions(deps: BuildPaletteDeps): PaletteAction[] {
  const {
    mutating,
    refreshing,
    statusLoading,
    graphLoading,
    opActive,
    canPullPush,
    hasHeadBranch,
    onFetch,
    onPull,
    onPush,
    onRefresh,
    onNewBranch,
    onNewWorktree,
    onOpenSearch,
    onOpenHistory,
    branches,
    graph,
    revealCommitByOid,
    appCommands,
  } = deps;

  const busy = mutating || refreshing;
  const out: PaletteAction[] = [];

  // Repo-scoped actions (non-destructive: remote sync, refresh, dialog-openers).
  out.push({
    id: 'repo.fetch',
    title: 'Fetch',
    hint: 'Ctrl+Shift+F',
    group: 'action',
    keywords: 'remote sync download',
    disabled: busy,
    run: onFetch,
  });
  out.push({
    id: 'repo.pull',
    title: 'Pull (fast-forward)',
    hint: 'Ctrl+Shift+P',
    group: 'action',
    keywords: 'remote sync merge',
    disabled: busy || !canPullPush,
    run: onPull,
  });
  out.push({
    id: 'repo.push',
    title: 'Push',
    hint: 'Ctrl+Shift+U',
    group: 'action',
    keywords: 'remote sync upload',
    disabled: busy || !canPullPush,
    run: onPush,
  });
  out.push({
    id: 'repo.refresh',
    title: 'Refresh',
    hint: 'Ctrl+R',
    group: 'action',
    keywords: 'reload rescan',
    disabled: refreshing || statusLoading || graphLoading || mutating,
    run: onRefresh,
  });
  out.push({
    id: 'repo.newBranch',
    title: 'New branch…',
    group: 'action',
    keywords: 'create',
    disabled: busy || opActive || !hasHeadBranch,
    run: onNewBranch,
  });
  out.push({
    id: 'repo.newWorktree',
    title: 'New worktree…',
    group: 'action',
    keywords: 'create linked',
    disabled: busy || opActive,
    run: onNewWorktree,
  });
  out.push({
    id: 'repo.search',
    title: 'Search commits…',
    hint: 'Ctrl+F',
    group: 'action',
    keywords: 'find grep message author',
    run: onOpenSearch,
  });
  out.push({
    id: 'repo.askHistory',
    title: 'Ask history…',
    hint: '✨',
    group: 'action',
    keywords: 'ai semantic history question ask why when relevant commits search',
    run: onOpenHistory,
  });

  // App-level actions (toggle theme/lists, open Settings / AI Assets / Health,
  // open repo / clone / new) — threaded down as pre-built PaletteActions.
  for (const c of appCommands) out.push(c);

  // Branch jumps (local + remote): reveal the tip in the graph. NON-mutating —
  // this never checks out, it only selects+scrolls the row (revealCommitByOid).
  for (const b of branches?.local ?? []) {
    out.push({
      id: `branch.local.${b.name}`,
      title: b.name,
      hint: shortOid(b.tip),
      group: 'branch',
      keywords: 'branch local jump reveal',
      run: () => revealCommitByOid(b.tip),
    });
  }
  for (const r of branches?.remote ?? []) {
    out.push({
      id: `branch.remote.${r.name}`,
      title: r.name,
      hint: shortOid(r.tip),
      group: 'branch',
      keywords: 'branch remote jump reveal',
      run: () => revealCommitByOid(r.tip),
    });
  }

  // Tag jumps: resolve the oid from the graph; reveal it (or toast "not in view").
  const tagMap = tagOidMap(graph);
  for (const t of branches?.tags ?? []) {
    const oid = tagMap.get(t);
    out.push({
      id: `tag.${t}`,
      title: t,
      hint: oid !== undefined ? shortOid(oid) : undefined,
      group: 'tag',
      keywords: 'tag jump reveal',
      run: () => revealCommitByOid(oid ?? ''),
    });
  }

  return out;
}

/** Case-insensitive subsequence score with a contiguous-run bonus and a
 *  word-boundary bonus. Returns -1 when `text` does not contain `query` as a
 *  subsequence; 0 for an empty query (matches everything, neutral). Pure. */
export function fuzzyScore(query: string, text: string): number {
  const q = query.trim().toLowerCase();
  if (q === '') return 0;
  const t = text.toLowerCase();
  let qi = 0;
  let score = 0;
  let run = 0;
  for (let ti = 0; ti < t.length && qi < q.length; ti += 1) {
    if (t[ti] === q[qi]) {
      run += 1;
      score += 1 + run; // reward contiguous runs
      if (ti === 0) {
        score += 3; // start-of-string
      } else {
        const prev = t[ti - 1];
        if (prev === ' ' || prev === '/' || prev === '-' || prev === '_' || prev === '.') {
          score += 2; // start-of-word
        }
      }
      qi += 1;
    } else {
      run = 0;
    }
  }
  return qi === q.length ? score : -1;
}

/** Filter + rank actions against the query (fuzzy over `title` + `keywords`).
 *  Empty query → identity (original order preserved). Ties keep source order so
 *  the default listing is stable. Grouping for display happens in CommandPalette. */
export function filterActions(actions: PaletteAction[], query: string): PaletteAction[] {
  const q = query.trim();
  if (q === '') return actions;
  const scored: { a: PaletteAction; s: number; i: number }[] = [];
  actions.forEach((a, i) => {
    const hay = a.keywords !== undefined ? `${a.title} ${a.keywords}` : a.title;
    const s = fuzzyScore(q, hay);
    if (s >= 0) scored.push({ a, s, i });
  });
  scored.sort((x, y) => y.s - x.s || x.i - y.i);
  return scored.map((e) => e.a);
}
