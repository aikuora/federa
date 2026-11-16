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

use crate::{connection, mcp, token::Token};

#[derive(Clone)]
struct Served {
    token: Arc<Token>,
    kernel: Arc<Kernel>,
}

/// Serves the protocol over WebSocket on `listener`, and the kernel's tools to agents at `/mcp`. A
/// connection that does not present `token` as `Authorization: Bearer <token>` is refused before
/// the upgrade, with 401.
pub(crate) async fn serve(
    listener: TcpListener,
    token: Token,
    kernel: Arc<Kernel>,
) -> io::Result<()> {
    axum::serve(listener, app(token, kernel)).await
}

fn app(token: Token, kernel: Arc<Kernel>) -> Router {
    let served = Served {
        token: Arc::new(token),
        kernel: kernel.clone(),
    };
    Router::new()
        .route("/", get(upgrade))
        .with_state(served)
        .merge(mcp::router(kernel))
}

/// The token a request presents as `Authorization: Bearer <token>`.
pub(crate) fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
}

// The upgrade is taken as a `Result` so the token is checked first: a request without it gets 401
// whatever else it carries.
async fn upgrade(
    State(served): State<Served>,
    headers: HeaderMap,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    if !bearer(&headers).is_some_and(|presented| served.token.matches(presented)) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match upgrade {
        Ok(upgrade) => upgrade.on_upgrade(|socket| connection::run(socket, served.kernel)),
        Err(rejection) => rejection.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path, process::Command as Process};

    use axum::{body::Body, http::Request};
    use kernel::{AgentIdentity, Role};
    use protocol::Command;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;

    /// The kernel with the organization `acme` open and the asset `billing` registered in it.
    async fn kernel(dir: &Path) -> Arc<Kernel> {
        let billing = dir.join("billing");
        let federa = billing.join(".federa");
        fs::create_dir_all(&federa).unwrap();
        fs::write(
            federa.join("asset.toml"),
            "schema = 1\nname = \"billing\"\n",
        )
        .unwrap();
        fs::write(federa.join("knowledge.md"), "# billing\n").unwrap();
        // A git hook sets these for the repository it runs in, and git would init that one instead.
        let init = Process::new("git")
            .arg("init")
            .arg(&billing)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .unwrap();
        assert!(init.status.success());
        let kernel = Arc::new(Kernel::new("ada@example.com".to_owned()));
        for command in [
            Command::CreateOrganization {
                folder: dir.join("acme"),
                name: "Acme".to_owned(),
            },
            Command::RegisterAsset { folder: billing },
        ] {
            assert!(matches!(
                kernel.handle(command).await,
                protocol::Outcome::Done { .. }
            ));
        }
        kernel
    }

    fn peer(kernel: &Kernel, asset: &str) -> String {
        let agent = AgentIdentity {
            role: Role::Peer,
            session: "peer-1".to_owned(),
            asset: federation::AssetName::parse(asset).unwrap(),
            goal: None,
        };
        kernel.admit(agent).as_str().to_owned()
    }

    /// Posts one JSON-RPC request to `/mcp`, and returns the status and the body.
    async fn rpc(app: &Router, token: Option<&str>, body: Value) -> (StatusCode, Option<Value>) {
        let mut request = Request::post("/mcp").header("content-type", "application/json");
        if let Some(token) = token {
            request = request.header(AUTHORIZATION, format!("Bearer {token}"));
        }
        let request = request.body(Body::from(body.to_string())).unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).ok())
    }

    #[tokio::test]
    async fn an_agent_calls_the_tool_and_gets_the_knowledge_of_its_asset() {
        let dir = tempfile::tempdir().unwrap();
        let kernel = kernel(dir.path()).await;
        let token = peer(&kernel, "billing");
        let app = app(Token::generate(), kernel);

        let initialize = json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "capabilities": {} },
        });
        let (status, body) = rpc(&app, Some(&token), initialize).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.unwrap()["result"]["protocolVersion"], "2025-06-18");
        let unversioned = json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize" });
        let (_, body) = rpc(&app, Some(&token), unversioned).await;
        assert_eq!(body.unwrap()["result"]["protocolVersion"], "2025-11-25");

        let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        let (status, _) = rpc(&app, Some(&token), initialized).await;
        assert_eq!(status, StatusCode::ACCEPTED);

        let list = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });
        let (_, body) = rpc(&app, Some(&token), list).await;
        assert_eq!(
            body.unwrap()["result"]["tools"][0]["name"],
            "asset_knowledge"
        );

        let call = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": { "name": "asset_knowledge", "arguments": {} },
        });
        let (_, body) = rpc(&app, Some(&token), call).await;
        assert_eq!(
            body.unwrap(),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "result": {
                    "content": [{ "type": "text", "text": "# billing\n" }],
                    "isError": false,
                },
            })
        );

        let unknown = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": { "name": "deploy" },
        });
        let (_, body) = rpc(&app, Some(&token), unknown).await;
        assert_eq!(body.unwrap()["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn a_tool_that_fails_answers_an_error_for_the_agent_to_read() {
        let dir = tempfile::tempdir().unwrap();
        let kernel = kernel(dir.path()).await;
        let token = peer(&kernel, "ledger");
        let app = app(Token::generate(), kernel);

        let call = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "asset_knowledge" },
        });
        let (status, body) = rpc(&app, Some(&token), call).await;
        assert_eq!(status, StatusCode::OK);
        let result = &body.unwrap()["result"];
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("ledger"), "{text}");
    }

    #[tokio::test]
    async fn a_request_without_an_agents_token_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let kernel = kernel(dir.path()).await;
        let agent = peer(&kernel, "billing");
        let client = Token::generate();
        let client_token = client.as_str().to_owned();
        let app = app(client, kernel);

        let call = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "asset_knowledge", "arguments": {} },
        });
        let guess = "0".repeat(agent.len());
        for token in [None, Some(client_token.as_str()), Some(guess.as_str())] {
            let (status, body) = rpc(&app, token, call.clone()).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{token:?}");
            assert_eq!(body, None);
        }

        // An agent's token opens no client command either.
        let upgrade = Request::get("/")
            .header(AUTHORIZATION, format!("Bearer {agent}"))
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(upgrade).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
