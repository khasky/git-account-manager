//! Signed updates from GitHub Releases.
//!
//! Reads the same `latest.json` and minisign signatures the Tauri updater of
//! earlier versions read, with the same public key, so a copy installed before
//! this updater existed still trusts the releases made with it.

use std::collections::HashMap;
use std::path::PathBuf;

use base64::Engine;
use serde::Deserialize;

const ENDPOINT: &str =
    "https://github.com/khasky/git-account-manager/releases/latest/download/latest.json";
const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEE0MzRBMTYwN0M3N0U3NEUKUldSTzUzZDhZS0UwcEQ2UWVsNDNDMDVuK2pBcU5PaGt4Z2NSL01nRi9maHlsYlBSR21PejliMW0K";

#[derive(Deserialize)]
struct Manifest {
    version: String,
    platforms: HashMap<String, Platform>,
}

#[derive(Clone, Debug, Deserialize)]
struct Platform {
    url: String,
    signature: String,
}

#[derive(Clone, Debug)]
pub struct Update {
    pub version: String,
    platform: Platform,
}

fn target() -> Result<String, String> {
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "darwin",
        "linux" => "linux",
        other => return Err(format!("Updates are not available for {other}")),
    };
    Ok(format!("{os}-{}", std::env::consts::ARCH))
}

fn newer(manifest: Manifest, current: &str, target: &str) -> Result<Option<Update>, String> {
    let current = semver::Version::parse(current).map_err(|e| e.to_string())?;
    let version = semver::Version::parse(manifest.version.trim_start_matches('v'))
        .map_err(|e| format!("Invalid update version: {e}"))?;
    if version <= current {
        return Ok(None);
    }
    let platform = manifest
        .platforms
        .get(target)
        .cloned()
        .ok_or_else(|| format!("The update does not include {target}"))?;
    Ok(Some(Update {
        version: version.to_string(),
        platform,
    }))
}

pub async fn check() -> Result<Option<Update>, String> {
    let manifest: Manifest = crate::http::client()
        .get(ENDPOINT)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    newer(manifest, env!("CARGO_PKG_VERSION"), &target()?)
}

fn verify(public_key: &str, bytes: &[u8], signature: &str) -> Result<(), String> {
    let decode = |value: &str| {
        base64::engine::general_purpose::STANDARD
            .decode(value)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
    };
    let key = decode(public_key)
        .and_then(|key| minisign_verify::PublicKey::decode(&key).ok())
        .ok_or("Invalid update public key")?;
    let signature = decode(signature)
        .and_then(|signature| minisign_verify::Signature::decode(&signature).ok())
        .ok_or("Invalid update signature")?;
    key.verify(bytes, &signature, true)
        .map_err(|e| format!("Update signature verification failed: {e}"))
}

/// Downloads the update and verifies its signature before returning it.
pub async fn download(
    update: &Update,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<Vec<u8>, String> {
    let mut response = crate::http::client()
        .get(&update.platform.url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?;
    let total = response.content_length();
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        bytes.extend_from_slice(&chunk);
        progress(bytes.len() as u64, total);
    }
    verify(PUBLIC_KEY, &bytes, &update.platform.signature)?;
    Ok(bytes)
}

#[cfg_attr(target_os = "linux", allow(dead_code))]
fn temporary(name: &str) -> Result<PathBuf, String> {
    let directory =
        std::env::temp_dir().join(format!("git-account-manager-update-{}", std::process::id()));
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    Ok(directory.join(name))
}

/// Installs verified update bytes and starts the updated application. The
/// caller exits after a successful return.
#[cfg(target_os = "windows")]
pub fn install(bytes: &[u8]) -> Result<(), String> {
    let installer = temporary("Git-Account-Manager-setup.exe")?;
    std::fs::write(&installer, bytes).map_err(|e| e.to_string())?;
    // The NSIS installer closes the running version and relaunches the new one.
    std::process::Command::new(&installer)
        .args(["/P", "/R"])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Cannot start the installer: {e}"))
}

#[cfg(target_os = "macos")]
pub fn install(bytes: &[u8]) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let bundle = executable
        .ancestors()
        .find(|path| path.extension().is_some_and(|ext| ext == "app"))
        .ok_or("Updates require Git Account Manager.app")?
        .to_path_buf();
    let extracted = temporary("extracted")?;
    tar::Archive::new(flate2::read::GzDecoder::new(bytes))
        .unpack(&extracted)
        .map_err(|e| format!("Cannot extract the update: {e}"))?;
    let new_bundle = std::fs::read_dir(&extracted)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "app"))
        .ok_or("The update does not contain an application")?;
    let backup = bundle.with_extension("app.old");
    let _ = std::fs::remove_dir_all(&backup);
    std::fs::rename(&bundle, &backup).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::rename(&new_bundle, &bundle) {
        let _ = std::fs::rename(&backup, &bundle);
        return Err(format!("Cannot replace the application: {e}"));
    }
    let _ = std::fs::remove_dir_all(&backup);
    std::process::Command::new("open")
        .arg("-n")
        .arg(&bundle)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "linux")]
