use futures_util::{SinkExt, StreamExt};
use protocol::{ClientFrame, RuntimeFrame};
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        http::{HeaderValue, header::AUTHORIZATION},
    },
};

use crate::runtime::Endpoint;

// The relay never reads a frame's meaning: the interface speaks the protocol, and this side only
// carries it, so a new command needs no change here.

/// The open connection to the runtime.
pub(crate) struct Relay {
    outgoing: mpsc::Sender<ClientFrame>,
}

impl Relay {
    /// Connects with the endpoint's token and hands every frame the runtime sends to `incoming`, in
    /// order, until the connection ends.
    pub(crate) async fn connect(
        endpoint: Endpoint,
        incoming: mpsc::Sender<RuntimeFrame>,
    ) -> Result<Self, String> {
        let mut request = format!("ws://127.0.0.1:{}/", endpoint.port)
            .into_client_request()
            .map_err(|e| e.to_string())?;
        let bearer = HeaderValue::from_str(&format!("Bearer {}", endpoint.token))
            .map_err(|_| "el runtime publicó un token que no es texto".to_owned())?;
        request.headers_mut().insert(AUTHORIZATION, bearer);
        let (socket, _) = connect_async(request)
            .await
            .map_err(|e| format!("el runtime no aceptó la conexión: {e}"))?;
        let (mut sink, mut stream) = socket.split();

        let (outgoing, mut frames) = mpsc::channel::<ClientFrame>(32);
        // Ends when the relay is dropped, closing the connection, or when the runtime is gone.
        tokio::spawn(async move {
            while let Some(frame) = frames.recv().await {
                let text = serde_json::to_string(&frame).expect("a frame serializes");
                if sink.send(Message::text(text)).await.is_err() {
                    return;
                }
            }
            let _ = sink.close().await;
        });
        // Ends with the connection, dropping `incoming`, which tells its receiver.
        tokio::spawn(async move {
            while let Some(Ok(message)) = stream.next().await {
                let text = match message {
                    Message::Text(text) => text,
                    // The stream itself ends only once the closing handshake is answered, which
                    // takes the writing half.
                    Message::Close(_) => return,
                    _ => continue,
                };
                // A frame outside the protocol could be a reply the interface waits for, so it ends
                // the connection rather than pass unnoticed.
                let Ok(frame) = serde_json::from_str(&text) else {
                    return;
                };
                if incoming.send(frame).await.is_err() {
                    return;
                }
            }
        });
        Ok(Self { outgoing })
    }

    pub(crate) async fn send(&self, frame: ClientFrame) -> Result<(), String> {
        self.outgoing
            .send(frame)
            .await
            .map_err(|_| "se cerró la conexión con el runtime".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use protocol::{Command, Hello, Outcome, Reply, RequestId};
    use tokio::{
        net::{TcpListener, TcpStream},
        task::JoinHandle,
    };
    use tokio_tungstenite::{
        WebSocketStream, accept_hdr_async,
        tungstenite::handshake::server::{Request, Response},
    };

    use super::*;

    type Accepted = JoinHandle<(Option<String>, WebSocketStream<TcpStream>)>;

    /// A runtime that admits one connection, and keeps the `Authorization` header it came with.
    #[expect(
        clippy::result_large_err,
        reason = "the handshake callback's error type is tungstenite's own"
    )]
    async fn runtime() -> (u16, Accepted) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let accepted = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut authorization = None;
            let socket = accept_hdr_async(stream, |request: &Request, response: Response| {
                authorization = request
                    .headers()
                    .get(AUTHORIZATION)
                    .map(|value| value.to_str().unwrap().to_owned());
                Ok(response)
            })
            .await
            .unwrap();
            (authorization, socket)
        });
        (port, accepted)
    }

    async fn connected() -> (
        Relay,
        mpsc::Receiver<RuntimeFrame>,
        Option<String>,
        WebSocketStream<TcpStream>,
    ) {
        let (port, accepted) = runtime().await;
        let (incoming, received) = mpsc::channel(8);
        let endpoint = Endpoint {
            port,
            token: "secret".to_owned(),
        };
        let relay = Relay::connect(endpoint, incoming).await.unwrap();
        let (authorization, socket) = accepted.await.unwrap();
        (relay, received, authorization, socket)
    }

    async fn receive(socket: &mut WebSocketStream<TcpStream>) -> ClientFrame {
        let Some(Ok(Message::Text(text))) = socket.next().await else {
            panic!("no text frame");
        };
        serde_json::from_str(&text).unwrap()
    }

    async fn send(socket: &mut WebSocketStream<TcpStream>, frame: &RuntimeFrame) {
        let text = serde_json::to_string(frame).unwrap();
        socket.send(Message::text(text)).await.unwrap();
    }

    #[tokio::test]
    async fn connects_with_the_token_and_carries_frames_both_ways_in_order() {
        let (relay, mut received, authorization, mut socket) = connected().await;
        assert_eq!(authorization.as_deref(), Some("Bearer secret"));

        let hello = ClientFrame::Hello(Hello { protocol: 1 });
        let command = ClientFrame::Command {
            id: RequestId(1),
            command: Command::ListAssets,
        };
        relay.send(hello.clone()).await.unwrap();
        relay.send(command.clone()).await.unwrap();
        assert_eq!(receive(&mut socket).await, hello);
        assert_eq!(receive(&mut socket).await, command);

        let greeting = RuntimeFrame::Hello(Hello { protocol: 1 });
        let reply = RuntimeFrame::Reply {
            id: RequestId(1),
            outcome: Outcome::Done {
                reply: Reply::Assets { assets: vec![] },
            },
        };
        send(&mut socket, &greeting).await;
        send(&mut socket, &reply).await;
        assert_eq!(received.recv().await, Some(greeting));
        assert_eq!(received.recv().await, Some(reply));
    }

    #[tokio::test]
    async fn ends_on_a_frame_outside_the_protocol() {
        let (_relay, mut received, _, mut socket) = connected().await;
        socket.send(Message::text("not a frame")).await.unwrap();
        assert_eq!(received.recv().await, None);
    }

    #[tokio::test]
    async fn ends_when_the_runtime_closes_the_connection() {
        let (_relay, mut received, _, mut socket) = connected().await;
        socket.close(None).await.unwrap();
        assert_eq!(received.recv().await, None);
    }
}
