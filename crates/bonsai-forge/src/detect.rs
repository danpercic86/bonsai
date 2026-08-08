//! PURE provider detection from an `origin` remote URL (P62 contract §6, P64
//! §3b).
//!
//! No I/O — accepts every git remote URL form (https, ssh, scp-like) and
//! extracts `host`/`owner`/`repo`. `github.com` ⇒ [`ForgeKind::GitHub`];
//! `gitlab.com` ⇒ [`ForgeKind::GitLab`] — GitLab supports nested groups, so
//! `owner` is the FULL namespace path (may contain `/`) and `repo` is the last
//! segment (OQ-A6). Any other parseable host ⇒ [`ForgeKind::Unknown`]
//! (enterprise host override is a deferred setting, OQ-6); an unparseable URL ⇒
//! `None`.

use crate::types::ForgeKind;

/// Detection result: the forge identity resolved from a remote URL. Internal to
/// the crate (not a wire DTO); `open()` uses it to construct the provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeTarget {
    pub kind: ForgeKind,
    pub host: String,
    pub owner: String,
    pub repo: String,
    pub web_url: String,
}

/// Parse a remote URL into a [`ForgeTarget`], or `None` if it is not a
/// recognizable `owner/repo` git remote. PURE (§6 pseudocode).
pub fn detect_provider(remote_url: &str) -> Option<ForgeTarget> {
    let (host, path) = split_host_path(remote_url.trim())?;
    let host = host.to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }

    // Strip a trailing "/", then a single trailing ".git", then any residual "/".
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let path = path.trim_end_matches('/');

    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segs.len() < 2 {
        return None; // need at least owner + repo
    }

    let kind = kind_for_host(&host);
    // GitLab supports nested groups (`group/subgroup/project`): the namespace is
    // EVERYTHING but the last segment (may contain `/`) and the project is the
    // last segment. GitHub (and unknown hosts) keep P62's exactly-first-two
    // behavior — extra path segments are ignored.
    let (owner, repo) = match kind {
        ForgeKind::GitLab => (
            segs[..segs.len() - 1].join("/"),
            segs[segs.len() - 1].to_string(),
        ),
        _ => (segs[0].to_string(), segs[1].to_string()),
    };

    let web_url = format!("https://{host}/{owner}/{repo}");
    Some(ForgeTarget {
        kind,
        host,
        owner,
        repo,
        web_url,
    })
}

/// Host → forge kind for the public SaaS hosts recognized in v1. Enterprise /
/// self-managed host override is deferred (OQ-6), so anything else is `Unknown`.
fn kind_for_host(host: &str) -> ForgeKind {
    match host {
        "github.com" => ForgeKind::GitHub,
        "gitlab.com" => ForgeKind::GitLab,
        _ => ForgeKind::Unknown,
    }
}

/// Split a remote URL into `(host, path)`. Handles `https://`/`http://`,
/// `ssh://`, and scp-like `[user@]host:path`. Any other scheme ⇒ `None`.
fn split_host_path(url: &str) -> Option<(&str, &str)> {
    if let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("ssh://"))
    {
        return Some(split_authority_path(rest));
    }
    if url.contains("://") {
        return None; // unsupported scheme (git://, ftp://, file://, …)
    }
    // scp-like: `[user@]host:owner/repo`. The first ':' separates host and path.
    let (authority, path) = url.split_once(':')?;
    let host = strip_userinfo(authority);
    Some((host, path))
}

/// Split `authority/path…` for URL-scheme forms, returning `(host, path)`. The
/// authority may carry userinfo and a port, both stripped from the host.
fn split_authority_path(rest: &str) -> (&str, &str) {
    let (authority, path) = match rest.split_once('/') {
        Some((a, p)) => (a, p),
        None => (rest, ""),
    };
    (strip_port(strip_userinfo(authority)), path)
}

/// Drop `user[:pass]@` userinfo, keeping only what follows the last `@`.
fn strip_userinfo(authority: &str) -> &str {
    match authority.rfind('@') {
        Some(i) => &authority[i + 1..],
        None => authority,
    }
}

