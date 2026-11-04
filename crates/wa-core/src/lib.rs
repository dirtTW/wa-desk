//! Session layer for the desktop UI.
//!
//! This crate owns the `whatsapp-rust` client, translates its protocol events
//! into a small serializable vocabulary ([`UiEvent`]) and exposes the handful
//! of actions the frontend needs. The UI never touches protocol types.
//!
//! Everything here is transport agnostic: the Tauri shell forwards [`UiEvent`]
//! values to the webview, but the same events could feed a CLI or a tray app.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
pub use wa_types::*;
use whatsapp_rust::MediaRetryResult;
use whatsapp_rust::download::{DownloadParams, MediaType};
use whatsapp_rust::pair_code::PairCodeOptions;
use whatsapp_rust::prelude::*;
use whatsapp_rust::types::call::{CallAction, IncomingCall};
use whatsapp_rust::types::presence::ReceiptType;
use whatsapp_rust::voip::{CallEvent, CallHandle, VideoFrame};

mod calls;
mod channels;
mod contacts;
mod groups;
mod history;
mod media;
mod messaging;
mod status;
mod sticker_packs;

/// Durable copy of a media-message protobuf, used to download decrypted media
/// after a restart.
#[derive(Clone)]
pub struct CachedMedia {
    message: Arc<wa::Message>,
    chat: String,
    from_me: bool,
    participant: Option<String>,
}

/// One live call plus the channels the webview feeds and drains.
struct ActiveCall {
    handle: CallHandle,
    audio_in: async_channel::Sender<Vec<i16>>,
    video_in: Option<async_channel::Sender<Vec<u8>>>,
    info: CallInfo,
}

/// Set when the session shuts down.
///
/// Background work started by the session (deferred app-state retries, for
/// example) holds an `Arc<Client>`, and the client owns the SQLite store — a task
/// still sleeping on a two-minute retry therefore keeps `whatsapp.db` locked, so
/// the logout wipe fails with "used by another process". Sleeping through this
/// stopper ends the work (and releases the handle) the moment the session stops.
#[derive(Clone, Default)]
pub(crate) struct Stopper {
    stopping: Arc<std::sync::atomic::AtomicBool>,
    stop: Arc<tokio::sync::Notify>,
}

impl Stopper {
    /// Wake every waiter and make future waits return at once.
    pub(crate) fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.stop.notify_waiters();
    }

    pub(crate) fn stopped(&self) -> bool {
        self.stopping.load(Ordering::SeqCst)
    }

    /// Sleep, cut short by a shutdown. `false` means the session is stopping and
    /// the caller must give up instead of holding the client any longer.
    pub(crate) async fn pause(&self, delay: Duration) -> bool {
        if self.stopped() {
            return false;
        }
        tokio::select! {
            () = tokio::time::sleep(delay) => !self.stopped(),
            () = self.stop.notified() => false,
        }
    }
}

/// Handle to a live (or linking) WhatsApp session.
pub struct Session {
    client: Arc<Client>,
    /// Chat id -> parsed JID, filled from inbound traffic so the UI never has
    /// to parse or invent JIDs. Shared with the event handler.
    chats: Arc<Mutex<HashMap<String, Jid>>>,
    /// Saved-contact names from the phone's address book, keyed by user part.
    contacts: Arc<Mutex<HashMap<String, String>>>,
    /// Message ids of stories this account posted. Status "view" receipts name
    /// one of these ids, which is how a view is told apart from a chat receipt.
    own_statuses: Arc<Mutex<std::collections::HashSet<String>>>,
    /// Where the saved-contact names and the one-off sync marker live.
    contacts_path: PathBuf,
    /// Recently seen messages, so the UI can ask for their media later without
    /// having to ship protobuf around.
    messages: Arc<Mutex<HashMap<String, CachedMedia>>>,
    /// Where decrypted media is written for the webview to load.
    media_dir: PathBuf,
    /// Offers waiting for the user to accept or reject.
    incoming_calls: Arc<Mutex<HashMap<String, IncomingCall>>>,
    /// The one call this client is currently in.
    active_call: Arc<Mutex<Option<ActiveCall>>>,
    /// Last deliberate reconnect, used to rate-limit recovery (ban safety).
    last_reconnect_ms: AtomicU64,
    /// Chats the UI asked on-demand history for, newest last. The phone answers
    /// with the peer's canonical id, which can differ from the id the user
    /// clicked (LID vs phone number); this lets the handler report the alias.
    history_targets: Arc<Mutex<Vec<(String, i64)>>>,
    /// Durable copy of media-message protobufs, so media still downloads after
    /// a restart (history is only synced once per pairing).
    msg_dir: PathBuf,
    /// Kept alive for the lifetime of the session: dropping the handle aborts
    /// the bot.
    #[allow(dead_code)]
    handle: Mutex<Option<BotHandle>>,
    /// Cut background retries short when the session stops, so nothing keeps the
    /// client (and its SQLite store) alive after `shutdown()`.
    stopper: Stopper,
    tx: mpsc::UnboundedSender<UiEvent>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("connected", &self.client.is_connected())
            .field("logged_in", &self.client.is_logged_in())
            .finish_non_exhaustive()
    }
}

mod events;
mod session;
#[cfg(test)]
mod tests;
mod util;

pub(crate) use events::dispatch;
pub(crate) use util::*;
