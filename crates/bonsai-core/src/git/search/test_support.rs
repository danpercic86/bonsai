//! Shared test fixtures for the `search` test modules (fake runners, query
//! builders, and the oracle git repos). Extracted verbatim from the former
//! inline `mod tests`; `pub(super)` so both sibling test modules reuse them.

#[allow(unused_imports)]
use super::*;
use std::cell::RefCell;
use std::process::Command;

pub(super) fn have_git() -> bool {
    let ok = Command::new("git").arg("--version").output().is_ok();
    if !ok && std::env::var("BONSAI_REQUIRE_GIT_STRICT").as_deref() == Ok("1") {
        panic!("BONSAI_REQUIRE_GIT_STRICT=1: `git` CLI required on PATH but not found");
    }
    ok
}

// ---------------------------------------------------------- query builders

pub(super) fn q(field: SearchField, text: &str) -> SearchQuery {
    SearchQuery {
        text: text.to_string(),
        field,
        regex: false,
        case_sensitive: false,
        max_results: 0,
        scope_ref: None,
    }
}

// ---------------------------------------------------------- fake runners

/// Records every `run` argv and returns canned stdout — no git launched.
pub(super) struct FakeGitRunner {
    pub(super) stdout: String,
    pub(super) calls: RefCell<Vec<Vec<String>>>,
}
impl FakeGitRunner {
    pub(super) fn new(stdout: &str) -> FakeGitRunner {
        FakeGitRunner {
            stdout: stdout.to_string(),
            calls: RefCell::new(Vec::new()),
        }
    }
}
impl GitRunner for FakeGitRunner {
    fn run(&self, args: &[String], _cwd: &Path) -> Result<String, AppError> {
        self.calls.borrow_mut().push(args.to_vec());
        Ok(self.stdout.clone())
    }
}

/// Panics if ever called — proves empty/whitespace `text` shells out to nothing.
pub(super) struct PanicRunner;
impl GitRunner for PanicRunner {
    fn run(&self, _args: &[String], _cwd: &Path) -> Result<String, AppError> {
        panic!("runner must not be called");
    }
}

// ---------------------------------------------------------- arg building

pub(super) fn base_args(max_count: &str) -> Vec<String> {
    vec![
        "--glob-pathspecs".to_string(),
        "log".to_string(),
        "--format=%H%x1f%s%x1f%an%x1f%at".to_string(),
        "--max-count".to_string(),
        max_count.to_string(),
    ]
}
pub(super) fn record(oid: &str, summary: &str, author: &str, ts: &str) -> String {
    format!("{oid}\u{1f}{summary}\u{1f}{author}\u{1f}{ts}")
}
// ---------------------------------------------------------- oracle fixture

/// Init a `main`-headed repo with a pinned identity + `core.autocrlf=false`
/// (shared by the oracle fixtures so `git log` and the git2 revwalk agree on
/// order and identity).
pub(super) fn init_repo(dir: &Path) -> git2::Repository {
    let repo = git2::Repository::init_opts(
        dir,
        git2::RepositoryInitOptions::new().initial_head("main"),
    )
    .expect("init repo");
    {
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    }
    repo
}

/// One commit built directly from `parent`'s tree + `files`, on HEAD
/// (refs/heads/main), with BOTH author and committer time pinned to `t` so
/// `git log` and the git2 revwalk share one deterministic order.
pub(super) fn mk_commit(
    repo: &git2::Repository,
    parent: Option<git2::Oid>,
    files: &[(&str, &str)],
    msg: &str,
    author: &str,
    t: i64,
) -> git2::Oid {
    let email = format!("{}@example.com", author.to_lowercase().replace(' ', "."));
    let sig = git2::Signature::new(author, &email, &git2::Time::new(t, 0)).expect("sig");
    let parent_commit = parent.map(|p| repo.find_commit(p).expect("parent"));
    let mut tb = match &parent_commit {
        Some(pc) => repo
            .treebuilder(Some(&pc.tree().expect("parent tree")))
            .expect("treebuilder"),
        None => repo.treebuilder(None).expect("treebuilder"),
    };
    for (name, content) in files {
        let blob = repo.blob(content.as_bytes()).expect("blob");
        tb.insert(name, blob, 0o100_644).expect("insert");
    }
    let tree = repo.find_tree(tb.write().expect("write tree")).expect("tree");
    let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
    repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
        .expect("commit")
}

