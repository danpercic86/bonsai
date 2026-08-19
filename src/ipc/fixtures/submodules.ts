// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import { fixtureOid, randomOid } from './oids';
import type { GraphFixture, RepoKind } from '../mock/repoState';
import type { SubmoduleInfo } from '../types';

/** P73 §8.2: pathological submodule path (deep, 91 chars) — the overflow fixture. */
const LONG_SUBMODULE_PATH =
  'src/Hamilton.Voyager.Protocol/protocol/vendor/third-party/generated-openapi-client-bindings';

/** Seed the DEFAULT repo's submodules so the sidebar section shows every badge
 *  state (P19 §5). Non-default repos get []. */
export function seedSubmodules(kind: RepoKind, graphFixture: GraphFixture): SubmoduleInfo[] {
  if (kind !== 'default' || graphFixture !== 'default') return [];
  return [
    {
      name: 'vendor/libcore',
      path: 'vendor/libcore',
      absPath: '/mock/repo/vendor/libcore',
      url: 'https://example.com/libcore.git',
      headOid: fixtureOid(1),
      indexOid: fixtureOid(1),
      wtOid: null,
      status: 'uninitialized',
    },
    {
      name: 'vendor/theme',
      path: 'vendor/theme',
      absPath: '/mock/repo/vendor/theme',
      url: 'https://example.com/theme.git',
      headOid: fixtureOid(2),
      indexOid: fixtureOid(2),
      wtOid: fixtureOid(2),
      status: 'upToDate',
    },
    {
      name: 'docs/spec',
      path: 'docs/spec',
      absPath: '/mock/repo/docs/spec',
      url: 'https://example.com/spec.git',
      headOid: fixtureOid(4),
      indexOid: fixtureOid(4),
      wtOid: randomOid(),
      status: 'outOfSync',
    },
    {
      name: 'tools/ci',
      path: 'tools/ci',
      absPath: '/mock/repo/tools/ci',
      url: 'https://example.com/ci.git',
      headOid: fixtureOid(5),
      indexOid: fixtureOid(5),
      wtOid: fixtureOid(5),
      status: 'modifiedWorkdir',
    },
    // P73 §8.2: the reported real-world case at worst-case length (91 chars) —
    // proves row ellipsis, badge placement and toast wrapping all hold.
    {
      name: LONG_SUBMODULE_PATH,
      path: LONG_SUBMODULE_PATH,
      absPath: `/mock/repo/${LONG_SUBMODULE_PATH}`,
      url: 'https://dev.azure.com/example/_git/Hamilton.Voyager.Protocol.Generated.Client.Bindings',
      headOid: fixtureOid(6),
      indexOid: fixtureOid(6),
      wtOid: null,
      status: 'uninitialized',
    },
  ];
}
