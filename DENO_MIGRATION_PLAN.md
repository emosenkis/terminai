# Deno + TypeScript Migration Plan

**Date:** 2026-01-14
**Last Updated:** 2026-01-15
**Status:** Phase 1-3 Complete ✅ | Phase 4-5 In Progress

---

## Executive Summary

Replace the Python runtime and AG-UI protocol HTTP communication with an **embedded Deno runtime** running **TypeScript code** that uses the **Anthropic Agent TypeScript SDK** directly. This eliminates:

- Python dependency and complex installation (`uv`, Python 3.11+)
- AG-UI protocol HTTP communication overhead
- Custom `ag-ui-claude-sdk` wrapper dependency
- Subprocess management complexity
- Port allocation and secret handshake protocol

**Benefits:**
- Simpler installation (no Python required)
- Faster communication (direct Rust ↔ TypeScript calls)
- Better type safety (TypeScript types match Rust types)
- Official SDK support (no custom wrappers)
- Single language ecosystem for scripting (TypeScript)

---

## Current Architecture (Python + AG-UI Protocol)

```
┌─────────────────────────────────────────────────────────────┐
│ Rust Application (Termin.AI)                                │
│                                                              │
│  ┌────────────────┐         HTTP POST                       │
│  │ AgUiClient     │────────────────────┐                    │
│  │  - Provider    │                    │                    │
│  │  - Model       │                    ▼                    │
│  │  - Context     │         ┌─────────────────────┐        │
│  └────────────────┘         │  LlmSubprocess      │        │
│         │                   │  - Secret (UUID)    │        │
│         │                   │  - Port (18080+)    │        │
│         │                   │  - Child Process    │        │
│         ▼                   └─────────────────────┘        │
│  ┌────────────────┐                   │                    │
│  │StreamingSub-   │                   │ uv run python      │
│  │scriber         │                   │ -m terminai_agent  │
│  │  - Text Stream │                   │ --secret ...       │
│  │  - Tool Calls  │                   │ --port-range ...   │
│  └────────────────┘                   │                    │
└─────────────────────────────────────────────────────────────┘
                                         │
                                         │ stdout: AG_UI_PORT=
                                         │
                    ┌────────────────────┴─────────────────────┐
                    │                                          │
                    ▼                                          │
         ┌─────────────────────────┐                           │
         │ Python Subprocess       │                           │
         │                         │                           │
         │  ┌──────────────────┐   │                           │
         │  │ FastAPI Server   │   │ ◄─── HTTP POST           │
         │  │  (server.py)     │   │      x-ag-ui-secret      │
         │  │  - /health       │───┼──────────────────────────┘
         │  │  - POST /        │   │      RunAgentInput
         │  └──────────────────┘   │      - forwarded_props
         │           │              │      - messages
         │           ▼              │      - tools
         │  ┌──────────────────┐   │      - context
         │  │ ClaudeAgent      │   │
         │  │ Adapter          │   │ ────► Event Stream
         │  │  (agent.py)      │   │      (TextMessageContent,
         │  │  - System Prompt │   │       ToolCall,
         │  │  - Tools         │   │       ToolResult, ...)
         │  │  - Streaming     │   │
         │  └──────────────────┘   │
         │           │              │
         │           ▼              │
         │  ┌──────────────────┐   │
         │  │ Claude Agent SDK │   │
         │  │  (Python)        │   │
         │  └──────────────────┘   │
         └─────────────────────────┘
```

**Problems:**
1. **Complex Installation:** Requires Python 3.11+, `uv`, and multiple Python packages
2. **HTTP Overhead:** Every request goes through HTTP with JSON serialization
3. **Port Management:** Need to find available port, print to stdout, parse from Rust
4. **Secret Handshake:** UUID generation, header validation, middleware overhead
5. **Custom Wrapper:** Depends on fork of AG-UI SDK with custom integration
6. **Process Management:** Subprocess lifecycle, stderr/stdout capture, timeouts
7. **Two-Language Ecosystem:** Python + Rust with different paradigms

---

## New Architecture (Embedded Deno + TypeScript)

