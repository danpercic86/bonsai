/**
 * P68 #7 / H1 — the TS twin of the Rust `resolution_is_novel` predicate
 * (`crates/bonsai-core/src/git/ai_resolve.rs`), for the mock IPC layer only.
 *
 * Kept BYTE-FOR-BYTE with the Rust rule so the browser harness enforces the SAME
 * gate as the backend: split on `\n` (a trailing `\r` is removed by `trim`, so
 * CRLF↔LF is never a false positive), `trim` each line (leading/trailing whitespace
 * only — NO interior-whitespace collapse, lowercasing, or tokenizing, which would let
 * an injected line alias an allowed one), and SKIP blank lines on both sides.
 *
 * Both mock handlers (`aiStream` classification and `aiApplyResolution` enforcement)
 * call this one function so they cannot disagree.
 */
function normalizedLines(text: string): string[] {
  return text
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

/** True iff `proposed` has ≥1 non-blank line whose normalized form appears in NONE of
 *  `sides` (the base/ours/theirs bodies available to the mock). Per-file verdict. */
export function resolutionIsNovel(sides: readonly string[], proposed: string): boolean {
  const allowed = new Set<string>();
  for (const side of sides) {
    for (const line of normalizedLines(side)) allowed.add(line);
  }
  return normalizedLines(proposed).some((line) => !allowed.has(line));
}
