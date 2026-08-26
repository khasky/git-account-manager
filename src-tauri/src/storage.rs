use crate::{models::AppState, repos, secrets};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Runs a read-modify-write of the state with no other one interleaved.
///
/// Every command that changes anything performs the same three steps: load the
/// state, mutate it, save it. Tauri runs commands on a thread pool, so two
/// overlapping ones would both read the same file, each mutate its own copy,
/// and the second write would drop the first one's change with nothing
/// reported — a profile saved from the form while the tray switched the active
/// one, and the activation is simply gone.
///
/// The machine sync that follows a save is inside the lock too: it rewrites
/// `~/.ssh/config` and the managed region of `~/.gitconfig` from the state it
/// was handed, and two of those interleaving leave whichever finished last on
/// disk rather than whichever was correct.
pub fn with_lock<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    static LOCK: Mutex<()> = Mutex::new(());
    // A command that panicked poisons the lock. `save_state` writes through a
    // temp file and a rename, so what is on disk is a whole state either way
    // and the next command may proceed.
    let _guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    f()
}

fn storage_dir() -> PathBuf {
    let data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    data_dir.join("git-account-manager")
}

fn storage_path() -> PathBuf {
    let dir = storage_dir();
    fs::create_dir_all(&dir).ok();
    dir.join("profiles.json")
}

/// Loads the persisted state.
///
/// A missing file is the normal first-run case and yields a default state.
/// Anything else — an unreadable file or JSON that fails to parse — returns an
/// error instead of silently falling back to an empty state, because the next
/// `save_state` would otherwise overwrite real data. On a parse failure the raw
/// bytes are first copied to a timestamped backup so they are never lost.
pub fn load_state() -> Result<AppState, String> {
    let path = storage_path();

    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(AppState::default()),
        Err(e) => return Err(format!("Could not read {}: {}", path.display(), e)),
    };

    match serde_json::from_str::<AppState>(&content) {
        Ok(mut state) => {
            repos::mark_pre_existing_exceptions(&state.repo_roots, &mut state.bindings);
            if secrets::migrate_plaintext_tokens(&mut state)? {
                save_state(&state)?;
            }
            Ok(state)
        }
        Err(e) => {
            let hint = match backup_corrupt_file(&path, &content) {
                Some(backup) => format!(
                    " A backup of the original file was saved to {}.",
                    backup.display()
                ),
                None => String::new(),
            };
            Err(format!(
                "Could not parse {}: {}.{}",
                path.display(),
                e,
                hint
            ))
        }
    }
}

/// Copies the unparseable file to `profiles.corrupt-<unix-seconds>.json` so the
/// original data is preserved before anything can overwrite it.
fn backup_corrupt_file(path: &Path, content: &str) -> Option<PathBuf> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup = path.with_file_name(format!("profiles.corrupt-{}.json", ts));
    fs::write(&backup, content).ok().map(|_| backup)
}

/// Persists the state with an atomic write (temp file + rename) so an
/// interrupted write cannot leave `profiles.json` half-written and corrupt.
pub fn save_state(state: &AppState) -> Result<(), String> {
    let path = storage_path();
    let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;

    let tmp = path.with_file_name("profiles.json.tmp");
    fs::write(&tmp, &json).map_err(|e| format!("Could not write {}: {}", tmp.display(), e))?;
    fs::rename(&tmp, &path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Could not save {}: {}", path.display(), e)
    })
}
