/** P7 §10 item 2: the mock-mode `window.__bonsai.p7SelfTest()` implementation
 *  (T3.6: moved verbatim out of GraphCanvas.tsx — dev/mock-only code, never on
 *  the paint path). Exercises the pure ref-band/avatar helpers against known
 *  answers; the orchestrator reads the returned pass/fail summary. */

import type { GraphNode, RefLabel } from '../ipc';
import { resolveTheme } from './colors';
import { avatarColor, avatarHit, initials, refColArea } from './geometry';
import { relativeDate } from './dates';
import type { P7SelfTestResult } from './frameStats';
import { METRICS } from './metrics';
import { groupRefs, layoutRefLabels } from './refLabels';
import type { GraphDisplayOptions } from './rightColumns';
import { HEAD_GUIDE_PAD, headGuide } from './viewport';

export function runP7SelfTest(canvas: HTMLCanvasElement | null): P7SelfTestResult {
  let pass = 0;
  const failures: string[] = [];
  const check = (name: string, cond: boolean): void => {
    if (cond) pass++;
    else failures.push(name);
  };

  // initials
  check('initials "Dan Percic"→"DP"', initials('Dan Percic') === 'DP');
  check('initials "torvalds"→"TO"', initials('torvalds') === 'TO');
  check('initials "x"→"X"', initials('x') === 'X');
  check('initials ""→"?"', initials('') === '?');
  check('initials "  a  b "→"AB"', initials('  a  b ') === 'AB');

  // avatarColor
  const c1 = avatarColor('Dan Percic');
  const c2 = avatarColor('Dan Percic');
  check('avatarColor deterministic', c1.bg === c2.bg);
  check('avatarColor bg format', /^hsl\(\d{1,3}, 52%, 42%\)$/.test(c1.bg));
  check('avatarColor text white', c1.text === '#ffffff');
  const distinct = new Set(
    ['Alice', 'Bob', 'Carol', 'Dan Percic', 'torvalds', 'Grace Hopper'].map(
      (n) => avatarColor(n).bg,
    ),
  );
  check('avatarColor varies across names', distinct.size >= 2);

  // groupRefs — same-commit collapse
  const sameCommit: RefLabel[] = [
    { name: 'main', kind: 'localBranch', isHead: true },
    { name: 'origin/main', kind: 'remoteBranch', isHead: false },
    { name: 'v1.0', kind: 'tag', isHead: false },
  ];
  const g = groupRefs(sameCommit);
  check('groupRefs same-commit length 2', g.length === 2);
  const b0 = g[0];
  check(
    'groupRefs same-commit branch main',
    b0 !== undefined &&
      b0.kind === 'branch' &&
      b0.name === 'main' &&
      b0.hasLocal === true &&
      b0.remotes.length === 1 &&
      b0.remotes[0] === 'origin/main' &&
      b0.isHead === true,
  );
  const t1 = g[1];
  check('groupRefs same-commit tag v1.0', t1 !== undefined && t1.kind === 'tag' && t1.name === 'v1.0');

  // groupRefs — diverged (each ref on its own node)
  const localFeat = groupRefs([{ name: 'feat', kind: 'localBranch', isHead: false }]);
  const lf = localFeat[0];
  check(
    'groupRefs diverged local feat',
    lf !== undefined &&
      lf.kind === 'branch' &&
      lf.name === 'feat' &&
      lf.hasLocal === true &&
      lf.remotes.length === 0,
  );
  const remoteFeat = groupRefs([{ name: 'origin/feat', kind: 'remoteBranch', isHead: false }]);
  const rf = remoteFeat[0];
  check(
    'groupRefs diverged remote feat',
    rf !== undefined &&
      rf.kind === 'branch' &&
      rf.name === 'feat' &&
      rf.hasLocal === false &&
      rf.remotes.length === 1 &&
      rf.remotes[0] === 'origin/feat',
  );

  // §14.1: a slashed branch name present as both local and remote on one
  // node collapses to ONE entity (strip only the remote name segment).
  const slashRefs: RefLabel[] = [
    { name: 'topic/x', kind: 'localBranch', isHead: false },
    { name: 'origin/topic/x', kind: 'remoteBranch', isHead: false },
  ];
  const slashEnts = groupRefs(slashRefs);
  check(
    'groupRefs slashed local+remote collapse',
    slashEnts.length === 1 &&
      slashEnts[0].kind === 'branch' &&
      slashEnts[0].name === 'topic/x' &&
      slashEnts[0].hasLocal === true &&
      slashEnts[0].remotes.length === 1,
  );

  // P9 §6.1: a stash is its OWN entity — never collapsed into a branch on
  // the same commit — and sorts LAST (after the branch).
  const stashEnts = groupRefs([
    { name: 'main', kind: 'localBranch', isHead: true },
    { name: 'stash@{0}', kind: 'stash', isHead: false },
  ]);
  check(
    'groupRefs stash not collapsed, sorts last',
    stashEnts.length === 2 &&
      stashEnts[0].kind === 'branch' &&
      stashEnts[1].kind === 'stash' &&
      stashEnts[1].name === 'stash@{0}',
  );

  // refColArea
  const area = refColArea(METRICS);
  check('refColArea startX', area.startX === METRICS.refColPadLeft);
  check(
    'refColArea budget',
    area.budget === METRICS.refColWidth - METRICS.refColPadLeft - METRICS.refColPadRight,
  );

  // avatarHit
  check('avatarHit center', avatarHit(10, 10, 10, 10, METRICS));
  check('avatarHit outside', !avatarHit(100, 100, 10, 10, METRICS));

  // relativeDate regression guard
  const now = 1_000_000_000;
  check('relativeDate now', relativeDate(now, now) === 'now');
  check('relativeDate 2m', relativeDate(now - 120, now) === '2m');
  check('relativeDate 2h', relativeDate(now - 7200, now) === '2h');
  check('relativeDate 2d', relativeDate(now - 172800, now) === '2d');

  // P67 §1: head guideline. Known answers over the pure geometry — the headless
  // harness pane produces no canvas pixel, so this is the only assertable seam.
  const RH = METRICS.rowHeight;
  const HALO = METRICS.avatarRadius + METRICS.avatarBgRingExtra;
  const guideArgs = {
    headIndex: 10 as number | null,
    layoutScrollTop: -RH, // raw scrollTop 0 with a WIP row
    wipOffset: 1,
    rowHeight: RH,
    avatarRadius: METRICS.avatarRadius,
    ringExtra: METRICS.avatarBgRingExtra,
    viewportHeight: 640,
  };
  check('headGuide null head → null', headGuide({ ...guideArgs, headIndex: null }) === null);
  const gTop = headGuide(guideArgs);
  check('headGuide at top anchors on the WIP dot', gTop !== null && gTop.y0 === RH / 2);
  check(
    'headGuide stops at the HEAD halo',
    gTop !== null && gTop.y1 === 10 * RH + RH / 2 + RH - HALO,
  );
  check(
    'headGuide at top: dashOffset 0, no edge, real segment',
    gTop !== null && gTop.dashOffset === 0 && gTop.edge === null && gTop.segment === true,
  );
  check('headGuide echoes headIndex', gTop !== null && gTop.headIndex === 10);
  // The user's bug: far past the WIP row the segment must STILL exist, bounded.
  const gDeep = headGuide({ ...guideArgs, headIndex: 2000, layoutScrollTop: 50_000 });
  check('headGuide survives deep scroll', gDeep !== null);
  check(
    'headGuide clamps BOTH ends at deep scroll',
    gDeep !== null &&
      gDeep.y0 >= -HEAD_GUIDE_PAD &&
      gDeep.y0 <= 640 + HEAD_GUIDE_PAD &&
      gDeep.y1 >= -HEAD_GUIDE_PAD &&
      gDeep.y1 <= 640 + HEAD_GUIDE_PAD,
  );
  // §6 acceptance (3) known answer at rowHeight 32 / viewportHeight 640, recomputed
  // by hand under A6.1 (§1.1b — the dashOffset sign was inverted in the first cut):
  //   anchor = 16 - (50_000 + 32) = -50_016, headCenter = 2000*32 + 16 - 50_000 = 14_016
  //   y0 = clamp(-50_016) = -8, y1 = clamp(14_016 - 12) = 648
  //   dashOffset = (y0 - anchor) mod 6 = 50_008 mod 6 = 4   (the inverted form gave 2)
  // ⇒ { y0: -8, y1: 648, dashOffset: 4, edge: 'bottom', segment: true }.
  check(
    'headGuide points down when HEAD is below',
    gDeep !== null && gDeep.edge === 'bottom' && gDeep.segment === true,
  );
  check(
    'headGuide deep-scroll known answer (y0 -8, y1 648, dashOffset 4)',
    gDeep !== null && gDeep.y0 === -8 && gDeep.y1 === 648 && gDeep.dashOffset === 4,
  );
  // HEAD just above the top edge (clean tree) → the marker points up.
  const gAbove = headGuide({ ...guideArgs, wipOffset: 0, headIndex: 0, layoutScrollTop: RH / 2 + 10 });
  check('headGuide points up when HEAD is above', gAbove !== null && gAbove.edge === 'top');
  // A5 (§1.1a): with a WIP row, scrolling PAST HEAD collapses the segment (both
  // ends clamp to -PAD) — the up-marker must survive, or the guide would vanish.
  const gPast = headGuide({ ...guideArgs, headIndex: 0, layoutScrollTop: 50_000 });
  check(
    'headGuide A5: past HEAD with a WIP row → marker-only',
    gPast !== null && gPast.segment === false && gPast.edge === 'top',
  );
  // …and returns null only when the collapse happens with HEAD on screen.
  check(
    'headGuide A5: collapsed with HEAD on screen → null',
    // rowHeight === halo ⇒ the WIP dot sits exactly one halo above HEAD's centre.
    headGuide({ ...guideArgs, headIndex: 0, rowHeight: HALO, layoutScrollTop: -HALO }) === null,
  );
  // Crawl guard: one dash period of scroll must reproduce the same phase.
  const gPhase = headGuide({ ...guideArgs, headIndex: 2000, layoutScrollTop: 50_006 });
  check(
    'headGuide dash phase is content-stable',
    gDeep !== null && gPhase !== null && gDeep.dashOffset === gPhase.dashOffset,
  );
  // A6.2: periodicity alone passes under EITHER sign — pin the phase. The stroke
  // starts at y0 and `lineDashOffset` shifts the pattern, so the on-screen dash
  // grid sits at y ≡ y0 - dashOffset; it must coincide with the anchor's grid.
  const mod6 = (v: number): number => ((v % 6) + 6) % 6;
  const anchorAt = (layoutScrollTop: number): number => RH / 2 - (layoutScrollTop + RH);
  check(
    'headGuide dash grid is pinned to the content (phase, not just period)',
    gDeep !== null &&
      gPhase !== null &&
      mod6(gDeep.y0 - gDeep.dashOffset) === mod6(anchorAt(50_000)) &&
      mod6(gPhase.y0 - gPhase.dashOffset) === mod6(anchorAt(50_006)),
  );
  // A6.3: HEAD's centre exactly ON the -PAD anchor (clean tree, layoutScrollTop
  // RH/2 + PAD) ⇒ dir === 0. The removed early return suppressed this marker.
  const gDirZero = headGuide({
    ...guideArgs,
    wipOffset: 0,
    headIndex: 0,
    layoutScrollTop: RH / 2 + HEAD_GUIDE_PAD,
  });
  check(
    'headGuide A6.3: dir === 0 still yields the top marker',
    gDirZero !== null && gDirZero.edge === 'top' && gDirZero.segment === false,
  );
  // Clean tree (no WIP row): the guide still runs from just above the top edge.
  const gClean = headGuide({ ...guideArgs, wipOffset: 0, layoutScrollTop: 0 });
  check('headGuide clean tree anchors at -PAD', gClean !== null && gClean.y0 === -HEAD_GUIDE_PAD);

  // layoutRefLabels overflow — needs a ctx + theme.
  const ctx = canvas?.getContext('2d') ?? null;
  const theme = canvas !== null ? resolveTheme(canvas) : null;
  if (ctx !== null && theme !== null) {
    const manyRefs: RefLabel[] = [
      { name: 'main', kind: 'localBranch', isHead: true },
      { name: 'develop', kind: 'localBranch', isHead: false },
      { name: 'feature-long-name', kind: 'localBranch', isHead: false },
      { name: 'release', kind: 'localBranch', isHead: false },
      { name: 'hotfix', kind: 'localBranch', isHead: false },
    ];
    const node: GraphNode = {
      id: '0'.repeat(40),
      lane: 0,
      parents: [],
      refs: manyRefs,
      summary: '',
      author: '',
      ts: 0,
      committerTs: 0,
    };
    const entities = groupRefs(manyRefs);
    const { startX } = refColArea(METRICS);
    // Budget wide enough to fit `main` + gap + a `+n` chip, yet still
    // narrow enough to force overflow of the full 5-branch set.
    const testBudget = 120;
    // Chip-disabled display: this overflow self-test predates P51c and must
    // stay independent of ahead/behind reservation.
    const noChipsDisplay: GraphDisplayOptions = {
      showSha: true,
      showAuthor: false,
      showDate: true,
      dateBasis: 'author',
      showAheadBehind: false,
      branchStats: new Map(),
      showSignatureBadge: false,
      showPrBadge: false,
      showCiStatus: false,
      prByBranch: new Map(),
      ciBySha: new Map(),
    };
    const laid = layoutRefLabels(ctx, entities, node, theme, startX, testBudget, noChipsDisplay);
    check('layoutRefLabels first entity laid', laid.length >= 1 && laid[0].entity !== null);
    const last = laid[laid.length - 1];
    check('layoutRefLabels trailing overflow chip', last !== undefined && last.entity === null);
    const shownCount = laid.filter((l) => l.entity !== null).length;
    const hiddenCount = entities.length - shownCount;
    check('layoutRefLabels overflow count', hiddenCount > 0);
    check(
      'layoutRefLabels chip label',
      last !== undefined && last.entity === null && last.style.label === `+${hiddenCount}`,
    );
    // P7e §13.1: the last laid entry (chip included) must fit the band.
    check(
      'layoutRefLabels last fits band',
      last !== undefined && last.x + last.w <= startX + testBudget,
    );
  } else {
    failures.push('layoutRefLabels: no canvas ctx/theme available');
  }

  const result: P7SelfTestResult = { pass, fail: failures.length, failures };
  if (import.meta.env.DEV) console.log(`[bonsai] p7SelfTest ${JSON.stringify(result)}`);
  return result;
}