```
┌────────────────────────────────────────────────────────────────┐
│ Rust Application (Termin.AI)                                   │
│                                                                 │
│  ┌────────────────┐                                            │
│  │ LlmClient      │                                            │
│  │  - Provider    │                                            │
│  │  - Model       │                                            │
│  │  - Context     │                                            │
│  └────────────────┘                                            │
│         │                                                       │
│         │ Direct Function Call                                 │
│         ▼                                                       │
│  ┌──────────────────────────────────────────────────┐          │
│  │ deno_core JsRuntime (Embedded V8)                │          │
│  │                                                  │          │
│  │  ┌────────────────────────────────────────────┐ │          │
│  │  │ Bundled TypeScript Agent (agent.js)        │ │          │
│  │  │ (built by vite, embedded via include_str!) │ │          │
│  │  │                                            │ │          │
│  │  │  import Anthropic from "@anthropic-ai/sdk" │ │          │
│  │  │                                            │ │          │
│  │  │  export async function chatStream(opts) {  │ │          │
│  │  │    const client = new Anthropic({apiKey})  │ │          │
│  │  │    const response = await client.messages  │ │          │
│  │  │      .create({                             │ │          │
│  │  │        model: opts.model,                  │ │          │
│  │  │        system: systemPrompt,               │ │          │
│  │  │        messages: [...],                    │ │          │
│  │  │        tools: getToolDefinitions(),        │ │          │
│  │  │      })                                    │ │          │
│  │  │    // Handle tool calls, return messages   │ │          │
│  │  │  }                                         │ │          │
│  │  └────────────────────────────────────────────┘ │          │
│  │                    │                             │          │
│  │                    ▼                             │          │
│  │  ┌────────────────────────────────────────────┐ │          │
│  │  │ Custom Tools (custom_tools.ts)             │ │          │
│  │  │                                            │ │          │
│  │  │  // Call back into Rust ops                │ │          │
│  │  │  async function suggestCommand(args) {     │ │          │
│  │  │    return await Deno.core.ops              │ │          │
│  │  │      .op_suggest_command(args)             │ │          │
│  │  │  }                                         │ │          │
│  │  │                                            │ │          │
│  │  │  async function readScrollback(args) {     │ │          │
│  │  │    return await Deno.core.ops              │ │          │
│  │  │      .op_read_scrollback(args)             │ │          │
│  │  │  }                                         │ │          │
│  │  └────────────────────────────────────────────┘ │          │
│  │                    │                             │          │
│  │  ┌────────────────────────────────────────────┐ │          │
│  │  │ Fetch Polyfill (fetch_polyfill.ts)         │ │          │
│  │  │                                            │ │          │
│  │  │  // Provides fetch() using Rust op         │ │          │
│  │  │  globalThis.fetch = async (url, init) => { │ │          │
│  │  │    return Deno.core.ops.op_fetch(url, init)│ │          │
│  │  │  }                                         │ │          │
│  │  └────────────────────────────────────────────┘ │          │
│  │                    │                             │          │
│  │                    │ Deno.core.ops.xxx()         │          │
│  │                    ▼                             │          │
│  └──────────────────────────────────────────────────┘          │
│                      │                                         │
│                      │ Rust Ops Extension                      │
│                      ▼                                         │
│  ┌──────────────────────────────────────────────────┐          │
│  │ Rust Ops (src/deno/ops.rs)                       │          │
│  │  - op_suggest_command  → ToolExecutor            │          │
│  │  - op_read_scrollback  → ToolExecutor            │          │
│  │  - op_fetch            → reqwest HTTP client     │          │
│  └──────────────────────────────────────────────────┘          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Advantages:**
1. **No External Process:** V8 runs in-process via `deno_core`
2. **Direct Communication:** Rust calls TypeScript functions directly (no HTTP)
3. **Type Safety:** Serialize/deserialize via `serde_v8` with shared types
4. **Simpler Deployment:** No Python installation required
5. **Better Performance:** No subprocess spawn, port allocation, HTTP roundtrips
6. **Build-time Bundling:** vite bundles all npm deps into single JS file

---

## Implementation Plan

### Phase 1: Create TypeScript Agent Module

**Location:** `typescript/agent/`

**Files:**
```
typescript/
├── deno.json              # Deno project configuration
├── deno.lock              # Dependency lock file
├── import_map.json        # Import map for SDK
├── agent/
│   ├── mod.ts             # Main entry point
│   ├── agent.ts           # Core agent implementation
│   ├── custom_tools.ts    # Termin.AI-specific tools
│   ├── types.ts           # TypeScript type definitions
│   └── system_prompt.ts   # System prompt builder
└── tests/
    ├── agent_test.ts      # Unit tests
    └── integration_test.ts # Integration tests
