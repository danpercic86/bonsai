// Every "a modal / confirm / prompt is armed" flag for one open repo, grouped in
// one place. Each member is independent state written by the container's
// handlers (or by an extracted action hook that receives its setter) and read by
// `WorkspaceDialogs` / `WorkspaceOverlays` / `TagSyncDialogs`; none is touched by
// `runRefreshRound` or any cross-cutting effect, which is what makes the group a
// leaf. `anyDialogArmed` is the disjunction over these members — the container
// ORs it with the armed flags that live outside this hook (the hook gate, the
// hook disclosure and the Ask-Bonsai pipeline) to derive `dialogOpen`.
//
// Extracted verbatim from RepoWorkspace; the container destructures the returned
// object under the ORIGINAL names, so no call site changed.
import { useRef, useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import type { LineSelection, RebaseTodoOp, ResetMode, UndoPlan } from '../../ipc';
import type { PendingForceSubmodule } from '../dialogs/SubmoduleDialogs';
import type { PendingDeleteRemoteTag, PendingForceMoveTag } from '../dialogs/TagSyncDialogs';
import type { NonFfPullInfo } from './useRemoteOps';

export interface PendingDropStash {
  index: number;
  oid?: string;
}

export interface PendingReservedStash {
  index: number;
  op: 'apply' | 'pop';
  paths: string[];
  oid?: string;
}

export interface PendingReset {
  oid: string;
  mode: ResetMode;
}

export interface PendingDiscardForce {
  paths: string[];
  modified: number;
  created: number;
  // The untracked (created) subset of `paths` — the files permanently deleted,
  // listed in the confirm dialog so the destruction is spelled out per-path.
  untracked: string[];
}

export interface CommitPushResolver {
  resolve: () => void;
  reject: (e: unknown) => void;
  // P58c: the sign choice parked alongside the message (forwarded to
  // doCommitAndPush once the set-upstream dialog is answered).
  sign: boolean | null;
  // P59a: the "Skip hooks" choice parked alongside the message.
  skipHooks: boolean;
}

export interface PendingHunkDiscard {
  path: string;
  origPath: string | null;
  hunkIndex: number;
}

export interface PendingLineDiscard {
  path: string;
  origPath: string | null;
  selection: LineSelection[];
}

export interface PendingCherrypick {
  oid: string;
  initialMessage: string;
  loading: boolean;
}

export interface PendingWorktreeRemove {
  name: string;
  absPath: string;
}

export interface RebasePlan {
  ontoOid: string;
  ontoLabel: string;
  initialTodos: RebaseTodoOp[];
  summaries: Record<string, string>;
}

export interface UseWorkspaceDialogState {
  abortConfirmOpen: boolean;
  setAbortConfirmOpen: Dispatch<SetStateAction<boolean>>;
  pendingDeleteBranch: string | null;
  setPendingDeleteBranch: Dispatch<SetStateAction<string | null>>;
  pendingRebase: { name: string; cur: string } | null;
  setPendingRebase: Dispatch<SetStateAction<{ name: string; cur: string } | null>>;
  pendingDeleteRemote: string | null;
  setPendingDeleteRemote: Dispatch<SetStateAction<string | null>>;
  pendingDropStash: PendingDropStash | null;
  setPendingDropStash: Dispatch<SetStateAction<PendingDropStash | null>>;
  pendingReservedStash: PendingReservedStash | null;
  setPendingReservedStash: Dispatch<SetStateAction<PendingReservedStash | null>>;
  pendingReset: PendingReset | null;
  setPendingReset: Dispatch<SetStateAction<PendingReset | null>>;
  pendingDiscard: string[] | null;
  setPendingDiscard: Dispatch<SetStateAction<string[] | null>>;
  pendingDiscardForce: PendingDiscardForce | null;
  setPendingDiscardForce: Dispatch<SetStateAction<PendingDiscardForce | null>>;
  pendingCommitPush: string | null;
  setPendingCommitPush: Dispatch<SetStateAction<string | null>>;
  commitPushResolver: { current: CommitPushResolver | null };
  pendingForcePush: boolean;
  setPendingForcePush: Dispatch<SetStateAction<boolean>>;
  pendingHunkDiscard: PendingHunkDiscard | null;
  setPendingHunkDiscard: Dispatch<SetStateAction<PendingHunkDiscard | null>>;
  pendingLineDiscard: PendingLineDiscard | null;
  setPendingLineDiscard: Dispatch<SetStateAction<PendingLineDiscard | null>>;
  pendingCreateBranch: { oid: string } | null;
  setPendingCreateBranch: Dispatch<SetStateAction<{ oid: string } | null>>;
  pendingRenameBranch: { name: string } | null;
  setPendingRenameBranch: Dispatch<SetStateAction<{ name: string } | null>>;
  pendingAddSubmodule: boolean;
  setPendingAddSubmodule: Dispatch<SetStateAction<boolean>>;
  pendingDeinitSubmodule: string | null;
  setPendingDeinitSubmodule: Dispatch<SetStateAction<string | null>>;
  pendingRemoveSubmodule: string | null;
  setPendingRemoveSubmodule: Dispatch<SetStateAction<string | null>>;
  pendingForceSubmodule: PendingForceSubmodule | null;
  setPendingForceSubmodule: Dispatch<SetStateAction<PendingForceSubmodule | null>>;
  pendingNonFfPull: NonFfPullInfo | null;
  setPendingNonFfPull: Dispatch<SetStateAction<NonFfPullInfo | null>>;
  pendingUndo: UndoPlan | null;
  setPendingUndo: Dispatch<SetStateAction<UndoPlan | null>>;
  pendingCherrypick: PendingCherrypick | null;
  setPendingCherrypick: Dispatch<SetStateAction<PendingCherrypick | null>>;
  pendingCreateTag: { oid: string } | null;
  setPendingCreateTag: Dispatch<SetStateAction<{ oid: string } | null>>;
  pendingDeleteTag: string | null;
  setPendingDeleteTag: Dispatch<SetStateAction<string | null>>;
  pendingDeleteRemoteTag: PendingDeleteRemoteTag | null;
  setPendingDeleteRemoteTag: Dispatch<SetStateAction<PendingDeleteRemoteTag | null>>;
  pendingForceMoveTag: PendingForceMoveTag | null;
  setPendingForceMoveTag: Dispatch<SetStateAction<PendingForceMoveTag | null>>;
  pendingAddRemote: boolean;
  setPendingAddRemote: Dispatch<SetStateAction<boolean>>;
  pendingRenameRemote: { name: string } | null;
  setPendingRenameRemote: Dispatch<SetStateAction<{ name: string } | null>>;
  pendingEditUrl: { name: string; url: string } | null;
  setPendingEditUrl: Dispatch<SetStateAction<{ name: string; url: string } | null>>;
  pendingRemoveRemote: string | null;
  setPendingRemoveRemote: Dispatch<SetStateAction<string | null>>;
  staleCleanupOpen: boolean;
  setStaleCleanupOpen: Dispatch<SetStateAction<boolean>>;
  newWorktreeOpen: boolean;
  setNewWorktreeOpen: Dispatch<SetStateAction<boolean>>;
  whatChangedOpen: boolean;
  setWhatChangedOpen: Dispatch<SetStateAction<boolean>>;
  changelogOpen: boolean;
  setChangelogOpen: Dispatch<SetStateAction<boolean>>;
  pendingWorktreeRemove: PendingWorktreeRemove | null;
  setPendingWorktreeRemove: Dispatch<SetStateAction<PendingWorktreeRemove | null>>;
  pendingWorktreeLock: string | null;
  setPendingWorktreeLock: Dispatch<SetStateAction<string | null>>;
  worktreeContextOpen: boolean;
  setWorktreeContextOpen: Dispatch<SetStateAction<boolean>>;
  rebasePlan: RebasePlan | null;
  setRebasePlan: Dispatch<SetStateAction<RebasePlan | null>>;
  rebasePlanError: string | null;
  setRebasePlanError: Dispatch<SetStateAction<string | null>>;
  /** True while ANY dialog owned by this hook is armed. The container ORs this
   *  with the armed flags declared outside it to derive `dialogOpen`, which
   *  suppresses the workspace keyboard shortcuts. */
  anyDialogArmed: boolean;
}

export function useWorkspaceDialogState(): UseWorkspaceDialogState {
  const [abortConfirmOpen, setAbortConfirmOpen] = useState(false);
  // P6 §4.5: pending branch/remote deletes drive the two confirm dialogs; the
  // shortcut effect is suppressed while either is up (derived `dialogOpen`).
  const [pendingDeleteBranch, setPendingDeleteBranch] = useState<string | null>(null);
  // Plain (non-interactive) rebase confirm gate. `name` = the branch rebased
  // onto; `cur` = the current branch whose commits get rewritten (for the copy).
  const [pendingRebase, setPendingRebase] = useState<{ name: string; cur: string } | null>(null);
  const [pendingDeleteRemote, setPendingDeleteRemote] = useState<string | null>(null);
  // F-A6-B: carry the rendered oid alongside the index so the Drop confirm hits
  // exactly the entry the user saw, even if the stack shifts before confirming.
  const [pendingDropStash, setPendingDropStash] = useState<PendingDropStash | null>(null);
  // Reserved-path recovery: a stash apply/pop hit Windows-reserved paths (e.g.
  // `NUL`). Arms a ConfirmDialog offering to apply the rest, skipping those.
  // `oid` (F-A6-B) is forwarded on the skip-reserved retry.
  const [pendingReservedStash, setPendingReservedStash] = useState<PendingReservedStash | null>(
    null,
  );
  // P20: destructive reset (all three modes confirm; hard warns extra) + discard.
  const [pendingReset, setPendingReset] = useState<PendingReset | null>(null);
  const [pendingDiscard, setPendingDiscard] = useState<string[] | null>(null);
  // Bulk "Discard all" (panel + folder): reverts modified tracked files AND
  // deletes new/untracked files. Carries per-kind counts so the confirm dialog
  // can warn precisely about permanent deletion of new files.
  const [pendingDiscardForce, setPendingDiscardForce] = useState<PendingDiscardForce | null>(null);
  // Commit & Push: when HEAD has no upstream, the message is parked here while a
  // ConfirmDialog asks to set upstream. The pending promise (resolves the
  // CommitBox submit) is held in commitPushResolver.
  const [pendingCommitPush, setPendingCommitPush] = useState<string | null>(null);
  const commitPushResolver = useRef<CommitPushResolver | null>(null);
  // P37b: force-push-with-lease confirm gate (targets the current branch).
  const [pendingForcePush, setPendingForcePush] = useState(false);
  // P28: pending "Discard hunk" confirmation (unstaged diffs only).
  const [pendingHunkDiscard, setPendingHunkDiscard] = useState<PendingHunkDiscard | null>(null);
  // P45: pending "Discard line(s)" confirmation (unstaged diffs only). Stores the
  // selection verbatim — arbitrary lines cannot be re-derived from a hunk index.
  const [pendingLineDiscard, setPendingLineDiscard] = useState<PendingLineDiscard | null>(null);
  // P11 §1.4: "Create branch here" target commit → drives the PromptDialog.
  const [pendingCreateBranch, setPendingCreateBranch] = useState<{ oid: string } | null>(null);
  // P60a: "Rename…" a local branch → drives the shared PromptDialog (prefilled).
  const [pendingRenameBranch, setPendingRenameBranch] = useState<{ name: string } | null>(null);
  // P60d: submodule add (url + path) / deinit / remove dialog state.
  const [pendingAddSubmodule, setPendingAddSubmodule] = useState(false);
  const [pendingDeinitSubmodule, setPendingDeinitSubmodule] = useState<string | null>(null);
  const [pendingRemoveSubmodule, setPendingRemoveSubmodule] = useState<string | null>(null);
  // P82 (F-A7-7): a plain deinit/remove refused because the submodule worktree is
  // dirty → the danger force-escalation dialog (attempt-then-offer-force).
  const [pendingForceSubmodule, setPendingForceSubmodule] =
    useState<PendingForceSubmodule | null>(null);
  // P60b: a non-fast-forward pull → drives NonFfPullDialog (Merge / Rebase).
  const [pendingNonFfPull, setPendingNonFfPull] = useState<NonFfPullInfo | null>(null);
  // P60c: one-click undo. The toolbar Undo button describes the last op
  // (read-only) into this plan; the UndoDialog confirms, then reuses resetBranch.
  const [pendingUndo, setPendingUndo] = useState<UndoPlan | null>(null);
  // P47d: cherry-pick message dialog. `handleCherrypick` opens this prefilled
  // with the source commit's full message (fetched via getCommitDiff); confirm
  // runs the pick with the edited message. `loading` gates the prefill fetch.
  const [pendingCherrypick, setPendingCherrypick] = useState<PendingCherrypick | null>(null);
  // P22 §7.1: tag + remote management dialog state.
  const [pendingCreateTag, setPendingCreateTag] = useState<{ oid: string } | null>(null);
  const [pendingDeleteTag, setPendingDeleteTag] = useState<string | null>(null);
  // P77: remote-tag destructive confirms (delete-on-remote, force-move-on-remote).
  const [pendingDeleteRemoteTag, setPendingDeleteRemoteTag] =
    useState<PendingDeleteRemoteTag | null>(null);
  const [pendingForceMoveTag, setPendingForceMoveTag] = useState<PendingForceMoveTag | null>(null);
  const [pendingAddRemote, setPendingAddRemote] = useState<boolean>(false);
  const [pendingRenameRemote, setPendingRenameRemote] = useState<{ name: string } | null>(null);
  const [pendingEditUrl, setPendingEditUrl] = useState<{ name: string; url: string } | null>(null);
  const [pendingRemoveRemote, setPendingRemoveRemote] = useState<string | null>(null);
  // P25d: B4 stale-branch cleanup dialog (opened from the Branches header).
  const [staleCleanupOpen, setStaleCleanupOpen] = useState(false);
  // P27 §6.5/§6.6: worktree dialogs — create (branch picker), remove confirm
  // (names the directory to delete), lock reason prompt.
  const [newWorktreeOpen, setNewWorktreeOpen] = useState(false);
  // P28 §7: "✨ What changed…" digest range picker (opened from the toolbar).
  const [whatChangedOpen, setWhatChangedOpen] = useState(false);
  // P56b §6: "✨ Release notes…" changelog range picker (opened from the palette).
  const [changelogOpen, setChangelogOpen] = useState(false);
  const [pendingWorktreeRemove, setPendingWorktreeRemove] =
    useState<PendingWorktreeRemove | null>(null);
  const [pendingWorktreeLock, setPendingWorktreeLock] = useState<string | null>(null);
  // P31 §7: the worktree × AI-context matrix (opened from the worktree menu).
  const [worktreeContextOpen, setWorktreeContextOpen] = useState(false);
  // P23b: interactive-rebase plan editor. `rebasePlan` holds the seeded plan +
  // display metadata; `rebasePlanError` shows a failed Start's error in-dialog.
  const [rebasePlan, setRebasePlan] = useState<RebasePlan | null>(null);
  const [rebasePlanError, setRebasePlanError] = useState<string | null>(null);

  const anyDialogArmed =
    pendingDeleteBranch !== null ||
    pendingRebase !== null ||
    pendingDeleteRemote !== null ||
    pendingDropStash !== null ||
    pendingReservedStash !== null ||
    pendingReset !== null ||
    pendingDiscard !== null ||
    pendingDiscardForce !== null ||
    pendingHunkDiscard !== null ||
    pendingLineDiscard !== null ||
    pendingCreateBranch !== null ||
    pendingRenameBranch !== null ||
    pendingAddSubmodule ||
    pendingDeinitSubmodule !== null ||
    pendingRemoveSubmodule !== null ||
    pendingForceSubmodule !== null ||
    pendingNonFfPull !== null ||
    pendingUndo !== null ||
    pendingCherrypick !== null ||
    pendingCreateTag !== null ||
    pendingDeleteTag !== null ||
    pendingDeleteRemoteTag !== null ||
    pendingForceMoveTag !== null ||
    pendingAddRemote ||
    pendingRenameRemote !== null ||
    pendingEditUrl !== null ||
    pendingRemoveRemote !== null ||
    staleCleanupOpen ||
    newWorktreeOpen ||
    whatChangedOpen ||
    changelogOpen ||
    pendingWorktreeRemove !== null ||
    pendingWorktreeLock !== null ||
    worktreeContextOpen ||
    rebasePlan !== null;

  return {
    abortConfirmOpen,
    setAbortConfirmOpen,
    pendingDeleteBranch,
    setPendingDeleteBranch,
    pendingRebase,
    setPendingRebase,
    pendingDeleteRemote,
    setPendingDeleteRemote,
    pendingDropStash,
    setPendingDropStash,
    pendingReservedStash,
    setPendingReservedStash,
    pendingReset,
    setPendingReset,
    pendingDiscard,
    setPendingDiscard,
    pendingDiscardForce,
    setPendingDiscardForce,
    pendingCommitPush,
    setPendingCommitPush,
    commitPushResolver,
    pendingForcePush,
    setPendingForcePush,
    pendingHunkDiscard,
    setPendingHunkDiscard,
    pendingLineDiscard,
    setPendingLineDiscard,
    pendingCreateBranch,
    setPendingCreateBranch,
    pendingRenameBranch,
    setPendingRenameBranch,
    pendingAddSubmodule,
    setPendingAddSubmodule,
    pendingDeinitSubmodule,
    setPendingDeinitSubmodule,
    pendingRemoveSubmodule,
    setPendingRemoveSubmodule,
    pendingForceSubmodule,
    setPendingForceSubmodule,
    pendingNonFfPull,
    setPendingNonFfPull,
    pendingUndo,
    setPendingUndo,
    pendingCherrypick,
    setPendingCherrypick,
    pendingCreateTag,
    setPendingCreateTag,
    pendingDeleteTag,
    setPendingDeleteTag,
    pendingDeleteRemoteTag,
    setPendingDeleteRemoteTag,
    pendingForceMoveTag,
    setPendingForceMoveTag,
    pendingAddRemote,
    setPendingAddRemote,
    pendingRenameRemote,
    setPendingRenameRemote,
    pendingEditUrl,
    setPendingEditUrl,
    pendingRemoveRemote,
    setPendingRemoveRemote,
    staleCleanupOpen,
    setStaleCleanupOpen,
    newWorktreeOpen,
    setNewWorktreeOpen,
    whatChangedOpen,
    setWhatChangedOpen,
    changelogOpen,
    setChangelogOpen,
    pendingWorktreeRemove,
    setPendingWorktreeRemove,
    pendingWorktreeLock,
    setPendingWorktreeLock,
    worktreeContextOpen,
    setWorktreeContextOpen,
    rebasePlan,
    setRebasePlan,
    rebasePlanError,
    setRebasePlanError,
    anyDialogArmed,
  };
}
