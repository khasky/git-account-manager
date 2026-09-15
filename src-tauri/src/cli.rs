//! What the console binary `gam` does with its arguments.
//!
//! Kept out of `bin/gam.rs` so it compiles into the library with everything it
//! calls, and is reachable from the test suite the app already runs.

use crate::{credential, secrets, storage};
use std::io::{Read, Write};

const USAGE: &str = "\
Usage: gam credential <get|store|erase>

Answers git's credential requests for the hosts Git Account Manager holds an
account and an HTTPS token for. Git runs this on its own once the HTTPS
credential helper is switched on in the app; there is nothing to type.

  -h, --help       print this message
  -V, --version    print the version
";

pub fn run(args: Vec<String>) -> i32 {
    match args.get(1).map(String::as_str) {
        Some("credential") => credential_command(args.get(2).map(String::as_str)),
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
        return silent(&format!(
            "no HTTPS credential for {}; add one for the active profile in Git Account Manager",
            request.host
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

    #[test]
    fn help_and_version_succeed() {
        assert_eq!(run(vec!["gam".to_string()]), 0);
        assert_eq!(run(vec!["gam".to_string(), "--help".to_string()]), 0);
        assert_eq!(run(vec!["gam".to_string(), "--version".to_string()]), 0);
    }
}