```

**Key Components:**

**`agent.ts`:**
```typescript
import { query, type Options } from "@anthropic-ai/claude-agent-sdk";

export interface ChatOptions {
  message: string;
  model: string;
  provider: string;
  systemPrompt?: string;
  terminalContext?: TerminalContext;
}

export interface TerminalContext {
  historyLines?: string[];
  cwd: string;
  lastExitCode?: number;
  osInfo?: string;
  shell?: string;
}

export async function* chatStream(options: ChatOptions) {
  const sdkOptions: Options = {
    model: options.model,
    systemPrompt: buildSystemPrompt(options.terminalContext),
    tools: ["Read", "Grep", "Bash"], // Built-in tools
    mcpServers: {
      terminai: createTerminaiServer() // Custom tools
    },
    permissionMode: "acceptEdits",
    includePartialMessages: false,
  };

  const result = query({
    prompt: options.message,
    options: sdkOptions
  });

  for await (const message of result) {
    yield message;
  }
}
```

**`custom_tools.ts`:**
```typescript
import { tool, createSdkMcpServer } from "@anthropic-ai/claude-agent-sdk";
import { z } from "zod";

const suggestCommand = tool(
  "suggest_command",
  "Suggest a shell command to execute in the terminal",
  {
    command: z.string().describe("The command to execute"),
    explanation: z.string().optional().describe("Explanation of what the command does")
  },
  async (args: { command: string; explanation?: string }) => {
    // Call back into Rust
    const result = await (globalThis as any).Deno.core.ops.suggest_command(args);
    return {
      content: [{
        type: "text",
        text: result
      }]
    };
  }
);

const readScrollback = tool(
  "read_scrollback",
  "Read the terminal scrollback history",
  {
    numLines: z.number().optional().default(100).describe("Number of lines to read")
  },
  async (args: { numLines?: number }) => {
    const result = await (globalThis as any).Deno.core.ops.read_scrollback(args);
    return {
      content: [{
        type: "text",
        text: result
      }]
    };
  }
);

export function createTerminaiServer() {
  return createSdkMcpServer({
    name: "terminai",
    version: "1.0.0",
    tools: [suggestCommand, readScrollback]
  });
}
```

### Phase 2: Implement Deno Runtime in Rust

**Location:** `src/deno/`

**Files:**
```
src/
├── deno/
│   ├── mod.rs              # Module exports
│   ├── runtime.rs          # Deno runtime initialization
│   ├── ops.rs              # Rust ops for TypeScript
│   ├── bridge.rs           # Rust ↔ TypeScript bridge
│   └── types.rs            # Shared type definitions
```

**Key Components:**

**`runtime.rs`:**
```rust
use deno_core::{JsRuntime, RuntimeOptions, Extension};
use deno_runtime::permissions::PermissionsContainer;
use deno_runtime::worker::{MainWorker, WorkerOptions};

pub struct DenoAgent {
    worker: MainWorker,
}

impl DenoAgent {
    pub async fn new() -> Result<Self> {
        // Create Deno runtime with custom ops
        let extensions = vec![
            create_terminai_ops_extension(),
        ];

        let options = WorkerOptions {
            extensions,
            // ... other options
        };

        let worker = MainWorker::bootstrap_from_options(
            module_specifier,
            permissions,
            options,
        );

        // Load TypeScript agent module
        worker.execute_main_module(&module_specifier).await?;

        Ok(Self { worker })
    }

    pub async fn chat_stream(
        &mut self,
        options: ChatOptions,
    ) -> Result<impl Stream<Item = Result<SdkMessage>>> {
        // Call TypeScript function
        let js_options = serde_json::to_value(&options)?;
        let result = self.worker.js_runtime.call_and_await(
            "chatStream",
            &[js_options]
        ).await?;

        // Convert to Rust stream
        let stream = convert_async_iterator_to_stream(result)?;
        Ok(stream)
    }
}
```

**`ops.rs`:**
```rust
use deno_core::op2;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct SuggestCommandArgs {
    pub command: String,
    pub explanation: Option<String>,
}

