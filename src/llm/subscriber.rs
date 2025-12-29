// TERMIN.AI: AG-UI subscriber for streaming text events

use ag_ui_client::agent::{AgentError, AgentStateMutation};
use ag_ui_client::subscriber::{AgentSubscriber, AgentSubscriberParams};
use ag_ui_core::event::{
  RunErrorEvent, TextMessageContentEvent, ToolCallArgsEvent, ToolCallEndEvent,
  ToolCallResultEvent, ToolCallStartEvent,
};
use ag_ui_core::types::ids::{MessageId, ToolCallId};
use ag_ui_core::types::tool::ToolCall;
use ag_ui_core::{AgentState, FwdProps};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use crate::llm::tool_executor::ToolExecutionRequest;

/// Internal buffer for accumulating tool call information during streaming
#[derive(Debug, Clone)]
struct PartialToolCall {
  tool_call_id: ToolCallId,
  parent_message_id: Option<MessageId>,
  tool_name: String,
}

/// Subscriber that captures streaming text events and sends them through a channel
pub struct StreamingSubscriber {
  text_sender: mpsc::UnboundedSender<anyhow::Result<String>>,
  tool_sender: mpsc::UnboundedSender<ToolExecutionRequest>,
  /// Buffer for accumulating tool call information
  /// Key: tool_call_id string (since we can't use ToolCallId as HashMap key directly)
  current_tool_calls: Arc<Mutex<HashMap<String, PartialToolCall>>>,
}

impl StreamingSubscriber {
  pub fn new(
    text_sender: mpsc::UnboundedSender<anyhow::Result<String>>,
    tool_sender: mpsc::UnboundedSender<ToolExecutionRequest>,
  ) -> Self {
    Self {
      text_sender,
      tool_sender,
      current_tool_calls: Arc::new(Mutex::new(HashMap::new())),
    }
  }
}

#[async_trait::async_trait]
impl<StateT, FwdPropsT> AgentSubscriber<StateT, FwdPropsT>
  for StreamingSubscriber
where
  StateT: AgentState,
  FwdPropsT: FwdProps,
{
  async fn on_text_message_content_event(
    &self,
    event: &TextMessageContentEvent,
    _text_message_buffer: &str,
    _params: AgentSubscriberParams<'async_trait, StateT, FwdPropsT>,
  ) -> Result<AgentStateMutation<StateT>, AgentError> {
    // Send the text delta through the channel
    let _ = self.text_sender.send(Ok(event.delta.clone()));
    Ok(AgentStateMutation::default())
  }

  async fn on_run_error_event(
    &self,
    event: &RunErrorEvent,
    _params: AgentSubscriberParams<'async_trait, StateT, FwdPropsT>,
  ) -> Result<AgentStateMutation<StateT>, AgentError> {
    // Send the error through the channel
    let _ = self
      .text_sender
      .send(Err(anyhow::anyhow!("LLM error: {}", event.message)));
    Ok(AgentStateMutation::default())
  }

  // Tool call event handlers - capture tool calls for execution
  async fn on_tool_call_start_event(
    &self,
    event: &ToolCallStartEvent,
    _params: AgentSubscriberParams<'async_trait, StateT, FwdPropsT>,
  ) -> Result<AgentStateMutation<StateT>, AgentError> {
    log::info!(
      "Tool call started - id: {:?}, name: {}, parent_message_id: {:?}",
      event.tool_call_id,
      event.tool_call_name,
      event.parent_message_id
    );

    // Buffer tool call information
    let partial = PartialToolCall {
      tool_call_id: event.tool_call_id.clone(),
      parent_message_id: event.parent_message_id.clone(),
      tool_name: event.tool_call_name.clone(),
    };

    // Store in buffer (use string representation as key)
    let key = format!("{:?}", event.tool_call_id);
    let mut buffer = self.current_tool_calls.lock().await;
    buffer.insert(key, partial);

    Ok(AgentStateMutation::default())
  }

  async fn on_tool_call_args_event(
    &self,
    event: &ToolCallArgsEvent,
    tool_call_buffer: &str,
    tool_call_name: &str,
    partial_tool_call_args: &HashMap<String, JsonValue>,
    _params: AgentSubscriberParams<'async_trait, StateT, FwdPropsT>,
  ) -> Result<AgentStateMutation<StateT>, AgentError> {
    log::debug!(
      "Tool call args chunk - id: {:?}, name: {}, delta: {}, buffer_len: {}, partial_args: {:?}",
      event.tool_call_id,
      tool_call_name,
      event.delta,
      tool_call_buffer.len(),
      partial_tool_call_args
    );
    Ok(AgentStateMutation::default())
  }

  async fn on_tool_call_end_event(
    &self,
    event: &ToolCallEndEvent,
    tool_call_name: &str,
    tool_call_args: &HashMap<String, JsonValue>,
    _params: AgentSubscriberParams<'async_trait, StateT, FwdPropsT>,
  ) -> Result<AgentStateMutation<StateT>, AgentError> {
    log::info!(
      "Tool call ended - id: {:?}, name: {}, args: {:?}",
      event.tool_call_id,
      tool_call_name,
      tool_call_args
    );

    // Retrieve buffered tool call info
    let key = format!("{:?}", event.tool_call_id);
    let mut buffer = self.current_tool_calls.lock().await;
    let partial = buffer.remove(&key);

    // Use tool name from buffer if parameter is empty (AG-UI SDK bug workaround)
    let final_tool_name = if tool_call_name.is_empty() {
      if let Some(ref p) = partial {
        log::warn!(
          "Tool name parameter is empty, using buffered name: {}",
          p.tool_name
        );
        p.tool_name.clone()
      } else {
        log::error!(
          "Tool name is empty and no buffered info found for {:?}",
          event.tool_call_id
        );
        return Err(AgentError::ExecutionError {
          message: "Tool call ended but no buffered info found".to_string(),
        });
      }
    } else {
      tool_call_name.to_string()
    };

    // Create tool execution request
    let request = ToolExecutionRequest {
      tool_call_id: event.tool_call_id.clone(),
      tool_name: final_tool_name.clone(),
      args: tool_call_args.clone(),
    };

    // Send to application layer for execution
    if let Err(e) = self.tool_sender.send(request) {
      log::error!("Failed to send tool execution request: {}", e);
    } else {
      log::info!(
        "Tool execution request sent for '{}' (id: {:?})",
        final_tool_name,
        event.tool_call_id
      );
    }

    Ok(AgentStateMutation::default())
  }

  async fn on_tool_call_result_event(
    &self,
    event: &ToolCallResultEvent,
    _params: AgentSubscriberParams<'async_trait, StateT, FwdPropsT>,
  ) -> Result<AgentStateMutation<StateT>, AgentError> {
    log::info!(
      "🔧 Tool call result received - id: {:?}, content: {}",
      event.tool_call_id,
      event.content
    );
    Ok(AgentStateMutation::default())
  }

  async fn on_new_tool_call(
    &self,
    tool_call: &ToolCall,
    _params: AgentSubscriberParams<'async_trait, StateT, FwdPropsT>,
  ) -> Result<(), AgentError> {
    log::info!(
      "🔧 New tool call added to messages - id: {:?}, name: {}, args: {:?}",
      tool_call.id,
      tool_call.function.name,
      tool_call.function.arguments
    );
    Ok(())
  }

  // All other events are ignored (default implementations)
}
