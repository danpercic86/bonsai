//! Path classification for the filesystem watcher.
//!
//! One function, `classify`, decides BOTH whether a filesystem event is
//! relevant at all and — when it is — whether it can possibly have changed the
//! commit graph. Keeping it as a single function (rather than a relevance
//! predicate plus a separate "is this graph-affecting" predicate) is deliberate:
//! two predicates would eventually drift apart and the frontend would then run
//! a narrow refresh for a burst that did move HEAD.

use std::path::Path;

/// What a single relevant filesystem path can affect.
///
/// Ordering matters conceptually: `Refs` is the conservative/wider class — when
/// a debounced burst mixes both, the burst counts as `Refs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathClass {
    /// Working-tree content: changes status/staging only. It CANNOT change the
    /// commit graph, so the frontend may refresh a narrow scope.
    Worktree,
    /// `.git/HEAD`, `.git/refs/**`, `.git/packed-refs` — or `.git/index`, see
    /// the belt-and-braces note in `classify`: may move HEAD or rewrite refs,
    /// so the graph must be re-streamed.
    Refs,
}

/// Classifies a filesystem event at `path`, or returns `None` when it is noise.
///
/// Anything outside `.git/` is workdir content — `Worktree`. Inside `.git/`,
/// only ref/HEAD/index state matters; lock files and object churn are noise.
pub fn classify(path: &Path, git_dir: &Path) -> Option<PathClass> {
    let rel = match path.strip_prefix(git_dir) {
        Err(_) => return Some(PathClass::Worktree), // not under .git → workdir content
        Ok(rel) => rel,
    };
    if rel
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".lock"))
    {
        return None; // index.lock etc. — churn
    }
    // P85 A3: Bonsai's private tag-sync scratch namespace. The fire-and-forget
    // fetch tag auto-sync (`tag_auto_sync.rs`) force-fetches into
    // `refs/bonsai-tagsync/*` for ancestry checks and cleans it up; that churn
    // must NOT trip a refresh. Real `refs/tags/*` adoptions ARE relevant and
    // fall through to the `starts_with("refs")` clause below. Must precede it.
    if rel.starts_with("refs/bonsai-tagsync") {
        return None;
    }
    // NON-OBVIOUS CALL: `.git/index` counts as Refs even though the index
    // itself never changes the commit graph (a `git add` writes the index and
    // nothing else).
    //
    // The reason is EVENT LOSS, not semantics. Before this classification
    // existed, correctness only required observing ANY relevant event in the
    // burst; now it requires observing the REF-FILE event specifically. As the
    // module header says, ReadDirectoryChangesW drops events on Windows — and
    // pre-change a dropped `refs/heads/main` event was harmless, because the
    // same commit's `index` write still forced a full refresh. Classified as
    // Worktree, that same drop would yield a Worktree-only burst and the graph
    // would silently never refresh. Treating the index as Refs restores that
    // belt-and-braces redundancy.
    //
    // The cost is negligible against the bug being fixed: a long checkout
    // writes the index ONCE, so this is one graph re-stream per checkout rather
    // than one per burst (thousands of files still land as pure Worktree
    // bursts).
    if rel == Path::new("HEAD")
        || rel == Path::new("index")
        // `refs` is a DIRECTORY, so component-wise `starts_with` is right; the
        // other three are single files and compare by equality.
        || rel.starts_with("refs")
        || rel == Path::new("packed-refs")
    {
        return Some(PathClass::Refs);
    }
    None
}

#[cfg(test)]
pub(crate) fn is_relevant(path: &Path, git_dir: &Path) -> bool {
    classify(path, git_dir).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_relevant_rules() {
        let git_dir = Path::new(r"C:\repo\.git");
        // Workdir content.
        assert!(is_relevant(Path::new(r"C:\repo\src\main.rs"), git_dir));
        // Relevant .git internals.
        assert!(is_relevant(&git_dir.join("HEAD"), git_dir));
        assert!(is_relevant(&git_dir.join("index"), git_dir));
        assert!(is_relevant(&git_dir.join("packed-refs"), git_dir));
        assert!(is_relevant(
            &git_dir.join("refs").join("heads").join("main"),
            git_dir
        ));
        // M6 §6.4: remote-tracking ref updates after a fetch must refresh.
        assert!(is_relevant(
            &git_dir
                .join("refs")
                .join("remotes")
                .join("origin")
                .join("main"),
            git_dir
        ));
        // P85 A3: a real tag adoption under refs/tags/* IS relevant…
        assert!(is_relevant(
            &git_dir.join("refs").join("tags").join("v1.0.0"),
            git_dir
        ));
        // …but Bonsai's private tag-sync scratch namespace is NOT (fetch churn).
        assert!(!is_relevant(
            &git_dir.join("refs").join("bonsai-tagsync").join("v1.0.0"),
            git_dir
        ));
        // Noise.
        assert!(!is_relevant(&git_dir.join("index.lock"), git_dir));
        assert!(!is_relevant(
            &git_dir.join("refs").join("heads").join("main.lock"),
            git_dir
        ));
        assert!(!is_relevant(
            &git_dir.join("objects").join("aa").join("bb"),
            git_dir
        ));
        assert!(!is_relevant(&git_dir.join("logs").join("HEAD"), git_dir));
        assert!(!is_relevant(&git_dir.join("FETCH_HEAD"), git_dir));
    }

    /// P110: the relevance filter and the graph-affecting classification are the
    /// SAME function, so this covers every branch of the decision table.
    #[test]
    fn classify_decision_table() {
        let git_dir = Path::new(r"C:\repo\.git");
        let c = |p: std::path::PathBuf| classify(&p, git_dir);

        // Worktree content → Worktree (status only, no graph).
        assert_eq!(
            classify(Path::new(r"C:\repo\src\main.rs"), git_dir),
            Some(PathClass::Worktree)
        );

        // Graph-affecting .git state → Refs. `.git/index` is deliberately here
        // and NOT Worktree: it is the redundancy that survives a dropped
        // ReadDirectoryChangesW ref event (see `classify`).
        assert_eq!(c(git_dir.join("index")), Some(PathClass::Refs));
        assert_eq!(c(git_dir.join("HEAD")), Some(PathClass::Refs));
        assert_eq!(c(git_dir.join("packed-refs")), Some(PathClass::Refs));
        assert_eq!(
            c(git_dir.join("refs").join("heads").join("x")),
            Some(PathClass::Refs)
        );
        assert_eq!(
            c(git_dir
                .join("refs")
                .join("remotes")
                .join("origin")
                .join("main")),
            Some(PathClass::Refs)
        );
        assert_eq!(
            c(git_dir.join("refs").join("tags").join("v1.0.0")),
            Some(PathClass::Refs)
        );

        // Excluded.
        assert_eq!(c(git_dir.join("index.lock")), None);
        assert_eq!(
            c(git_dir.join("refs").join("heads").join("main.lock")),
            None
        );
        assert_eq!(
            c(git_dir.join("refs").join("bonsai-tagsync").join("v1.0.0")),
            None
        );
        // A .git path that is none of the above (op-state/message files are NOT
        // watched today — see mod.rs's burst-class notes).
        assert_eq!(c(git_dir.join("COMMIT_EDITMSG")), None);
        assert_eq!(c(git_dir.join("MERGE_HEAD")), None);
        assert_eq!(c(git_dir.join("objects").join("aa").join("bb")), None);
    }
}
