//! What the console binary `gam` does with its arguments.
//!
//! Kept out of `bin/gam.rs` so it compiles into the library with everything it
//! calls, and is reachable from the test suite the app already runs.

use crate::models::{AppState, Profile, RepoRoot, PLATFORMS};
use crate::{commands, credential, guard, proc, secrets, storage};
use std::io::{Read, Write};

const USAGE: &str = "\
Usage: gam <command>

  list                 list the profiles; * marks the active one
  current              the active profile, the folder rule over this
                       directory and the identity git commits with here
  set <profile>        activate a profile by name or id, as the app does
  credential <get|store|erase>
                       answer git's HTTPS credential requests; git runs this
                       on its own once the helper is switched on in the app

  -h, --help           print this message
  -V, --version        print the version
";

pub fn run(args: Vec<String>) -> i32 {
    match args.get(1).map(String::as_str) {
        Some("credential") => credential_command(args.get(2).map(String::as_str)),
        Some("list") => with_state(|state| {
            print!("{}", format_list(state));
            0
        }),
        Some("current" | "status") => with_state(current),
        Some("set") => match args.get(2) {
            Some(query) => with_state(|state| set(state, query)),
            None => {
                eprint!("{}", USAGE);
                2
            }
        },
        Some("--version" | "-V") => {
            println!("gam {}", env!("CARGO_PKG_VERSION"));
            0
        }
        None | Some("--help" | "-h" | "help") => {
            print!("{}", USAGE);
            0
        }
        Some(other) => {
            eprintln!("gam: unknown command '{}'", other);
            eprint!("{}", USAGE);
            2
        }
    }
}

fn with_state(f: impl FnOnce(&AppState) -> i32) -> i32 {
    match storage::load_state() {
        Ok(state) => f(&state),
        Err(e) => {
            eprintln!("gam: {}", secrets::redact(&e));
            1
        }
    }
}

fn format_list(state: &AppState) -> String {
    if state.profiles.is_empty() {
        return "No profiles. Add one in Git Account Manager.\n".to_string();
    }
    let mut out = String::new();
    for profile in &state.profiles {
        let accounts: Vec<String> = PLATFORMS
            .iter()
            .filter_map(|p| {
                profile
                    .account(*p)
                    .map(|a| format!("{}: {}", p, a.username))
            })
            .collect();
        out.push_str(&format!(
            "{} {} ({})\n",
            if profile.is_active { "*" } else { " " },
            profile.name,
            accounts.join(", ")
        ));
    }
    out
}

/// An id is exact; a name is matched without regard to case, and refused when
/// two profiles share it rather than activating one of them at random.
fn find_profile<'a>(state: &'a AppState, query: &str) -> Result<&'a Profile, String> {
    if let Some(profile) = state.profiles.iter().find(|p| p.id == query) {
        return Ok(profile);
    }
    let named: Vec<&Profile> = state
        .profiles
        .iter()
        .filter(|p| p.name.eq_ignore_ascii_case(query))
        .collect();
    match named.as_slice() {
        [one] => Ok(one),
        [] => Err(format!("no profile named '{}'; see `gam list`", query)),
        _ => Err(format!(
            "several profiles are named '{}'; pass the id instead",
            query
        )),
    }
}

/// The deepest folder rule over `dir`, the one whose include git reads last.
/// Matched without regard to case, as the `gitdir/i:` pattern is.
fn folder_rule<'a>(dir: &str, roots: &'a [RepoRoot]) -> Option<&'a RepoRoot> {
    let dir = guard::normalize_folder(dir).to_lowercase();
    roots
        .iter()
        .filter(|root| {
            let root = guard::normalize_folder(&root.path).to_lowercase();
            dir == root || dir.starts_with(&format!("{}/", root))
        })
        .max_by_key(|root| guard::normalize_folder(&root.path).len())
}

fn set(state: &AppState, query: &str) -> i32 {
    let profile = match find_profile(state, query) {
        Ok(profile) => profile,
        Err(e) => {
            eprintln!("gam: {}", e);
            return 1;
        }
    };
    match commands::activate_profile_core(&profile.id) {
        Ok(()) => {
            println!("Active profile: {}", profile.name);
            0
        }
        Err(e) => {
            eprintln!("gam: {}", secrets::redact(&e));
            1
        }
    }
}

fn current(state: &AppState) -> i32 {
    let active = state.profiles.iter().find(|p| p.is_active);
    println!(
        "Active profile: {}",
        active.map_or("none", |p| p.name.as_str())
    );

    let here = std::env::current_dir()
        .map(|d| d.to_string_lossy().to_string())
        .unwrap_or_default();
    match folder_rule(&here, &state.repo_roots) {
        Some(root) => {
            let owner = state
                .profiles
                .iter()
                .find(|p| p.id == root.profile_id)
                .map_or("a deleted profile", |p| p.name.as_str());
            println!(
                "Folder rule:    {} -> {} ({})",
                root.path, owner, root.platform
            );
        }
        None => println!("Folder rule:    none over this directory"),
    }

    // What git itself resolves here: the folder rule, a repository's own
    // config and the global identity, in git's order.
    let resolved = |key: &str| {
        proc::hidden_command("git")
            .args(["config", "--get", key])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|v| !v.is_empty())
    };
    // A folder rule reaches repositories only, so outside one an empty answer
    // says nothing about what a repository cloned here would get.
    let in_repo = proc::hidden_command("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .is_ok_and(|o| o.status.success());
    match (resolved("user.name"), resolved("user.email")) {
        (Some(name), Some(email)) => println!("Git identity:   {} <{}>", name, email),
        _ if !in_repo => println!("Git identity:   none outside a repository"),
        _ => println!("Git identity:   none; git refuses to commit here"),
    }
    0
}