pub fn install(bytes: &[u8]) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let image = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .ok_or("Updates are only available for the AppImage")?;
    let staged = image.with_extension("update");
    std::fs::write(&staged, bytes).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
        .and_then(|()| std::fs::rename(&staged, &image))
        .map_err(|e| {
            let _ = std::fs::remove_file(&staged);
            format!("Cannot replace the AppImage: {e}")
        })?;
    std::process::Command::new(&image)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(version: &str) -> Manifest {
        serde_json::from_str(&format!(
            r#"{{"version":"{version}","notes":"","pub_date":"2026-01-01T00:00:00Z","platforms":{{"windows-x86_64":{{"signature":"c2ln","url":"https://example.com/setup.exe"}}}}}}"#
        ))
        .unwrap()
    }

    #[test]
    fn only_newer_versions_for_this_platform_are_offered() {
        let update = newer(manifest("v1.2.0"), "1.1.9", "windows-x86_64")
            .unwrap()
            .unwrap();
        assert_eq!(update.version, "1.2.0");
        assert_eq!(update.platform.url, "https://example.com/setup.exe");
        assert!(newer(manifest("1.1.9"), "1.1.9", "windows-x86_64")
            .unwrap()
            .is_none());
        assert!(newer(manifest("1.0.0"), "1.1.9", "windows-x86_64")
            .unwrap()
            .is_none());
        assert!(newer(manifest("1.2.0"), "1.1.9", "linux-x86_64").is_err());
    }

    #[test]
    fn unsigned_or_tampered_payloads_are_rejected() {
        assert!(verify(PUBLIC_KEY, b"payload", "c2ln").is_err());
        assert!(verify(PUBLIC_KEY, b"payload", "").is_err());
    }

    #[test]
    fn cargo_packager_signatures_verify() {
        // Produced by `cargo packager signer` with a throwaway key.
        let key = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDUxMzFBODU0Q0U4RUE3RTYKUldUbXA0N09WS2d4VWZPcC9URjE1NUZuQTJIVHVXVnloL0dhd1QreU5JeXBYcXVESWZRSjJockQK";
        let signature = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIGNhcmdvLXBhY2thZ2VyIHNlY3JldCBrZXkKUlVUbXA0N09WS2d4VVdYVUorazJLR3h2M0JzU0l1L2R2N2JUSFduMmdYeVNGa3hWdTZIcHQ0Y2l4SkRHZUN2V1hmNkhFb1Myd3VtdUFGdGhVQkJzRjIzWjJucmRJL3hYRmc4PQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0Mjc3MjEzCWZpbGU6cGF5bG9hZC5iaW4Kb2JMNTZRRlU1M1F0OVUwRVhBREpzcmhhYklocVFxVTBvcVljaGs2Y0p0LzdCZ0VNK1hnTXlRc0hENG9jTFltZi9yMDZ6N3orV1loaytmS2pNMUdGQ1E9PQo=";
        assert!(verify(key, b"emploidle update payload", signature).is_ok());
        assert!(verify(key, b"emploidle update payload!", signature).is_err());
        assert!(verify(PUBLIC_KEY, b"emploidle update payload", signature).is_err());
    }
}