#[op2(async)]
#[serde]
pub async fn op_suggest_command(
    state: Rc<RefCell<OpState>>,
    #[serde] args: SuggestCommandArgs,
) -> Result<String> {
    let tool_executor = state.borrow().borrow::<Arc<ToolExecutor>>().clone();

    let request = ToolExecutionRequest {
        tool_call_id: ToolCallId::new(),
        tool_name: "suggest_command".to_string(),
        args: serde_json::to_value(&args)?.as_object().unwrap().clone(),
    };

    let result = tool_executor.execute_tool(request).await;
    Ok(result.content)
}

#[op2(async)]
#[serde]
pub async fn op_read_scrollback(
    state: Rc<RefCell<OpState>>,
    #[serde] args: ReadScrollbackArgs,
) -> Result<String> {
    // Similar implementation
}

pub fn create_terminai_ops_extension() -> Extension {
    Extension::builder("terminai")
        .ops(vec![
            op_suggest_command::DECL,
            op_read_scrollback::DECL,
        ])
        .build()
}
```

### Phase 3: Replace Python Components

**Changes to `src/llm/`:**

1. **Remove:**
   - `llm_subprocess.rs` (entire file)
   - All AG-UI protocol code in `client.rs`
   - All HTTP communication code

2. **Add:**
   - `deno_client.rs` - New client using Deno runtime

3. **Update:**
   - `mod.rs` - Export new Deno client
   - `client.rs` - Simplify to just wrap Deno calls

**New `deno_client.rs`:**
```rust
use crate::deno::DenoAgent;
use crate::llm::{ChatOptions, TerminalContext};
use futures::Stream;

pub struct DenoLlmClient {
    agent: DenoAgent,
}

impl DenoLlmClient {
    pub async fn new() -> Result<Self> {
        let agent = DenoAgent::new().await?;
        Ok(Self { agent })
    }

    pub async fn chat_stream(
        &mut self,
        message: String,
        terminal_context: Option<&TerminalContext>,
    ) -> Result<impl Stream<Item = Result<SdkMessage>>> {
        let options = ChatOptions {
            message,
            model: "claude-sonnet-4-5".to_string(),
            provider: "anthropic".to_string(),
            system_prompt: None,
            terminal_context: terminal_context.cloned(),
        };

        self.agent.chat_stream(options).await
    }
}
```

### Phase 4: Remove Python Code

**Delete:**
```
python/
├── pyproject.toml
├── terminai_agent/
│   ├── __init__.py
│   ├── __main__.py
│   ├── server.py
│   ├── agent.py
│   ├── config.py
│   └── forwarded_props.py
└── tests/
```

**Remove from `Cargo.toml`:**
- All `ag-ui-*` dependencies
- HTTP client dependencies (if only used for LLM)

### Phase 5: Update Tests

**Convert Python tests to TypeScript:**
```typescript
// typescript/tests/agent_test.ts
import { assertEquals } from "https://deno.land/std@0.208.0/assert/mod.ts";
import { chatStream } from "../agent/agent.ts";

Deno.test("chatStream basic functionality", async () => {
  const options = {
    message: "Hello",
    model: "claude-sonnet-4-5",
    provider: "anthropic",
  };

  const stream = chatStream(options);
  let messageCount = 0;

  for await (const message of stream) {
    messageCount++;
    console.log(message);
  }

  assertEquals(messageCount > 0, true);
});
```

**Update Rust integration tests:**
```rust
#[tokio::test]
async fn test_deno_client_chat() {
    let mut client = DenoLlmClient::new().await.unwrap();

    let stream = client.chat_stream(
        "What is 2+2?".to_string(),
        None,
    ).await.unwrap();

    let messages: Vec<_> = stream.collect().await;
    assert!(!messages.is_empty());
}
```

---

## Type Mapping: TypeScript ↔ Rust

**Shared Types:**

| TypeScript Type | Rust Type | Notes |
|----------------|-----------|-------|
| `string` | `String` | Direct mapping |
| `number` | `i32`, `f64` | Depends on context |
| `boolean` | `bool` | Direct mapping |
| `Array<T>` | `Vec<T>` | Generic arrays |
| `Record<string, T>` | `HashMap<String, T>` | Key-value maps |
| `ChatOptions` | `ChatOptions` | Shared struct |
| `TerminalContext` | `TerminalContext` | Shared struct |
| `SDKMessage` | `SdkMessage` | Enum union |
| `SDKAssistantMessage` | `SdkAssistantMessage` | Message variant |
| `SDKUserMessage` | `SdkUserMessage` | Message variant |

**Example Shared Definition:**

**TypeScript (`typescript/agent/types.ts`):**
```typescript
export interface TerminalContext {
  historyLines?: string[];
  cwd: string;
  lastExitCode?: number;
  osInfo?: string;
  shell?: string;
}
```

**Rust (`src/deno/types.rs`):**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_lines: Option<Vec<String>>,
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
}
```

