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
  /// Accumulated arguments from streaming chunks (parsed JSON)
  accumulated_args: HashMap<String, JsonValue>,
  /// Raw string buffer for accumulating argument deltas (for streaming)
  raw_args_buffer: String,
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
      raw_args_buffer: String::new(),
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

    // Accumulate args using multiple strategies
    let key = format!("{:?}", event.tool_call_id);
    let mut buffer = self.current_tool_calls.lock().await;

    if let Some(partial) = buffer.get_mut(&key) {
      // Strategy 1: If SDK provides parsed args, use them
      if !partial_tool_call_args.is_empty() {
        log::info!("Using partial_tool_call_args from SDK");
        for (k, v) in partial_tool_call_args {
          partial.accumulated_args.insert(k.clone(), v.clone());
        }
      }
      // Strategy 2: If SDK provides complete buffer, parse it
      else if !tool_call_buffer.is_empty() {
        log::debug!("Tool call buffer available: {}", tool_call_buffer);
        if let Ok(parsed_args) =
          serde_json::from_str::<HashMap<String, JsonValue>>(tool_call_buffer)
        {
          log::info!("Parsed complete buffer as JSON");
          partial.accumulated_args = parsed_args;
        }
      }

      // Strategy 3: Always accumulate the raw delta for later parsing
      // This handles streaming where args come in chunks
      if !event.delta.is_empty() {
        partial.raw_args_buffer.push_str(&event.delta);
        log::debug!(
          "Accumulated raw args buffer ({} chars): {}",
          partial.raw_args_buffer.len(),
          partial.raw_args_buffer
        );
      }

      log::debug!(
        "Args state for {:?}: parsed={:?}, raw_buffer_len={}",
        event.tool_call_id,
        partial.accumulated_args.len(),
        partial.raw_args_buffer.len()
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
        "Using buffered data - name: {}, accumulated_args: {:?}, raw_buffer_len: {}",
        p.tool_name,
        p.accumulated_args,
        p.raw_args_buffer.len()
      );

      // Determine args using multiple fallback strategies:
      // 1. Use pre-parsed accumulated_args if available
      // 2. Try to parse the raw_args_buffer (accumulated from streaming deltas)
      // 3. Use SDK provided args
      // 4. Fall back to empty HashMap
      let args = if !p.accumulated_args.is_empty() {
        log::info!("Using pre-parsed accumulated_args");
        p.accumulated_args
      } else if !p.raw_args_buffer.is_empty() {
        // Try to parse the accumulated raw buffer as JSON
        log::info!("Parsing raw_args_buffer: {}", p.raw_args_buffer);
        match serde_json::from_str::<HashMap<String, JsonValue>>(
          &p.raw_args_buffer,
        ) {
          Ok(parsed) => {
            log::info!(
              "Successfully parsed raw_args_buffer as JSON: {:?}",
              parsed
            );
            parsed
          }
          Err(e) => {
            log::warn!("Failed to parse raw_args_buffer as JSON: {}", e);
            if !tool_call_args.is_empty() {
              log::info!("Using SDK provided args as fallback");
              tool_call_args.clone()
            } else {
              HashMap::new()
            }
          }
        }
      } else if !tool_call_args.is_empty() {
        log::info!("Using SDK provided args as fallback");
        tool_call_args.clone()
      } else {
        log::warn!("No args available from any source!");
        HashMap::new()
      };

      (p.tool_name, args)
    } else {
      log::error!(
        "No buffered info found for {:?}, falling back to SDK parameters",
        event.tool_call_id
      );
      (tool_call_name.to_string(), tool_call_args.clone())
    };

    // CRITICAL FIX: Add AssistantMessage with tool_call to history BEFORE sending request.
    // This fixes a race condition where on_new_tool_call is called by the SDK AFTER
    // on_tool_call_end_event, but submit_tool_result needs the AssistantMessage to be
    // in history when it's called.
    if let Some(history) = &self.message_history {
      let mut history = history.lock().await;

      // Check if we already have an Assistant message with this tool call
      let has_tool_call = history.iter().any(|msg| {
        if let Message::Assistant { tool_calls, .. } = msg {
          if let Some(calls) = tool_calls {
            return calls.iter().any(|tc| tc.id == event.tool_call_id);
          }
        }
        false
      });

      if !has_tool_call {
        // Construct a ToolCall from our buffered data
        // Use ToolCall::new() to properly construct with call_type="function"
        let args_string =
          serde_json::to_string(&final_args).unwrap_or_default();
        let function_call = ag_ui_core::types::message::FunctionCall {
          name: final_tool_name.clone(),
          arguments: args_string,
        };
        let tool_call =
          ToolCall::new(event.tool_call_id.clone(), function_call);

        let message_id = MessageId::random();
        log::info!(
          "Adding Assistant message with tool_call {:?} to history BEFORE sending request (message_id: {:?})",
          event.tool_call_id,
          message_id
        );

        history.push(Message::Assistant {
          id: message_id,
          content: None,
          name: None,
          tool_calls: Some(vec![tool_call]),
        });
      }
    }

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
