# P110 — Watcher burst classification → narrow refresh scope

Follow-up to `84bbf85` (which fixed the visible selection flicker); this removes the wasted work
behind it. A checkout that rewrites thousands of files storms `notify`. The 300 ms debounce
coalesces a *continuous* storm into one fire, but a long checkout has natural gaps > 300 ms, so it
fires several times. Every fire used to emit a reason-less `"fs"` event → `refresh('watcher',
'full')` → a full graph re-stream **per burst**. Rust already knew each path's class and threw it
away; P110 carries it through to the event.

## 1. Classifier (`src-tauri/src/watcher/classify.rs`)

`classify(path, git_dir) -> Option<PathClass>` is **one** function: it is both the relevance filter
and the graph-affecting classification. Two separate predicates would eventually drift and the
frontend would then run a narrow refresh for a burst that did move HEAD.

```rust
pub enum PathClass { Worktree, Refs }
```

Decision table (evaluated in this order):

| Path | Result |
| --- | --- |
| anything **not** under `.git/` | `Some(Worktree)` |
| `*.lock` under `.git/` (`index.lock`, `refs/heads/main.lock`) | `None` — churn |
| `.git/refs/bonsai-tagsync/**` | `None` — P85 A3 tag-sync scratch namespace (must precede `refs`) |
| `.git/HEAD` | `Some(Refs)` |
| `.git/index` | `Some(Refs)` — see §1.1 |
| `.git/refs/**` (incl. `refs/tags/*`, `refs/remotes/*`) | `Some(Refs)` |
| `.git/packed-refs` | `Some(Refs)` |
| any other path under `.git/` (`COMMIT_EDITMSG`, `MERGE_HEAD`, `logs/HEAD`, `FETCH_HEAD`, `objects/**`, `modules/**`) | `None` |

The **relevance set is unchanged** from pre-P110: no burst that fired before stops firing, and none
that was silent starts firing. P110 only adds a label to the ones that already fired.

### 1.1 Load-bearing rationale — why `.git/index` is `Refs`

Semantically the index is status, not graph. It is classified `Refs` because of **event loss**:
before P110 correctness required observing *any* relevant event in the burst; now it requires
observing the **ref-file** event specifically. `ReadDirectoryChangesW` drops events on Windows
(see the `watcher` module header). Pre-P110 a dropped `refs/heads/main` event was harmless — the
same commit's `index` write still forced a full refresh. Classified `Worktree`, that same drop
would produce a Worktree-only burst and the graph would silently never refresh. Cost of the
belt-and-braces: a long checkout writes the index **once**, so this is one graph re-stream per
checkout instead of one per burst (thousands of file events still coalesce into pure `Worktree`
bursts).

## 2. Burst accumulation (`src-tauri/src/watcher/mod.rs`)

Each notify batch reduces to `WatchTick { paths, relevant, refs }` where `refs` is true if **any**
classified path in the batch was `Refs`. The debounce window folds ticks through `BurstAcc::absorb`:

- counts: `saturating_add`;
- class: `refs |= t.refs` — **conservative**, one `Refs` path anywhere in the coalesced burst makes
  the whole burst `Refs`. Order-independent.
- a notify `Err` batch counts as relevant **and** as `Refs` (we don't know what was dropped).

`on_change(BurstClass)` fires once per quiet period. `WatcherHandle` drop-and-join semantics and
`DEBOUNCE = 300 ms` are unchanged.

Observability (P91 §2.4): the `fired` `watcher` record carries `burstClass: 'worktree' | 'refs'`.
The field is `skip_serializing_if = "Option::is_none"` and is set on `fired` records only, so the
pre-P110 record shape is byte-identical everywhere else.

## 3. IPC surface

`RepoChangedPayload { repoId, reason }` — `reason` values:

| reason | Emitted when | Frontend route |
| --- | --- | --- |
| `"fs"` | burst class `Refs` (or a notify error) | `refresh('watcher', 'full')` |
| `"fsWorktree"` | burst class `Worktree` | `refresh('watcher', 'worktree')` |
| `"fetch"` | backend-confirmed remote update | `refresh('external', 'remoteMeta')` |
| `"tags"` | P85 A3 tag auto-sync adopted/moved | `refresh('external', 'refsOnly')` |
| anything else / absent | — | `refresh('watcher', 'full')` (always safe) |

`"fs"` keeps its wire value, so an older listener is unaffected. Both filesystem reasons use the
`watcher` **origin** — a burst may well be our own mutation's echo, so it must stay
echo-suppressible; `external` would bypass suppression and is wrong here.

## 4. Scope `worktree` = `{ status, opState, submodules }`

- **P99 invariant** (`refreshScope.ts`): every `openRepo: false` scope must never move HEAD.
  Satisfied **by construction** — a burst carrying any HEAD/refs path is classified `Refs` in Rust
  and arrives as `"fs"`, never as `"fsWorktree"`. This is the load-bearing reason the narrow scope
  is safe.
- **`submodules` slice**: `git -C sub checkout other` writes the submodule's refs under
  `<super>/.git/modules/sub/refs/**` (classified `None` — pre-existing) and its worktree files
  under `<workdir>/sub/**` (`Worktree`), so the burst is Worktree-only. Without the slice the
  submodule panel would go stale until a manual refresh — a regression against the pre-P110 `full`.
  Adding a **slice** restores prior refresh behaviour without changing **which** bursts fire;
  reclassifying `.git/modules/**` would widen the relevance set and is deliberately not done.
- Narrowed self-healing (accepted): pre-P110 an echo-suppressed refs burst was incidentally healed
  by the next `fs` burst, also `full`; that next burst may now be `worktree`. Acceptable because
  suppression only happens inside a mutation's own armed window, and that mutation's own refresh
  round already refetched the graph.

## 5. Acceptance criteria

1. Relevance set unchanged: `is_relevant_rules` passes untouched.
2. `classify` covers every decision-table row (`classify_decision_table`).
3. Accumulation is conservative and order-independent, proven without wall-clock timing
   (`burst_accumulation_is_conservative`).
4. A real fs burst of worktree writes classifies `Worktree`; a burst containing `.git/HEAD`
   classifies `Refs` (`worktree_only_burst_classifies_as_worktree`, `mixed_burst_classifies_as_refs`).
5. `burstClass` present on `fired` records, absent otherwise (`watcher_record_shape`).
6. `"fsWorktree"` routes to `('watcher', 'worktree')`; `"fs"`, unknown, and absent reasons route to
   `('watcher', 'full')`; other repos ignored; listener removed on unmount
   (`useRepoChangeSubscription.test.tsx`).
7. Mock IPC exposes `window.__bonsaiEmitWatcher(reason?, repoId?)` so the browser harness can drive
   both filesystem reasons.
8. `DEBOUNCE` (300 ms) and `ECHO_TAIL_MS` (600 ms) unchanged.
