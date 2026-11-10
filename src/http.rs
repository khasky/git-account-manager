//! The one HTTP client the app makes requests with.

use reqwest::Client;
use std::sync::OnceLock;

/// One client, one connection pool, one TLS configuration.
///
/// `Client::new()` per call built all three from scratch every time, so no
/// connection was ever reused and every request paid a fresh handshake — on a
/// path that fires several times per profile (verify the token, list the keys,
/// upload one). The User-Agent lives here too: GitHub rejects requests without
/// one, and setting it in ten call sites is ten chances to forget.
pub fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent("git-account-manager")
            .build()
            .expect("the HTTP client is built from static configuration")
    })
}