---

## Message Flow Example

**1. User types in AI overlay:**
```
User: "What files are in this directory?"
```

**2. Rust app calls Deno:**
```rust
let stream = deno_client.chat_stream(
    "What files are in this directory?".to_string(),
    Some(&terminal_context),
).await?;
```

**3. Deno executes TypeScript:**
```typescript
const result = query({
  prompt: "What files are in this directory?",
  options: {
    model: "claude-sonnet-4-5",
    systemPrompt: "You are a terminal assistant...",
    tools: ["Read", "Grep", "Bash"],
    // ...
  }
});
```

**4. Claude SDK calls built-in Bash tool:**
```typescript
// SDK automatically handles this
// Bash tool executes: ls
```

**5. TypeScript yields message back to Rust:**
```typescript
for await (const msg of result) {
  yield msg; // Rust receives this via stream
}
```

**6. Rust displays in UI:**
```rust
while let Some(msg) = stream.next().await {
    match msg {
        SdkMessage::Assistant { message, .. } => {
            // Display assistant's response in overlay
        }
        SdkMessage::Result { result, .. } => {
            // Show final result
        }
        _ => {}
    }
}
```

---

## Error Handling

**TypeScript:**
```typescript
try {
  const result = query({ ... });
  for await (const msg of result) {
    yield msg;
  }
} catch (error) {
  yield {
    type: "result",
    subtype: "error_during_execution",
    errors: [error.message],
    // ...
  };
}
```

**Rust:**
```rust
match deno_client.chat_stream(...).await {
    Ok(stream) => {
        // Process stream
    }
    Err(e) => {
        log::error!("Chat stream error: {}", e);
        // Show error in UI
    }
}
```

---

## Performance Considerations

**Startup Time:**
- **Python:** ~500ms (spawn subprocess, wait for port, HTTP handshake)
- **Deno:** ~50ms (load module into existing runtime)
- **Improvement:** 10x faster startup

**Request Latency:**
- **Python:** ~5-10ms (HTTP round-trip + JSON serialization)
- **Deno:** <1ms (direct function call + serde serialization)
- **Improvement:** 5-10x lower latency

**Memory:**
- **Python:** ~50MB (subprocess + FastAPI + dependencies)
- **Deno:** ~10MB (embedded V8 runtime)
- **Improvement:** 5x lower memory usage

---

## Migration Checklist

### Code Changes
- [x] Create `typescript/agent/` directory structure
- [x] Implement TypeScript agent using Anthropic SDK (see Implementation Notes)
- [x] Create custom tools with Rust ops bridge
- [x] Implement `src/deno/` Rust module
- [x] Create Deno runtime initialization
- [x] Implement ops for custom tools (`op_suggest_command`, `op_read_scrollback`, `op_fetch`)
- [x] Create `deno_client.rs` with thread-safe channel-based architecture
- [x] Update `src/llm/mod.rs` exports (removed AG-UI, kept only Deno)
- [x] Update `src/ai_proc/chat_process.rs` to use `DenoLlmClient`
- [x] Update `src/bin/terminai.rs` to use new tool notification system
- [x] Create local `ToolCallId` type (removed ag-ui dependency)

### Dependencies
- [x] Add Deno runtime dependencies to `Cargo.toml` (`deno_core`, `serde_v8`, `deno_error`)
- [x] Create TypeScript build config (`package.json` + `vite.config.ts` - see Implementation Notes)
- [x] Add `@anthropic-ai/sdk` dependency (see Implementation Notes)
- [ ] Remove Python dependencies (`pyproject.toml`)
- [ ] Remove AG-UI Rust dependencies from `Cargo.toml`

### Testing
- [ ] Write TypeScript unit tests (`agent_test.ts`)
- [x] Write Rust integration tests for Deno bridge (7 tests in `runtime.rs`)
- [x] Write Rust integration tests for DenoLlmClient (3 tests)
- [ ] Test custom tools end-to-end (suggest_command, read_scrollback)
- [x] Test streaming message flow (basic test passes)
- [ ] Test error handling
- [ ] Remove Python tests

