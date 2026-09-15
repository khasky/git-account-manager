//! The console companion to the desktop app, and the credential helper git
//! runs.
//!
//! Its own binary rather than a branch of the app's: the app is built for the
//! Windows GUI subsystem, where a process has no console handles to speak the
//! credential protocol over, and it holds a single-instance lock that a second
//! launch is refused by.

fn main() {
    std::process::exit(git_account_manager_lib::cli::run(
        std::env::args().collect(),
    ));
}
