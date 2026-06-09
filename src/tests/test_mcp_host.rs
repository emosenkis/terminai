use rmcp::{ServerHandler, handler::server::wrapper::Parameters};
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

  assert!(state.get_tool("read_terminal").is_some());
  assert!(state.get_tool("get_terminal_context").is_some());
  assert!(state.get_tool("suggest_input").is_some());
  assert!(state.get_tool("get_suggestion_status").is_some());
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

  let server = start_http_mcp_server(state)
    .await
    .expect("rmcp Streamable HTTP server should start");

  assert!(server.url.starts_with("http://127.0.0.1:"));
  assert!(server.url.ends_with("/mcp"));
}
