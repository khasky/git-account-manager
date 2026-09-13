//! Whether the folder rules still hold, and what to do when one stops holding.
//!
//! Two depths, because they are asked at different rates. `watch` is three file
//! reads per folder and runs on a timer, so a folder that was moved or renamed
//! surfaces on its own instead of waiting for someone to open the profile.
//! `report` walks the folders and asks git what it actually resolves, which is
//! the only way to catch a repository whose own config quietly beats the rule.

use crate::git;
use crate::guard::{self, GuardStatus};
use crate::hooks;
use crate::models::{GuardSettings, Platform, Profile, RepoRoot};
use crate::repos::{self, FolderRepo};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The local keys a folder rule is supposed to supply, and therefore the ones
/// that beat it when a repository carries its own.
const OVERRIDING_KEYS: [&str; 6] = [
    "user.name",
    "user.email",
    "user.signingkey",
    "commit.gpgsign",
    "core.sshcommand",
    "gam.allowedemail",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderCheck {
    pub id: String,
    pub ok: bool,
    pub detail: String,
}

fn check(id: &str, ok: bool, detail: String) -> FolderCheck {
    FolderCheck {
        id: id.to_string(),
        ok,
        detail,
    }
}

/// One repository that does not follow its folder's rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoIssue {
    pub path: String,
    pub relative: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderStatus {
    pub path: String,
    pub profile_id: String,
    pub profile_name: String,
    pub platform: Platform,
    pub expected_email: String,
    pub repos: usize,
    pub checks: Vec<FolderCheck>,
    /// Repositories whose own config beats the rule. Fixable from here.
    pub overrides: Vec<RepoIssue>,
    /// Repositories whose own `core.hooksPath` routes hooks past the guard.
    /// Reported, never changed: that setting belongs to the project's tooling.
    pub bypassed: Vec<RepoIssue>,
    /// Where the folder appears to have gone, when it is no longer at its path.
    pub moved_to: Option<String>,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub guard: GuardStatus,
    pub folders: Vec<FolderStatus>,
}

/// The cheap answer, for the timer: is the folder still there, does its rule
/// still exist, is the guard still in force. Where a missing folder went is not
/// answered here — that search walks the disk, and this runs every minute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderWatch {
    pub path: String,
    pub profile_id: String,
    pub profile_name: String,
    /// `ok` | `missing` | `no-rule` | `guard-off`
    pub state: String,
}

fn rule_present(root: &RepoRoot, profile: &Profile, region: &str) -> bool {
    let Ok(line) = guard::rule_line(root, profile) else {
        return false;
    };
    let Ok(file) = guard::identity_file(profile, root.platform) else {
        return false;
    };
    region.contains(&line) && file.exists()
}

pub fn watch(
    settings: &GuardSettings,
    profiles: &[Profile],
    roots: &[RepoRoot],
) -> Vec<FolderWatch> {
    let region = guard::region_text();
    let guard_ok = !settings.guard_commits || hooks::status().ours;

    roots
        .iter()
        .filter_map(|root| {
            let profile = profiles.iter().find(|p| p.id == root.profile_id)?;
            let state = if !Path::new(&root.path).is_dir() {
                "missing"
            } else if !rule_present(root, profile, &region) {
                "no-rule"
            } else if !guard_ok {
                "guard-off"
            } else {
                "ok"
            };
            Some(FolderWatch {
                path: root.path.clone(),
                profile_id: root.profile_id.clone(),
                profile_name: profile.name.clone(),
                state: state.to_string(),
            })
        })
        .collect()
}

pub fn report(settings: &GuardSettings, profiles: &[Profile], roots: &[RepoRoot]) -> DoctorReport {
    let region = guard::region_text();
    let folders = roots
        .iter()
        .filter_map(|root| {
            let profile = profiles.iter().find(|p| p.id == root.profile_id)?;
            Some(inspect(root, profile, settings, &region, profiles))
        })
        .collect();

    DoctorReport {
        guard: guard::status(settings, profiles, roots),
        folders,
    }
}

