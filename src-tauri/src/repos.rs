//! What lives under a watched folder.
//!
//! Nothing here writes: the folder rule in `~/.gitconfig` (`guard.rs`) is what
//! hands an identity to the repositories, and this only looks at them, so the
//! user can see the whole hierarchy a rule covers before saving it and so the
//! doctor can tell a folder that moved from one that is gone.

use crate::git;
use crate::models::{Platform, Profile, RepoRoot, PLATFORMS};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How deep a watched folder is walked. Deep enough for the usual
/// `<root>/<org>/<repo>` layouts without turning a scan into a full disk crawl.
const MAX_SCAN_DEPTH: usize = 6;

/// Directories that never hold a repository worth showing and cost a lot to
/// walk.
pub const SKIP_DIRS: [&str; 12] = [
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".output",
    "vendor",
    ".venv",
    "__pycache__",
    ".cache",
    ".pnpm-store",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteRef {
    pub host: String,
    pub owner: String,
    pub repo: String,
}

fn strip_scheme(url: &str) -> Option<&str> {
    for scheme in ["ssh://", "git://", "https://", "http://"] {
        if let Some(rest) = url.strip_prefix(scheme) {
            return Some(rest);
        }
    }
    None
}

/// Understands the four forms a Git remote actually takes: `git@host:owner/repo`,
/// `ssh://git@host[:port]/owner/repo`, `https://host/owner/repo` and the same
/// with an SSH host alias in place of the real host. GitLab subgroups stay in
/// `repo` (`group/sub/name`), so `owner` is always the top-level namespace.
pub fn parse_remote_url(url: &str) -> Option<RemoteRef> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }

    let (host, path) = match strip_scheme(url) {
        Some(rest) => {
            let rest = rest.rsplit_once('@').map(|(_, r)| r).unwrap_or(rest);
            let (host_port, path) = rest.split_once('/')?;
            (host_port.split(':').next()?.to_string(), path.to_string())
        }
        None => {
            let rest = url.rsplit_once('@').map(|(_, r)| r).unwrap_or(url);
            let (host, path) = rest.split_once(':')?;
            (host.to_string(), path.to_string())
        }
    };

    // A bare Windows path ("D:/repos/x") parses as host "D"; a real host always
    // carries a dot or an alias hyphen.
    if host.len() < 2 || !(host.contains('.') || host.contains('-')) {
        return None;
    }

    let path = path.trim_start_matches('/').trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repo) = path.split_once('/')?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }

    Some(RemoteRef {
        host,
        owner: owner.to_string(),
        repo: repo.to_string(),
    })
}

/// The SSH host alias that pins a remote to one profile's key. Folders carry
/// their key through `core.sshCommand` instead, but a remote a user aliased by
/// hand still has to resolve, so the aliases stay in `~/.ssh/config`.
pub fn host_alias(platform: Platform, profile: &Profile) -> String {
    format!("{}-{}", platform.as_str(), profile.slug())
}

/// Resolves a remote host to a platform, an SSH alias for one of the profiles
/// included.
pub fn platform_for_host(host: &str, profiles: &[Profile]) -> Option<Platform> {
    for platform in PLATFORMS {
        if host.eq_ignore_ascii_case(platform.canonical_host()) {
            return Some(platform);
        }
    }
    for profile in profiles {
        for platform in PLATFORMS {
            if profile.account(platform).is_some()
                && host.eq_ignore_ascii_case(&host_alias(platform, profile))
            {
                return Some(platform);
            }
        }
    }
    None
}

/// One repository the folder rule covers. Everything shown about it is read off
/// disk; a rule reaches all of them equally, so there is nothing to choose here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderRepo {
    pub path: String,
    /// Where it sits under the watched folder, so the hierarchy is legible
    /// without reading absolute paths.
    pub relative: String,
    pub name: String,
    pub root_path: String,
    /// Empty for a repository that has no `origin` yet. The rule still covers
    /// it, which is the reason it is listed rather than skipped.
    pub remote_url: String,
    /// `owner/repo`, empty when the remote says nothing usable.
    pub full_name: String,
    /// The remote's host is not the platform this folder was given, so the
    /// identity it will receive belongs to a different site.
    pub foreign_host: bool,
}

fn walk(dir: &Path, depth: usize, max_depth: usize, out: &mut Vec<PathBuf>) {
    if depth > max_depth {
        return;
    }
    if git::is_repo(dir) {
        out.push(dir.to_path_buf());
        // Keep descending: a container repository can hold independent
        // repositories that its own .gitignore hides.
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if SKIP_DIRS.iter().any(|s| name.as_ref() == *s) {
            continue;
        }
        walk(&entry.path(), depth + 1, max_depth, out);
    }
}

fn posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Every repository under one folder, in path order, deepest nesting included.
pub fn scan_one(root: &RepoRoot, profiles: &[Profile]) -> Vec<FolderRepo> {
    let base = root.path.replace('\\', "/");
    let base = base.trim_end_matches('/');
    let mut paths = Vec::new();
    walk(Path::new(&root.path), 0, MAX_SCAN_DEPTH, &mut paths);

    let mut found: Vec<FolderRepo> = Vec::new();
    for path in paths {
        let dir = posix(&path);
        let relative = dir
            .strip_prefix(base)
            .map(|r| r.trim_start_matches('/').to_string())
            .unwrap_or_else(|| dir.clone());
        let url = git::repo_remote_url(&dir, "origin").unwrap_or_default();
        let remote = parse_remote_url(&url);
        let foreign_host = remote
            .as_ref()
            .is_some_and(|r| platform_for_host(&r.host, profiles) != Some(root.platform));
        found.push(FolderRepo {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| dir.clone()),
            relative: if relative.is_empty() {
                ".".to_string()
            } else {
                relative
            },
            root_path: root.path.clone(),
            full_name: remote
                .as_ref()
                .map(|r| format!("{}/{}", r.owner, r.repo))
                .unwrap_or_default(),
            remote_url: url,
            foreign_host,
            path: dir,
        });
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found
}

