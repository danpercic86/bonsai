# P112 — the compile-time tool catalog (static data)

Data half of `docs/contracts/P112-external-tool-detection.md` (split per the `docs/contracts/INDEX.md`
~500-line rule; CLAUDE.md's "large static tables live in their own module" applies to contracts too).
**Only the implementer of `crates/bonsai-core/src/tools/catalog.rs` needs this file** — `reviewer` and
`ui-designer` read the spec alone. Type definitions (`ToolEntry`, `Rung`, `Recipe`, `AutoRung`), probe
semantics and the launch mapping are spec §2/§3/§4.

Shorthand below maps 1:1 onto the spec §2 literals. Derived, never stored:
`hide_console = (kind == Editor)`; `wait_for_exit = true` iff the built spec is an `open` spec.
`id` is unique **per `(kind, os)`**, not globally. `app_name` is the macOS `open -a` name and is
`Some` exactly for the rows that show one.

## Terminals

| id | label | os | rungs, in order | recipe | app_name |
|---|---|---|---|---|---|
| `windows-terminal` | Windows Terminal | Win | `OnPath`("wt"), `WinFolder(LOCALAPPDATA, Microsoft\WindowsApps\wt.exe)` | `DirLastArg(["-d"])` | — |
| `powershell` | Windows PowerShell | Win | `BuiltIn`("powershell") | `DirCwd([])` | — |
| `pwsh` | PowerShell 7 | Win | `OnPath`("pwsh"), `WinFolder(ProgramFiles, PowerShell\7\pwsh.exe)` | `DirCwd([])` | — |
| `cmd` | Command Prompt | Win | `BuiltIn`("cmd") | `DirCwd(["/K"])` | — |
| `git-bash` | Git Bash | Win | `WinFolder(ProgramFiles, Git\git-bash.exe)`, `WinFolder(LOCALAPPDATA, Programs\Git\git-bash.exe)` | `DirJoinedArg([], "--cd=")` — **flag unverified, spec OQ4** | — |
| `apple-terminal` | Terminal | mac | `Bundle(/System/Applications/Utilities/Terminal.app)`, `Bundle(/Applications/Utilities/Terminal.app)` | `MacOpen` | `Terminal` |
| `iterm2` | iTerm | mac | `Bundle(/Applications/iTerm.app)`, `Bundle(Applications/iTerm.app, home)` | `MacOpen` | `iTerm` |
| `warp` | Warp | mac | `Bundle(/Applications/Warp.app)`, `Bundle(Applications/Warp.app, home)` | `MacOpen` | `Warp` |
| `kitty` | kitty | mac | `Bundle(/Applications/kitty.app)` | `MacOpen` | `kitty` |
| `gnome-terminal` | GNOME Terminal | Linux | `OnPath`, `UnixFile(/usr/bin/gnome-terminal)` | `DirJoinedArg([], "--working-directory=")` | — |
| `konsole` | Konsole | Linux | `OnPath`, `UnixFile(/usr/bin/konsole)` | `DirLastArg(["--workdir"])` | — |
| `kitty-linux` | kitty | Linux | `OnPath`("kitty"), `UnixFile(/usr/bin/kitty)` | `DirLastArg(["--directory"])` | — |
| `alacritty` | Alacritty | Linux | `OnPath`, `UnixFile(/usr/bin/alacritty)`, `UnixFile(/snap/bin/alacritty)` | `DirLastArg(["--working-directory"])` | — |
| `wezterm` | WezTerm | Linux | `OnPath` | `DirLastArg(["start", "--cwd"])` | — |
| `xfce4-terminal` | Xfce Terminal | Linux | `OnPath` | `DirJoinedArg([], "--working-directory=")` | — |
| `x-terminal-emulator` | System terminal | Linux | `OnPath`, `UnixFile(/usr/bin/x-terminal-emulator)` | `DirCwd([])` | — |

## Editors

**Admission rule: a GUI program that accepts a FOLDER argument.** No console editors (`nvim`, `nano`,
`micro`): launched without a terminal they are invisible. Every recipe here is `DirLastArg([])` (a
`Bundle` hit overrides it with `MacOpen` at launch, spec §4), so the recipe column is omitted.

| id | label | os | rungs, in order | app_name |
|---|---|---|---|---|
| `vscode` | Visual Studio Code | Win | `OnPath`("code"), `AppPaths(Code.exe)`, `WinFolder(LOCALAPPDATA, Programs\Microsoft VS Code\Code.exe)`, `WinFolder(ProgramFiles, Microsoft VS Code\Code.exe)` | — |
| `vscode` | Visual Studio Code | mac | `Bundle(/Applications/Visual Studio Code.app)`, `Bundle(Applications/Visual Studio Code.app, home)`, `OnPath`("code") | `Visual Studio Code` |
| `vscode` | Visual Studio Code | Linux | `OnPath`("code"), `UnixFile(/usr/bin/code)`, `UnixFile(/snap/bin/code)`, `UnixFile(/var/lib/flatpak/exports/bin/com.visualstudio.code)` | — |
| `vscode-insiders` | VS Code Insiders | Win | `OnPath`("code-insiders"), `WinFolder(LOCALAPPDATA, Programs\Microsoft VS Code Insiders\Code - Insiders.exe)` | — |
| `vscode-insiders` | VS Code Insiders | mac | `Bundle(/Applications/Visual Studio Code - Insiders.app)`, `OnPath`("code-insiders") | `Visual Studio Code - Insiders` |
| `vscode-insiders` | VS Code Insiders | Linux | `OnPath`("code-insiders") | — |
| `vscodium` | VSCodium | Win | `OnPath`("codium"), `WinFolder(LOCALAPPDATA, Programs\VSCodium\VSCodium.exe)` | — |
| `vscodium` | VSCodium | Linux | `OnPath`("codium"), `UnixFile(/snap/bin/codium)` | — |
| `cursor` | Cursor | Win | `OnPath`("cursor"), `WinFolder(LOCALAPPDATA, Programs\cursor\Cursor.exe)` | — |
| `cursor` | Cursor | mac | `Bundle(/Applications/Cursor.app)`, `OnPath`("cursor") | `Cursor` |
| `sublime` | Sublime Text | Win | `OnPath`("subl"), `AppPaths(sublime_text.exe)`, `WinFolder(ProgramFiles, Sublime Text\sublime_text.exe)` | — |
| `sublime` | Sublime Text | mac | `Bundle(/Applications/Sublime Text.app)`, `OnPath`("subl") | `Sublime Text` |
| `sublime` | Sublime Text | Linux | `OnPath`("subl"), `UnixFile(/usr/bin/subl)` | — |
| `notepadpp` | Notepad++ | Win | `AppPaths(notepad++.exe)`, `WinFolder(ProgramFiles, Notepad++\notepad++.exe)`, `WinFolder(ProgramFiles(x86), Notepad++\notepad++.exe)` | — |
| `idea` | IntelliJ IDEA | Win | `OnPath`("idea") | — |
| `zed` | Zed | mac | `Bundle(/Applications/Zed.app)`, `Bundle(Applications/Zed.app, home)`, `OnPath`("zed") | `Zed` |
| `zed` | Zed | Linux | `OnPath`("zed") | — |
| `kate` | Kate | Linux | `OnPath`("kate"), `UnixFile(/usr/bin/kate)` | — |
| `gedit` | Text Editor | Linux | `OnPath`("gedit"), `UnixFile(/usr/bin/gedit)` | — |

### `AppPaths` probe convention (Windows)

Key: `HKCU`, then `HKLM`, of `SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\<exe>`, **default
value**, quotes trimmed. `gitbin::HostGitEnv::registry_string` runs `reg query <key> /v <value>`, so
define the convention **`value == ""` ⇒ `reg query <key> /ve`**, parsed with
`parse_reg_query(stdout, "(Default)")`. `parse_reg_query` (`gitbin.rs:203`) is private → widen to
`pub(crate)`. Per-vendor registry keys are deliberately **not** asserted: App Paths is the one
documented Windows mechanism, and every rung degrades to `None`, so an app that does not register
there is found by a later rung or simply not offered.

## Auto ladders — ordered ids per `(os, kind)`; **byte-identical to today's hardcoded ladders**

```rust
AUTO_TERMINAL_WIN   = [Name("windows-terminal"), Name("powershell"), Name("cmd")]
AUTO_TERMINAL_MAC   = [MacApp("apple-terminal")]
AUTO_TERMINAL_LINUX = [Name("gnome-terminal"), Name("konsole"), Name("x-terminal-emulator")]
AUTO_EDITOR_WIN     = [Name("vscode"), Name("vscode-insiders")]
AUTO_EDITOR_MAC     = [MacApp("vscode"), MacApp("vscode-insiders"), Name("vscode")]
AUTO_EDITOR_LINUX   = [Name("vscode"), Name("vscode-insiders")]
```

`Name` uses `entry.program` + `entry.recipe`; `MacApp` uses `entry.app_name` — **never** `program`, or
`AUTO_EDITOR_MAC` would emit `open -a code` instead of today's `open -a "Visual Studio Code"`.
`AUTO_EDITOR_MAC`'s third rung is the CLI-stub rung of the same entry, which is why `AutoVia` exists.
No second program table: every string comes from the referenced entry.

## `LEGACY_ALIASES: &[((ToolKind, &'static str), &'static str)]` — normalised stem → id

Normalisation is spec §5.3 (`legacy_tool_id`). Lookup is exact and kind-scoped; a miss is `""`.

**Editor:** `code`→`vscode` · `code-insiders`→`vscode-insiders` · `codium`→`vscodium` ·
`vscodium`→`vscodium` · `cursor`→`cursor` · `subl`→`sublime` · `sublime_text`→`sublime` ·
`notepad++`→`notepadpp` · `zed`→`zed` · `idea`→`idea` · `idea64`→`idea` · `kate`→`kate` ·
`gedit`→`gedit`

**Terminal:** `wt`→`windows-terminal` · `powershell`→`powershell` · `pwsh`→`pwsh` · `cmd`→`cmd` ·
`git-bash`→`git-bash` · `terminal`→`apple-terminal` · `iterm`→`iterm2` · `iterm2`→`iterm2` ·
`warp`→`warp` · `gnome-terminal`→`gnome-terminal` · `konsole`→`konsole` · `kitty`→`kitty` ·
`alacritty`→`alacritty` · `wezterm`→`wezterm` · `xfce4-terminal`→`xfce4-terminal` ·
`x-terminal-emulator`→`x-terminal-emulator`

`kitty`→`kitty` resolves on macOS; on Linux the id is `kitty-linux`, covered by `find`'s
host-OS-first-then-any-OS rule (spec §2). Adding a Linux-only `kitty`→`kitty-linux` alias instead is
equally acceptable — AC8 pins only that every alias target exists in `CATALOG`.

## Extending the catalog (the only way to add a tool — spec §8.2)

Add a row here plus the matching `ToolEntry`. **Never** reintroduce a settings-supplied program
string: that is the capability P112 exists to delete.
