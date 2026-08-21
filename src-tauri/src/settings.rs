//! App settings persistence (P1 contract §3.1).
//!
//! Hand-rolled `settings.json` under the app config dir — no `tauri-plugin-store`
//! (one tiny struct, no new capability surface). All file functions are
//! path-parameterized so they stay unit-testable without an `AppHandle`; only
//! [`settings_file`] touches Tauri.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use bonsai_core::error::AppError;

/// Serializes every load→mutate→save cycle in this process (audit §2.3).
///
/// `save_to` is atomic on its own, but two concurrent [`update`] cycles on
/// different blocking threads (e.g. a pane-width drag save racing the MCP
/// token persist) would each load, mutate their own field, and rename — the
/// last rename silently reverting the other's field. Pure reads (`load_from`)
/// stay lock-free: they never write, and a torn read is impossible through the
/// atomic rename.
static SETTINGS_IO: Mutex<()> = Mutex::new(());

pub const MAX_RECENT_REPOS: usize = 10;
pub const SETTINGS_VERSION: u32 = 1;

/// One recently-opened repository.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentRepo {
    /// Absolute workdir path as reported by `read_repo_info` (canonical root).
    pub path: String,
    /// Seconds since epoch (UTC) of the last successful open.
    pub last_opened: i64,
}

/// One named identity profile (P44). Global app setting; applied to a repo's
/// Local git config on demand. `id` is a stable frontend-generated UUID.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityProfile {
    /// Stable id (frontend-generated `crypto.randomUUID()`); never reused.
    pub id: String,
    /// Display label, e.g. "Work". Empty/duplicate allowed but discouraged
    /// (frontend soft-validates non-empty).
    pub label: String,
    pub user_name: String,
    pub user_email: String,
    /// Optional `user.signingkey`. None/empty ⇒ not written on apply.
    pub signing_key: Option<String>,
}

/// P79: one forge host Bonsai has stored a PAT for. The keychain is the store of
/// record for the token; this index only remembers WHICH hosts exist (the
/// keychain can't be enumerated portably) plus a display hint. NEVER holds the
/// token.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeHostRecord {
    /// Lowercased host, e.g. "github.com". Keychain account key.
    pub host: String,
    /// Provider kind, so add-for-host / list can pick the right API without a
    /// repo. Serialized as `bonsai_forge::ForgeKind` camelCase ("gitHub" | ...).
    pub kind: bonsai_forge::ForgeKind,
    /// Last-known login for offline display (avatar is fetched fresh / from the
    /// viewer cache; not persisted). `None` until first successful validation.
    pub login: Option<String>,
}

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

pub const SIDEBAR_MIN: u32 = 180;
pub const SIDEBAR_MAX: u32 = 480;
pub const RIGHT_PANEL_MIN: u32 = 280;
pub const RIGHT_PANEL_MAX: u32 = 640;

