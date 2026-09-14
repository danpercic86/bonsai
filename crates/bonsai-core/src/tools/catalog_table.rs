//! **Static data only** — the candidate table, the auto ladders and the legacy
//! alias map. One row here per row of `docs/contracts/P112-tool-catalog.md`,
//! in the same order.
//!
//! No filesystem, no registry, no spawn: this file is a table, [`super::detect`]
//! is the prober, and [`super::catalog`] holds the types and the lookups.
//!
//! **Adding a tool is the only supported way to extend what Bonsai can launch**
//! (P112 §8.2): add a row to the contract and a row here. Never reintroduce a
//! settings-supplied program string — that is the capability P112 exists to
//! delete.

use crate::external::TargetOs;

use super::catalog::{AutoRung, AutoVia, Recipe, Rung, ToolEntry};
use super::ToolKind;

/// One catalog row, in the contract table's field order.
///
/// A macro (rather than 36 spelled-out struct literals) keeps the table one
/// screen-scannable block that diffs 1:1 against the contract, and keeps this
/// file inside the ~500-line rule. It adds no behaviour: the expansion is the
/// plain [`ToolEntry`] literal.
macro_rules! tool {
    ($id:expr, $label:expr, $kind:ident, $os:ident,
     program: $program:expr, app: $app:expr,
     rungs: [$($rung:expr),+ $(,)?], recipe: $recipe:expr $(,)?) => {
        ToolEntry {
            id: $id,
            label: $label,
            kind: ToolKind::$kind,
            os: TargetOs::$os,
            program: $program,
            app_name: $app,
            rungs: &[$($rung),+],
            recipe: $recipe,
        }
    };
}

