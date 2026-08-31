export type ConflictKind =
  | 'bothModified'
  | 'bothAdded'
  | 'deletedByUs'
  | 'deletedByThem'
  | 'addedByUs'
  | 'addedByThem'
  | 'bothDeleted';

export interface ConflictEntry {
  path: string;
  kind: ConflictKind;
  hasBase: boolean;
  hasOurs: boolean;
  hasTheirs: boolean;
}

export interface ConflictFile {
  path: string;
  kind: ConflictKind;
  binary: boolean;
  tooLarge: boolean;
  /** Worktree file missing (deletion conflicts). text is '' when true. */
  missing: boolean;
  /** Worktree contents INCLUDING <<<<<<< ======= >>>>>>> markers. */
  text: string;
  /** Stage-2 (OURS) blob text. '' when the ours side is absent or text is suppressed. */
  ours: string;
  /** Stage-3 (THEIRS) blob text. '' when the theirs side is absent or text is suppressed. */
  theirs: string;
}

export type ConflictResolution = 'ours' | 'theirs' | 'markResolved';
