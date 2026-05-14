use std::net::SocketAddr;

use anyhow::Result;
use serde_json::{Value as JsonValue, json};
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpListener,
  task::JoinHandle,
};

use super::tools::TerminaiMcpState;

pub struct McpServerHandle {
  pub url: String,
  _task: JoinHandle<()>,
}

pub async fn start_http_mcp_server(
  state: TerminaiMcpState,
) -> Result<McpServerHandle> {
  let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
  let addr = listener.local_addr()?;
  let url = format!("http://{}/mcp", addr);
  let task = tokio::spawn(async move {
    loop {
      match listener.accept().await {
        Ok((mut stream, _peer)) => {
          let state = state.clone();
          tokio::spawn(async move {
            let mut buf = vec![0u8; 1024 * 1024];
            let n = match stream.read(&mut buf).await {
              Ok(0) | Err(_) => return,
              Ok(n) => n,
            };
            let request = String::from_utf8_lossy(&buf[..n]);
            let response = handle_http_request(&state, &request).await;
            let _ = stream.write_all(response.as_bytes()).await;
          });
        }
        Err(err) => {
          log::error!("Termin.AI MCP accept error: {}", err);
          break;
        }
      }
    }
  });

  Ok(McpServerHandle { url, _task: task })
}

async fn handle_http_request(
  state: &TerminaiMcpState,
  request: &str,
) -> String {
  let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
  let value = serde_json::from_str::<JsonValue>(body).unwrap_or_else(|_| {
    json!({
      "jsonrpc": "2.0",
      "id": null,
      "method": "invalid"
    })
  });
  let payload = handle_json_rpc(state, value).await;
  http_json_response(payload)
}

fn http_json_response(payload: JsonValue) -> String {
  let body = payload.to_string();
  format!(
    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
    body.len(),
    body
  )
}

pub async fn handle_json_rpc(
  state: &TerminaiMcpState,
  request: JsonValue,
) -> JsonValue {
  let id = request.get("id").cloned().unwrap_or(JsonValue::Null);
  let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");

  let result = match method {
    "initialize" => Ok(json!({
      "protocolVersion": "2024-11-05",
      "capabilities": {
        "tools": {}
      },
      "serverInfo": {
        "name": "terminai",
        "version": env!("CARGO_PKG_VERSION")
      }
    })),
    "tools/list" => Ok(json!({
      "tools": TerminaiMcpState::tool_definitions()
    })),
    "tools/call" => {
      let params = request.get("params").cloned().unwrap_or(JsonValue::Null);
      let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
      let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
      match state.call_tool(name, args).await {
        Ok(response) => Ok(json!({
          "content": [
            {
              "type": "text",
              "text": response.text
            }
          ],
          "structuredContent": response.data
        })),
        Err(err) => Err(err.to_string()),
      }
    }
    "notifications/initialized" => return json!({}),
    _ => Err(format!("Unsupported MCP method: {}", method)),
  };

  match result {
    Ok(result) => json!({
      "jsonrpc": "2.0",
      "id": id,
      "result": result
    }),
    Err(message) => json!({
      "jsonrpc": "2.0",
      "id": id,
      "error": {
        "code": -32601,
        "message": message
      }
    }),
  }
}

#[allow(dead_code)]
fn _assert_send_sync(_: SocketAddr) {}
