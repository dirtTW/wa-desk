//! AI auto-reply.
//!
//! The API key is encrypted at rest with AES-256-GCM. A random 32-byte key kept
//! in `ai.key` (0600 on Unix) is used; the ciphertext lives in `ai.json`. The
//! plaintext key never leaves the Rust side except as the `Authorization`
//! header of the provider request.
//!
//! Module layout:
//!
//! - [`state`] — settings, the encrypted key and the bounded chat context
//! - [`prompt`] — system prompts, guards, mention matching and reply gating
//! - [`provider`] — chat-completion calls and the vision sticker picker
//! - [`media`] — photo/video inputs for the vision model
//! - [`reply`] — the live event handler that decides and sends replies
//! - [`commands`] — the `#[tauri::command]` surface used by the settings UI
//! - [`util`] — clock, randomness and small text helpers

pub(crate) mod commands;
mod media;
mod prompt;
mod provider;
mod reply;
mod state;
mod util;

#[cfg(test)]
mod tests;

pub use reply::handle_event;
pub use state::AiState;
