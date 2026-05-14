use serde_json::json;
use tokio::sync::mpsc;

use crate::mcp_host::server::handle_json_rpc;
use crate::mcp_host::TerminaiMcpState;
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

  let response = handle_json_rpc(
    &state,
    json!({
      "jsonrpc": "2.0",
      "id": 1,
      "method": "tools/list"
    }),
  )
  .await;

  let tools = response["result"]["tools"].as_array().unwrap();
  let names: Vec<_> = tools
    .iter()
    .map(|tool| tool["name"].as_str().unwrap())
    .collect();
  assert!(names.contains(&"read_terminal"));
  assert!(names.contains(&"get_terminal_context"));
  assert!(names.contains(&"suggest_input"));
}

#[tokio::test]
async fn suggest_input_queues_approval_for_wrapped_shell() {
  let (shell, _rx) =
    Shell::spawn_command("/bin/sh", &["-c".to_string(), "sleep 1".to_string()], 24, 80)
      .expect("test shell should spawn");
  let (tx, mut suggestion_rx) = mpsc::unbounded_channel();
  let state = TerminaiMcpState::new(shell.vt.clone(), tx);

  let response = handle_json_rpc(
    &state,
    json!({
      "jsonrpc": "2.0",
      "id": 2,
      "method": "tools/call",
      "params": {
        "name": "suggest_input",
        "arguments": {
          "input": "git status\\r",
          "explanation": "Check repository status."
        }
      }
    }),
  )
  .await;

  assert_eq!(response["result"]["structuredContent"]["queued"], true);
  let pending = suggestion_rx.try_recv().unwrap();
  assert_eq!(pending.command, "git status\\r");
  assert_eq!(
    pending.explanation.as_deref(),
    Some("Check repository status.")
  );
}
