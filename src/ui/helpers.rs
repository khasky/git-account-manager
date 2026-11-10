//! What the window computes from the backend's answers, kept apart from the
//! drawing so it can be tested on its own.

use crate::commands::SaveProgress;
use crate::models::Platform;
use crate::platform::azure_org_from_input;
use crate::repos::FolderRepo;

/// Where a platform that will not take a key from a token has it pasted.
pub const HUGGINGFACE_KEYS_URL: &str = "https://huggingface.co/settings/keys";

pub fn profile_url(platform: Platform, username: &str) -> String {
    let base = match platform {
        Platform::Github => "https://github.com/",
        Platform::Gitlab => "https://gitlab.com/",
        Platform::Bitbucket => "https://bitbucket.org/",
        // An Azure DevOps account here is an organization, so this is its home.
        Platform::Azure => "https://dev.azure.com/",
        Platform::Codeberg => "https://codeberg.org/",
        Platform::Huggingface => "https://huggingface.co/",
    };
    format!("{base}{username}")
}

/// Where each platform issues the token git authenticates HTTPS remotes with.
/// Azure DevOps issues tokens per organization; without one, `dev.azure.com`
/// opens the organization the user last visited.
pub fn token_page_url(platform: Platform, username: &str) -> String {
    match platform {
        Platform::Github => "https://github.com/settings/tokens".into(),
        Platform::Gitlab => "https://gitlab.com/-/user_settings/personal_access_tokens".into(),
        Platform::Bitbucket => "https://id.atlassian.com/manage-profile/security/api-tokens".into(),
        Platform::Azure => azure_settings_url(username, "tokens"),
        Platform::Codeberg => "https://codeberg.org/user/settings/applications".into(),
        Platform::Huggingface => "https://huggingface.co/settings/tokens".into(),
    }
}

/// Azure DevOps keeps SSH keys per organization, and a token cannot add one,
/// so this is where the user pastes it.
pub fn azure_keys_page_url(organization: &str) -> String {
    azure_settings_url(organization, "keys")
}

fn azure_settings_url(input: &str, page: &str) -> String {
    match azure_org_from_input(input) {
        Ok(org) => format!(
            "https://dev.azure.com/{}/_usersSettings/{page}",
            urlencoding::encode(&org)
        ),
        Err(_) => "https://dev.azure.com/".into(),
    }
}

/// One row of the hierarchy a folder rule covers.
#[derive(Debug, PartialEq, Eq)]
pub struct TreeRow {
    /// How far to indent: the number of directories between it and the folder.
    pub depth: usize,
    /// The directories above it, without a trailing slash, or empty at the top.
    pub prefix: String,
    /// The directory the repository itself is in.
    pub label: String,
}

/// Where a repository sits under its watched folder. "." is the watched folder
/// itself being a repository.
pub fn tree_row(repo: &FolderRepo) -> TreeRow {
    let parts: Vec<&str> = if repo.relative == "." {
        Vec::new()
    } else {
        repo.relative
            .split('/')
            .filter(|part| !part.is_empty())
            .collect()
    };
    match parts.split_last() {
        None => TreeRow {
            depth: 0,
            prefix: String::new(),
            label: repo.name.clone(),
        },
        Some((label, above)) => TreeRow {
            depth: above.len(),
            prefix: above.join("/"),
            label: label.to_string(),
        },
    }
}

/// Where the save's progress bar stands, in percent. Every folder owns an equal
/// share of the bar and applying the rules owns one share more; inside a folder
/// the share fills with the repositories read.
pub fn save_percent(p: &SaveProgress) -> u32 {
    let shares = (p.folder_count + 1) as f64;
    let in_folder = if p.repos_total > 0 {
        p.repos_done as f64 / p.repos_total as f64
    } else {
        0.
    };
    let done = if p.stage == "apply" {
        p.folder_count as f64
    } else {
        p.folder_index as f64 + in_folder
    };
    (done / shares * 100.).round().clamp(0., 100.) as u32
}