pub fn scan(roots: &[RepoRoot], profiles: &[Profile]) -> Vec<FolderRepo> {
    let mut found = Vec::new();
    for root in roots {
        for repo in scan_one(root, profiles) {
            if !found.iter().any(|r: &FolderRepo| r.path == repo.path) {
                found.push(repo);
            }
        }
    }
    found
}

/// How many `owner/repo` names a folder is remembered by. Enough to tell two
/// folders apart, small enough to keep the state file readable.
const FINGERPRINT_CAP: usize = 64;

/// What a folder is recognised by after it moves: the names of the repositories
/// that were in it. Paths would not survive the move, and the folder's own name
/// is not unique enough on a disk full of `src` and `web` directories.
pub fn fingerprint(repos: &[FolderRepo]) -> Vec<String> {
    let mut names: Vec<String> = repos
        .iter()
        .filter(|r| !r.full_name.is_empty())
        .map(|r| r.full_name.to_lowercase())
        .collect();
    names.sort();
    names.dedup();
    names.truncate(FINGERPRINT_CAP);
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PlatformAccount;
    use std::process::Command;

    fn r(host: &str, owner: &str, repo: &str) -> Option<RemoteRef> {
        Some(RemoteRef {
            host: host.to_string(),
            owner: owner.to_string(),
            repo: repo.to_string(),
        })
    }

    #[test]
    fn parses_every_remote_form_a_repository_actually_carries() {
        assert_eq!(
            parse_remote_url("git@github.com:octo/demo.git"),
            r("github.com", "octo", "demo")
        );
        assert_eq!(
            parse_remote_url("https://gitlab.com/group/sub/name.git"),
            r("gitlab.com", "group", "sub/name")
        );
        assert_eq!(
            parse_remote_url("ssh://git@bitbucket.org:7999/team/app.git"),
            r("bitbucket.org", "team", "app")
        );
        assert_eq!(
            parse_remote_url("git@github-work:octo/demo.git"),
            r("github-work", "octo", "demo")
        );
        // A local path is not a remote, and "D" is not a host.
        assert_eq!(parse_remote_url("D:/repos/demo"), None);
        assert_eq!(parse_remote_url("/srv/git/bare.git"), None);
        assert_eq!(parse_remote_url(""), None);
    }

    fn profile() -> Profile {
        Profile {
            id: "p1".to_string(),
            name: "Personal".to_string(),
            default_platform: None,
            github: Some(PlatformAccount {
                username: "octo".to_string(),
                git_name: "Octo".to_string(),
                git_email: "octo@example.com".to_string(),
                ssh_private_key_path: String::new(),
                ssh_public_key_path: String::new(),
                sign_commits: false,
                token: None,
            }),
            gitlab: None,
            bitbucket: None,
            is_active: true,
        }
    }

    fn root_at(path: &str) -> RepoRoot {
        RepoRoot {
            path: path.to_string(),
            profile_id: "p1".to_string(),
            platform: Platform::Github,
            fingerprint: vec![],
        }
    }

    fn init(path: &Path, remote: Option<&str>) {
        std::fs::create_dir_all(path).unwrap();
        let p = posix(path);
        Command::new("git")
            .args(["init", "-q", &p])
            .output()
            .expect("git must be on PATH");
        if let Some(url) = remote {
            Command::new("git")
                .args(["-C", &p, "remote", "add", "origin", url])
                .output()
                .unwrap();
        }
    }

    /// The point of the list is that the user sees everything the folder rule
    /// will reach: a repository nested inside another one, a repository whose
    /// remote points at another site, and one with no remote at all.
    #[test]
    fn a_scan_lists_the_whole_hierarchy_the_rule_covers() {
        let dir = std::env::temp_dir().join(format!("gam-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        init(&dir.join("outer"), Some("git@github.com:octo/outer.git"));
        init(
            &dir.join("outer").join("nested"),
            Some("git@github.com:octo/inner.git"),
        );
        init(
            &dir.join("group").join("deep").join("elsewhere"),
            Some("git@gitlab.com:octo/other.git"),
        );
        init(&dir.join("fresh"), None);

        let root = root_at(&posix(&dir));
        let found = scan_one(&root, &[profile()]);
        let rows: Vec<(&str, &str, bool)> = found
            .iter()
            .map(|r| (r.relative.as_str(), r.full_name.as_str(), r.foreign_host))
            .collect();
        assert_eq!(
            rows,
            vec![
                ("fresh", "", false),
                ("group/deep/elsewhere", "octo/other", true),
                ("outer", "octo/outer", false),
                ("outer/nested", "octo/inner", false),
            ]
        );

        // Two of the four carry a usable name, and that is what a moved folder
        // is recognised by.
        assert_eq!(
            fingerprint(&found),
            vec!["octo/inner", "octo/other", "octo/outer"]
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