### Documentation
- [ ] Update README.md (no Python requirement)
- [ ] Update IMPLEMENTATION_PLAN.md
- [ ] Update MPROCS_BORROWED.md (if affected)
- [x] Update DENO_MIGRATION_PLAN.md (this document)
- [ ] Update build instructions

### Cleanup (Phase 4 - Next)
- [ ] Delete entire `python/` directory
- [ ] Delete `llm_subprocess.rs`
- [ ] Delete `src/llm/client.rs` (old AG-UI client)
- [ ] Delete `src/llm/subscriber.rs` (AG-UI subscriber)
- [ ] Delete `src/llm/tool_coordinator.rs` (AG-UI coordinator)
- [ ] Delete `src/llm/forwarded_props.rs` (AG-UI props)
- [ ] Remove AG-UI Rust dependencies from `Cargo.toml`
- [ ] Remove unused HTTP dependencies

---

## Rollback Plan

If migration encounters critical issues:

1. **Git revert:** Revert to commit before migration started
2. **Keep Python:** Maintain Python implementation temporarily
3. **Parallel implementation:** Run Deno alongside Python with feature flag
4. **Gradual migration:** Migrate one feature at a time (e.g., only Anthropic provider first)

**Feature Flag Example:**
```rust
enum LlmBackend {
    Python,  // Original implementation
    Deno,    // New implementation
}

let backend = if cfg!(feature = "deno-backend") {
    LlmBackend::Deno
} else {
    LlmBackend::Python
};
```

---

## Security Considerations

**Sandboxing:**
- Deno has built-in permissions system
- Can restrict file system access, network access, environment variables
- Configure via `deno_runtime::permissions::PermissionsContainer`

**API Key Protection:**
- Environment variables passed to Deno runtime
- Never log or expose API keys
- Use same security practices as Python implementation

**Tool Execution:**
- Bash tool still requires user approval (same as before)
- Risk assessment happens in Rust (no change)
- Deno runtime isolated from terminal PTY

---

## Open Questions (Resolved)

1. **Multi-Provider Support:**
   - Claude SDK primarily supports Anthropic
   - How to handle OpenAI, Gemini, Ollama, OpenRouter?
   - **Answer:** Using `@anthropic-ai/sdk` directly. For other providers, would need separate SDK integrations or a unified HTTP client approach.

2. **TypeScript Compilation:**
   - Compile TS to JS ahead of time, or use Deno's JIT?
   - **Answer:** Compile ahead of time using `vite`. Native Deno module loading doesn't work with `deno_core` (requires full `deno_runtime`).

3. **Distribution:**
   - Bundle TypeScript files with Rust binary?
   - **Answer:** Yes, using `include_str!()` to embed the vite-bundled `dist/agent.js` at compile time.

