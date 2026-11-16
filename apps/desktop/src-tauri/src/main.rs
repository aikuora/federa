//! The desktop app's native side: a thin adapter between the runtime and the interface. It finds
//! or starts the person's runtime, holds the one connection to it with the token the web view never
//! sees, and relays the protocol's frames both ways untouched.

// Without it, a release build on Windows opens a console beside the window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod relay;
mod runtime;

use protocol::{ClientFrame, RuntimeFrame};
use tauri::{State, ipc::Channel};
use tokio::sync::{Mutex, mpsc};

use crate::relay::Relay;

/// The relay of the web view's connection; a reload of the web view connects again and replaces it.
#[derive(Default)]
struct Connection(Mutex<Option<Relay>>);

// Each command's error is the text the interface shows the person, so it is written in Spanish.

/// Reaches the runtime and connects to it, then hands every frame it sends to `frames`, in order,
/// and `null` once the connection ends.
#[tauri::command]
async fn connect(
    frames: Channel<Option<RuntimeFrame>>,
    connection: State<'_, Connection>,
) -> Result<(), String> {
    let endpoint = runtime::reach().await.map_err(|e| e.to_string())?;
    let (incoming, mut received) = mpsc::channel(32);
    let relay = Relay::connect(endpoint, incoming).await?;
    *connection.0.lock().await = Some(relay);
    tauri::async_runtime::spawn(async move {
        while let Some(frame) = received.recv().await {
            if frames.send(Some(frame)).is_err() {
                return;
            }
        }
        let _ = frames.send(None);
    });
    Ok(())
}

#[tauri::command]
async fn send(frame: ClientFrame, connection: State<'_, Connection>) -> Result<(), String> {
    match connection.0.lock().await.as_ref() {
        Some(relay) => relay.send(frame).await,
        None => Err("no hay conexión con el runtime".to_owned()),
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Connection::default())
        .invoke_handler(tauri::generate_handler![connect, send])
        .run(tauri::generate_context!())
        .expect("the app runs");
}