/// Clamps to the documented ranges; called by both `load_from` (defend
/// against a hand-edited file) and the setter commands (defend against a
/// future UI bug).
pub fn clamp_pane_widths(w: PaneWidths) -> PaneWidths {
    PaneWidths {
        sidebar: w.sidebar.clamp(SIDEBAR_MIN, SIDEBAR_MAX),
        right_panel: w.right_panel.clamp(RIGHT_PANEL_MIN, RIGHT_PANEL_MAX),
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

pub const HEALTH_REFRESH_INTERVAL_MIN: u32 = 1;
pub const HEALTH_REFRESH_INTERVAL_MAX: u32 = 240;

/// Clamps the health-refresh interval to its documented range; called by both
/// `load_from` (defend a hand-edited file) and the setter command (P30 D12,
/// mirrors [`clamp_auto_fetch`]).
pub fn clamp_health_refresh(h: HealthRefresh) -> HealthRefresh {
    HealthRefresh {
        enabled: h.enabled,
        interval_minutes: h
            .interval_minutes
            .clamp(HEALTH_REFRESH_INTERVAL_MIN, HEALTH_REFRESH_INTERVAL_MAX),
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

pub const AUTO_FETCH_INTERVAL_MIN: u32 = 1;
pub const AUTO_FETCH_INTERVAL_MAX: u32 = 120;
pub const AVATAR_RADIUS_MIN: u32 = 6;
pub const AVATAR_RADIUS_MAX: u32 = 16;
pub const ROW_HEIGHT_MIN: u32 = 24;
pub const ROW_HEIGHT_MAX: u32 = 48;
pub const LANE_WIDTH_MIN: u32 = 10;
pub const LANE_WIDTH_MAX: u32 = 28;

/// Clamps the auto-fetch interval to its documented range; called by both
/// `load_from` (defend a hand-edited file) and the setter command.
pub fn clamp_auto_fetch(a: AutoFetch) -> AutoFetch {
    AutoFetch {
        enabled: a.enabled,
        interval_minutes: a
            .interval_minutes
            .clamp(AUTO_FETCH_INTERVAL_MIN, AUTO_FETCH_INTERVAL_MAX),
    }
}

/// Clamps each graph geometry knob to its documented range; called by both
/// `load_from` (defend a hand-edited file) and the setter command. The P51
/// detail toggles + `date_basis` have no numeric range — they pass through
/// unclamped via struct-update (`..g`). Keep the `..g`: dropping it would
/// silently reset every toggle to its field default on every load/save.
pub fn clamp_graph_prefs(g: GraphPrefs) -> GraphPrefs {
    GraphPrefs {
        avatar_radius: g.avatar_radius.clamp(AVATAR_RADIUS_MIN, AVATAR_RADIUS_MAX),
        row_height: g.row_height.clamp(ROW_HEIGHT_MIN, ROW_HEIGHT_MAX),
        lane_width: g.lane_width.clamp(LANE_WIDTH_MIN, LANE_WIDTH_MAX),
        ..g // toggles + date_basis pass through unclamped
    }
}

// ---- P68 §8.3: streaming AI-run ranges. `0` is a documented SENTINEL for the
// first two (watchdog disabled / no hard cap) and for the budget (no flag), so the
// clamps are "0 or in range", never a plain `clamp` that would silently turn an
// intentional 0 into a minimum.
pub const AI_IDLE_TIMEOUT_DEFAULT: u32 = 300;
pub const AI_IDLE_TIMEOUT_MIN: u32 = 30;
pub const AI_IDLE_TIMEOUT_MAX: u32 = 3600;
pub const AI_HARD_CAP_MIN: u32 = 60;
pub const AI_HARD_CAP_MAX: u32 = 86_400;
pub const AI_MAX_TURNS_MIN: u32 = 1;
pub const AI_MAX_TURNS_MAX: u32 = 20;
pub const AI_BULK_MAX_BYTES_DEFAULT: u32 = 400_000;
pub const AI_BULK_MAX_BYTES_MIN: u32 = 20_000;
pub const AI_BULK_MAX_BYTES_MAX: u32 = 4_000_000;
pub const AI_MAX_BUDGET_USD_MAX: f64 = 100.0;
pub const AI_DOCK_HEIGHT_DEFAULT: u32 = 180;
pub const AI_DOCK_HEIGHT_MIN: u32 = 120;
pub const AI_DOCK_HEIGHT_MAX: u32 = 600;

/// Clamps the P68 streaming-AI knobs to their documented ranges; called by both
/// `load_from` (defend a hand-edited file) and `apply_patch` (defend a future UI
/// bug), exactly like `clamp_pane_widths` / `clamp_graph_prefs`.
///
/// Mutates in place rather than returning a struct, because these are ten
/// TOP-LEVEL fields (A8) rather than one nested preference object.
pub fn clamp_ai_settings(s: &mut Settings) {
    if s.ai_idle_timeout_secs != 0 {
        s.ai_idle_timeout_secs =
            s.ai_idle_timeout_secs.clamp(AI_IDLE_TIMEOUT_MIN, AI_IDLE_TIMEOUT_MAX);
    }
    if s.ai_hard_cap_secs != 0 {
        s.ai_hard_cap_secs = s.ai_hard_cap_secs.clamp(AI_HARD_CAP_MIN, AI_HARD_CAP_MAX);
    }
    s.ai_max_turns = s.ai_max_turns.clamp(AI_MAX_TURNS_MIN, AI_MAX_TURNS_MAX);
    s.ai_bulk_max_bytes =
        s.ai_bulk_max_bytes.clamp(AI_BULK_MAX_BYTES_MIN, AI_BULK_MAX_BYTES_MAX);
    // NaN/inf would poison the `{:.4}` argv formatting, and a negative budget is
    // meaningless — both collapse to "no cap".
    if !s.ai_max_budget_usd.is_finite() || s.ai_max_budget_usd < 0.0 {
        s.ai_max_budget_usd = 0.0;
    } else if s.ai_max_budget_usd > AI_MAX_BUDGET_USD_MAX {
        s.ai_max_budget_usd = AI_MAX_BUDGET_USD_MAX;
    }
    s.ai_dock_height = s.ai_dock_height.clamp(AI_DOCK_HEIGHT_MIN, AI_DOCK_HEIGHT_MAX);
}

/// On-disk settings wire format:
/// `{ "version": 1, "recentRepos": [ { "path": "...", "lastOpened": 0 } ],
///    "theme": "dark", "paneWidths": { "sidebar": 240, "rightPanel": 380 },
///    "listView": "tree", "aiEnabled": true, "aiConflictAutonomy":
///    "proposeReview", "aiConsented": false }`.
///
/// `SETTINGS_VERSION` stays `1`: `theme`, `pane_widths`, `list_view`,
/// `panel_density`, `open_repos`, `active_repo`, `auto_fetch`, `graph`,
/// `ai_enabled`, `ai_conflict_autonomy`, `ai_consented`, and the P68 streaming-AI
/// run knobs (`ai_idle_timeout_secs`, `ai_hard_cap_secs`, `ai_max_turns`,
/// `ai_stream_log`, `ai_include_partial_messages`, `ai_conflict_tools`,
/// `ai_bulk_max_bytes`, `ai_max_budget_usd`, `ai_dock_height`,
/// `ai_dock_collapsed`) are all additive
/// `#[serde(default)]` fields
/// (on the whole struct already, via the container-level `default`) — an old
/// settings.json containing only `recentRepos` deserializes fine, missing
/// fields fall back to their type defaults. No migration code is needed. A
/// future genuine breaking change (e.g. renaming/removing a field with no safe
/// default) IS when a version bump becomes necessary — this precedent documents
/// the bar for that.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub version: u32,
    pub recent_repos: Vec<RecentRepo>,
    pub theme: ThemeChoice,
    pub pane_widths: PaneWidths,
    pub list_view: ListView,
    /// P67: right-panel vertical density. Additive `#[serde(default)]` (via the
    /// container-level `default`); a pre-P67 settings.json without this key loads
    /// `PanelDensity::default()` (Cozy). No version bump — meets the documented
    /// bar above. NOT clamped (no numeric range; `clamp_graph_prefs` untouched).
    pub panel_density: PanelDensity,
    /// Open tabs, in display order (repoIds == canonical workdir paths).
    /// Additive (P3e §6.1); a legacy file without this key loads as empty.
    pub open_repos: Vec<String>,
    /// The active tab's repoId; `None` ⇒ activate the first still-openable one.
    /// Additive (P3e §6.1); a legacy file without this key loads as `None`.
    pub active_repo: Option<String>,
    /// Auto-fetch preference (P11). Additive `#[serde(default)]`; a legacy file
    /// without this key loads with `AutoFetch::default()`.
    pub auto_fetch: AutoFetch,
    /// Health-refresh background job (P30). Additive `#[serde(default)]`; a
    /// legacy file without this key loads with `HealthRefresh::default()`.
    pub health_refresh: HealthRefresh,
    /// Graph geometry knobs (P11). Additive `#[serde(default)]`; a legacy file
    /// without this key loads with `GraphPrefs::default()`.
    pub graph: GraphPrefs,
    /// AI features master toggle (P13). Defaults `true`, but the consent gate
    /// (`ai_consented`) is what actually unlocks the feature. Additive
    /// `#[serde(default)]`; a legacy file without this key loads as `true`.
    pub ai_enabled: bool,
    /// AI conflict-resolution autonomy (P13). Additive `#[serde(default)]`; a
    /// legacy file without this key loads with `AiAutonomy::default()`
    /// (ProposeReview).
    pub ai_conflict_autonomy: AiAutonomy,
    /// One-time consent to send repo content to the local Claude CLI (P13).
    /// Defaults `false`; additive `#[serde(default)]`; a legacy file without
    /// this key loads as `false`.
    pub ai_consented: bool,
    /// Embedded MCP server enabled (P16). Default `false`. Auto-started at
    /// launch ONLY when this persisted flag is true (P44a — the user opted in
    /// previously); still never started without that prior explicit opt-in.
    pub mcp_enabled: bool,
    /// Embedded MCP write-gate (P16). Default `false`. P16b forces the running
    /// server read-only regardless; P16c wires this to (re)register write tools.
    pub mcp_allow_write: bool,
    /// One-time consent to expose open repos to an external MCP client for
    /// READING (P16). Defaults `false`; additive `#[serde(default)]`.
    pub mcp_consented: bool,
    /// One-time consent to let an external MCP client MODIFY open repos (P16c).
    /// A strictly stronger grant than `mcp_consented` (read) — kept as its own
    /// flag so enabling write requires its own explicit confirmation and a
    /// read-only consent never silently implies write. Defaults `false`;
    /// additive `#[serde(default)]`.
    pub mcp_write_consented: bool,
    /// Persisted bound port for the embedded MCP server (P16 §8.5, D-4).
    /// `None` until first enable; preferred on later runs (ephemeral fallback).
    pub mcp_port: Option<u16>,
    /// Persisted bearer token for the embedded MCP server (P16 §8.2, D-4).
    /// Generated on first enable and reused across runs so the user's
    /// `claude mcp add` line keeps working. `None` until first enable.
    pub mcp_token: Option<String>,
    /// P43: first-run onboarding shown+dismissed. Additive `#[serde(default)]`;
    /// a legacy settings.json without this key loads as `false` (⇒ show once).
    pub onboarding_seen: bool,
    /// P42 D4: auto-check for updates on launch. Default `false` (privacy — no
    /// surprise outbound call before opt-in). Additive `#[serde(default)]`; a
    /// legacy file without this key loads as `false`.
    pub auto_check_updates: bool,
    /// P44: named identity profiles (global). Additive `#[serde(default)]`; a
    /// legacy file without this key loads as an empty Vec.
    pub profiles: Vec<IdentityProfile>,
    /// P79: forge hosts with a stored PAT (known-hosts index; the keychain is the
    /// source of record for the token). Additive `#[serde(default)]` ⇒ a pre-P79
    /// file loads `[]`. Stores only host + kind + optional last-known login,
    /// NEVER a token.
    pub forge_hosts: Vec<ForgeHostRecord>,
    /// P49: terminal launch command template (`{path}` placeholder). Empty ⇒
    /// per-OS auto-detect (see `bonsai_core::external`). Additive
    /// `#[serde(default)]` ⇒ a pre-P49 file loads `""`.
    pub terminal_command: String,
    /// P49: editor launch command template (`{path}` placeholder). Empty ⇒
    /// auto-detect the VS Code family. Additive `#[serde(default)]` ⇒ a pre-P49
    /// file loads `""`.
    pub editor_command: String,
    // ---- P68 §8.3: streaming AI-run knobs. All additive `#[serde(default)]`
    // (via the container-level `default`), all clamped by `clamp_ai_settings`, NO
    // version bump — a pre-P68 settings.json loads every one of them at its
    // default (asserted by `old_settings_file_without_ai_run_fields_loads_defaults`).
    /// Seconds with NO output from the CLI before a streaming run is killed.
    /// `0` disables the watchdog. Streaming has no wall-clock deadline by design
    /// (the user cancels instead — D3/D7), so this is the only automatic reaper.
    pub ai_idle_timeout_secs: u32,
    /// Absolute cap on one streaming run, in seconds. **`0` = UNBOUNDED, and that
    /// is the shipped default** (the locked user decision: no hard timeout +
    /// Cancel). Paused while the run awaits a human answer (D3).
    pub ai_hard_cap_secs: u32,
    /// Max `result` lines (turns) before a still-questioning model is failed
    /// (P68 §B rule 3). Shares `bonsai_core::ai::DEFAULT_MAX_TURNS` with
    /// `RunLimits::default()` rather than repeating the literal.
    pub ai_max_turns: u32,
    /// Stream the CLI's log lines to the dock. `false` suppresses `Log` events at
    /// the SOURCE, so turning it off costs no IPC (§8.3).
    pub ai_stream_log: bool,
    /// `--include-partial-messages`. Default OFF: the delta shape is unverified
    /// (spike §1.8) and unknown lines only degrade to `log`.
    pub ai_include_partial_messages: bool,
    /// Repo access for a conflict run (D10). Default `ReadOnly`.
    pub ai_conflict_tools: AiConflictTools,
    /// Payload byte cap that triggers batch SPLITTING for a bulk resolve (§6.3).
    /// Never a truncation point — a request too big for one payload is split into
    /// sequential batches, and a single file that alone exceeds it is reported as
    /// an individual failure.
    pub ai_bulk_max_bytes: u32,
    /// `--max-budget-usd`. **`0.0` = NO CAP, the shipped default** (the locked
    /// decision: opt-in, because a surprise mid-run stop is worse than a run the
    /// user can see and cancel). The flag is passed ONLY when > 0.
    pub ai_max_budget_usd: f64,
    /// Persisted px height of the AI activity dock (P68 §E, A8: a top-level
    /// field, NOT a member of `PaneWidths` — that struct is about widths).
    pub ai_dock_height: u32,
    /// Persisted collapsed state of the AI activity dock (P68 §E).
    pub ai_dock_collapsed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: SETTINGS_VERSION,
            recent_repos: Vec::new(),
            theme: ThemeChoice::default(),
            pane_widths: PaneWidths::default(),
            list_view: ListView::default(),
            panel_density: PanelDensity::default(),
            open_repos: Vec::new(),
            active_repo: None,
            auto_fetch: AutoFetch::default(),
            health_refresh: HealthRefresh::default(),
            graph: GraphPrefs::default(),
            ai_enabled: true,
            ai_conflict_autonomy: AiAutonomy::default(),
            ai_consented: false,
            mcp_enabled: false,
            mcp_allow_write: false,
            mcp_consented: false,
            mcp_write_consented: false,
            mcp_port: None,
            mcp_token: None,
            onboarding_seen: false,
            auto_check_updates: false,
            profiles: Vec::new(),
            forge_hosts: Vec::new(),
            terminal_command: String::new(),
            editor_command: String::new(),
            ai_idle_timeout_secs: AI_IDLE_TIMEOUT_DEFAULT,
            // 0 = unbounded (locked user decision).
            ai_hard_cap_secs: 0,
            ai_max_turns: bonsai_core::ai::DEFAULT_MAX_TURNS,
            ai_stream_log: true,
            ai_include_partial_messages: false,
            ai_conflict_tools: AiConflictTools::default(),
            ai_bulk_max_bytes: AI_BULK_MAX_BYTES_DEFAULT,
            // 0.0 = no `--max-budget-usd` flag at all (locked user decision).
            ai_max_budget_usd: 0.0,
            ai_dock_height: AI_DOCK_HEIGHT_DEFAULT,
            ai_dock_collapsed: false,
        }
    }
}

/// Loads settings from `file`. Missing file, unreadable file, or unparseable
/// JSON all yield `Settings::default()` — settings are best-effort and this
/// NEVER errors (P1 contract §3.1).
pub fn load_from(file: &Path) -> Settings {
    let mut s: Settings = match std::fs::read_to_string(file) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Settings::default(),
    };
    // Defends against a hand-edited or future-version file with out-of-range
    // values (contract §2.1).
    s.pane_widths = clamp_pane_widths(s.pane_widths);
    s.auto_fetch = clamp_auto_fetch(s.auto_fetch);
    s.health_refresh = clamp_health_refresh(s.health_refresh);
    s.graph = clamp_graph_prefs(s.graph);
    clamp_ai_settings(&mut s);
    s
}

