# Audit: commit 2a0b8f1 (MCP tool descriptions) - FINAL findings

## Q1 capability/params/auth: CLEAN (airtight)
`git show 2a0b8f1 --unified=0 -- tools_read.rs tools_write.rs | grep -vE '^[+-]\s*///'` -> EMPTY.
Zero non-doc-comment lines. No new tool, no signature/Parameters change, no router change.
Gate structural + untouched: crates/bonsai-mcp/src/server.rs:156.
BUT rmcp-macros-3.1.4/src/common.rs:44-51 concatenates ALL doc lines -> 222 lines DO ship as
model-facing JSON-Schema descriptions on 34 tools. Not inert.

## HIGH 1 (PRE-EXISTING, not introduced) - bonsai_stage accepts arbitrary paths the UI never offers
Root cause: stage.rs:119-143 `stage_paths` calls only the LEXICAL validate_rel_path, and does NOT call
`ensure_within_workdir` (stage.rs:76) - though conflict.rs:151/271/348, discard.rs:109 and
stage_partial.rs:104 all do. Partial-staging guarded, full-file staging not => oversight.
(1a) Ancestor-symlink escape -> arbitrary out-of-repo file read.
  Chain: hostile repo commits `cfg -> /home/u/.ssh` (symlinks are git content; materialize on
  Unix/macOS, core.symlinks=false on Windows default) -> injected instruction in repo content reaching
  the model -> bonsai_stage(["cfg/id_rsa"]) -> validate_rel_path passes (no .., not abs, no backslash)
  -> stage.rs:136 wd.join(rel).symlink_metadata() follows the symlinked ANCESTOR -> index.add_path ->
  libgit2 index.c:1004-1015 lstat leaf + blob.c:210-233 reads the real out-of-repo file into the ODB.
  libgit2 has NO "beyond a symbolic link" guard (string absent from all of libgit2/src; git CLI has it);
  git_repository_workdir_path (repository.c:3237-3251) validates only string LENGTH.
  Read-back: bonsai_get_workdir_file_diff{staged:true} -> diff/api.rs:110-112 diff_tree_to_index ->
  the file's bytes return to the model as typed hunks. => secret into AI transcript, no push needed.
(1b) MEDIUM: index.add_path has `git add -f` semantics -> gitignored files (.env) stageable/committable.
  stage.rs:117 comment justifies it with "the UI only offers paths already present in StatusSnapshot" -
  an invariant the MCP caller breaks. tools_write.rs:22 turns it into ADVICE, not enforcement.
Also reachable from the webview: src-tauri/src/commands/staging.rs:20 passes frontend paths straight
through (renderer-compromise angle).
Mitigations: MCP write default-OFF + explicit consent + bearer token; Windows default safe for 1a.
Fix: call ensure_within_workdir in stage_paths (guard already exists, same file), and/or enforce
membership in read_status() output, which is what the description already claims.

## LOW 2 (INTRODUCED BY THIS COMMIT) - false claim "a path that does not exist fails the batch"
tools_write.rs:22-23 vs stage.rs:134-141: missing worktree path -> index.remove_path -> stages a
DELETION (tracked path) or silent Ok (untracked; libgit2 index.c:1641-1650 swallows GIT_ENOTFOUND).
Index-only + recoverable, but it is the one factual error the commit added, in the commit whose
message asserts "Every claim was checked against the actual signature and outcome enum".

## LOW 3 (PRE-EXISTING) - MCP commit-producing tools run repo hooks; disclosure unreachable
tools_write.rs:66 skip_hooks=false; commit.rs:92-98 bonsai.runHooks default true. Same for
bonsai_commit_merge: merge/finalize.rs:56 pre-commit (BLOCKING) + :134 post-commit.
src-tauri/src/commands/hooks.rs:1-10 states "The gate itself lives in the frontend" - so standalone
`bonsai-mcp --repo X --allow-write` (no frontend at all) never discloses. Deliberate + narrow vector
(hooks are not cloned). CLAUDE.md requires hook execution be "user-consented and clearly disclosed".

## LOW 4 - read-tool descriptions carry no untrusted-data labelling
grep untrusted|instruction|do not follow in tools_read.rs -> 0 hits. Content-returning tools:
get_conflict (ours/theirs/marker blob text), 3 diff families, list_branches, get_status.
Mitigation: JSON structured_content (helpers.rs:21-33) => structural delimitation, no framing escape;
residual risk is instruction-following. This commit rewrote all 12 read descriptions and omitted it.

