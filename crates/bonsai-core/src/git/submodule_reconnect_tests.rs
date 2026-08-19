//! Unit tests for [`crate::git::submodule_reconnect`] (P73 §8.2 helpers),
//! split out to keep the module under the ~500-line limit (CLAUDE.md). Same
//! `#[cfg(test)] #[path = ...] mod tests;` pattern as `cred.rs`.

use super::*;

#[test]
fn urls_equivalent_table() {
    // `.git` / trailing-`/` insensitivity, both orders, repeated.
    assert!(urls_equivalent("https://x/y.git", "https://x/y"));
    assert!(urls_equivalent("https://x/y", "https://x/y.git"));
    assert!(urls_equivalent("https://x/y.git/", "https://x/y"));
    assert!(urls_equivalent("https://x/y/.git", "https://x/y"));
    assert!(urls_equivalent("  https://x/y.git  ", "https://x/y/"));
    // Case-insensitive accept (logged).
    assert!(urls_equivalent("HTTPS://X/Y.git", "https://x/y"));
    // Clear non-matches.
    assert!(!urls_equivalent("https://a/y", "https://b/y"));
    assert!(!urls_equivalent("https://x/y", "https://x/z"));
    assert!(!urls_equivalent("https://x/a%20b", "https://x/a b"));
    assert!(!urls_equivalent("", "https://x/y"));
}

#[test]
fn rel_path_table() {
    let base = Path::new("/r");
    // Nested → hops up then down, forward slashes only.
    assert_eq!(
        rel_path(
            &base.join("vendor/sub"),
            &base.join(".git/modules/vendor/sub")
        ),
        "../../.git/modules/vendor/sub"
    );
    assert_eq!(
        rel_path(
            &base.join(".git/modules/vendor/sub"),
            &base.join("vendor/sub")
        ),
        "../../../../vendor/sub"
    );
    // Sibling.
    assert_eq!(rel_path(&base.join("a"), &base.join("b")), "../b");
    // Identical.
    assert_eq!(rel_path(&base.join("a"), &base.join("a")), ".");
    // Descendant.
    assert_eq!(rel_path(base, &base.join("a/b")), "a/b");
    // No backslashes, no verbatim prefix ever leaks.
    let out = rel_path(&base.join("a/b"), &base.join("c"));
    assert!(!out.contains('\\') && !out.contains('?'), "{out}");
}

#[test]
fn strip_verbatim_normalizes() {
    assert_eq!(
        strip_verbatim(Path::new(r"\\?\D:\other\.git\modules\sub")),
        "D:/other/.git/modules/sub"
    );
    assert_eq!(strip_verbatim(Path::new("/r/sub")), "/r/sub");
}

#[test]
fn workdir_is_empty_table() {
    let td = tempfile::tempdir().expect("tempdir");
    let r = td.path();

    // Absent ⇒ true.
    assert!(workdir_is_empty(&r.join("nope")));
    // Empty dir ⇒ true.
    let empty = r.join("empty");
    std::fs::create_dir_all(&empty).expect("mkdir");
    assert!(workdir_is_empty(&empty));
    // Dir of empty dirs (recursive) ⇒ true.
    let nested = r.join("nested");
    std::fs::create_dir_all(nested.join("a/b/c")).expect("mkdir");
    assert!(workdir_is_empty(&nested));
    // Dir containing a file ⇒ false (at depth, too).
    let withfile = r.join("withfile");
    std::fs::create_dir_all(withfile.join("a")).expect("mkdir");
    std::fs::write(withfile.join("a/f.txt"), "x").expect("write");
    assert!(!workdir_is_empty(&withfile));
    // Dir containing `.git` as a FILE ⇒ false.
    let gf = r.join("gitfile");
    std::fs::create_dir_all(&gf).expect("mkdir");
    std::fs::write(gf.join(".git"), "gitdir: x\n").expect("write");
    assert!(!workdir_is_empty(&gf));
    // Dir containing `.git` as a DIR ⇒ false.
    let gd = r.join("gitdir");
    std::fs::create_dir_all(gd.join(".git")).expect("mkdir");
    assert!(!workdir_is_empty(&gd));
    // A plain file at the path ⇒ false.
    let f = r.join("plain");
    std::fs::write(&f, "x").expect("write");
    assert!(!workdir_is_empty(&f));
}

