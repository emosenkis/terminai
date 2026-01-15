/**
 * Deno ops - Rust functions callable from TypeScript
 *
 * These operations are registered with deno_core and can be called
 * from JavaScript via `Deno.core.ops.op_name(args)`.
 */
// Import everything from deno_core for macro support
use deno_core::*;

use crate::deno::types::{
  FetchOptions, FetchResponse, ReadScrollbackArgs, SuggestCommandArgs,
};
use crate::llm::tool_executor::{
  ToolCallId, ToolExecutionRequest, ToolExecutor,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// Operation state that holds references to Rust components
pub struct TerminaiOpState {
  pub tool_executor: Arc<ToolExecutor>,
  pub http_client: reqwest::Client,
}

/// Op: suggest_command
/// Called from TypeScript when the LLM wants to suggest a command
#[op2(async(lazy))]
#[string]
pub async fn op_suggest_command(
  state: Rc<RefCell<OpState>>,
  #[serde] args: SuggestCommandArgs,
) -> Result<String, deno_error::JsErrorBox> {
  let tool_executor = {
    let state_borrow = state.borrow();
    let op_state = state_borrow.borrow::<TerminaiOpState>();
    op_state.tool_executor.clone()
  };

  // Convert args to HashMap for ToolExecutionRequest
  let mut tool_args = HashMap::new();
  tool_args.insert("command".to_string(), serde_json::json!(args.command));
  if let Some(explanation) = args.explanation {
    tool_args.insert("explanation".to_string(), serde_json::json!(explanation));
  }

  let request = ToolExecutionRequest {
    tool_call_id: ToolCallId::new(),
    tool_name: "suggest_command".to_string(),
    args: tool_args,
  };

  let result = tool_executor.execute_tool(request).await;

  if result.is_error {
    Err(deno_error::JsErrorBox::generic(result.content))
  } else {
    Ok(result.content)
  }
}

/// Op: read_scrollback
/// Called from TypeScript when the LLM wants to read terminal history
#[op2(async(lazy))]
#[string]
pub async fn op_read_scrollback(
  state: Rc<RefCell<OpState>>,
  #[serde] args: ReadScrollbackArgs,
) -> Result<String, deno_error::JsErrorBox> {
  let tool_executor = {
    let state_borrow = state.borrow();
    let op_state = state_borrow.borrow::<TerminaiOpState>();
    op_state.tool_executor.clone()
  };

  let mut tool_args = HashMap::new();
  if let Some(num_lines) = args.num_lines {
    tool_args.insert("num_lines".to_string(), serde_json::json!(num_lines));
  }

  let request = ToolExecutionRequest {
    tool_call_id: ToolCallId::new(),
    tool_name: "read_scrollback".to_string(),
    args: tool_args,
  };

  let result = tool_executor.execute_tool(request).await;

  if result.is_error {
    Err(deno_error::JsErrorBox::generic(result.content))
  } else {
    Ok(result.content)
  }
}

/// Op: fetch
/// HTTP fetch operation for making API calls from TypeScript
/// This provides a fetch-compatible interface for the agent to make HTTP requests
#[op2(async(lazy))]
#[serde]
pub async fn op_fetch(
  state: Rc<RefCell<OpState>>,
  #[string] url: String,
  #[serde] options: FetchOptions,
) -> Result<FetchResponse, deno_error::JsErrorBox> {
  let client = {
    let state_borrow = state.borrow();
    let op_state = state_borrow.borrow::<TerminaiOpState>();
    op_state.http_client.clone()
  };

  // Build request
  let method = options.method.parse::<reqwest::Method>().map_err(|e| {
    deno_error::JsErrorBox::generic(format!("Invalid HTTP method: {}", e))
  })?;

  let mut request = client.request(method, &url);

  // Add headers
  for (key, value) in options.headers {
    request = request.header(&key, &value);
  }

  // Add body if present
  if let Some(body) = options.body {
    request = request.body(body);
  }

  // Send request
  let response = request.send().await.map_err(|e| {
    deno_error::JsErrorBox::generic(format!("HTTP request failed: {}", e))
  })?;

  let status = response.status().as_u16();
  let body = response.text().await.map_err(|e| {
    deno_error::JsErrorBox::generic(format!(
      "Failed to read response body: {}",
      e
    ))
  })?;

  Ok(FetchResponse { status, body })
}

// Op declarations for extension registration
const OP_SUGGEST_COMMAND_DECL: OpDecl = op_suggest_command();
const OP_READ_SCROLLBACK_DECL: OpDecl = op_read_scrollback();
const OP_FETCH_DECL: OpDecl = op_fetch();

/// Create the extension with all Termin.AI ops
pub fn create_terminai_extension() -> Extension {
  Extension {
    name: "terminai",
    ops: std::borrow::Cow::Borrowed(&[
      OP_SUGGEST_COMMAND_DECL,
      OP_READ_SCROLLBACK_DECL,
      OP_FETCH_DECL,
    ]),
    ..Default::default()
  }
}
