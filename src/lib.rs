pub mod cli;
mod commands;
mod credential;
mod doctor;
mod events;
mod gh;
mod git;
mod guard;
mod hooks;
mod http;
mod instance;
mod migrate;
mod models;
mod oauth;
mod openssh_integration;
mod platform;
mod proc;
mod repos;
mod secrets;
mod ssh;
mod storage;
mod tray;
mod ui;
mod updater;

use std::sync::OnceLock;

/// The runtime the network commands run on. The window's own executor drives
/// everything else; `reqwest` needs this one.
pub(crate) fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("the async runtime starts")
    })
}

pub fn run() {
    let Some(events) = events::listen() else {
        return;
    };
    // A second launch brings the running window forward and exits.
    if !instance::acquire() {
        return;
    }

    // Re-applied on every launch because the registry value and
    // `core.sshCommand` live outside this app and can be changed by anything.
    // `openssh_integration` is a no-op off Windows.
    let openssh_enabled = storage::load_state()
        .map(|s| s.oauth.use_openssh_for_git_tools)
        .unwrap_or(false);
    if openssh_enabled {
        let _ = openssh_integration::apply(true);
    }

    // Off the main thread: the clean-up walks every watched folder and the
    // window must not wait for it.
    std::thread::spawn(|| {
        let _ = storage::with_lock(|| {
            let mut state = storage::load_state()?;

            // An install upgraded from the per-repository model still carries
            // this app's identity and its hook inside its repositories, and a
            // local `user.email` beats the folder rule that replaced it. Taken
            // back once.
            if !state.migrated_to_folder_rules {
                migrate::run(&state);
                state.migrated_to_folder_rules = true;
                storage::save_state(&state)?;
            }

            // Everything an activation writes lives where anything can edit
            // it: `~/.ssh/config`, the global identity, the helper path (stale
            // after a reinstall elsewhere), the folder rules and the hooks. So
            // the active profile is re-applied on every launch, the same way a
            // click on it applies it. It is also what first writes the rules in
            // the shape this version reads, on an install that has saved
            // nothing since the upgrade.
            commands::sync_machine(&state)
        });
    });

    ui::run(events);
}