fn inspect(
    root: &RepoRoot,
    profile: &Profile,
    settings: &GuardSettings,
    region: &str,
    profiles: &[Profile],
) -> FolderStatus {
    let account = profile.account(root.platform);
    let expected_email = account.map(|a| a.git_email.clone()).unwrap_or_default();
    let exists = Path::new(&root.path).is_dir();

    let mut checks = vec![check(
        "exists",
        exists,
        if exists {
            String::new()
        } else {
            root.path.clone()
        },
    )];

    let has_rule = rule_present(root, profile, region);
    checks.push(check(
        "rule",
        has_rule,
        guard::identity_file(profile, root.platform)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default(),
    ));

    let repos: Vec<FolderRepo> = if exists {
        repos::scan_one(root, profiles)
    } else {
        Vec::new()
    };

    // Asked of a real repository rather than inferred from the region: an
    // include git does not apply is the failure this whole model turns on, and
    // only git can answer whether it applied.
    let (identity_ok, identity_detail) = match repos.first() {
        Some(repo) => match git::repo_config_origin(&repo.path, "user.email") {
            Some((origin, value)) => {
                let from_rule = guard::identity_file(profile, root.platform)
                    .map(|f| origin.eq_ignore_ascii_case(&f.to_string_lossy().replace('\\', "/")))
                    .unwrap_or(false);
                (
                    from_rule && value == expected_email,
                    format!("{} ({})", value, origin),
                )
            }
            None => (false, String::new()),
        },
        // Nothing to ask yet. A folder waiting for its first clone is not a
        // problem, and reporting one would train the user to ignore the report.
        None => (true, String::new()),
    };
    if exists {
        checks.push(check("identity", identity_ok, identity_detail));
    }

    let mut overrides = Vec::new();
    let mut bypassed = Vec::new();
    for repo in &repos {
        let local: Vec<String> = OVERRIDING_KEYS
            .iter()
            .filter(|key| git::repo_config_get_local(&repo.path, key).is_some())
            .map(|key| (*key).to_string())
            .collect();
        if !local.is_empty() {
            overrides.push(RepoIssue {
                path: repo.path.clone(),
                relative: repo.relative.clone(),
                detail: local.join(", "),
            });
        }
        if let Some(path) = git::repo_config_get_local(&repo.path, "core.hooksPath") {
            bypassed.push(RepoIssue {
                path: repo.path.clone(),
                relative: repo.relative.clone(),
                detail: path,
            });
        }
    }
    checks.push(check(
        "local",
        overrides.is_empty(),
        overrides.len().to_string(),
    ));

    let guard_state = hooks::status();
    let guard_ok = !settings.guard_commits || guard_state.ours;
    checks.push(check(
        "guard",
        guard_ok,
        guard_state.hooks_path.unwrap_or_default(),
    ));

    let moved_to = if exists {
        None
    } else {
        find_moved(root, profiles)
    };

    FolderStatus {
        ok: checks.iter().all(|c| c.ok),
        path: root.path.clone(),
        profile_id: root.profile_id.clone(),
        profile_name: profile.name.clone(),
        platform: root.platform,
        expected_email,
        repos: repos.len(),
        checks,
        overrides,
        bypassed,
        moved_to,
    }
}

/// Puts a folder back under its rule: the repositories inside it stop carrying
/// their own copy of what the rule supplies. Only the keys the rule covers are
/// removed, and only from repositories under this folder.
pub fn clean_overrides(root: &RepoRoot, profiles: &[Profile]) -> Result<usize, String> {
    let mut cleaned = 0;
    for repo in repos::scan_one(root, profiles) {
        let had = OVERRIDING_KEYS
            .iter()
            .any(|key| git::repo_config_get_local(&repo.path, key).is_some());
        if had {
            git::repo_config_unset_local(&repo.path, &OVERRIDING_KEYS)?;
            cleaned += 1;
        }
    }
    Ok(cleaned)
}

/// How far below an existing ancestor a moved folder is looked for. A move is
/// nearly always sideways or one level up or down; a deeper search would turn
/// a missing folder into a disk crawl.
const MOVE_SEARCH_DEPTH: usize = 3;

/// How much of the remembered fingerprint a candidate has to carry. Half allows
/// for repositories added or removed since the folder was last scanned, while
/// still refusing a folder that merely shares a name.
const MOVE_MATCH_RATIO: f32 = 0.5;

