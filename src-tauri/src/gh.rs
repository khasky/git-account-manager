//! Keeps the GitHub CLI's active account in step with the active profile.
//!
//! `gh` holds several accounts per host and picks one with `gh auth switch`;
//! this app never sees its tokens and never writes its config. Everything
//! here is best effort: a machine without `gh`, or an account not logged in
//! there, leaves the rest of a profile switch untouched.

use crate::proc::hidden_command;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GhProbe {
    /// `gh` answered on `PATH`.
    pub available: bool,
    /// Logins `gh` is signed in to on github.com.
    pub logins: Vec<String>,
    /// The login `gh` currently acts as.
    pub active: Option<String>,
}

pub fn probe() -> GhProbe {
    let output = hidden_command("gh")
        .args([
            "auth",
            "status",
            "--hostname",
            "github.com",
            "--json",
            "hosts",
        ])
        .output();
    match output {
        Ok(out) => parse_status(&String::from_utf8_lossy(&out.stdout)),
        Err(_) => GhProbe {
            available: false,
            logins: Vec::new(),
            active: None,
        },
    }
}

/// `gh auth status --json hosts` answers with the accounts per host; a `gh`
/// with nobody signed in still answers, with an empty list.
fn parse_status(json: &str) -> GhProbe {
    let accounts: Vec<serde_json::Value> = serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v["hosts"]["github.com"].as_array().cloned())
        .unwrap_or_default();
    let login = |a: &serde_json::Value| a["login"].as_str().map(str::to_string);
    GhProbe {
        available: true,
        active: accounts
            .iter()
            .find(|a| a["active"].as_bool() == Some(true))
            .and_then(login),
        logins: accounts.iter().filter_map(login).collect(),
    }
}

pub fn switch_account(login: &str) -> Result<(), String> {
    let out = hidden_command("gh")
        .args([
            "auth",
            "switch",
            "--hostname",
            "github.com",
            "--user",
            login,
        ])
        .output()
        .map_err(|e| format!("Failed to run gh: {}", e))?;
    if out.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_logins_and_the_active_one() {
        let json = r#"{"hosts":{"github.com":[
            {"active":true,"host":"github.com","login":"work","tokenSource":"keyring"},
            {"active":false,"host":"github.com","login":"home","tokenSource":"keyring"}
        ]}}"#;
        let probe = parse_status(json);
        assert!(probe.available);
        assert_eq!(probe.logins, vec!["work", "home"]);
        assert_eq!(probe.active.as_deref(), Some("work"));
    }

    #[test]
    fn nobody_signed_in_is_available_with_no_logins() {
        let probe = parse_status(r#"{"hosts":{}}"#);
        assert!(probe.available);
        assert!(probe.logins.is_empty());
        assert_eq!(probe.active, None);
    }

    #[test]
    fn unreadable_output_is_treated_as_no_logins() {
        let probe = parse_status("not json");
        assert!(probe.logins.is_empty());
    }
}