/// Loads, mutates, and saves settings as ONE serialized transaction (audit
/// §2.3): the process-wide [`SETTINGS_IO`] lock is held across the whole
/// load→mutate→save cycle so concurrent writers can never revert each other's
/// fields. ALL read-modify-write callers must go through this; `load_from`
/// alone remains fine for pure reads. Returns the saved settings so callers
/// can derive their response without a second (unlocked) read.
pub fn update(file: &Path, mutate: impl FnOnce(&mut Settings)) -> Result<Settings, AppError> {
    // Poison recovery: a panicking mutator leaves the settings FILE untouched
    // (the save never ran), so later updates are safe to proceed.
    let _io = SETTINGS_IO
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut s = load_from(file);
    mutate(&mut s);
    save_to(file, &s)?;
    Ok(s)
}

/// Like [`update`], but skips the (temp-write + atomic-rename) save when the
/// mutator reports no change by returning `false`. For hot read paths that only
/// *occasionally* mutate — e.g. the P79 lazy backfill on `forge_repo_context`,
/// which would otherwise rewrite `settings.json` on every panel open. Holds the
/// same [`SETTINGS_IO`] lock across the whole cycle.
pub fn update_if(
    file: &Path,
    mutate: impl FnOnce(&mut Settings) -> bool,
) -> Result<Settings, AppError> {
    let _io = SETTINGS_IO
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut s = load_from(file);
    if mutate(&mut s) {
        save_to(file, &s)?;
    }
    Ok(s)
}

