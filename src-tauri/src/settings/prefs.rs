//! Pure display-only preference data types for [`crate::settings`].
//!
//! These carry no logic beyond their `Default` impls; the clamping that guards
//! their numeric ranges lives in [`super::clamp`]. Split out of `settings.rs`
//! to keep whole-file reads cheap (CLAUDE.md file-size discipline). Re-exported
//! from the `settings` module so external call sites are unchanged.

/// Dark or light chrome (P2 contract §2.1). Lane colors are theme-invariant —
/// only chrome (`--bg-*`/`--text-*`/etc.) differs; this enum is purely a UI
/// preference with no effect on Git logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    #[default]
    Dark,
    Light,
}

/// Flat vs tree-grouped list rendering for sidebar refs and file lists
/// (P3b contract §2). Pure UI preference; display-only, no Git effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListView {
    #[default]
    Tree,
    Flat,
}

/// Right-panel vertical density (P67). Pure UI preference; display-only, no Git
/// effect. `Cozy` is the P67b tightened default; `Compact` squeezes rows,
/// paddings and fonts further. INDEPENDENT of `GraphPrefs::compact` (which is
/// canvas row geometry) — Settings only cross-references the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelDensity {
    #[default]
    Cozy,
    Compact,
}

/// Which commit button is emphasized in the Working tab (P80 D1). Pure UI
/// preference; display-only, no Git effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PrimaryCommitAction {
    #[default]
    Commit,
    CommitPush,
}

/// AI conflict-resolution autonomy (P13). ProposeReview = user accepts before
/// anything is written/staged (default); AutoResolve = write+stage immediately,
/// user reviews the staged diff before commit_merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiAutonomy {
    #[default]
    ProposeReview,
    AutoResolve,
}

/// Repo access granted to a streaming conflict-resolution run (P68 §B/D10).
/// `ReadOnly` maps to `--tools "Read,Grep,Glob"` — the model must be able to look
/// at the rest of the repository to resolve a conflict sensibly, which is the
/// actual fix for the "Claude timed out without understanding the app" report:
/// today's run passes `--tools ""` and is BLIND to the repo. `None` reproduces
/// that older behaviour. There is deliberately NO write/edit/bash option: Bonsai
/// writes nothing itself either — staging stays the separate, explicit
/// `resolve_conflict_text` call after review (D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiConflictTools {
    #[default]
    ReadOnly,
    None,
}

/// Persisted sidebar/right-panel widths in px (P2 contract §2.1). Clamped to
/// documented sane bounds on BOTH read (`load_from`) and write (setter
/// commands) — this is the "persisted sanity" bound; the frontend additionally
/// applies a dynamic live-drag clamp against the current window width and the
/// graph pane's 480px floor, which is a deliberately separate check (contract
/// §2.5) not duplicated here.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PaneWidths {
    pub sidebar: u32,
    pub right_panel: u32,
}

impl Default for PaneWidths {
    fn default() -> Self {
        PaneWidths {
            sidebar: 240,
            right_panel: 380,
        }
    }
}

/// Auto-fetch preference (P11). OFF by default; interval in minutes.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutoFetch {
    pub enabled: bool,
    pub interval_minutes: u32,
}

impl Default for AutoFetch {
    fn default() -> Self {
        AutoFetch {
            enabled: false,
            interval_minutes: 5,
        }
    }
}

/// Health-refresh background job preference (P30 D7/D12). OFF by default;
/// interval in minutes. A pure `repo-changed` signal job — no git work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HealthRefresh {
    pub enabled: bool,
    pub interval_minutes: u32,
}

impl Default for HealthRefresh {
    fn default() -> Self {
        HealthRefresh {
            enabled: false,
            interval_minutes: 30,
        }
    }
}

/// Which timestamp the graph's date column + relative/absolute date use (P51).
/// Pure UI preference; no Git effect. `Author` is the M2 baseline behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphDateBasis {
    #[default]
    Author,
    Committer,
}

/// Graph geometry knobs + per-row detail toggles (P11/P51). Geometry defaults
/// EQUAL the frontend METRICS defaults (avatar 10 / row 32 / lane 16) — the
/// "no override" baseline. Every P51 toggle is `#[serde(default)]` (via the
/// container-level `default`) so an OLD settings.json without them still
/// deserializes, falling back to the sensible defaults below. `dot_radius` was
/// removed (P51 D7 — a dead no-op field); an old file carrying `dotRadius` is
/// ignored (serde has no `deny_unknown_fields`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GraphPrefs {
    pub avatar_radius: u32,
    pub row_height: u32,
    pub lane_width: u32,
    /// P51: show the short-SHA column (+ verified-badge slot). Default true.
    pub show_sha: bool,
    /// P51: show the optional full author-NAME text column. Default false
    /// (the avatar already conveys author; the name is the clutter-iest column).
    pub show_author: bool,
    /// P51: show the date column. Default true (M2 baseline showed it always).
    pub show_date: bool,
    /// P51: which timestamp the date column/tooltip use. Default Author.
    pub date_basis: GraphDateBasis,
    /// P51: ahead/behind chip on local-branch-tip pills. Default true (renders
    /// only on diverged branches — low clutter, high value).
    pub show_ahead_behind: bool,
    /// P51: compact (denser) rows preset. Default false.
    pub compact: bool,
    /// P58c: light the per-row signature badge (verified/unverified/unknown)
    /// from `verify_commits`. Default true. When false the P51 faint stub
    /// renders unchanged AND no verification is requested (clutter principle;
    /// individually toggleable, like every other detail column).
    pub show_signature_badge: bool,
    /// P63: PR-state badge on branch-tip pills. Default false — forge signals
    /// need a network round-trip AND a stored PAT, so they are inert (a dead
    /// toggle firing surprise API calls) without a connected forge; opt-in.
    pub show_pr_badge: bool,
    /// P63: CI/build-status dot on branch-tip pills. Default false (same
    /// network+auth gating as `show_pr_badge`).
    pub show_ci_status: bool,
}

impl Default for GraphPrefs {
    fn default() -> Self {
        GraphPrefs {
            avatar_radius: 10,
            row_height: 32,
            lane_width: 16,
            show_sha: true,
            show_author: false,
            show_date: true,
            date_basis: GraphDateBasis::Author,
            show_ahead_behind: true,
            compact: false,
            show_signature_badge: true,
            show_pr_badge: false,
            show_ci_status: false,
        }
    }
}