/// `write_gitlink` must produce a RELATIVE, forward-slash `gitdir:` line and
/// set `core.worktree` back to the worktree (no `\\?\`, no absolute path).
#[test]
fn write_gitlink_writes_relative_pair() {
    let td = tempfile::tempdir().expect("tempdir");
    let root = td.path();
    let sub_wd = root.join("vendor/sub");
    std::fs::create_dir_all(&sub_wd).expect("mkdir sub");
    let module_dir = root.join(".git/modules/vendor/sub");
    std::fs::create_dir_all(&module_dir).expect("mkdir mod");
    git2::Repository::init_bare(&module_dir).expect("init module gitdir");

    write_gitlink(&sub_wd, &module_dir, root).expect("write_gitlink");

    let link = std::fs::read_to_string(sub_wd.join(".git")).expect("read gitlink");
    assert_eq!(link, "gitdir: ../../.git/modules/vendor/sub\n", "{link}");
    assert!(!link.contains('\\') && !link.contains('?'), "{link}");
    // The temp file must not survive.
    assert!(!sub_wd.join(".git.bonsai-tmp").exists());

    let sub_repo = git2::Repository::open_ext(
        &module_dir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        &[] as &[&OsStr],
    )
    .expect("open module");
    let cfg = sub_repo.config().expect("config");
    assert_eq!(
        cfg.get_string("core.worktree").expect("core.worktree"),
        "../../../../vendor/sub"
    );
    assert!(!cfg.get_bool("core.bare").expect("core.bare"));
}

// ----------------------------------------------- P73 §8.2 fixture helpers

/// A non-bare repo with one commit (submodule status needs a HEAD tree).
fn init_repo_with_commit(dir: &Path) -> git2::Repository {
    let repo = git2::Repository::init(dir).expect("init");
    std::fs::write(dir.join("top.txt"), "super\n").expect("write");
    let mut index = repo.index().expect("index");
    index.add_path(Path::new("top.txt")).expect("add");
    index.write().expect("index write");
    let tree_id = index.write_tree().expect("write_tree");
    {
        let tree = repo.find_tree(tree_id).expect("find_tree");
        let sig = git2::Signature::new("T", "t@example.com", &git2::Time::new(1_700_000_000, 0))
            .expect("signature");
        repo.commit(Some("HEAD"), &sig, &sig, "super: initial", &tree, &[])
            .expect("commit");
    }
    repo
}

/// An upstream sub-repo with one commit, plus its `file://` url.
fn build_upstream_sub(dir: &Path) -> String {
    let repo = git2::Repository::init(dir).expect("init sub");
    std::fs::write(dir.join("lib.txt"), "sub v1\n").expect("write");
    let mut index = repo.index().expect("index");
    index.add_path(Path::new("lib.txt")).expect("add");
    index.write().expect("index write");
    let tree_id = index.write_tree().expect("write_tree");
    let tree = repo.find_tree(tree_id).expect("find_tree");
    let sig = git2::Signature::new("T", "t@example.com", &git2::Time::new(1_700_000_000, 0))
        .expect("signature");
    repo.commit(Some("HEAD"), &sig, &sig, "sub v1", &tree, &[])
        .expect("commit");
    let canon = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    format!("file:///{}", strip_verbatim(&canon).trim_start_matches('/'))
}

/// Entry names directly under `<root>/.git/modules`, sorted.
fn modules_entries(root: &Path) -> Vec<String> {
    let mut v: Vec<String> = match std::fs::read_dir(root.join(".git/modules")) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => Vec::new(),
    };
    v.sort();
    v
}

// -------------------------------------------------- §8.2 name/traversal

