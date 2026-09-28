//! hooks unit tests. Extracted verbatim from the former inline `mod tests`
//! (file-size discipline).

use super::*;
use crate::git::commit::create_commit;
use crate::git::exec::SpawnGitExec;
use crate::git::stage::stage_paths;
use std::process::Command;

// ---- shared fixtures (oracle + detection) ----------------------------

fn init_repo(dir: &Path) -> git2::Repository {
    let repo = git2::Repository::init(dir).expect("init");
    let mut cfg = repo.config().expect("config");
    cfg.set_str("user.name", "Hook Tester").expect("name");
    cfg.set_str("user.email", "hooks@example.com")
        .expect("email");
    cfg.set_bool("core.autocrlf", false).expect("autocrlf");
    drop(cfg);
    repo
}

fn hooks_dir(repo: &git2::Repository) -> PathBuf {
    repo.commondir().join("hooks")
}

/// Write an executable `#!/bin/sh` hook with LF endings (git's bundled sh
/// parses the shebang on Windows too — the point of A-D1).
fn write_hook(dir: &Path, name: &str, body: &str) {
    std::fs::create_dir_all(dir).expect("mkdir hooks");
    let path = dir.join(name);
    std::fs::write(&path, body.replace("\r\n", "\n")).expect("write hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
    }
}

mod detection;
mod oracle;
mod pure;