/// Fixture: 4 commits on `main` with distinct timestamps, known messages /
/// authors, and real blob edits; plus an `early` branch at C1 (scope subset).
/// Returns the owning dir + `[c0, c1, c2, c3]`.
pub(super) fn build_fixture() -> (tempfile::TempDir, [git2::Oid; 4]) {
    let dir = crate::testutil::scratch_dir();
    let repo = init_repo(dir.path());
    let c0 = mk_commit(&repo, None, &[("a.txt", "alpha\n")], "grace period work", "Ada Lovelace", 1000);
    let c1 = mk_commit(&repo, Some(c0), &[("b.txt", "beta\n")], "add beta module", "Grace Hopper", 2000);
    let c2 = mk_commit(&repo, Some(c1), &[("a.txt", "alpha and more\n")], "fix alpha work", "Ada Lovelace", 3000);
    let c3 = mk_commit(&repo, Some(c2), &[("c.txt", "gamma\n")], "Feature gamma", "Linus Torvalds", 4000);
    repo.branch("early", &repo.find_commit(c1).expect("c1"), false)
        .expect("branch early");
    (dir, [c0, c1, c2, c3])
}

/// Like [`mk_commit`] but attaches the new commit to NO ref (dangling), so the
/// caller can point a remote-tracking ref or tag at it — letting a fixture put
/// a commit out of reach of every LOCAL branch.
pub(super) fn mk_dangling(
    repo: &git2::Repository,
    parent: git2::Oid,
    files: &[(&str, &str)],
    msg: &str,
    author: &str,
    t: i64,
) -> git2::Oid {
    let email = format!("{}@example.com", author.to_lowercase().replace(' ', "."));
    let sig = git2::Signature::new(author, &email, &git2::Time::new(t, 0)).expect("sig");
    let parent_commit = repo.find_commit(parent).expect("parent");
    let mut tb = repo
        .treebuilder(Some(&parent_commit.tree().expect("parent tree")))
        .expect("treebuilder");
    for (name, content) in files {
        let blob = repo.blob(content.as_bytes()).expect("blob");
        tb.insert(name, blob, 0o100_644).expect("insert");
    }
    let tree = repo.find_tree(tb.write().expect("write tree")).expect("tree");
    repo.commit(None, &sig, &sig, msg, &tree, &[&parent_commit])
        .expect("commit")
}

/// Fixture exercising the FULL `seed_all_refs` seeding (not just local
/// branches): `c_remote` is reachable ONLY via a remote-tracking ref
/// (`refs/remotes/origin/feature`), `c_tag` ONLY via a lightweight tag
/// (`refs/tags/v1.0`), and an `origin/HEAD` symbolic ref is present so the
/// `*/HEAD`-skip branch is exercised (and must not break the walk). All three
/// real commits carry "work" in the message so an all-refs search is
/// cross-checkable against `git log --all --grep=work`. Distinct timestamps
/// (1000/2000/3000) fix a deterministic newest-first order.
/// Returns the owning dir + `[c_base, c_remote, c_tag]`.
pub(super) fn build_refs_fixture() -> (tempfile::TempDir, [git2::Oid; 3]) {
    let dir = crate::testutil::scratch_dir();
    let repo = init_repo(dir.path());
    let c_base = mk_commit(
        &repo,
        None,
        &[("base.txt", "base\n")],
        "base setup work",
        "Ada Lovelace",
        1000,
    );
    // Reachable ONLY via a remote-tracking ref (no local branch points here).
    let c_remote = mk_dangling(
        &repo,
        c_base,
        &[("r.txt", "remote\n")],
        "remote feature work",
        "Ada Lovelace",
        2000,
    );
    repo.reference("refs/remotes/origin/feature", c_remote, false, "seed remote")
        .expect("remote ref");
    // A `*/HEAD` remote ref that seed_all_refs must SKIP (never peeled).
    repo.reference_symbolic(
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/feature",
        false,
        "seed origin/HEAD",
    )
    .expect("origin/HEAD");
    // Reachable ONLY via a tag.
    let c_tag = mk_dangling(
        &repo,
        c_base,
        &[("t.txt", "tag\n")],
        "tagged release work",
        "Ada Lovelace",
        3000,
    );
    let tag_obj = repo
        .find_object(c_tag, Some(git2::ObjectType::Commit))
        .expect("tag object");
    repo.tag_lightweight("v1.0", &tag_obj, false).expect("tag");
    (dir, [c_base, c_remote, c_tag])
}

/// oids our search returns, newest-first.
pub(super) fn our_oids(dir: &Path, query: &SearchQuery) -> Vec<String> {
    search_commits(dir, &SpawnGitRunner, query)
        .expect("search")
        .matches
        .into_iter()
        .map(|m| m.oid)
        .collect()
}

/// `git log …` full oids (newest-first, empty lines dropped).
pub(super) fn cli_oids(dir: &Path, args: &[&str]) -> Vec<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("spawn git log");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

pub(super) fn oid_hex(o: git2::Oid) -> String {
    o.to_string()
}
