//! `#[tauri::command]` handlers grouped by area.
//!
//! `main.rs` registers them with their full path (for example
//! `commands::session::get_status`). That path is also how `generate_handler!`
//! resolves the wrapper macros emitted by `#[tauri::command]`, so every command
//! here is `pub` and lives directly in its area module.

pub mod calls;
pub mod chat;
pub mod contacts;
pub mod groups;
pub mod history;
pub mod links;
pub mod media;
pub mod messages;
pub mod session;
pub mod status;
pub mod ui;
pub mod update;