/// Saves settings to `file` atomically: creates parent dirs, writes pretty
/// JSON to a uniquely-named `<file>.<pid>.<n>.tmp` (same volume), then renames
/// over `file`. On Windows `std::fs::rename` replaces an existing destination
/// file. The tmp name carries the process id + a process-local counter so
/// concurrent PROCESSES (the in-process serialization is [`update`]'s job)
/// can never collide on one tmp path (audit §2.3).
pub fn save_to(file: &Path, s: &Settings) -> Result<(), AppError> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Io(format!("create settings dir {}: {e}", parent.display())))?;
    }
    let json = serde_json::to_string_pretty(s)
        .map_err(|e| AppError::Io(format!("serialize settings: {e}")))?;

    static TMP_SEQ: AtomicU64 = AtomicU64::new(0);
    let mut tmp_name = file.as_os_str().to_owned();
    tmp_name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        TMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let tmp = PathBuf::from(tmp_name);

    std::fs::write(&tmp, json)
        .map_err(|e| AppError::Io(format!("write {}: {e}", tmp.display())))?;
    std::fs::rename(&tmp, file).map_err(|e| {
        // Best-effort cleanup so a failed rename doesn't leave the tmp behind.
        let _ = std::fs::remove_file(&tmp);
        AppError::Io(format!(
            "rename {} -> {}: {e}",
            tmp.display(),
            file.display()
        ))
    })?;
    Ok(())
}