/// Every candidate tool, grouped by kind then OS. Order inside a `(kind, os)`
/// group is the order the picker lists them in, and rungs are probed in the
/// order written.
pub static CATALOG: &[ToolEntry] = &[
    // ---- Terminals — Windows -------------------------------------------------
    tool! { "windows-terminal", "Windows Terminal", Terminal, Windows,
        program: "wt", app: None,
        rungs: [Rung::OnPath,
                Rung::WinFolder { var: "LOCALAPPDATA", suffix: r"Microsoft\WindowsApps\wt.exe" }],
        recipe: Recipe::DirLastArg(&["-d"]) },
    tool! { "powershell", "Windows PowerShell", Terminal, Windows,
        program: "powershell", app: None,
        rungs: [Rung::BuiltIn],
        recipe: Recipe::DirCwd(&[]) },
    tool! { "pwsh", "PowerShell 7", Terminal, Windows,
        program: "pwsh", app: None,
        rungs: [Rung::OnPath,
                Rung::WinFolder { var: "ProgramFiles", suffix: r"PowerShell\7\pwsh.exe" }],
        recipe: Recipe::DirCwd(&[]) },
    tool! { "cmd", "Command Prompt", Terminal, Windows,
        program: "cmd", app: None,
        rungs: [Rung::BuiltIn],
        recipe: Recipe::DirCwd(&["/K"]) },
    // `--cd=` is the one catalog flag not verifiable from this repo's source
    // (spec OQ4): if the native checkpoint shows Git Bash does not start in the
    // directory, DROP this row rather than guessing another flag.
    tool! { "git-bash", "Git Bash", Terminal, Windows,
        program: "git-bash", app: None,
        rungs: [Rung::WinFolder { var: "ProgramFiles", suffix: r"Git\git-bash.exe" },
                Rung::WinFolder { var: "LOCALAPPDATA", suffix: r"Programs\Git\git-bash.exe" }],
        recipe: Recipe::DirJoinedArg(&[], "--cd=") },
    // ---- Terminals — macOS ---------------------------------------------------
    // `program: "open"` on every bundle-only row: such a row can only resolve
    // through a `Bundle` rung, which launches `open -a <bundle> <dir>`, so
    // "open" is literally the program — not a placeholder.
    tool! { "apple-terminal", "Terminal", Terminal, MacOs,
        program: "open", app: Some("Terminal"),
        rungs: [Rung::Bundle { path: "/System/Applications/Utilities/Terminal.app", home: false },
                Rung::Bundle { path: "/Applications/Utilities/Terminal.app", home: false }],
        recipe: Recipe::MacOpen },
    tool! { "iterm2", "iTerm", Terminal, MacOs,
        program: "open", app: Some("iTerm"),
        rungs: [Rung::Bundle { path: "/Applications/iTerm.app", home: false },
                Rung::Bundle { path: "Applications/iTerm.app", home: true }],
        recipe: Recipe::MacOpen },
    tool! { "warp", "Warp", Terminal, MacOs,
        program: "open", app: Some("Warp"),
        rungs: [Rung::Bundle { path: "/Applications/Warp.app", home: false },
                Rung::Bundle { path: "Applications/Warp.app", home: true }],
        recipe: Recipe::MacOpen },
    tool! { "kitty", "kitty", Terminal, MacOs,
        program: "open", app: Some("kitty"),
        rungs: [Rung::Bundle { path: "/Applications/kitty.app", home: false }],
        recipe: Recipe::MacOpen },
    // ---- Terminals — Linux ---------------------------------------------------
    tool! { "gnome-terminal", "GNOME Terminal", Terminal, Linux,
        program: "gnome-terminal", app: None,
        rungs: [Rung::OnPath, Rung::UnixFile { path: "/usr/bin/gnome-terminal" }],
        recipe: Recipe::DirJoinedArg(&[], "--working-directory=") },
    tool! { "konsole", "Konsole", Terminal, Linux,
        program: "konsole", app: None,
        rungs: [Rung::OnPath, Rung::UnixFile { path: "/usr/bin/konsole" }],
        recipe: Recipe::DirLastArg(&["--workdir"]) },
    tool! { "kitty-linux", "kitty", Terminal, Linux,
        program: "kitty", app: None,
        rungs: [Rung::OnPath, Rung::UnixFile { path: "/usr/bin/kitty" }],
        recipe: Recipe::DirLastArg(&["--directory"]) },
    tool! { "alacritty", "Alacritty", Terminal, Linux,
        program: "alacritty", app: None,
        rungs: [Rung::OnPath,
                Rung::UnixFile { path: "/usr/bin/alacritty" },
                Rung::UnixFile { path: "/snap/bin/alacritty" }],
        recipe: Recipe::DirLastArg(&["--working-directory"]) },
    tool! { "wezterm", "WezTerm", Terminal, Linux,
        program: "wezterm", app: None,
        rungs: [Rung::OnPath],
        recipe: Recipe::DirLastArg(&["start", "--cwd"]) },
    tool! { "xfce4-terminal", "Xfce Terminal", Terminal, Linux,
        program: "xfce4-terminal", app: None,
        rungs: [Rung::OnPath],
        recipe: Recipe::DirJoinedArg(&[], "--working-directory=") },
    tool! { "x-terminal-emulator", "System terminal", Terminal, Linux,
        program: "x-terminal-emulator", app: None,
        rungs: [Rung::OnPath, Rung::UnixFile { path: "/usr/bin/x-terminal-emulator" }],
        recipe: Recipe::DirCwd(&[]) },
    // ---- Editors -------------------------------------------------------------
    // Admission rule: a GUI program that accepts a FOLDER argument. No console
    // editors (`nvim`, `nano`, `micro`) — launched without a terminal they are
    // invisible. Every recipe is `DirLastArg(&[])`; a `Bundle` hit overrides it
    // with `MacOpen` in `picked()`.
    tool! { "vscode", "Visual Studio Code", Editor, Windows,
        program: "code", app: None,
        rungs: [Rung::OnPath,
                Rung::AppPaths { exe: "Code.exe" },
                Rung::WinFolder { var: "LOCALAPPDATA",
                                  suffix: r"Programs\Microsoft VS Code\Code.exe" },
                Rung::WinFolder { var: "ProgramFiles", suffix: r"Microsoft VS Code\Code.exe" }],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "vscode", "Visual Studio Code", Editor, MacOs,
        program: "code", app: Some("Visual Studio Code"),
        rungs: [Rung::Bundle { path: "/Applications/Visual Studio Code.app", home: false },
                Rung::Bundle { path: "Applications/Visual Studio Code.app", home: true },
                Rung::OnPath],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "vscode", "Visual Studio Code", Editor, Linux,
        program: "code", app: None,
        rungs: [Rung::OnPath,
                Rung::UnixFile { path: "/usr/bin/code" },
                Rung::UnixFile { path: "/snap/bin/code" },
                Rung::UnixFile { path: "/var/lib/flatpak/exports/bin/com.visualstudio.code" }],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "vscode-insiders", "VS Code Insiders", Editor, Windows,
        program: "code-insiders", app: None,
        rungs: [Rung::OnPath,
                Rung::WinFolder {
                    var: "LOCALAPPDATA",
                    suffix: r"Programs\Microsoft VS Code Insiders\Code - Insiders.exe" }],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "vscode-insiders", "VS Code Insiders", Editor, MacOs,
        program: "code-insiders", app: Some("Visual Studio Code - Insiders"),
        rungs: [Rung::Bundle { path: "/Applications/Visual Studio Code - Insiders.app",
                               home: false },
                Rung::OnPath],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "vscode-insiders", "VS Code Insiders", Editor, Linux,
        program: "code-insiders", app: None,
        rungs: [Rung::OnPath],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "vscodium", "VSCodium", Editor, Windows,
        program: "codium", app: None,
        rungs: [Rung::OnPath,
                Rung::WinFolder { var: "LOCALAPPDATA", suffix: r"Programs\VSCodium\VSCodium.exe" }],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "vscodium", "VSCodium", Editor, Linux,
        program: "codium", app: None,
        rungs: [Rung::OnPath, Rung::UnixFile { path: "/snap/bin/codium" }],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "cursor", "Cursor", Editor, Windows,
        program: "cursor", app: None,
        rungs: [Rung::OnPath,
                Rung::WinFolder { var: "LOCALAPPDATA", suffix: r"Programs\cursor\Cursor.exe" }],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "cursor", "Cursor", Editor, MacOs,
        program: "cursor", app: Some("Cursor"),
        rungs: [Rung::Bundle { path: "/Applications/Cursor.app", home: false }, Rung::OnPath],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "sublime", "Sublime Text", Editor, Windows,
        program: "subl", app: None,
        rungs: [Rung::OnPath,
                Rung::AppPaths { exe: "sublime_text.exe" },
                Rung::WinFolder { var: "ProgramFiles", suffix: r"Sublime Text\sublime_text.exe" }],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "sublime", "Sublime Text", Editor, MacOs,
        program: "subl", app: Some("Sublime Text"),
        rungs: [Rung::Bundle { path: "/Applications/Sublime Text.app", home: false },
                Rung::OnPath],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "sublime", "Sublime Text", Editor, Linux,
        program: "subl", app: None,
        rungs: [Rung::OnPath, Rung::UnixFile { path: "/usr/bin/subl" }],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "notepadpp", "Notepad++", Editor, Windows,
        program: "notepad++", app: None,
        rungs: [Rung::AppPaths { exe: "notepad++.exe" },
                Rung::WinFolder { var: "ProgramFiles", suffix: r"Notepad++\notepad++.exe" },
                Rung::WinFolder { var: "ProgramFiles(x86)", suffix: r"Notepad++\notepad++.exe" }],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "idea", "IntelliJ IDEA", Editor, Windows,
        program: "idea", app: None,
        rungs: [Rung::OnPath],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "zed", "Zed", Editor, MacOs,
        program: "zed", app: Some("Zed"),
        rungs: [Rung::Bundle { path: "/Applications/Zed.app", home: false },
                Rung::Bundle { path: "Applications/Zed.app", home: true },
                Rung::OnPath],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "zed", "Zed", Editor, Linux,
        program: "zed", app: None,
        rungs: [Rung::OnPath],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "kate", "Kate", Editor, Linux,
        program: "kate", app: None,
        rungs: [Rung::OnPath, Rung::UnixFile { path: "/usr/bin/kate" }],
        recipe: Recipe::DirLastArg(&[]) },
    tool! { "gedit", "Text Editor", Editor, Linux,
        program: "gedit", app: None,
        rungs: [Rung::OnPath, Rung::UnixFile { path: "/usr/bin/gedit" }],
        recipe: Recipe::DirLastArg(&[]) },
];

