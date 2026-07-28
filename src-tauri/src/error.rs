/// Application-level error surfaced to the frontend.
///
/// Serialized as `{ "kind": "git" | "io" | "other" | "noRepo" | "emptyMessage"
/// | "configMissing" | "nothingToCommit" | "branchExists" | "invalidName"
/// | "checkoutConflict" | "unmergedBranch" | "branchNotFound",
/// "message": "..." }`.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("git error: {0}")]
    Git(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("{0}")]
    Other(String),
    #[error("no repository is open")]
    NoRepo,
    #[error("commit message is empty")]
    EmptyMessage,
    #[error("{0}")]
    ConfigMissing(String),
    #[error("nothing to commit (index matches HEAD)")]
    NothingToCommit,
    #[error("{0}")]
    BranchExists(String),
    #[error("{0}")]
    InvalidName(String),
    #[error("{0}")]
    CheckoutConflict(String),
    #[error("{0}")]
    UnmergedBranch(String),
    #[error("{0}")]
    BranchNotFound(String),
}

impl AppError {
    fn kind(&self) -> &'static str {
        match self {
            AppError::Git(_) => "git",
            AppError::Io(_) => "io",
            AppError::Other(_) => "other",
            AppError::NoRepo => "noRepo",
            AppError::EmptyMessage => "emptyMessage",
            AppError::ConfigMissing(_) => "configMissing",
            AppError::NothingToCommit => "nothingToCommit",
            AppError::BranchExists(_) => "branchExists",
            AppError::InvalidName(_) => "invalidName",
            AppError::CheckoutConflict(_) => "checkoutConflict",
            AppError::UnmergedBranch(_) => "unmergedBranch",
            AppError::BranchNotFound(_) => "branchNotFound",
        }
    }

    fn message(&self) -> &str {
        match self {
            AppError::Git(m)
            | AppError::Io(m)
            | AppError::Other(m)
            | AppError::ConfigMissing(m)
            | AppError::BranchExists(m)
            | AppError::InvalidName(m)
            | AppError::CheckoutConflict(m)
            | AppError::UnmergedBranch(m)
            | AppError::BranchNotFound(m) => m,
            AppError::NoRepo => "no repository is open",
            AppError::EmptyMessage => "commit message is empty",
            AppError::NothingToCommit => "nothing to commit (index matches HEAD)",
        }
    }
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", self.message())?;
        s.end()
    }
}

impl From<git2::Error> for AppError {
    fn from(e: git2::Error) -> Self {
        AppError::Git(e.message().to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}
