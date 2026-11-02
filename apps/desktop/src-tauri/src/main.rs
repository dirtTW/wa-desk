#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Tauri shell around `wa-core`.
//!
//! The Rust side owns the WhatsApp session, the SQLite device store and the
//! protocol events. The webview is a pure view: it receives `UiEvent`s on the
//! `wa:event` channel and calls the commands in [`commands`] to act.
//!
//! Module layout:
//!
//! - [`app`] — builder, plugins, tray, event pump and the run loop
//! - [`state`] — `AppState`, session lifecycle and window helpers
//! - [`commands`] — `#[tauri::command]` handlers grouped by area
//! - [`notifications`] — Windows push notifications from the event pump
//! - [`ai`] — auto-reply engine (settings, prompts, provider, media, replies)

mod ai;
mod app;
mod commands;
mod notifications;
mod state;

fn main() {
    app::run();
}
