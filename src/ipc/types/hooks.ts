/** First-time per-repo git-hook execution disclosure (`get_repo_hooks_disclosure`). */
export interface RepoHooksDisclosure {
  /** The repo has ≥1 runnable git hook Bonsai would run on commit/merge/push. */
  hasHooks: boolean;
  /** The user has already acknowledged this repo's one-time hook disclosure. */
  acknowledged: boolean;
}
