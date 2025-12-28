// TERMIN.AI: AG-UI subscriber for streaming text events

use ag_ui_client::agent::{AgentError, AgentStateMutation};
use ag_ui_client::subscriber::{AgentSubscriber, AgentSubscriberParams};
use ag_ui_core::event::{RunErrorEvent, TextMessageContentEvent};
use ag_ui_core::{AgentState, FwdProps};
use tokio::sync::mpsc;

/// Subscriber that captures streaming text events and sends them through a channel
pub struct StreamingSubscriber {
  text_sender: mpsc::UnboundedSender<anyhow::Result<String>>,
}

impl StreamingSubscriber {
  pub fn new(
    text_sender: mpsc::UnboundedSender<anyhow::Result<String>>,
  ) -> Self {
    Self { text_sender }
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

  // All other events are ignored (default implementations)
}