/// `<app_config_dir>/settings.json` (resolves under `%APPDATA%/com.bonsai.app`
/// on Windows).
pub fn settings_file(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Other(format!("cannot resolve app config dir: {e}")))?;
    Ok(dir.join("settings.json"))
}

/// Upserts `path` at the front of the recents list, stamping `last_opened`,
/// and truncating to [`MAX_RECENT_REPOS`].
///
/// Dedupe goes through [`crate::commands::same_repo_path`] — the same
/// `fs::canonicalize` compare the open-repo scan uses — so the recents list and
/// the repo-registry agree on what "the same repo" is. The previous
/// `eq_ignore_ascii_case` was wrong in BOTH directions: it missed non-ASCII
/// case variants and 8.3 short names on Windows, and on a case-SENSITIVE
/// filesystem (ext4) it merged `/home/u/Repo` and `/home/u/repo`, which are two
/// genuinely different repositories there.
///
/// `same_repo_path` falls back to the old ASCII-case-insensitive compare
/// whenever either side cannot be canonicalized (a recents entry whose folder
/// was deleted or is on a detached drive), so no entry is ever silently dropped
/// and the previous behaviour is preserved for unresolvable paths. This is the
/// one non-pure step in this module; everything it needs is still injectable in
/// tests because unresolvable temp paths take the string-compare branch.
pub fn record_recent(s: &mut Settings, path: &str, now: i64) {
    s.recent_repos
        .retain(|r| !crate::commands::same_repo_path(&r.path, path));
    s.recent_repos.insert(
        0,
        RecentRepo {
            path: path.to_string(),
            last_opened: now,
        },
    );
    s.recent_repos.truncate(MAX_RECENT_REPOS);
}

