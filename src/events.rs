//! What the backend tells the window while a command is still running.
//!
//! Commands run on worker threads and the window lives on the main one, so the
//! two talk through a channel the window drains. Before the window exists, and
//! in the console helper, nothing is listening and an event is dropped.

use std::sync::OnceLock;

use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};

use crate::commands::SaveProgress;

pub enum Event {
    /// A profile save has reached another step.
    SaveProgress(SaveProgress),
    /// Text the window puts on the clipboard, which only its thread can reach.
    Clipboard(String),
    /// The stored profiles changed: the tray menu is rebuilt from them.
    ProfilesChanged,
    /// A second launch asked this one to come forward.
    Show,
}

static SENDER: OnceLock<UnboundedSender<Event>> = OnceLock::new();

/// Opens the channel. Only the first call gets the receiver.
pub fn listen() -> Option<UnboundedReceiver<Event>> {
    let (sender, receiver) = unbounded();
    SENDER.set(sender).ok().map(|()| receiver)
}

pub fn emit(event: Event) {
    if let Some(sender) = SENDER.get() {
        let _ = sender.unbounded_send(event);
    }
}
