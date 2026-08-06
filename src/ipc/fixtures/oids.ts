// Split out of the former monolithic mock.ts (pure refactor; no behavior change).


/** Deterministic 40-hex oid for a default-fixture row — MUST match
 *  fixtures/graph.ts `oid(row)` so seeded stash baseOids line up with the graph
 *  pills (index 0 → row 3 `core work 4`; indices 1 & 2 → row 6 `core work 2`). */
export function fixtureOid(row: number): string {
  return row.toString(16).padStart(2, '0').repeat(20);
}
export function randomOid(): string {
  return Array.from({ length: 40 }, () => '0123456789abcdef'[Math.floor(Math.random() * 16)]).join(
    '',
  );
}
export function mockNodeOid(row: number): string {
  return row.toString(16).padStart(2, '0').repeat(20);
}
