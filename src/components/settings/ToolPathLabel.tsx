/** P112 §3 / §16.9 — the leaf-preserving label for a PLATFORM path.
 *
 *  `RefLabel` is the §3.0-R3 member of this family, but it delegates to
 *  `splitPath`, which splits on `/` ONLY: a Windows path comes back as one leaf
 *  with `dir: null`, and since `.ref-label-leaf` shrinks last the path overflows
 *  the popover instead of truncating. **Widening `splitPath` is explicitly not
 *  the route** — it serves git paths, which are always `/`, and loosening a
 *  shared parser to fix a display case in another subsystem is how a regression
 *  gets planted in the status tree.
 *
 *  Two properties this must keep:
 *    * **The truncation is CSS-only.** `value` is never sliced short, never
 *      shortened with `…` in JS, never measured — the whole string is in the DOM,
 *      which is what keeps the accessible name correct (P111 §5).
 *    * **No transform of any kind** (§16.9). `detail` is what the resolver
 *      produced: `PATHEXT` casing is kept (`code.CMD`), no extension is parsed,
 *      and the string is not assumed to be a well-formed path at all — a
 *      backend-sanitized `detail` may already end mid-path in a `…`. Splitting on
 *      the last separator is the maximum this renderer may know about it.
 */
export function ToolPathLabel({ value }: { value: string }) {
  // -1 when the string has no separator (a bare program name, or a `detail`
  // truncated before its first one): head is then empty and the whole string is
  // the leaf. The `+ 1` keeps the separator ON the head, so a leading `/` is
  // never dropped from an absolute unix path.
  const cut = Math.max(value.lastIndexOf('/'), value.lastIndexOf('\\'));
  const head = cut < 0 ? '' : value.slice(0, cut + 1);
  const leaf = value.slice(cut + 1);
  return (
    <span className="ref-label" title={value}>
      {head !== '' && <span className="ref-label-head">{head}</span>}
      <span className="ref-label-leaf">{leaf}</span>
    </span>
  );
}
