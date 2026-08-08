//! The `ForgeProvider` trait (overview §F2) — the shared abstraction every
//! forge backend implements. GitHub is the first (and, in v1, only) impl.
//!
//! Only provider-NEUTRAL [`crate::types`] cross this boundary — never a
//! `serde_json::Value` and never a GitHub wire struct. The trait carries the
//! full Phase-4 surface: P62 implements all of it; P63/P64 only wire new IPC
//! to methods that already exist here.

use bonsai_core::error::AppError;

// `ForgeKind` is a provider-neutral DTO; its canonical definition lives in
// `types` (with the wire-shape test). Re-exported here so callers can refer to
// `provider::ForgeKind` alongside the trait, per contract §2a.
pub use crate::types::ForgeKind;
use crate::types::{
    CommitStatus, CreatePrInput, ForgeRepoContext, ForgeViewer, PrDetail, PrListQuery, PrPage,
    ReviewComment,
};

pub trait ForgeProvider: Send + Sync {
    /// Repo identity — known at construction from detection + keychain
    /// presence; no network.
    fn repo_context(&self) -> ForgeRepoContext;

    /// The authenticated user (GitHub `GET /user`). Used by `forge_set_token`
    /// (P62b) to validate a pasted PAT and by P64. Requires a token.
    fn viewer(&self) -> Result<ForgeViewer, AppError>;

    fn list_prs(&self, query: &PrListQuery) -> Result<PrPage, AppError>;
    fn get_pr(&self, number: u64) -> Result<PrDetail, AppError>;
    /// Requires a token ⇒ `ForgeAuthRequired` when none is stored.
    fn create_pr(&self, input: &CreatePrInput) -> Result<PrDetail, AppError>;
    fn list_review_comments(&self, number: u64) -> Result<Vec<ReviewComment>, AppError>;

    /// Defined + implemented in P62; exposed as an IPC command in P63.
    fn combined_status(&self, sha: &str) -> Result<CommitStatus, AppError>;
}
