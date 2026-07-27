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
/// M0a stub: reports every existing directory as "not a repo". The real
/// implementation (git2 open + HEAD inspection per contract §4) lands in M0b.
pub fn read_repo_info(path: &std::path::Path) -> Result<RepoInfo, AppError> {
    if !path.is_dir() {
        return Err(AppError::Io(format!(
            "path does not exist or is not a directory: {}",
            path.display()
        )));
    }
    Ok(RepoInfo {
        path: path.to_string_lossy().into_owned(),
        is_repo: false,
        head: None,
    })
}
