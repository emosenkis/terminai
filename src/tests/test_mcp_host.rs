use rmcp::{
  ServerHandler, ServiceExt,
  handler::server::wrapper::Parameters,
  transport::{
    StreamableHttpClientTransport,
    streamable_http_client::StreamableHttpClientTransportConfig,
  },
};
use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::mcp_host::tools::SuggestInputArgs;
use crate::mcp_host::{TerminaiMcpState, start_http_mcp_server};
use crate::shell::Shell;

#[tokio::test]
async fn mcp_lists_terminal_tools_for_cli_agents() {
  let (shell, _rx) = Shell::spawn_command(
    "/bin/sh",
    &["-c".to_string(), "printf 'hello from shell\\n'".to_string()],
    24,
    80,
  )
  .expect("test shell should spawn");
  let (tx, _suggestion_rx) = mpsc::unbounded_channel();
  let state = TerminaiMcpState::new(shell.vt.clone(), tx);

  assert!(state.get_tool("check_for_updates").is_some());
  assert!(state.get_tool("read_terminal").is_some());
  assert!(state.get_tool("get_terminal_context").is_some());
  assert!(state.get_tool("suggest_input").is_some());
  assert!(state.get_tool("get_suggestion_status").is_some());
}

#[tokio::test]
async fn check_for_updates_reports_cwd_change_once() {
  let (shell, _rx) = Shell::spawn_command(
    "/bin/sh",
    &["-c".to_string(), "sleep 1".to_string()],
    24,
    80,
  )
  .expect("test shell should spawn");
  let (tx, _suggestion_rx) = mpsc::unbounded_channel();
  let state = TerminaiMcpState::new(shell.vt.clone(), tx);

  state.update_cwd(PathBuf::from("/tmp/terminai-project"));
  let first = state.check_for_updates().await.unwrap();
  let first_data = first.structured_content.unwrap();
  assert_eq!(first_data["has_updates"], serde_json::Value::Bool(true));
  assert_eq!(first_data["cwd_change"], "/tmp/terminai-project");
  assert_eq!(first_data["updates"][0]["type"], "cwd_changed");

  let second = state.check_for_updates().await.unwrap();
  let second_data = second.structured_content.unwrap();
  assert_eq!(second_data["has_updates"], serde_json::Value::Bool(false));
  assert!(second_data["cwd_change"].is_null());
}

#[tokio::test]
async fn terminal_context_reports_cwd_from_mcp_state() {
  let (shell, _rx) = Shell::spawn_command(
    "/bin/sh",
    &["-c".to_string(), "sleep 1".to_string()],
    24,
    80,
  )
  .expect("test shell should spawn");
  let (tx, _suggestion_rx) = mpsc::unbounded_channel();
  let state = TerminaiMcpState::new(shell.vt.clone(), tx);

  state.update_cwd(PathBuf::from("/tmp/terminai-project"));
  let first = state.get_terminal_context().await.unwrap();
  let first_data = first.structured_content.unwrap();
  assert_eq!(first_data["cwd"], "/tmp/terminai-project");
  assert_eq!(first_data["cwd_change"], "/tmp/terminai-project");
  assert_eq!(
    first_data["cwd_changed_since_last_context"],
    serde_json::Value::Bool(true)
  );

  let second = state.get_terminal_context().await.unwrap();
  let second_data = second.structured_content.unwrap();
  assert_eq!(second_data["cwd"], "/tmp/terminai-project");
  assert!(second_data["cwd_change"].is_null());
  assert_eq!(
    second_data["cwd_changed_since_last_context"],
    serde_json::Value::Bool(false)
  );
}

