//! One running copy of the app.
//!
//! The first launch listens on a loopback port and records it beside the
//! state file. A later launch finds the port, asks the running copy to come
//! forward, and exits. The running copy answers with a fixed reply, so a port
//! some other program has since taken is not mistaken for this app.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use crate::events::{self, Event};

const REQUEST: &str = "git-account-manager show";
const REPLY: &str = "git-account-manager ok";

fn port_file() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("git-account-manager")
        .join("instance.port")
}

/// Asks a running copy to show its window. True when one answered.
fn signal_running() -> bool {
    let Some(port) = std::fs::read_to_string(port_file())
        .ok()
        .and_then(|text| text.trim().parse::<u16>().ok())
    else {
        return false;
    };
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(500)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    if writeln!(stream, "{REQUEST}").is_err() {
        return false;
    }
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).is_ok() && reply.trim() == REPLY
}

fn serve(listener: TcpListener) {
    for stream in listener.incoming().flatten() {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut line = String::new();
        let mut reader = BufReader::new(&stream);
        if reader.read_line(&mut line).is_ok() && line.trim() == REQUEST {
            let _ = writeln!(&stream, "{REPLY}");
            events::emit(Event::Show);
        }
    }
}

/// False when another copy is running and has been brought forward; the
/// caller then exits.
pub fn acquire() -> bool {
    if signal_running() {
        return false;
    }
    // Without a listener the app still runs; it only cannot be found by a
    // second launch.
    if let Ok(listener) = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)) {
        if let Ok(address) = listener.local_addr() {
            let path = port_file();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, address.port().to_string());
            std::thread::spawn(move || serve(listener));
        }
    }
    true
}