// ---- auto ladders (the `""` setting) -----------------------------------------
//
// Ordered ids per `(os, kind)`, byte-identical in effect to the ladders
// hardcoded in `external.rs` today (AC9). `Name` uses the entry's `program` +
// `recipe`; `MacApp` uses its `app_name` — never `program`, or AUTO_EDITOR_MAC
// would emit `open -a code` instead of `open -a "Visual Studio Code"`.
// AUTO_EDITOR_MAC's third rung is the CLI-stub rung of the SAME entry, which is
// why `AutoVia` exists at all.

const fn name(id: &'static str) -> AutoRung {
    AutoRung {
        id,
        via: AutoVia::Name,
    }
}

const fn mac_app(id: &'static str) -> AutoRung {
    AutoRung {
        id,
        via: AutoVia::MacApp,
    }
}

pub static AUTO_TERMINAL_WIN: &[AutoRung] =
    &[name("windows-terminal"), name("powershell"), name("cmd")];
pub static AUTO_TERMINAL_MAC: &[AutoRung] = &[mac_app("apple-terminal")];
pub static AUTO_TERMINAL_LINUX: &[AutoRung] = &[
    name("gnome-terminal"),
    name("konsole"),
    name("x-terminal-emulator"),
];
pub static AUTO_EDITOR_WIN: &[AutoRung] = &[name("vscode"), name("vscode-insiders")];
pub static AUTO_EDITOR_MAC: &[AutoRung] = &[
    mac_app("vscode"),
    mac_app("vscode-insiders"),
    name("vscode"),
];
pub static AUTO_EDITOR_LINUX: &[AutoRung] = &[name("vscode"), name("vscode-insiders")];

