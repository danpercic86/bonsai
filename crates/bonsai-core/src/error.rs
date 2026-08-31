/// Application-level error surfaced to the frontend.
///
/// Serialized as `{ "kind": "git" | "io" | "other" | "noRepo" | "emptyMessage"
/// | "configMissing" | "nothingToCommit" | "branchExists" | "invalidName"
/// | "checkoutConflict" | "branchCheckedOutElsewhere"
/// | "unmergedBranch" | "branchNotFound"
/// | "noRemote" | "noUpstream" | "authFailed" | "networkError"
/// | "pushRejected" | "operationInProgress" | "noOperationInProgress"
/// | "unresolvedConflicts" | "aiUnavailable" | "aiFailed" | "aiNeedsReview"
/// | "externalToolFailed" | "hookRejected" | "gitNotFound"
/// | "forgeUnsupported" | "forgeAuthRequired" | "forgeRateLimited" | "forgeApi",
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
    BranchCheckedOutElsewhere(String),
    #[error("{0}")]
    UnmergedBranch(String),
    #[error("{0}")]
    BranchNotFound(String),
    #[error("{0}")]
    NoRemote(String),
    #[error("{0}")]
    NoUpstream(String),
    #[error("{0}")]
    AuthFailed(String),
    #[error("{0}")]
    NetworkError(String),
    #[error("{0}")]
    PushRejected(String),
    #[error("{0}")]
    OperationInProgress(String),
    #[error("{0}")]
    NoOperationInProgress(String),
    #[error("{0}")]
    UnresolvedConflicts(String),
    // AI subprocess layer (P13).
    #[error("{0}")]
    AiUnavailable(String),
    #[error("{0}")]
    AiFailed(String),
    /// P68 #7 / H1: a body the novel-content gate refused to auto-stage — it has ≥1
    /// line present in NO version of base/ours/theirs. Distinct from `AiFailed` so the
    /// frontend routes it to review instead of showing a raw "failed" error toast.
    #[error("{0}")]
    AiNeedsReview(String),
    /// The user cancelled a streaming AI run via `ai_cancel_run` (P68 §B). NOT a
    /// failure — the frontend shows no error toast, only a `cancelled` run state.
    /// Distinct from `AiFailed` so the single catch path can tell them apart.
    #[error("{0}")]
    AiCancelled(String),
    /// External-tool launch failed (P49): no terminal/file-manager/editor
    /// candidate could be spawned. Carries a message naming the last program
    /// tried; the frontend adds a "set a command in Settings" hint.
    #[error("{0}")]
    ExternalToolFailed(String),
    /// A BLOCKING git hook (pre-commit / commit-msg / pre-push) exited non-zero
    /// (P59a). Carries `"<hook> hook failed:\n<combined stdout+stderr>"` so the
    /// frontend can render the hook's own output in a dedicated dialog. A failing
    /// blocking hook is NEVER a silent success — the operation aborts with this.
    #[error("{0}")]
    HookRejected(String),
    /// P70: no runnable `git` executable could be resolved (PATH inherited from
    /// an installer, Git not installed, override pointing nowhere). Distinct
    /// from `Git` so the frontend can show ONE persistent banner instead of N
    /// toasts, and distinct from `AuthFailed` so a launch failure is NEVER
    /// reported as a credential problem. Carries
    /// `gitbin::git_not_found_message()`.
    #[error("{0}")]
    GitNotFound(String),
    // Forge / PR integration (P62). Provider-abstracted; GitHub first.
    /// The `origin` host is not a known forge provider (non-`github.com` or an
    /// unparseable remote URL). A DATA command was invoked against it; the
    /// friendly empty state comes from `forge_repo_context`, not this error.
    #[error("{0}")]
    ForgeUnsupported(String),
    /// The requested forge operation needs a stored PAT but none is present for
    /// the host. Raised BEFORE any network request (e.g. `create_pr` unauthed).
    #[error("{0}")]
    ForgeAuthRequired(String),
    /// The forge API returned a rate-limit response (403 with
    /// `X-RateLimit-Remaining: 0`, or 429). Carries a message including the
    /// `X-RateLimit-Reset` epoch hint when available.
    #[error("{0}")]
    ForgeRateLimited(String),
    /// A forge API call failed with an unexpected status (404, other 4xx/5xx)
    /// or a malformed/unparseable response body. NEVER carries a token or an
    /// `Authorization` header value.
    #[error("{0}")]
    ForgeApi(String),
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
            AppError::BranchCheckedOutElsewhere(_) => "branchCheckedOutElsewhere",
            AppError::UnmergedBranch(_) => "unmergedBranch",
            AppError::BranchNotFound(_) => "branchNotFound",
            AppError::NoRemote(_) => "noRemote",
            AppError::NoUpstream(_) => "noUpstream",
            AppError::AuthFailed(_) => "authFailed",
            AppError::NetworkError(_) => "networkError",
            AppError::PushRejected(_) => "pushRejected",
            AppError::OperationInProgress(_) => "operationInProgress",
            AppError::NoOperationInProgress(_) => "noOperationInProgress",
            AppError::UnresolvedConflicts(_) => "unresolvedConflicts",
            AppError::AiUnavailable(_) => "aiUnavailable",
            AppError::AiFailed(_) => "aiFailed",
            AppError::AiNeedsReview(_) => "aiNeedsReview",
            AppError::AiCancelled(_) => "aiCancelled",
            AppError::ExternalToolFailed(_) => "externalToolFailed",
            AppError::HookRejected(_) => "hookRejected",
            AppError::GitNotFound(_) => "gitNotFound",
            AppError::ForgeUnsupported(_) => "forgeUnsupported",
            AppError::ForgeAuthRequired(_) => "forgeAuthRequired",
            AppError::ForgeRateLimited(_) => "forgeRateLimited",
            AppError::ForgeApi(_) => "forgeApi",
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
            | AppError::BranchCheckedOutElsewhere(m)
            | AppError::UnmergedBranch(m)
            | AppError::BranchNotFound(m)
            | AppError::NoRemote(m)
            | AppError::NoUpstream(m)
            | AppError::AuthFailed(m)
            | AppError::NetworkError(m)
            | AppError::PushRejected(m)
            | AppError::OperationInProgress(m)
            | AppError::NoOperationInProgress(m)
            | AppError::UnresolvedConflicts(m)
            | AppError::AiUnavailable(m)
            | AppError::AiFailed(m)
            | AppError::AiNeedsReview(m)
            | AppError::AiCancelled(m)
            | AppError::ExternalToolFailed(m)
            | AppError::HookRejected(m)
            | AppError::GitNotFound(m)
            | AppError::ForgeUnsupported(m)
            | AppError::ForgeAuthRequired(m)
            | AppError::ForgeRateLimited(m)
            | AppError::ForgeApi(m) => m,
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