#[tokio::test]
async fn suggest_input_queues_approval_for_wrapped_shell() {
  let (shell, _rx) = Shell::spawn_command(
    "/bin/sh",
    &["-c".to_string(), "sleep 1".to_string()],
    24,
    80,
  )
  .expect("test shell should spawn");
  let (tx, mut suggestion_rx) = mpsc::unbounded_channel();
  let state = TerminaiMcpState::new(shell.vt.clone(), tx);

  let response = state
    .suggest_input(Parameters(SuggestInputArgs {
      input: "git status\\r".to_string(),
      explanation: Some("Check repository status.".to_string()),
    }))
    .await
    .unwrap();

  assert_eq!(
    response.structured_content.unwrap()["queued"],
    serde_json::Value::Bool(true)
  );
  let pending = suggestion_rx.try_recv().unwrap();
  assert_eq!(pending.command, "git status\\r");
  assert_eq!(
    pending.explanation.as_deref(),
    Some("Check repository status.")
  );
}

#[tokio::test]
async fn starts_streamable_http_mcp_server_for_cli_agents() {
  let (shell, _rx) = Shell::spawn_command(
    "/bin/sh",
    &["-c".to_string(), "sleep 1".to_string()],
    24,
    80,
  )
  .expect("test shell should spawn");
  let (tx, _suggestion_rx) = mpsc::unbounded_channel();
  let state = TerminaiMcpState::new(shell.vt.clone(), tx);

  let server = start_http_mcp_server(state, "test-token".to_string())
    .await
    .expect("rmcp Streamable HTTP server should start");

  assert!(server.url.starts_with("http://127.0.0.1:"));
  assert!(server.url.ends_with("/mcp"));
  assert!(server.port > 0);
  assert_eq!(server.auth_token, "test-token");
}

#[tokio::test]
async fn http_mcp_server_rejects_missing_bearer_token() {
  let (shell, _rx) = Shell::spawn_command(
    "/bin/sh",
    &["-c".to_string(), "sleep 1".to_string()],
    24,
    80,
  )
  .expect("test shell should spawn");
  let (tx, _suggestion_rx) = mpsc::unbounded_channel();
  let state = TerminaiMcpState::new(shell.vt.clone(), tx);

  let server = start_http_mcp_server(state, "test-token".to_string())
    .await
    .expect("rmcp Streamable HTTP server should start");
  let status_line = raw_post_status_line(server.port, None).await;

  assert!(status_line.starts_with("HTTP/1.1 401"));
}

#[tokio::test]
async fn http_mcp_server_accepts_authorized_mcp_client() {
  let (shell, _rx) = Shell::spawn_command(
    "/bin/sh",
    &["-c".to_string(), "sleep 1".to_string()],
    24,
    80,
  )
  .expect("test shell should spawn");
  let (tx, _suggestion_rx) = mpsc::unbounded_channel();
  let state = TerminaiMcpState::new(shell.vt.clone(), tx);

  let server = start_http_mcp_server(state, "test-token".to_string())
    .await
    .expect("rmcp Streamable HTTP server should start");
  let transport = StreamableHttpClientTransport::from_config(
    StreamableHttpClientTransportConfig::with_uri(server.url.clone())
      .auth_header("test-token"),
  );
  let client =
    ().serve(transport)
      .await
      .expect("authorized MCP client should connect");
  let tools = client
    .peer()
    .list_all_tools()
    .await
    .expect("authorized MCP client should list tools");

  assert!(tools.iter().any(|tool| tool.name == "read_terminal"));
}

async fn raw_post_status_line(
  port: u16,
  authorization: Option<&str>,
) -> String {
  use tokio::io::{AsyncReadExt, AsyncWriteExt};

  let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
    .await
    .expect("test should connect to MCP server");
  let authorization = authorization
    .map(|value| format!("Authorization: {value}\r\n"))
    .unwrap_or_default();
  let request = format!(
    "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{authorization}Content-Length: 0\r\n\r\n"
  );
  stream
    .write_all(request.as_bytes())
    .await
    .expect("test should write request");

  let mut response = vec![0; 256];
  let bytes = stream
    .read(&mut response)
    .await
    .expect("test should read response");
  String::from_utf8_lossy(&response[..bytes])
    .lines()
    .next()
    .unwrap_or_default()
    .to_string()
}
