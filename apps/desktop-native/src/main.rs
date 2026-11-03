//! WA Desk, rendered natively in Rust.
//!
//! This binary is the "no WebView" front end: it owns the window, paints with
//! `egui` and talks to the protocol layer directly. Formatting, labels, previews,
//! ticks and every other decision still come from `wa-ui`, the same crate the
//! WebView version uses, so the two front ends cannot drift apart.
//!
//! `--selftest` builds the model headlessly, applies a scripted set of protocol
//! events and prints what the window would show, then exits non-zero if any
//! expectation fails. That is how the UI is verified in CI without a display.

mod app;
mod model;

use std::sync::Arc;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wa_core=info,wa_desktop_native=info".into()),
        )
        .init();

    if std::env::args().any(|arg| arg == "--selftest") {
        match model::selftest() {
            Ok(report) => {
                println!("{report}");
                return;
            }
            Err(err) => {
                eprintln!("selftest failed: {err}");
                std::process::exit(1);
            }
        }
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<wa_types::UiEvent>();
    let (command_tx, command_rx) = std::sync::mpsc::channel::<model::Command>();

    let store = model::store_path();
    let media = model::media_dir();
    tracing::info!(store = %store, media = %media, "starting session");

    let handle = runtime.handle().clone();
    let session_slot: Arc<std::sync::Mutex<Option<Arc<wa_core::Session>>>> =
        Arc::new(std::sync::Mutex::new(None));
    let slot = session_slot.clone();
    handle.spawn(async move {
        match wa_core::Session::start(&store, media.into(), event_tx).await {
            Ok(session) => {
                *slot.lock().expect("session slot") = Some(session);
            }
            Err(err) => tracing::error!(%err, "session start failed"),
        }
    });

    // Commands from the UI thread are executed on the runtime.
    let slot = session_slot.clone();
    handle.spawn(async move {
        while let Ok(command) = command_rx.recv() {
            let Some(session) = slot.lock().expect("session slot").clone() else {
                continue;
            };
            match command {
                model::Command::SendText { chat, text } => {
                    if let Err(err) = session.send_text(&chat, &text).await {
                        tracing::warn!(%err, "send failed");
                    }
                }
                model::Command::MarkRead { chat } => {
                    let _ = session.mark_read(&chat).await;
                }
                model::Command::Typing { chat, typing } => {
                    let _ = session.set_typing(&chat, typing).await;
                }
            }
        }
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 780.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("WA Desk"),
        ..Default::default()
    };

    let app = app::NativeApp::new(event_rx, command_tx, handle);
    if let Err(err) = eframe::run_native(
        "WA Desk",
        options,
        Box::new(move |cc| Ok(Box::new(app::WaDesk::new(cc, app)))),
    ) {
        eprintln!("could not open the window: {err}");
        std::process::exit(1);
    }
}