/// Where a folder went, when it is no longer at its path.
///
/// The name alone is not evidence: a disk holds many folders called `work`. The
/// repositories inside are, so a candidate has to carry most of the names the
/// folder was remembered by. A folder that was never scanned has no names to
/// match, and is only accepted when exactly one candidate carries its name.
pub fn find_moved(root: &RepoRoot, profiles: &[Profile]) -> Option<String> {
    let missing = root.path.replace('\\', "/");
    let missing = missing.trim_end_matches('/');
    let name = Path::new(missing)
        .file_name()?
        .to_string_lossy()
        .to_string();

    let mut candidates = Vec::new();
    for base in search_bases(missing) {
        collect_named(&base, &name, 0, MOVE_SEARCH_DEPTH, &mut candidates);
    }
    candidates.retain(|c| !c.eq_ignore_ascii_case(missing));
    candidates.sort();
    candidates.dedup();

    if root.fingerprint.is_empty() {
        return match candidates.len() {
            1 => Some(candidates.remove(0)),
            _ => None,
        };
    }

    let mut scored: Vec<(usize, String)> = candidates
        .into_iter()
        .map(|path| {
            let probe = RepoRoot {
                path: path.clone(),
                ..root.clone()
            };
            let found = repos::fingerprint(&repos::scan_one(&probe, profiles));
            let hits = found
                .iter()
                .filter(|name| root.fingerprint.contains(name))
                .count();
            (hits, path)
        })
        .collect();
    scored.sort_by_key(|(hits, _)| std::cmp::Reverse(*hits));

    let (hits, path) = scored.first()?.clone();
    let needed = (root.fingerprint.len() as f32 * MOVE_MATCH_RATIO).ceil() as usize;
    if hits == 0 || hits < needed.max(1) {
        return None;
    }
    // Two folders that match equally well are not an answer, they are a
    // question; offering either one would be a guess dressed as a finding.
    if scored.iter().filter(|(h, _)| *h == hits).count() > 1 {
        return None;
    }
    Some(path)
}

/// Where to start looking: the nearest directory above the missing folder that
/// still exists, and the one above that, so a folder moved to a sibling of its
/// old parent is still in reach.
fn search_bases(missing: &str) -> Vec<std::path::PathBuf> {
    let mut bases = Vec::new();
    let mut current = Path::new(missing).parent();
    while let Some(dir) = current {
        if dir.is_dir() {
            bases.push(dir.to_path_buf());
            if let Some(up) = dir.parent() {
                if up.is_dir() && bases.len() < 2 {
                    bases.push(up.to_path_buf());
                }
            }
            break;
        }
        current = dir.parent();
    }
    bases
}