/// The key pair an account actually carries, as opposed to one merely
/// highlighted in the picker: browsing the existing-key list writes the
/// highlighted paths into the section before anything is uploaded, and that
/// list holds the keys other profiles depend on. `key_uploaded` is set by
/// generating, by uploading and by loading a saved profile, and nothing else.
pub fn attached_key(key_uploaded: bool, private: &str, public: &str) -> (String, String) {
    if key_uploaded {
        (private.to_string(), public.to_string())
    } else {
        (String::new(), String::new())
    }
}

/// `m:ss`, as the auth flows count down.
pub fn clock(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn azure_links_open_the_organization_or_fall_back_to_the_portal() {
        assert_eq!(
            token_page_url(Platform::Azure, "dev.azure.com/thebestplan"),
            "https://dev.azure.com/thebestplan/_usersSettings/tokens"
        );
        assert_eq!(
            azure_keys_page_url("https://dev.azure.com/thebestplan"),
            "https://dev.azure.com/thebestplan/_usersSettings/keys"
        );
        assert_eq!(
            token_page_url(Platform::Azure, ""),
            "https://dev.azure.com/"
        );
        assert_eq!(
            token_page_url(Platform::Azure, "acme corp"),
            "https://dev.azure.com/"
        );
        assert_eq!(
            profile_url(Platform::Gitlab, "n.petrenko"),
            "https://gitlab.com/n.petrenko"
        );
    }

    fn repo(relative: &str, name: &str) -> FolderRepo {
        FolderRepo {
            path: format!("D:/repos/work/{relative}"),
            relative: relative.into(),
            name: name.into(),
            root_path: "D:/repos/work".into(),
            remote_url: "git@github.com:octo/demo.git".into(),
            full_name: "octo/demo".into(),
            foreign_host: false,
        }
    }

    #[test]
    fn rows_indent_by_depth_and_the_folder_itself_sits_at_the_top() {
        let row = |depth, prefix: &str, label: &str| TreeRow {
            depth,
            prefix: prefix.into(),
            label: label.into(),
        };
        assert_eq!(tree_row(&repo("demo", "demo")), row(0, "", "demo"));
        assert_eq!(
            tree_row(&repo("group/demo", "demo")),
            row(1, "group", "demo")
        );
        assert_eq!(
            tree_row(&repo("group/deep/demo", "demo")),
            row(2, "group/deep", "demo")
        );
        assert_eq!(tree_row(&repo(".", "work")), row(0, "", "work"));
    }

    fn progress(
        stage: &'static str,
        index: usize,
        count: usize,
        done: usize,
        total: usize,
    ) -> SaveProgress {
        SaveProgress {
            stage,
            folder: "D:/repos/work".into(),
            folder_index: index,
            folder_count: count,
            repos_done: done,
            repos_total: total,
        }
    }

    #[test]
    fn the_bar_gives_each_folder_and_the_rules_an_equal_share() {
        assert_eq!(save_percent(&progress("scan", 0, 2, 0, 0)), 0);
        assert_eq!(save_percent(&progress("scan", 1, 2, 0, 0)), 33);
        assert_eq!(save_percent(&progress("scan", 0, 1, 11, 22)), 25);
        assert_eq!(save_percent(&progress("scan", 0, 1, 22, 22)), 50);
        assert_eq!(save_percent(&progress("apply", 1, 1, 0, 0)), 50);
        assert_eq!(save_percent(&progress("apply", 3, 3, 0, 0)), 75);
    }

    #[test]
    fn only_a_key_the_account_carries_is_offered_for_deletion() {
        let (private, public) = (
            "/home/a/.ssh/id_ed25519_gam_github_octo",
            "/home/a/.ssh/id_ed25519_gam_github_octo.pub",
        );
        assert_eq!(
            attached_key(false, private, public),
            (String::new(), String::new())
        );
        assert_eq!(
            attached_key(true, private, public),
            (private.to_string(), public.to_string())
        );
        assert_eq!(attached_key(true, "", ""), (String::new(), String::new()));
        assert_eq!(clock(899), "14:59");
        assert_eq!(clock(5), "0:05");
    }
}