/// Drop a trailing `:port` from a host authority.
fn strip_port(host: &str) -> &str {
    match host.split_once(':') {
        Some((h, _)) => h,
        None => host,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(url: &str) -> Option<ForgeTarget> {
        detect_provider(url)
    }

    /// Every recognized form of a github.com remote resolves to the same
    /// identity; enterprise hosts parse as `Unknown`; junk ⇒ `None`.
    #[test]
    fn detect_table() {
        // https, with and without .git, and a trailing slash.
        for url in [
            "https://github.com/owner/repo",
            "https://github.com/owner/repo.git",
            "https://github.com/owner/repo/",
            "https://github.com/owner/repo.git/",
        ] {
            let got = t(url).unwrap_or_else(|| panic!("expected Some for {url}"));
            assert_eq!(got.kind, ForgeKind::GitHub, "{url}");
            assert_eq!(got.host, "github.com", "{url}");
            assert_eq!(got.owner, "owner", "{url}");
            assert_eq!(got.repo, "repo", "{url}");
            assert_eq!(got.web_url, "https://github.com/owner/repo", "{url}");
        }

        // ssh:// form (with and without a port and userinfo).
        for url in [
            "ssh://git@github.com/owner/repo.git",
            "ssh://git@github.com:22/owner/repo.git",
            "ssh://github.com/owner/repo",
        ] {
            let got = t(url).unwrap_or_else(|| panic!("expected Some for {url}"));
            assert_eq!(got.kind, ForgeKind::GitHub, "{url}");
            assert_eq!(got.host, "github.com", "{url}");
            assert_eq!(got.owner, "owner", "{url}");
            assert_eq!(got.repo, "repo", "{url}");
        }

        // scp-like form.
        for url in [
            "git@github.com:owner/repo.git",
            "git@github.com:owner/repo",
        ] {
            let got = t(url).unwrap_or_else(|| panic!("expected Some for {url}"));
            assert_eq!(got.kind, ForgeKind::GitHub, "{url}");
            assert_eq!(got.host, "github.com", "{url}");
            assert_eq!(got.owner, "owner", "{url}");
            assert_eq!(got.repo, "repo", "{url}");
        }

        // Extra path segments (≥3) still resolve to the first two = owner/repo.
        for url in [
            "https://github.com/owner/repo/extra/stuff",
            "https://github.com/owner/repo/pull/42",
        ] {
            let got = t(url).unwrap_or_else(|| panic!("expected Some for {url}"));
            assert_eq!(got.kind, ForgeKind::GitHub, "{url}");
            assert_eq!(got.owner, "owner", "{url}");
            assert_eq!(got.repo, "repo", "{url}");
            assert_eq!(got.web_url, "https://github.com/owner/repo", "{url}");
        }

        // Host is lowercased; owner/repo case is preserved.
        let mixed = t("https://GitHub.com/Owner/Repo.git").unwrap();
        assert_eq!(mixed.kind, ForgeKind::GitHub);
        assert_eq!(mixed.host, "github.com");
        assert_eq!(mixed.owner, "Owner");
        assert_eq!(mixed.repo, "Repo");

        // Enterprise / self-hosted host ⇒ Unknown but still parses owner/repo.
        let ent = t("https://github.example.com/owner/repo.git").unwrap();
        assert_eq!(ent.kind, ForgeKind::Unknown);
        assert_eq!(ent.host, "github.example.com");
        assert_eq!(ent.owner, "owner");
        assert_eq!(ent.repo, "repo");
        let ent_ssh = t("git@git.example.org:team/project.git").unwrap();
        assert_eq!(ent_ssh.kind, ForgeKind::Unknown);
        assert_eq!(ent_ssh.host, "git.example.org");

        // Non-git / unparseable ⇒ None.
        assert!(t("not a url").is_none());
        assert!(t("https://github.com/owner").is_none(), "single segment");
        assert!(t("https://github.com/").is_none(), "no path");
        assert!(t("https://github.com").is_none(), "host only");
        assert!(t("git://github.com/owner/repo.git").is_none(), "git:// scheme");
        assert!(t("ftp://github.com/owner/repo").is_none(), "ftp scheme");
        assert!(t("").is_none(), "empty");
    }

    /// gitlab.com in every remote form: flat `owner/repo`, nested groups
    /// (`owner` carries the full namespace path), with/without `.git`, ssh, scp.
    #[test]
    fn detect_table_gitlab() {
        // Flat owner/repo across https (+.git / trailing slash), ssh, scp.
        for url in [
            "https://gitlab.com/owner/repo",
            "https://gitlab.com/owner/repo.git",
            "https://gitlab.com/owner/repo/",
            "https://gitlab.com/owner/repo.git/",
            "ssh://git@gitlab.com/owner/repo.git",
            "ssh://git@gitlab.com:22/owner/repo.git",
            "git@gitlab.com:owner/repo.git",
            "git@gitlab.com:owner/repo",
        ] {
            let got = t(url).unwrap_or_else(|| panic!("expected Some for {url}"));
            assert_eq!(got.kind, ForgeKind::GitLab, "{url}");
            assert_eq!(got.host, "gitlab.com", "{url}");
            assert_eq!(got.owner, "owner", "{url}");
            assert_eq!(got.repo, "repo", "{url}");
            assert_eq!(got.web_url, "https://gitlab.com/owner/repo", "{url}");
        }

        // Nested groups: `owner` is the FULL namespace path (may contain '/'),
        // `repo` is the last segment. (For GitHub this same URL would clip to
        // owner=group, repo=subgroup — GitLab must NOT.)
        for url in [
            "https://gitlab.com/group/subgroup/project.git",
            "git@gitlab.com:group/subgroup/project.git",
            "ssh://git@gitlab.com/group/subgroup/project",
        ] {
            let got = t(url).unwrap_or_else(|| panic!("expected Some for {url}"));
            assert_eq!(got.kind, ForgeKind::GitLab, "{url}");
            assert_eq!(got.owner, "group/subgroup", "{url}");
            assert_eq!(got.repo, "project", "{url}");
            assert_eq!(
                got.web_url, "https://gitlab.com/group/subgroup/project",
                "{url}"
            );
        }

        // Deeply nested (group/subgroup/subsubgroup/project) still resolves.
        let deep = t("https://gitlab.com/a/b/c/proj.git").unwrap();
        assert_eq!(deep.owner, "a/b/c");
        assert_eq!(deep.repo, "proj");

        // Host lowercased; owner/repo case preserved.
        let mixed = t("https://GitLab.com/Group/Proj.git").unwrap();
        assert_eq!(mixed.kind, ForgeKind::GitLab);
        assert_eq!(mixed.host, "gitlab.com");
        assert_eq!(mixed.owner, "Group");
        assert_eq!(mixed.repo, "Proj");

        // Single segment is still not a repo.
        assert!(t("https://gitlab.com/owner").is_none(), "single segment");

        // Contrast: GitHub keeps exactly-two even with extra segments.
        let gh = t("https://github.com/group/subgroup/project").unwrap();
        assert_eq!(gh.kind, ForgeKind::GitHub);
        assert_eq!(gh.owner, "group");
        assert_eq!(gh.repo, "subgroup");
    }
}
