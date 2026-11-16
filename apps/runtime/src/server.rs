use std::{io, sync::Arc};

use axum::{
    Router,
    extract::{State, WebSocketUpgrade, ws::rejection::WebSocketUpgradeRejection},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::{IntoResponse, Response},
    routing::get,
};
use kernel::Kernel;
use tokio::net::TcpListener;

use crate::{connection, token::Token};

#[derive(Clone)]
struct Served {
    token: Arc<Token>,
    kernel: Arc<Kernel>,
}

/// Serves the protocol over WebSocket on `listener`. A connection that does not present `token` as
/// `Authorization: Bearer <token>` is refused before the upgrade, with 401.
pub(crate) async fn serve(
    listener: TcpListener,
    token: Token,
    kernel: Arc<Kernel>,
) -> io::Result<()> {
    let served = Served {
        token: Arc::new(token),
        kernel,
    };
    let app = Router::new().route("/", get(upgrade)).with_state(served);
    axum::serve(listener, app).await
}

// The upgrade is taken as a `Result` so the token is checked first: a request without it gets 401
// whatever else it carries.
async fn upgrade(
    State(served): State<Served>,
    headers: HeaderMap,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    let presented = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if !presented.is_some_and(|presented| served.token.matches(presented)) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match upgrade {
        Ok(upgrade) => upgrade.on_upgrade(|socket| connection::run(socket, served.kernel)),
        Err(rejection) => rejection.into_response(),
    }
}
