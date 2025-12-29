# AG-UI Subscriber-Based Streaming Refactor Plan

## Current Architecture

**Problem:** The low-level `HttpAgent::run()` method has lifetime constraints that prevent us from returning a stream that owns the input data.

**Current Flow:**
```
AIChatProcess::start_streaming()
  → AgUiClient::chat_stream()
  → HttpAgent::run(&input) [LIFETIME ISSUE HERE]
  → Returns Stream<Item = Event>
  → Map to Stream<Item = String> (text chunks)
```

## New Architecture Using Subscribers

### Overview

The official AG-UI SDK provides a higher-level `run_agent()` method that:
1. Takes `RunAgentParams` (not a reference)
2. Accepts subscriber(s) that implement `AgentSubscriber` trait
3. Runs the agent to completion and calls subscriber hooks for each event
4. Returns `RunAgentResult` with final state and messages

### Key Insight

Instead of returning a stream directly, we'll:
1. Create a custom subscriber that captures streaming events
2. Use a tokio channel to send text chunks from the subscriber to the caller
3. Run `run_agent()` in a background task
4. Return the channel receiver as our stream

### Implementation Plan

#### 1. Create `StreamingSubscriber` (new file: `src/llm/subscriber.rs`)

```rust
pub struct StreamingSubscriber {
    text_sender: mpsc::UnboundedSender<Result<String>>,
}

impl AgentSubscriber for StreamingSubscriber {
    async fn on_text_message_content_event(&self, event: &TextMessageContentEvent, ...) -> ... {
        // Send event.delta through channel
        let _ = self.text_sender.send(Ok(event.delta.clone()));
        Ok(AgentStateMutation::default())
    }

    async fn on_run_error_event(&self, event: &RunErrorEvent, ...) -> ... {
        // Send error through channel
        let _ = self.text_sender.send(Err(anyhow!("LLM error: {}", event.message)));
        Ok(AgentStateMutation::default())
    }

    // Implement other required trait methods as no-ops
}
```

**Details:**
- Uses `mpsc::UnboundedSender` to send text chunks
- Implements `AgentSubscriber<JsonValue, JsonValue>` trait
- Only handles events we care about (text content, errors)
- Other events are no-ops with `Ok(AgentStateMutation::default())`

#### 2. Update `AgUiClient::chat_stream()` (in `src/llm/client.rs`)

```rust
pub async fn chat_stream(
    &self,
    message: impl Into<String>,
    context_items: Option<Vec<Context>>,
) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::UnboundedReceiverStream;

    let message_str = message.into();

    // Create channel for streaming text
    let (tx, rx) = mpsc::unbounded_channel();

    // Create subscriber
    let subscriber = StreamingSubscriber::new(tx);

    // Build RunAgentParams
    let mut params = RunAgentParams::new()
        .add_message(Message::new_user(message_str))
        .with_forwarded_props(json!({
            "provider": self.provider,
            "model": self.model,
        }));

    // Add tools
    for tool in Self::default_tools() {
        params = params.add_tool(tool);
    }

    // Add context
    if let Some(context) = context_items {
        for ctx in context {
            params = params.add_context(ctx);
        }
    }

    // Clone agent for background task
    let agent = self.http_agent.clone(); // Need to make HttpAgent Clone

    // Spawn background task to run agent
    tokio::spawn(async move {
        if let Err(e) = agent.run_agent(&params, subscriber).await {
            // Send error if agent fails
            let _ = tx.send(Err(anyhow!("{}", e)));
        }
        // Channel closes when tx is dropped
    });

    // Return receiver as stream
    let stream = UnboundedReceiverStream::new(rx);
    Ok(Box::pin(stream))
}
```

**Key Changes:**
- No more lifetime issues - params is moved into spawn
- Background task runs `run_agent()` to completion
- Subscriber sends events through channel
- Return type stays the same: `Stream<Item = Result<String>>`

#### 3. Make `HttpAgent` Cloneable

**Problem:** HttpAgent doesn't implement Clone

**Options:**
a. Wrap in `Arc` in our AgUiClient
b. Recreate HttpAgent in the spawn (store URL + headers)
c. Check if HttpAgent can be made Clone

**Recommended:** Wrap in Arc

```rust
pub struct AgUiClient {
    http_agent: Arc<HttpAgent>,  // Changed from HttpAgent
    subprocess: LlmSubprocess,
    provider: String,
    model: String,
}
```

#### 4. Handle AIChatProcess (minimal changes)

The `AIChatProcess::start_streaming()` signature and usage stays the same!
- Still returns `Stream<Item = Result<String>>`
- Still maps events to text chunks
- The only change is internal to AgUiClient

### File Changes Summary

**New Files:**
- `src/llm/subscriber.rs` - StreamingSubscriber implementation

**Modified Files:**
- `src/llm/client.rs`:
  - Wrap `http_agent` in `Arc`
  - Rewrite `chat_stream()` to use subscriber + background task
  - Add dependency on `tokio-stream`

- `src/llm/mod.rs`:
  - Add `pub mod subscriber;`
  - Export StreamingSubscriber if needed (probably not)

- `src/Cargo.toml`:
  - Add `tokio-stream = "0.1"` dependency

**No Changes Needed:**
- `src/ai_proc/chat_process.rs` - API stays the same
- `src/bin/terminai.rs` - No changes needed

### Dependencies to Add

```toml
[dependencies]
tokio-stream = "0.1"  # For UnboundedReceiverStream
```

### Benefits of This Approach

1. **No lifetime issues** - Data is owned by the background task
2. **Uses official SDK properly** - Following the intended pattern
3. **Minimal API changes** - AIChatProcess doesn't need updates
4. **Better error handling** - Subscriber can catch and forward all errors
5. **Future extensibility** - Easy to add more event handlers

### Potential Issues & Solutions

**Issue 1:** HttpAgent might not be thread-safe
- **Solution:** Check if it's Send + Sync, wrap in Arc if so

**Issue 2:** Background task might outlive client
- **Solution:** Store JoinHandle, add shutdown method

**Issue 3:** Channel could fill up with unbounded sender
- **Solution:** Use bounded channel if needed (unlikely for text streaming)

### Testing Strategy

1. **Unit test** StreamingSubscriber with mock events
2. **Integration test** AgUiClient::chat_stream with test server
3. **Manual test** Full flow with Ollama/functiongemma
4. **Verify** old llm_old tests still pass

### Migration Path

1. Implement StreamingSubscriber
2. Update AgUiClient with Arc wrapper
3. Rewrite chat_stream() method
4. Test with existing AIChatProcess (should work unchanged)
5. Clean up any unused imports
6. Mark old custom client code for removal

### Open Questions

1. Should we implement all AgentSubscriber methods or just the ones we need?
   - **Answer:** Just the ones we need, rest are default impls returning Ok(default())

2. Do we need to handle tool calls in the subscriber?
   - **Answer:** Not for now - we can add later when implementing tool execution

3. Should subscriber be generic over state/props?
   - **Answer:** No, use `JsonValue` for both - simpler

## Next Steps

1. Get user approval on this plan
2. Implement StreamingSubscriber
3. Update AgUiClient
4. Test end-to-end
5. Clean up old code