## LOW 5 - "Trust the caller" now has a model as the caller
conflict.rs:332 + :309-311 rely on the frontend hasUnresolvedMarkers Save gate, absent over MCP ->
a model can stage/commit a file containing <<<<<<< markers. New text (tools_write.rs:77-83) mitigates
in prose only. Fix in the MCP tool, not the shared primitive.

## INFO 6 - process
`docs(mcp)` literally true (only /// lines) but materially understates: model-facing contracts on all
worktree-destructive tools. Finding 2 shows the message's own "every claim was checked" assertion was
false. Fixes: path-based review trigger on crates/bonsai-mcp/src/server/tools_*.rs; a test snapshotting
list_all() descriptions (the commit noted "No test asserts on description text" as reassurance - it is
the gap). Minor: every write description says "Requires --allow-write" (the CLI flag); the embedded
server's gate is the mcpAllowWrite setting - wrong name for half the deployments.

## VERIFIED CLEAN (do not re-audit)
- resolve_conflict_text traversal/symlink: conflict.rs:334-348 = validate_rel_path (stage.rs:43-52) ->
  find_conflict (must be currently conflicted) -> ensure_within_workdir (stage.rs:76-106) -> fs::write.
  Three layers, no escape found. resolve_conflict same. (NOT true of stage_paths - see HIGH 1.)
- abort_merge refuses NoOperationInProgress (merge/finalize.rs:184-188) and restores only
  merge-touched paths (not a blanket reset). rebase_abort refuses (rebase.rs:357-361) + untracked
  collision guard (rebase.rs:371-375). The new "aborting when none is in flight fails" claims are true.
- create_branch_here DOES checkout (create.rs:133) with rollback -> new claim accurate.
- checkout_branch = non-autostash variant; checkout_branch_autostash (checkout.rs:123) NOT used by MCP
  -> "Does not autostash" accurate.
- delete_branch: branchNotFound (delete.rs:25-27), current-branch refusal (delete.rs:31-35), unmerged
  check (delete.rs:42+), no force param in NameArgs.
- parse_resolution ours|theirs|markResolved -> InvalidName (server.rs:378-388).
- stage atomicity claim TRUE (all paths validated first, single index.write at end).
- 40-char oid claims effectively true (git2 Oid::from_str zero-pads short hex; refusal arrives as
  git/"commit not found" rather than invalidName - cosmetic).
- Multi-line descriptions break no framing: prompts_are_single_line guards bonsai-core argv prompts
  (ai_*.rs), a different surface; MCP descriptions are JSON-encoded; no .description consumer in
  src-tauri.
- Write router exposes no push/force/reset/clean/discard tool -> no direct network exfil from MCP.
- No commit touched these two files between 2a0b8f1 and HEAD (so cited line numbers are current).

## Not verified
No build/run, no scratch repo; all behavioral conclusions from Rust + vendored libgit2 source reading.
HIGH 1a not empirically demonstrated (Windows host, core.symlinks=false by default) - needs a
macOS/Linux scratch-repo test to confirm end to end. Embedded-server token/CSP/capability surfaces not
re-audited (untouched by this commit).

## ADDENDUM - HIGH 1a survives the file/directory collision (verified)
HEAD holds `cfg` as a leaf symlink entry, so adding `cfg/id_rsa` is a file-vs-directory collision.
libgit2 does NOT error: git_index_add_bypath calls index_insert(..., replace=1, ...) (index.c:1587)
-> check_file_directory_collision(..., replace) (index.c:1412) -> has_dir_name(..., ok_to_replace=1)
-> index.c:1178-1185 index_remove_entry() removes the colliding `cfg` entry and continues.
Chain intact.

## CLAIMS NOT CHECKED (so a later session knows the CLEAN register's boundary)
merge_branch / rebase_branch `operationInProgress`; merge autostash-and-restore + stashPopConflicts;
create_stash "reverts the worktree to HEAD" + includeUntracked; apply/pop outcome tags
(appliedPartially / notApplied / reservedPaths); unstage "equally atomic"; commit `hookRejected`
vs `configMissing` kind mapping; rebase_skip "not recoverable"; list_repos/select_repo session
semantics. LOW 2 is the one factual error FOUND, not the only one possible.