4. **Version Pinning:**
   - How to pin SDK version?
   - **Answer:** Using `pnpm-lock.yaml` (npm ecosystem, not Deno's deno.lock)

---

## Success Criteria

Migration is successful when:

1. ✅ All existing LLM features work (chat, streaming, tools)
2. ✅ No Python installation required
3. ✅ Startup time < 100ms (vs ~500ms with Python)
4. ✅ Request latency < 1ms (vs ~5-10ms with Python)
5. ✅ All tests pass
6. ✅ No regressions in user-visible behavior
7. ✅ Documentation updated
8. ✅ Can remove entire `python/` directory

---

## Timeline

- **Phase 1:** TypeScript agent - 2 days
- **Phase 2:** Deno runtime - 2 days
- **Phase 3:** Replace Python - 1 day
- **Phase 4:** Remove Python - 0.5 days
- **Phase 5:** Update tests - 1 day
- **Total:** ~6.5 days

---

## References

- **Anthropic Agent SDK (TypeScript):** https://docs.anthropic.com/en/docs/agent-sdk/typescript
- **Deno Runtime:** https://deno.com/blog/roll-your-own-javascript-runtime-pt2
- **deno_core crate:** https://docs.rs/deno_core/latest/deno_core/
- **Current Python Implementation:** `python/terminai_agent/`
- **Current Rust Integration:** `src/llm_subprocess.rs`, `src/llm/client.rs`

---

## Implementation Notes & Advice for Future Work

*Added 2026-01-15 after completing Phase 1 & 2*

### Major Deviations from Original Plan

#### 1. Using `@anthropic-ai/sdk` NOT `@anthropic-ai/claude-agent-sdk`

The original plan specified using the Claude Agent SDK (`@anthropic-ai/claude-agent-sdk`). **This won't work** for our use case because:

- The Agent SDK spawns its own subprocess to run tools
- This defeats the purpose of embedding Deno (we'd still have subprocess management)
- The Agent SDK is designed for CLI tools, not embedded runtimes

**Solution:** Use the plain `@anthropic-ai/sdk` which is a pure HTTP client. We implement tool handling ourselves in TypeScript, calling back into Rust ops for custom tools.

#### 2. Using `pnpm` + `vite` NOT Native Deno

The original plan assumed using native Deno with `deno.json` and import maps. **This doesn't work well** because:

- `deno_core` is a bare V8 runtime without Deno's module loader
- npm specifiers (`npm:@anthropic-ai/sdk`) require `deno_runtime` (very complex)
- Loading external modules at runtime requires network access and module resolution

**Solution:** Bundle TypeScript to a single JavaScript file at build time:

```
typescript/
├── package.json          # pnpm project config
├── vite.config.ts        # Vite bundler config  
├── tsconfig.json         # TypeScript config
└── agent/
    └── *.ts              # Source files
```

The `build.rs` script runs `pnpm build` which produces `dist/agent.js`, embedded via `include_str!()`.

#### 3. Using `deno_core` NOT `deno_runtime`

The original plan suggested using `deno_runtime::worker::MainWorker`. **This is extremely complex** because:

- `deno_runtime` pulls in 20+ extension crates with strict version coupling
- Enabling `fetch` requires `deno_fetch` → `deno_web` → `deno_net` → `deno_tls` → `deno_permissions`
- The `deno_permissions` crate requires implementing complex traits (`PermissionDescriptorParser`)
- Version mismatches between crates cause type conflicts (`deno_core::Extension` vs `deno_runtime::deno_core::Extension`)

**Solution:** Use only `deno_core` (v0.376.0) with custom ops:

```toml
# Cargo.toml - minimal dependencies
deno_core = "0.376.0"
serde_v8 = "0.285.0"
deno_error = "0.7.0"
```

#### 4. Custom `op_fetch` Instead of Deno's Native Fetch

The Anthropic SDK requires `fetch()`. Deno's native fetch requires the full extension chain mentioned above.

**Solution:** Implement a minimal fetch polyfill in TypeScript that calls a Rust op:

```typescript
// typescript/agent/fetch_polyfill.ts
globalThis.fetch = async (url, init) => {
  const response = await Deno.core.ops.op_fetch(url, { method, headers, body });
  return new FetchResponse(response.status, response.body, url);
};
```

```rust
// src/deno/ops.rs
#[op2(async(lazy))]
#[serde]
pub async fn op_fetch(
  state: Rc<RefCell<OpState>>,
  #[string] url: String,
  #[serde] options: FetchOptions,
) -> Result<FetchResponse, deno_error::JsErrorBox> {
  // Use reqwest for HTTP requests
  let client = state.borrow().borrow::<TerminaiOpState>().http_client.clone();
  // ... make request ...
}
```

### Critical Implementation Details

#### Event Loop Handling

**Problem:** Async functions would hang forever when using `run_event_loop()` because it waits for ALL pending ops to complete (including background timers from SDK initialization).

**Solution:** Use `with_event_loop_promise()` which returns when the specific promise resolves:

```rust
// WRONG - hangs forever
let resolved = self.runtime.resolve(result_global).await?;
self.runtime.run_event_loop(PollEventLoopOptions::default()).await?;

// CORRECT - returns when promise resolves
let resolve_future = self.runtime.resolve(result_global);
let resolved = self.runtime
  .with_event_loop_promise(resolve_future, PollEventLoopOptions::default())
  .await?;
```

#### Custom Cfg Flag for Conditional Compilation

The build system sets `has_bundled_agent` when the TypeScript bundle exists. This must be registered in `build.rs`:

```rust
// build.rs
fn main() {
  // REQUIRED: Register the custom cfg flag
  println!("cargo::rustc-check-cfg=cfg(has_bundled_agent)");
  
  // Set the flag if bundle exists
  if bundle_path.exists() {
    println!("cargo:rustc-cfg=has_bundled_agent");
  }
}
```

#### V8 Scope Access

Use the `deno_core::scope!` macro to access V8 scope for deserialization:

```rust
deno_core::scope!(scope, self.runtime);
let local = v8::Local::new(scope, resolved_global);
let result: T = serde_v8::from_v8(scope, local)?;
```

### File Structure (Actual Implementation)

```
typescript/
├── package.json              # pnpm dependencies + build scripts
├── pnpm-lock.yaml           # Lock file
├── vite.config.ts           # Vite bundler config (IIFE output)
├── tsconfig.json            # TypeScript config
├── dist/
│   └── agent.js             # Bundled output (embedded in Rust)
└── agent/
    ├── mod.ts               # Entry point, exports to globalThis
    ├── agent.ts             # chatStream using @anthropic-ai/sdk
    ├── custom_tools.ts      # Tools that call Rust ops
    ├── types.ts             # TypeScript interfaces
    ├── system_prompt.ts     # System prompt builder
    └── fetch_polyfill.ts    # fetch() using op_fetch

src/deno/
├── mod.rs                   # Module exports
├── runtime.rs               # DenoAgent struct with JsRuntime
├── ops.rs                   # op_suggest_command, op_read_scrollback, op_fetch
├── types.rs                 # Rust structs matching TypeScript
└── fallback_agent.js        # Fallback when bundle unavailable
```

### Working Tests (as of 2026-01-15)

All 7 tests pass:
- `test_deno_agent_creation` - Runtime initializes correctly
- `test_deno_agent_is_loaded` - Module loads and sets `terminai.isLoaded`
- `test_deno_agent_version` - Can read version from JS
- `test_deno_agent_echo` - Sync Rust→JS→Rust roundtrip
- `test_deno_agent_add` - Numeric operations work
- `test_deno_agent_test_function` - Async function resolves
- `test_deno_agent_chat_stream` - Returns StreamMessage array

### Phase 3 Completion Notes (2026-01-15)

#### Thread Safety Solution

The `JsRuntime` from `deno_core` is `!Send` (cannot be sent between threads). Since `AIChatProcess` is used in `Arc<Mutex<...>>` for async access, we needed a thread-safe solution.

**Solution:** `DenoLlmClient` uses a channel-based architecture:
1. The actual `DenoAgent` runs in a dedicated thread with its own Tokio runtime
2. `DenoLlmClient` holds only a channel sender (which is `Send + Sync`)
3. Requests are sent via channel and processed by the agent thread
4. Responses stream back via unbounded channels

```rust
// DenoLlmClient is now Send + Sync
pub struct DenoLlmClient {
  request_tx: mpsc::UnboundedSender<DenoRequest>,  // Send + Sync
  provider: String,
  model: String,
}
```

#### Removed AG-UI Dependencies

The following modules were removed from `src/llm/mod.rs` exports:
- `client.rs` (AgUiClient)
- `subscriber.rs` (StreamingSubscriber)
- `tool_coordinator.rs` (ToolCoordinator)
- `forwarded_props.rs` (TerminAIForwardedProps)

These files still exist but are no longer compiled/exported. They should be deleted in Phase 4.

#### New `ToolCallId` Type

Created a local `ToolCallId` type in `tool_executor.rs` to replace `ag_ui_core::types::ids::ToolCallId`:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolCallId(String);
```

### Next Steps for Phase 4 (Cleanup)

1. **Delete Python directory** - `python/` is no longer needed
2. **Delete old LLM files** - Remove `client.rs`, `subscriber.rs`, `tool_coordinator.rs`, `forwarded_props.rs`
3. **Delete `llm_subprocess.rs`** - No longer used
4. **Remove AG-UI from Cargo.toml** - `ag-ui-client`, `ag-ui-core` dependencies
5. **Test end-to-end** - Verify the full flow works with real API key

### Potential Issues to Watch

1. **API Key passing** - Currently passed in `ChatOptions.apiKey`; ensure it's not logged
2. **Thread lifecycle** - Agent thread created on `DenoLlmClient::new()`, shutdown on `Drop`
3. **Error propagation** - JS errors surface via the text stream channel
4. **Memory management** - Long-running agent may accumulate V8 heap; consider runtime recycling

---

**Current Status:** Phase 1-3 complete. `DenoLlmClient` integrated into `chat_process.rs`, code compiles.
**Next Step:** Phase 4 - Delete Python code and AG-UI dependencies, test end-to-end