/// Normalised legacy stem ⇒ catalog id, for the one-shot P112 §5.3 migration of
/// the deleted `terminalCommand` / `editorCommand` settings.
///
/// Lookup is EXACT and kind-scoped; a miss is `""` (accepted, not preserved — a
/// legacy free-text command must never become a launchable program again).
/// `kitty` resolves to the macOS row; on Linux the id is `kitty-linux`, which
/// [`super::catalog::find`]'s host-OS-first-then-any-OS rule covers.
pub static LEGACY_ALIASES: &[((ToolKind, &str), &str)] = &[
    // Editors.
    ((ToolKind::Editor, "code"), "vscode"),
    ((ToolKind::Editor, "code-insiders"), "vscode-insiders"),
    ((ToolKind::Editor, "codium"), "vscodium"),
    ((ToolKind::Editor, "vscodium"), "vscodium"),
    ((ToolKind::Editor, "cursor"), "cursor"),
    ((ToolKind::Editor, "subl"), "sublime"),
    ((ToolKind::Editor, "sublime_text"), "sublime"),
    ((ToolKind::Editor, "notepad++"), "notepadpp"),
    ((ToolKind::Editor, "zed"), "zed"),
    ((ToolKind::Editor, "idea"), "idea"),
    ((ToolKind::Editor, "idea64"), "idea"),
    ((ToolKind::Editor, "kate"), "kate"),
    ((ToolKind::Editor, "gedit"), "gedit"),
    // Terminals.
    ((ToolKind::Terminal, "wt"), "windows-terminal"),
    ((ToolKind::Terminal, "powershell"), "powershell"),
    ((ToolKind::Terminal, "pwsh"), "pwsh"),
    ((ToolKind::Terminal, "cmd"), "cmd"),
    ((ToolKind::Terminal, "git-bash"), "git-bash"),
    ((ToolKind::Terminal, "terminal"), "apple-terminal"),
    ((ToolKind::Terminal, "iterm"), "iterm2"),
    ((ToolKind::Terminal, "iterm2"), "iterm2"),
    ((ToolKind::Terminal, "warp"), "warp"),
    ((ToolKind::Terminal, "gnome-terminal"), "gnome-terminal"),
    ((ToolKind::Terminal, "konsole"), "konsole"),
    ((ToolKind::Terminal, "kitty"), "kitty"),
    ((ToolKind::Terminal, "alacritty"), "alacritty"),
    ((ToolKind::Terminal, "wezterm"), "wezterm"),
    ((ToolKind::Terminal, "xfce4-terminal"), "xfce4-terminal"),
    ((ToolKind::Terminal, "x-terminal-emulator"), "x-terminal-emulator"),
];
