use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use kernel::{Kernel, Tool, ToolError};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::server::bearer;

// MCP over HTTP is how every engine's agents reach the kernel's tools. The bearer token here is
// the agent's own, never the client's, so a client token opens no tool and an agent token opens
// no command.

// Hand-written rather than the official SDK: an agent needs only `initialize`, `tools/list` and
// `tools/call`, each answered with one JSON body and no session, which is what claude accepts. A
// later MCP revision that changes those three methods has to be followed here by hand.

/// The MCP endpoint, at `/mcp`: each request presents an agent's token as `Authorization: Bearer
/// <token>`, and one the kernel did not admit gets 401.
pub(crate) fn router(kernel: Arc<Kernel>) -> Router {
    Router::new().route("/mcp", post(answer)).with_state(kernel)
}

/// A JSON-RPC request; one without an id is a notification, which gets no answer.
#[derive(Deserialize)]
struct Request {
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Deserialize)]
struct Call {
    name: String,
    #[serde(default = "no_arguments")]
    arguments: Value,
}

fn no_arguments() -> Value {
    json!({})
}

// The body is taken as a `Result` so the token is checked first: a request without it gets 401
// whatever else it carries.
async fn answer(
    State(kernel): State<Arc<Kernel>>,
    headers: HeaderMap,
    request: Result<Json<Request>, JsonRejection>,
) -> Response {
    let token = bearer(&headers).unwrap_or_default();
    let tools = match kernel.tools(token) {
        Ok(tools) => tools,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    let Json(request) = match request {
        Ok(request) => request,
        Err(rejection) => return rejection.into_response(),
    };
    let Some(id) = request.id else {
        return StatusCode::ACCEPTED.into_response();
    };
    let result = match request.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": request.params.get("protocolVersion").unwrap_or(&json!(LATEST)),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "federa", "version": env!("CARGO_PKG_VERSION") },
        })),
        "tools/list" => Ok(json!({ "tools": tools.iter().map(listed).collect::<Vec<_>>() })),
        "tools/call" => match serde_json::from_value::<Call>(request.params) {
            Ok(call) => match kernel.call(token, &call.name, call.arguments).await {
                Ok(text) => Ok(content(text, false)),
                Err(ToolError::Unauthorized) => return StatusCode::UNAUTHORIZED.into_response(),
                // A tool that ran and failed is the agent's to read, as MCP has it.
                Err(ToolError::Failed(message)) => Ok(content(message, true)),
                Err(refused) => Err((INVALID_PARAMS, refused.to_string())),
            },
            Err(error) => Err((INVALID_PARAMS, error.to_string())),
        },
        _ => Err((
            METHOD_NOT_FOUND,
            format!("there is no method {}", request.method),
        )),
    };
    Json(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message },
        }),
    })
    .into_response()
}

// The version a client that names none gets: the latest one claude's MCP client knows, as its
// 2.1.312 binary lists it.
const LATEST: &str = "2025-11-25";

const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;

fn listed(tool: &Tool) -> Value {
    json!({
        "name": tool.name,
        "description": tool.description,
        "inputSchema": tool.input_schema,
    })
}

fn content(text: String, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}
