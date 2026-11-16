use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use kernel::Kernel;
use protocol::{ClientFrame, Hello, RuntimeFrame, VERSION};

/// One client, from its `Hello` to its close. The runtime answers the client's `Hello` with its
/// own, and closes the connection when the client's is missing or declares another version; then
/// every command goes to the kernel and its outcome goes back as the reply of that request.
pub(crate) async fn run(mut socket: WebSocket, kernel: Arc<Kernel>) {
    let Some(ClientFrame::Hello(hello)) = receive(&mut socket).await else {
        return;
    };
    // Answered even when the versions differ, so the client can tell the person which one runs.
    let ours = RuntimeFrame::Hello(Hello { protocol: VERSION });
    if !send(&mut socket, &ours).await || hello.protocol != VERSION {
        return;
    }
    // A frame that is not a command carries no request id to reply to, so it ends the connection.
    while let Some(ClientFrame::Command { id, command }) = receive(&mut socket).await {
        let outcome = kernel.handle(command).await;
        if !send(&mut socket, &RuntimeFrame::Reply { id, outcome }).await {
            return;
        }
    }
}

/// The next frame, or `None` when the client closed or sent something that is not one.
async fn receive(socket: &mut WebSocket) -> Option<ClientFrame> {
    loop {
        match socket.recv().await?.ok()? {
            Message::Text(text) => return serde_json::from_str(&text).ok(),
            Message::Ping(_) | Message::Pong(_) => continue,
            Message::Binary(_) | Message::Close(_) => return None,
        }
    }
}

/// Whether the frame reached the client.
async fn send(socket: &mut WebSocket, frame: &RuntimeFrame) -> bool {
    let text = serde_json::to_string(frame).expect("a frame serializes");
    socket.send(Message::Text(text.into())).await.is_ok()
}
