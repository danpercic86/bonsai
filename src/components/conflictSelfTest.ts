import {
  parseConflictRegions,
  applyResolution,
  hasUnresolvedMarkers,
} from '../utils/conflictRegions';
import type { P7SelfTestResult } from '../graph/frameStats';

// ---- self-test (P12 §2.2) ----------------------------------------------

// Inline copy of the mock `MERGE_AUTH_TEXT` fixture (§2.2 permits an inline
// copy rather than importing mock internals). Keep in sync with src/ipc/mock.ts.
const SELFTEST_TEXT = [
  'import { hash } from "./crypto";',
  '',
  'export function login(user: string, password: string): Session {',
  '<<<<<<< HEAD',
  '  const token = hash(`${user}:${password}:v2`);',
  '  return { user, token };',
  '=======',
  '  const token = hash(password + user);',
  '  return { user: user.toLowerCase(), token };',
  '>>>>>>> feature/login',
  '}',
  '',
].join('\n');

/** Run the pure conflict-region helper assertions; logs one line, mirroring
 *  `p7SelfTest`. Mock/dev only (registered by the mount effect). */
export function conflictSelfTest(): P7SelfTestResult {
  let pass = 0;
  const failures: string[] = [];
  const check = (name: string, cond: boolean): void => {
    if (cond) pass++;
    else failures.push(name);
  };

  const regions = parseConflictRegions(SELFTEST_TEXT);
  check('parse finds 1 region', regions.length === 1);
  const r = regions[0];
  if (r !== undefined) {
    check('region index 0', r.index === 0);
    check('region startLine', r.startLine === 3);
    check('region sepLine', r.sepLine === 6);
    check('region endLine', r.endLine === 9);
    check('region oursLabel HEAD', r.oursLabel === 'HEAD');
    check('region theirsLabel feature/login', r.theirsLabel === 'feature/login');
    check(
      'region oursLines',
      r.oursLines.length === 2 &&
        r.oursLines[0] === '  const token = hash(`${user}:${password}:v2`);' &&
        r.oursLines[1] === '  return { user, token };',
    );
    check(
      'region theirsLines',
      r.theirsLines.length === 2 &&
        r.theirsLines[0] === '  const token = hash(password + user);' &&
        r.theirsLines[1] === '  return { user: user.toLowerCase(), token };',
    );
  } else {
    failures.push('region 0 undefined');
  }

  check('parse "no markers" -> []', parseConflictRegions('no markers').length === 0);
  check('hasUnresolvedMarkers true on fixture', hasUnresolvedMarkers(SELFTEST_TEXT) === true);

  if (r !== undefined) {
    // §3.4: all three accept choices on the single-region fixture.
    const OURS_BODY = ['  const token = hash(`${user}:${password}:v2`);', '  return { user, token };'];
    const THEIRS_BODY = [
      '  const token = hash(password + user);',
      '  return { user: user.toLowerCase(), token };',
    ];
    const oursText = applyResolution(SELFTEST_TEXT, r, 'ours');
    check('applyResolution ours has no markers', hasUnresolvedMarkers(oursText) === false);
    check(
      'applyResolution ours keeps ours body',
      oursText.includes(OURS_BODY.join('\n')) && !oursText.includes('hash(password + user)'),
    );

    const theirsText = applyResolution(SELFTEST_TEXT, r, 'theirs');
    check('applyResolution theirs has no markers', hasUnresolvedMarkers(theirsText) === false);
    check(
      'applyResolution theirs keeps theirs body',
      theirsText.includes(THEIRS_BODY.join('\n')) &&
        !theirsText.includes('hash(`${user}:${password}:v2`)'),
    );

    const bothText = applyResolution(SELFTEST_TEXT, r, 'both');
    check('applyResolution both has no markers', hasUnresolvedMarkers(bothText) === false);
    check(
      'applyResolution both is ours-then-theirs',
      bothText.includes([...OURS_BODY, ...THEIRS_BODY].join('\n')),
    );
  }

  // §3.4: two-region synthetic fixture — resolving region 0 leaves exactly one
  // remaining region, correctly re-indexed (the property P12c's buttons rely on).
  const TWO_REGION_TEXT = [
    'top',
    '<<<<<<< HEAD',
    'a-ours',
    '=======',
    'a-theirs',
    '>>>>>>> branch-a',
    'middle',
    '<<<<<<< HEAD',
    'b-ours',
    '=======',
    'b-theirs',
    '>>>>>>> branch-b',
    'bottom',
  ].join('\n');
  const twoRegions = parseConflictRegions(TWO_REGION_TEXT);
  check('two-region fixture parses 2 regions', twoRegions.length === 2);
  const first = twoRegions[0];
  if (first !== undefined) {
    const afterFirst = applyResolution(TWO_REGION_TEXT, first, 'ours');
    const remaining = parseConflictRegions(afterFirst);
    check('after resolving region 0, exactly 1 region remains', remaining.length === 1);
    const only = remaining[0];
    if (only !== undefined) {
      // region 1 was at lines 7/9/11; region 0's block (lines 1..5, 5 lines)
      // collapsed to its 1-line ours body removed 4 lines, so it shifts up by 4.
      check('remaining region re-indexed to 0', only.index === 0);
      check('remaining region startLine', only.startLine === 3);
      check('remaining region sepLine', only.sepLine === 5);
      check('remaining region endLine', only.endLine === 7);
      check(
        'remaining region bodies intact',
        only.oursLines.length === 1 &&
          only.oursLines[0] === 'b-ours' &&
          only.theirsLines.length === 1 &&
          only.theirsLines[0] === 'b-theirs',
      );
    } else {
      failures.push('remaining region undefined');
    }
  } else {
    failures.push('two-region first undefined');
  }

  const result: P7SelfTestResult = { pass, fail: failures.length, failures };
  if (import.meta.env.DEV) console.log(`[bonsai] conflictSelfTest ${JSON.stringify(result)}`);
  return result;
}
