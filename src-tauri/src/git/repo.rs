use crate::error::AppError;

/// State of a repository's HEAD.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadInfo {
    /// `None` when detached or unborn.
    pub branch_name: Option<String>,
    /// Full 40-char hex; `""` when unborn.
    pub oid: String,
    pub detached: bool,
    pub unborn: bool,
}

/// Result of opening a folder as a repository.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    /// Canonical workdir path as passed in.
    pub path: String,
    pub is_repo: bool,
    /// `None` iff `is_repo == false`.
    pub head: Option<HeadInfo>,
}

/// Blocking. Reads repository info for `path`.
///
/// Opens the repo with `NO_SEARCH` (no walking up parent directories), so a
/// subdirectory of a repo reports `is_repo: false`. A directory that is not a
/// repo returns `Ok(RepoInfo { is_repo: false, head: None })`, not `Err`;
/// `Err` is reserved for real failures (missing path / not a directory / IO).
pub fn read_repo_info(path: &std::path::Path) -> Result<RepoInfo, AppError> {
    if !path.is_dir() {
        return Err(AppError::Io(format!(
            "path does not exist or is not a directory: {}",
            path.display()
        )));
    }

    let path_str = path.to_string_lossy().into_owned();

    let repo = match git2::Repository::open_ext(
        path,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    ) {
        Ok(repo) => repo,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Ok(RepoInfo {
                path: path_str,
                is_repo: false,
                head: None,
            });
        }
        Err(e) => return Err(e.into()),
    };

    let head = read_head_info(&repo)?;
    Ok(RepoInfo {
        path: path_str,
        is_repo: true,
        head: Some(head),
    })
}

/// Inspects HEAD of an opened repository.
fn read_head_info(repo: &git2::Repository) -> Result<HeadInfo, AppError> {
    let head = match repo.head() {
        Ok(head) => head,
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
            // Unborn HEAD: read the branch name from the symbolic target of HEAD.
            let head_ref = repo.find_reference("HEAD")?;
            let branch_name = head_ref
                .symbolic_target()
                .map(|t| t.strip_prefix("refs/heads/").unwrap_or(t).to_string());
            return Ok(HeadInfo {
                branch_name,
                oid: String::new(),
                detached: false,
                unborn: true,
            });
        }
        Err(e) => return Err(e.into()),
    };

    let detached = repo.head_detached()?;
    let oid = head
        .target()
        .map(|o| o.to_string())
        .ok_or_else(|| AppError::Git("HEAD has no target commit".to_string()))?;
    let branch_name = if detached {
        None
    } else {
        head.shorthand().map(|s| s.to_string())
    };

    Ok(HeadInfo {
        branch_name,
        oid,
        detached,
        unborn: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Initializes a repo in a fresh temp dir with local user config set.
    fn init_repo() -> (tempfile::TempDir, git2::Repository) {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        {
            let mut config = repo.config().expect("open config");
            config.set_str("user.name", "Test User").expect("set name");
            config
                .set_str("user.email", "test@example.com")
                .expect("set email");
        }
        (dir, repo)
    }

    /// Writes a file, stages it, and creates an initial commit. Returns its oid.
    fn commit_file(repo: &git2::Repository) -> git2::Oid {
        let workdir = repo.workdir().expect("workdir");
        std::fs::write(workdir.join("hello.txt"), "hello bonsai\n").expect("write file");

        let mut index = repo.index().expect("open index");
        index
            .add_path(std::path::Path::new("hello.txt"))
            .expect("stage file");
        index.write().expect("write index");
        let tree_oid = index.write_tree().expect("write tree");
        let tree = repo.find_tree(tree_oid).expect("find tree");

        let sig = git2::Signature::now("Test User", "test@example.com").expect("signature");
        repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
            .expect("commit")
    }

    /// Reads the default branch name from the fixture's symbolic HEAD target.
    fn head_branch_name(repo: &git2::Repository) -> String {
        let head_ref = repo.find_reference("HEAD").expect("find HEAD");
        let target = head_ref.symbolic_target().expect("symbolic HEAD");
        target
            .strip_prefix("refs/heads/")
            .unwrap_or(target)
            .to_string()
    }

    #[test]
    fn repo_with_one_commit() {
        let (dir, repo) = init_repo();
        let expected_branch = head_branch_name(&repo);
        let oid = commit_file(&repo);

        let info = read_repo_info(dir.path()).expect("read_repo_info");
        assert!(info.is_repo);
        let head = info.head.expect("head present");
        assert!(!head.unborn);
        assert!(!head.detached);
        assert_eq!(head.branch_name.as_deref(), Some(expected_branch.as_str()));
        assert_eq!(head.oid, oid.to_string());
    }

    #[test]
    fn non_repo_dir() {
        let dir = tempfile::TempDir::new().expect("create temp dir");

        let info = read_repo_info(dir.path()).expect("read_repo_info");
        assert!(!info.is_repo);
        assert!(info.head.is_none());
    }

    #[test]
    fn unborn_head() {
        let (dir, _repo) = init_repo();

        let info = read_repo_info(dir.path()).expect("read_repo_info");
        assert!(info.is_repo);
        let head = info.head.expect("head present");
        assert!(head.unborn);
        assert!(!head.detached);
        assert_eq!(head.oid, "");
        assert!(head.branch_name.is_some());
    }

    #[test]
    fn detached_head() {
        let (dir, repo) = init_repo();
        let oid = commit_file(&repo);
        repo.set_head_detached(oid).expect("detach HEAD");

        let info = read_repo_info(dir.path()).expect("read_repo_info");
        assert!(info.is_repo);
        let head = info.head.expect("head present");
        assert!(head.detached);
        assert!(!head.unborn);
        assert_eq!(head.branch_name, None);
        assert_eq!(head.oid, oid.to_string());
    }

    #[test]
    fn missing_path() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let missing = dir.path().join("does-not-exist");

        let err = read_repo_info(&missing).expect_err("must be Err");
        assert!(matches!(err, AppError::Io(_)));
    }

    #[test]
    fn subdirectory_of_repo_is_not_a_repo() {
        let (dir, repo) = init_repo();
        commit_file(&repo);
        let subdir = dir.path().join("nested");
        std::fs::create_dir(&subdir).expect("create subdir");

        let info = read_repo_info(&subdir).expect("read_repo_info");
        assert!(!info.is_repo);
        assert!(info.head.is_none());
    }
}
