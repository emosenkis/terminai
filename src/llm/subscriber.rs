// TERMIN.AI: AG-UI subscriber for streaming text events

use ag_ui_client::agent::{AgentError, AgentStateMutation};
use ag_ui_client::subscriber::{AgentSubscriber, AgentSubscriberParams};
use ag_ui_core::event::{
  RunErrorEvent, TextMessageContentEvent, ToolCallArgsEvent, ToolCallEndEvent,
  ToolCallResultEvent, ToolCallStartEvent,
};
use ag_ui_core::types::ids::{MessageId, ToolCallId};
use ag_ui_core::types::message::Message;
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
  tool_name: String,
  /// Accumulated arguments from streaming chunks
  accumulated_args: HashMap<String, JsonValue>,
}

/// Subscriber that captures streaming text events and sends them through a channel
pub struct StreamingSubscriber {
  text_sender: mpsc::UnboundedSender<anyhow::Result<String>>,
  tool_sender: mpsc::UnboundedSender<ToolExecutionRequest>,
  /// Buffer for accumulating tool call information
  /// Key: tool_call_id string (since we can't use ToolCallId as HashMap key directly)
  current_tool_calls: Arc<Mutex<HashMap<String, PartialToolCall>>>,
  /// Shared message history that needs to be updated when SDK adds tool calls
  message_history: Option<Arc<Mutex<Vec<Message>>>>,
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
      message_history: None,
    }
  }

  pub fn with_message_history(
    text_sender: mpsc::UnboundedSender<anyhow::Result<String>>,
    tool_sender: mpsc::UnboundedSender<ToolExecutionRequest>,
    message_history: Arc<Mutex<Vec<Message>>>,
  ) -> Self {
    Self {
      text_sender,
      tool_sender,
      current_tool_calls: Arc::new(Mutex::new(HashMap::new())),
      message_history: Some(message_history),
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
      tool_name: event.tool_call_name.clone(),
      accumulated_args: HashMap::new(),
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
    log::info!(
      "Tool call args chunk - id: {:?}, name: {}, delta: {}, buffer_len: {}, partial_args: {:?}",
      event.tool_call_id,
      tool_call_name,
      event.delta,
      tool_call_buffer.len(),
      partial_tool_call_args
    );

    // CRITICAL FIX: Parse and accumulate args from the delta or buffer
    let key = format!("{:?}", event.tool_call_id);
    let mut buffer = self.current_tool_calls.lock().await;

    if let Some(partial) = buffer.get_mut(&key) {
      // Try multiple sources for the args:

      // 1. If partial_tool_call_args has data, use it
      if !partial_tool_call_args.is_empty() {
        log::info!("Using partial_tool_call_args from SDK");
        for (k, v) in partial_tool_call_args {
          partial.accumulated_args.insert(k.clone(), v.clone());
        }
      }
      // 2. Otherwise, try parsing the buffer as complete JSON
      else if !tool_call_buffer.is_empty() {
        log::info!("Parsing tool_call_buffer: {}", tool_call_buffer);
        match serde_json::from_str::<HashMap<String, JsonValue>>(
          tool_call_buffer,
        ) {
          Ok(parsed_args) => {
            log::info!("Successfully parsed buffer as JSON");
            partial.accumulated_args = parsed_args;
          }
          Err(e) => {
            log::warn!("Failed to parse tool_call_buffer as JSON: {}", e);
          }
        }
      }
      // 3. Otherwise, try parsing the delta as JSON (may be incremental or complete)
      else if !event.delta.is_empty() {
        log::info!("Parsing delta: {}", event.delta);
        match serde_json::from_str::<HashMap<String, JsonValue>>(&event.delta) {
          Ok(delta_args) => {
            log::info!("Successfully parsed delta as complete JSON");
            // Merge delta args into accumulated args
            for (k, v) in delta_args {
              partial.accumulated_args.insert(k, v);
            }
          }
          Err(e) => {
            log::debug!("Delta is not complete JSON (may be streaming): {}", e);
            // Delta might be partial JSON - we'll rely on buffer accumulation
          }
        }
      }

      log::info!(
        "Accumulated args for {:?}: {:?}",
        event.tool_call_id,
        partial.accumulated_args
      );
    } else {
      log::warn!(
        "Received args event for unknown tool call {:?}",
        event.tool_call_id
      );
    }

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
      "Tool call ended - id: {:?}, SDK provided name: '{}', SDK provided args: {:?}",
      event.tool_call_id,
      tool_call_name,
      tool_call_args
    );

    // Retrieve buffered tool call info
    let key = format!("{:?}", event.tool_call_id);
    let mut buffer = self.current_tool_calls.lock().await;
    let partial = buffer.remove(&key);

    // CRITICAL FIX: Use buffered data instead of SDK parameters
    // The AG-UI SDK doesn't properly provide tool_call_name and tool_call_args in this callback
    let (final_tool_name, final_args) = if let Some(p) = partial {
      log::info!(
        "Using buffered data - name: {}, accumulated_args: {:?}",
        p.tool_name,
        p.accumulated_args
      );

      // Use accumulated args from our buffer (fallback to SDK args if our buffer is empty)
      let args = if p.accumulated_args.is_empty() {
        if !tool_call_args.is_empty() {
          log::info!("Using SDK provided args as fallback");
          tool_call_args.clone()
        } else {
          log::warn!("Both buffered and SDK args are empty!");
          HashMap::new()
        }
      } else {
        p.accumulated_args
      };

      (p.tool_name, args)
    } else {
      log::error!(
        "No buffered info found for {:?}, falling back to SDK parameters",
        event.tool_call_id
      );
      (tool_call_name.to_string(), tool_call_args.clone())
    };

    let request = ToolExecutionRequest {
      tool_call_id: event.tool_call_id.clone(),
      tool_name: final_tool_name.clone(),
      args: final_args,
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
    params: AgentSubscriberParams<'async_trait, StateT, FwdPropsT>,
  ) -> Result<(), AgentError> {
    log::info!(
      "🔧 New tool call added to messages - id: {:?}, name: {}, args: {:?}",
      tool_call.id,
      tool_call.function.name,
      tool_call.function.arguments
    );

    // If we have access to message history, add the Assistant message with this tool call
    // This is critical for submit_tool_result to work correctly - the tool result must
    // reference a tool call that exists in an Assistant message in the history
    if let Some(history) = &self.message_history {
      let mut history = history.lock().await;

      // Check if we already have an Assistant message with this tool call
      let has_tool_call = history.iter().any(|msg| {
        if let Message::Assistant { tool_calls, .. } = msg {
          if let Some(calls) = tool_calls {
            return calls.iter().any(|tc| tc.id == tool_call.id);
          }
        }
        false
      });

      if !has_tool_call {
        // Generate a random message ID (AG-UI SDK manages the canonical IDs internally)
        let message_id = MessageId::random();

        log::debug!(
          "Adding Assistant message with tool_call {:?} to history (message_id: {:?})",
          tool_call.id,
          message_id
        );

        // Add Assistant message with this tool call
        history.push(Message::Assistant {
          id: message_id,
          content: None, // Tool calls typically don't have text content
          name: None,
          tool_calls: Some(vec![tool_call.clone()]),
        });
      }
    }

    Ok(())
  }

  // All other events are ignored (default implementations)
}