/// A hostile submodule NAME is refused by `validate_modules_name` in step 1,
/// BEFORE any filesystem decision: no new entry appears under `.git/modules`,
/// no worktree folder is created, and a sentinel outside `modules/` survives.
#[test]
fn reattach_rejects_hostile_name_before_touching_disk() {
    let td = tempfile::tempdir().expect("tempdir");
    let root = td.path();
    let repo = init_repo_with_commit(root);

    // A registered submodule (never cloned) just to obtain a handle: only the
    // NAME argument is hostile, everything else is legitimate.
    let sm = repo
        .submodule("file:///nowhere", Path::new("vendor/sub"), true)
        .expect("register submodule");

    // A sentinel a traversing key could reach if the guard were missing.
    let sentinel = root.join(".git/sentinel.txt");
    std::fs::write(&sentinel, b"keep me").expect("sentinel");
    let before = modules_entries(root);

    for hostile in ["../escape", "..", "/abs", "C:/abs", "a//b", "", "   "] {
        match reattach_module_gitdir(&repo, &sm, hostile) {
            Err(AppError::Git(m)) => {
                assert!(m.contains("unsafe name"), "wrong refusal for {hostile:?}: {m}")
            }
            other => panic!("hostile name {hostile:?} must be refused, got {other:?}"),
        }
        // Nothing can ever hand such a key back as a gitdir either.
        assert_eq!(
            module_gitdir(&repo, hostile, hostile).expect("module_gitdir"),
            None,
            "hostile name must never resolve to a gitdir: {hostile:?}"
        );
    }

    assert_eq!(
        std::fs::read(&sentinel).expect("read sentinel"),
        b"keep me",
        "the sentinel outside modules/ is byte-identical"
    );
    assert_eq!(
        modules_entries(root),
        before,
        "no new entry was created under .git/modules"
    );
    assert!(!root.join("escape").exists(), "no escaped folder was created");
    assert!(!root.join("..").join("escape").exists(), "nothing above the repo either");
}

/// OPEN-1's decided default: when name and path differ and BOTH
/// `<modules>/<name>` and `<modules>/<path>` exist, `name` (git's canonical key)
/// wins; the `path` key is only the fallback.
#[test]
fn module_gitdir_prefers_name_over_path() {
    let td = tempfile::tempdir().expect("tempdir");
    let root = td.path();
    let repo = init_repo_with_commit(root);
    let modules = root.join(".git/modules");
    std::fs::create_dir_all(modules.join("thename")).expect("mkdir name");
    std::fs::create_dir_all(modules.join("thepath")).expect("mkdir path");

    let found = module_gitdir(&repo, "thename", "thepath")
        .expect("module_gitdir")
        .expect("some gitdir");
    assert!(
        found.ends_with("thename"),
        "name must win over path, got {}",
        found.display()
    );

    std::fs::remove_dir_all(modules.join("thename")).expect("rm name");
    let found = module_gitdir(&repo, "thename", "thepath")
        .expect("module_gitdir")
        .expect("some gitdir");
    assert!(
        found.ends_with("thepath"),
        "path is the fallback key, got {}",
        found.display()
    );
}

/// A key that would resolve OUTSIDE `<commondir>/modules` yields `None` — never
/// a path outside the modules root, and never the root itself.
#[test]
fn module_gitdir_rejects_escaping_key() {
    let td = tempfile::tempdir().expect("tempdir");
    let root = td.path();
    let repo = init_repo_with_commit(root);
    std::fs::create_dir_all(root.join(".git/modules")).expect("mkdir modules");
    // A real, EXISTING directory just outside the modules root, so the only
    // thing stopping resolution is the guard (not a failing canonicalize).
    std::fs::create_dir_all(root.join(".git/escape")).expect("mkdir escape");

    for key in ["../escape", "..", "sub/../../escape", "/abs", "", ".", "./escape"] {
        let got = module_gitdir(&repo, key, key).expect("module_gitdir");
        assert_eq!(got, None, "escaping key must not resolve: {key:?}");
    }
    assert!(
        root.join(".git/escape").exists(),
        "nothing outside modules/ was touched"
    );
}

// ------------------------------------------------------- §8.2 commondir