/// P79: insert-or-replace the known-hosts index record for `host` (keyed by the
/// lowercased host). Called after every successful set-token (per-repo and
/// host-based). NEVER stores a token — only host + kind + optional login.
pub fn upsert_forge_host(
    s: &mut Settings,
    host: &str,
    kind: bonsai_forge::ForgeKind,
    login: Option<String>,
) {
    let host = host.to_ascii_lowercase();
    if host.is_empty() {
        return;
    }
    s.forge_hosts.retain(|r| r.host != host);
    s.forge_hosts
        .insert(0, ForgeHostRecord { host, kind, login });
}

/// P79: remove the known-hosts index record for `host`. Called after every
/// clear-token (per-repo and host-based). No-op when absent.
pub fn remove_forge_host(s: &mut Settings, host: &str) {
    let host = host.to_ascii_lowercase();
    s.forge_hosts.retain(|r| r.host != host);
}

/// P79 lazy backfill (OD-1): add a record for `host` ONLY when it is absent from
/// the index (a token exists in the keychain but was stored pre-P79 / by another
/// path). Does not clobber an existing record's login. Returns `true` if it
/// inserted, so the caller can skip the write when nothing changed.
pub fn backfill_forge_host(
    s: &mut Settings,
    host: &str,
    kind: bonsai_forge::ForgeKind,
    login: Option<String>,
) -> bool {
    let host = host.to_ascii_lowercase();
    if host.is_empty() || s.forge_hosts.iter().any(|r| r.host == host) {
        return false;
    }
    s.forge_hosts
        .insert(0, ForgeHostRecord { host, kind, login });
    true
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "settings_ui_tests.rs"]
mod ui_tests;

#[cfg(test)]
#[path = "settings_prefs_tests.rs"]
mod prefs_tests;

#[cfg(test)]
#[path = "settings_ai_tests.rs"]
mod ai_tests;
