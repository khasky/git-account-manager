//! Child processes that must not flash a console window.

use std::process::Command;

/// Windows opens a console window for every child process unless
/// `CREATE_NO_WINDOW` is set, and this app shells out to `git` and `ssh` on
/// paths a user hits repeatedly — one flash per call.
pub fn hidden_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    hide_window(&mut cmd);
    cmd
}

#[cfg(windows)]
fn hide_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_window(_cmd: &mut Command) {}