/// `store` and `erase` are accepted and do nothing: the credential belongs to
/// this app and its credential store, so there is nothing for git to teach us,
/// and a token deleted here because a server answered 401 would leave the user
/// with nothing and no way back.
fn credential_command(operation: Option<&str>) -> i32 {
    match operation {
        Some("get") => get(),
        Some("store" | "erase") => 0,
        _ => {
            eprint!("{}", USAGE);
            2
        }
    }
}

/// Always exits 0. A helper that fails tells git only that it failed; one that
/// prints nothing lets git carry on to the next helper or to the prompt, which
/// is what a user whose token is missing needs to happen.
fn get() -> i32 {
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        return silent(&format!("could not read the request: {}", e));
    }

    let request = credential::parse_request(&input);

    let state = match storage::load_state() {
        Ok(state) => state,
        Err(e) => return silent(&e),
    };

    let Some(answer) = credential::answer(&request, &state, &credential::Keychain) else {
        let owner = match request.username.as_str() {
            "" => "the active profile".to_string(),
            name => format!("the profile with account '{}'", name),
        };
        return silent(&format!(
            "no HTTPS credential for {}; add one for {} in Git Account Manager",
            request.host, owner
        ));
    };

    print!("{}", answer);
    let _ = std::io::stdout().flush();
    0
}

/// Git shows a helper's stderr to whoever ran the command, so the reason for an
/// unanswered request reaches them instead of a bare password prompt.
fn silent(reason: &str) -> i32 {
    eprintln!("gam: {}", secrets::redact(reason));
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_store_or_erase_request_is_accepted_and_does_nothing() {
        assert_eq!(credential_command(Some("store")), 0);
        assert_eq!(credential_command(Some("erase")), 0);
    }

    #[test]
    fn an_unknown_command_reports_usage_rather_than_succeeding() {
        assert_eq!(run(vec!["gam".to_string(), "frobnicate".to_string()]), 2);
        assert_eq!(credential_command(Some("sign")), 2);
        assert_eq!(credential_command(None), 2);
    }

    fn profile(id: &str, name: &str, active: bool) -> Profile {
        Profile {
            id: id.to_string(),
            name: name.to_string(),
            default_platform: None,
            github: Some(crate::models::PlatformAccount {
                username: format!("{}-gh", id),
                git_name: name.to_string(),
                git_email: format!("{}@example.com", id),
                ssh_private_key_path: String::new(),
                ssh_public_key_path: String::new(),
                sign_commits: false,
                token: None,
            }),
            gitlab: None,
            bitbucket: None,
            azure: None,
            is_active: active,
        }
    }

    fn root(path: &str, profile_id: &str) -> RepoRoot {
        RepoRoot {
            path: path.to_string(),
            profile_id: profile_id.to_string(),
            platform: crate::models::Platform::Github,
            fingerprint: vec![],
        }
    }

    fn state() -> AppState {
        AppState {
            profiles: vec![
                profile("p1", "Work", true),
                profile("p2", "Personal", false),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn the_list_marks_the_active_profile_and_names_its_accounts() {
        assert_eq!(
            format_list(&state()),
            "* Work (github: p1-gh)\n  Personal (github: p2-gh)\n"
        );
        assert!(format_list(&AppState::default()).starts_with("No profiles"));
    }

    #[test]
    fn a_profile_is_found_by_id_or_by_name_in_any_case() {
        let state = state();
        assert_eq!(find_profile(&state, "p2").unwrap().name, "Personal");
        assert_eq!(find_profile(&state, "personal").unwrap().id, "p2");
        assert!(find_profile(&state, "nobody").is_err());
    }

    #[test]
    fn a_name_two_profiles_share_is_refused_rather_than_guessed() {
        let mut state = state();
        state.profiles.push(profile("p3", "work", false));
        assert!(find_profile(&state, "Work")
            .unwrap_err()
            .contains("several"));
        assert_eq!(find_profile(&state, "p3").unwrap().id, "p3");
    }

    #[test]
    fn the_deepest_folder_rule_over_a_directory_is_the_one_reported() {
        let roots = vec![
            root("Z:/gam-missing/repos", "p1"),
            root("Z:/gam-missing/repos/client", "p2"),
        ];
        let pick = |dir: &str| folder_rule(dir, &roots).map(|r| r.profile_id.as_str());

        assert_eq!(pick("Z:/gam-missing/repos/client/app"), Some("p2"));
        assert_eq!(pick("z:/GAM-MISSING/repos/other"), Some("p1"));
        assert_eq!(pick("Z:/gam-missing/repos"), Some("p1"));
        // A sibling that only shares the prefix is not under the folder.
        assert_eq!(pick("Z:/gam-missing/repos-old"), None);
    }

    #[test]
    fn set_without_a_profile_reports_usage() {
        assert_eq!(run(vec!["gam".to_string(), "set".to_string()]), 2);
    }

    #[test]
    fn help_and_version_succeed() {
        assert_eq!(run(vec!["gam".to_string()]), 0);
        assert_eq!(run(vec!["gam".to_string(), "--help".to_string()]), 0);
        assert_eq!(run(vec!["gam".to_string(), "--version".to_string()]), 0);
    }
}
