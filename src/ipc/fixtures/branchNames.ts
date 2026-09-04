/** Canned AI branch-name candidates for the mock `aiSuggestBranchName`
 *  (P53c §5.3), keyed on the grounding source kind. Split out of
 *  `mock/handlers/ai.ts` so the pathological-length cases can be documented
 *  without growing the handler file.
 *
 *  These are the harness states for P111 §8, minus the trailing-`/` case: a
 *  suggested name must be a VALID branch name (invariant asserted by
 *  `mock/handlers/ai.test.tsx`), and `refs/heads/x/` is not one. That degenerate
 *  branch of `RefLabel` is covered by `RefLabel.test.tsx` instead. */
export const BRANCH_NAMES_FROM_WORKING: string[] = [
  'feat/ai-why-layer',
  'ai-why-layer',
  'feature/blame-why',
  // P102/P105 §8 pathological case: a 90-char branch name, so the harness can
  // prove `.branch-name-chip` (--text-1 on its accent tint) stays on ONE line,
  // ellipsizing its LEADING segments while the identifying leaf survives
  // (P111 R1/R3).
  'feature/observability/rewrite-the-per-commit-blame-why-layer-behind-a-cached-lane-index-v2',
  // P111 §3.3 degenerate case: no `/` at all — there is no leading segment to
  // sacrifice, so the leaf itself must ellipsize. It HARD-CLIPPED until the leaf
  // was given `flex: 0 1 auto` + `text-overflow` (measured 174px of overflow past
  // the chip). A hard clip is worse than an ellipsis here: it makes a truncated
  // ref look like a complete one, and `title` is hover-only, so it reaches
  // neither a keyboard nor a touch user. Guarded by `e2e/33-pill-truncation.spec.ts`.
  'rewrite-the-cached-lane-index-behind-a-flag-without-a-slash-anywhere',
];

export const BRANCH_NAMES_FROM_RANGE: string[] = [
  'feat/range-work',
  'range-work',
  'topic/selected-commits',
];