fn collect_named(dir: &Path, name: &str, depth: usize, max: usize, out: &mut Vec<String>) {
    if depth > max {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let entry_name = entry.file_name();
        let entry_name = entry_name.to_string_lossy();
        if repos::SKIP_DIRS.iter().any(|s| entry_name.as_ref() == *s) {
            continue;
        }
        if entry_name.eq_ignore_ascii_case(name) {
            out.push(entry.path().to_string_lossy().replace('\\', "/"));
            // A folder does not contain itself; nothing below a hit can be it.
            continue;
        }
        collect_named(&entry.path(), name, depth + 1, max, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PlatformAccount;
    use std::process::Command;

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

    fn root_at(path: &str, fingerprint: &[&str]) -> RepoRoot {
        RepoRoot {
            path: path.to_string(),
            profile_id: "p1".to_string(),
            platform: Platform::Github,
            fingerprint: fingerprint.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn init(path: &Path, remote: &str) {
        std::fs::create_dir_all(path).unwrap();
        let p = path.to_string_lossy().replace('\\', "/");
        Command::new("git")
            .args(["init", "-q", &p])
            .output()
            .expect("git must be on PATH");
        Command::new("git")
            .args(["-C", &p, "remote", "add", "origin", remote])
            .output()
            .unwrap();
    }

    fn temp(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("gam-doctor-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The finding a user acts on: the folder is gone, and this is where it
    /// went. A sibling that merely shares the name must not be offered, which
    /// is the whole reason the repositories inside are what decide.
    #[test]
    fn a_moved_folder_is_recognised_by_what_is_inside_it() {
        let dir = temp("moved");
        let old = dir.join("old-parent").join("work");
        std::fs::create_dir_all(&old).unwrap();

        let moved = dir.join("new-parent").join("work");
        init(&moved.join("a"), "git@github.com:octo/alpha.git");
        init(&moved.join("b"), "git@github.com:octo/beta.git");

        // Same name, different contents: not the folder that moved.
        let decoy = dir.join("decoy").join("work");
        init(&decoy.join("c"), "git@github.com:octo/gamma.git");

        let root = root_at(
            &old.to_string_lossy().replace('\\', "/"),
            &["octo/alpha", "octo/beta"],
        );
        std::fs::remove_dir_all(&old).unwrap();

        let found = find_moved(&root, &[profile()]).expect("the folder must be found");
        assert_eq!(found, moved.to_string_lossy().replace('\\', "/"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two equally good candidates are a question, not an answer.
    #[test]
    fn two_folders_that_match_equally_well_are_not_offered() {
        let dir = temp("ambiguous");
        let old = dir.join("home").join("work");
        std::fs::create_dir_all(&old).unwrap();
        for copy in ["one", "two"] {
            init(
                &dir.join(copy).join("work").join("a"),
                "git@github.com:octo/alpha.git",
            );
        }
        let root = root_at(&old.to_string_lossy().replace('\\', "/"), &["octo/alpha"]);
        std::fs::remove_dir_all(&old).unwrap();

        assert_eq!(find_moved(&root, &[profile()]), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A folder still in place is not a finding, and nothing is searched for.
    #[test]
    fn a_folder_that_is_where_it_belongs_reports_nothing_moved() {
        let dir = temp("present");
        init(&dir.join("a"), "git@github.com:octo/alpha.git");
        let root = root_at(&dir.to_string_lossy().replace('\\', "/"), &["octo/alpha"]);
        let watched = watch(&GuardSettings::default(), &[profile()], &[root]);
        assert_eq!(watched.len(), 1);
        // The rule is not written in this test's environment, so the state says
        // what is actually wrong rather than claiming the folder is gone.
        assert_eq!(watched[0].state, "no-rule");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Cleaning a folder takes back exactly what the rule supplies, and leaves
    /// everything else in the repository's config alone.
    #[test]
    fn cleaning_removes_the_keys_that_beat_the_rule_and_nothing_else() {
        let dir = temp("clean");
        let repo = dir.join("demo");
        init(&repo, "git@github.com:octo/demo.git");
        let path = repo.to_string_lossy().replace('\\', "/");
        git::repo_config_set_local(&path, "user.email", "stranger@example.com").unwrap();
        git::repo_config_set_local(&path, "user.name", "Stranger").unwrap();
        git::repo_config_set_local(&path, "core.autocrlf", "true").unwrap();

        let root = root_at(&dir.to_string_lossy().replace('\\', "/"), &[]);
        let profiles = [profile()];

        let before = inspect(
            &root,
            &profiles[0],
            &GuardSettings::default(),
            "",
            &profiles,
        );
        assert_eq!(before.overrides.len(), 1);
        assert_eq!(before.overrides[0].relative, "demo");
        assert!(!before.ok);

        assert_eq!(clean_overrides(&root, &profiles).unwrap(), 1);
        assert_eq!(git::repo_config_get_local(&path, "user.email"), None);
        assert_eq!(
            git::repo_config_get_local(&path, "core.autocrlf").as_deref(),
            Some("true"),
            "a setting the rule does not supply is not ours to remove"
        );

        let after = inspect(
            &root,
            &profiles[0],
            &GuardSettings::default(),
            "",
            &profiles,
        );
        assert!(after.overrides.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A repository that routes its own hooks is reported, never rewritten:
    /// that setting is the project's, and husky writes it on every install.
    #[test]
    fn a_repository_with_its_own_hooks_path_is_reported_not_changed() {
        let dir = temp("bypass");
        let repo = dir.join("demo");
        init(&repo, "git@github.com:octo/demo.git");
        let path = repo.to_string_lossy().replace('\\', "/");
        git::repo_config_set_local(&path, "core.hooksPath", ".husky/_").unwrap();

        let root = root_at(&dir.to_string_lossy().replace('\\', "/"), &[]);
        let profiles = [profile()];
        let status = inspect(
            &root,
            &profiles[0],
            &GuardSettings::default(),
            "",
            &profiles,
        );
        assert_eq!(status.bypassed.len(), 1);
        assert_eq!(status.bypassed[0].detail, ".husky/_");

        clean_overrides(&root, &profiles).unwrap();
        assert_eq!(
            git::repo_config_get_local(&path, "core.hooksPath").as_deref(),
            Some(".husky/_")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