/// P73 §6: `remove_cached_git_dir` resolves `<commondir>/modules`, not
/// `<repo.path()>/modules`. Plain repo: identical behaviour (commondir == path).
/// Linked worktree: it must reach the SHARED modules root — keying on
/// `repo.path()` (= `.git/worktrees/<wt>/`) silently no-opped there.
#[test]
fn remove_cached_git_dir_uses_commondir() {
    use crate::git::submodule::remove_cached_git_dir;

    let td = tempfile::tempdir().expect("tempdir");
    let root = td.path().join("super");
    std::fs::create_dir_all(&root).expect("mkdir super");
    let repo = init_repo_with_commit(&root);

    // (a) plain repo: commondir == path, and the cached dir is removed.
    assert_eq!(
        repo.commondir().canonicalize().expect("canon commondir"),
        repo.path().canonicalize().expect("canon path"),
        "in a plain repo commondir == path"
    );
    let cached = root.join(".git/modules/vendor/sub");
    std::fs::create_dir_all(&cached).expect("mkdir cached");
    std::fs::write(cached.join("HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
    remove_cached_git_dir(&repo, "vendor/sub");
    assert!(!cached.exists(), "plain repo: the cached gitdir is removed");

    // (b) linked worktree: path() is `.git/worktrees/<wt>/`, commondir() is the
    //     shared `.git/`, which is where `modules/` actually lives.
    let wt_path = td.path().join("wt");
    let wt = repo
        .worktree("wt", &wt_path, None)
        .expect("create linked worktree");
    let wt_repo = git2::Repository::open(wt.path()).expect("open worktree repo");
    assert_ne!(
        wt_repo.path().canonicalize().expect("canon wt path"),
        wt_repo.commondir().canonicalize().expect("canon wt commondir"),
        "in a linked worktree path() != commondir()"
    );
    assert!(
        !wt_repo.path().join("modules").exists(),
        "the worktree gitdir has no modules/ of its own — this was the old bug"
    );
    let shared = root.join(".git/modules/vendor/sub");
    std::fs::create_dir_all(&shared).expect("mkdir shared cached");
    std::fs::write(shared.join("HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
    remove_cached_git_dir(&wt_repo, "vendor/sub");
    assert!(
        !shared.exists(),
        "linked worktree: the SHARED cached gitdir is removed via commondir"
    );
}

// ------------------------------------------------- §8.2 salvage guardrails

/// The salvage must not perturb the normal flows: a healthy checked-out
/// submodule and a never-cloned (virgin) one both yield `NotApplicable`.
#[test]
fn salvage_is_not_applicable_for_healthy_and_virgin_submodules() {
    let td = tempfile::tempdir().expect("tempdir");
    let sub_dir = td.path().join("upstream");
    std::fs::create_dir_all(&sub_dir).expect("mkdir upstream");
    let url = build_upstream_sub(&sub_dir);

    let root = td.path().join("super");
    std::fs::create_dir_all(&root).expect("mkdir super");
    let _repo = init_repo_with_commit(&root);
    crate::git::submodule::add_submodule(&root, &url, "vendor/sub").expect("add_submodule");

    // (a) HEALTHY: checked out, gitlink present ⇒ not the wedged state.
    let repo = git2::Repository::open(&root).expect("reopen super");
    let sm = repo.find_submodule("vendor/sub").expect("find_submodule");
    assert_eq!(
        reattach_module_gitdir(&repo, &sm, "vendor/sub").expect("salvage healthy"),
        Salvage::NotApplicable,
        "a healthy submodule must be left entirely to libgit2"
    );
    assert!(
        root.join("vendor/sub/.git").exists(),
        "the healthy submodule's gitlink is untouched"
    );

    // (b) VIRGIN: registered but no cached data at all (worktree AND module
    //     gitdir removed) ⇒ a genuine first clone, still NotApplicable.
    drop(sm);
    std::fs::remove_dir_all(root.join("vendor/sub")).expect("rm worktree");
    std::fs::remove_dir_all(root.join(".git/modules/vendor/sub")).expect("rm module gitdir");
    let repo = git2::Repository::open(&root).expect("reopen super");
    let sm = repo.find_submodule("vendor/sub").expect("find_submodule");
    assert_eq!(
        reattach_module_gitdir(&repo, &sm, "vendor/sub").expect("salvage virgin"),
        Salvage::NotApplicable,
        "a never-cloned submodule must be left to libgit2's clone branch"
    );
    assert!(
        !root.join("vendor/sub/.git").exists(),
        "the salvage wrote no gitlink for a virgin submodule"
    );
}
