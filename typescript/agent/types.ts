/**
 * Type definitions for Termin.AI agent
 * These types should match the Rust types in src/deno/types.rs
 */

/**
 * Terminal context information passed from Rust
 */
export interface TerminalContext {
  /** Recent terminal history lines */
  historyLines?: string[];
  /** Current working directory */
  cwd: string;
  /** Exit code of last command */
  lastExitCode?: number;
  /** Operating system info (e.g., "Linux", "macOS") */
  osInfo?: string;
  /** Shell being used (e.g., "bash", "zsh", "fish") */
  shell?: string;
}

/**
 * Options for initiating a chat stream
 */
export interface ChatOptions {
  /** User message to send to the LLM */
  message: string;
  /** Model identifier (e.g., "claude-sonnet-4-5") */
  model: string;
  /** Provider name (e.g., "anthropic", "openai", "ollama") */
  provider: string;
  /** API key for the provider (if required) */
  apiKey?: string;
  /** Custom system prompt (optional, will be built automatically if not provided) */
  systemPrompt?: string;
  /** Terminal context for building system prompt */
  terminalContext?: TerminalContext;
  /** Maximum number of turns in the conversation */
  maxTurns?: number;
  /** Maximum budget in USD */
  maxBudgetUsd?: number;
}

/**
 * Arguments for suggest_command tool
 */
export interface SuggestCommandArgs {
  /** The command to suggest */
  command: string;
  /** Optional explanation of what the command does */
  explanation?: string;
}

/**
 * Arguments for read_scrollback tool
 */
export interface ReadScrollbackArgs {
  /** Number of lines to read from scrollback */
  numLines?: number;
}

/**
 * Result from a tool execution
 */
export interface ToolResult {
  /** Result content as text */
  content: string;
  /** Whether the tool execution resulted in an error */
  isError?: boolean;
}

/**
 * Token usage statistics
 */
export interface TokenUsage {
  inputTokens: number;
  outputTokens: number;
  cacheReadInputTokens?: number;
  cacheCreationInputTokens?: number;
}

/**
 * Simplified message type for streaming to Rust
 * This is a subset of SDK messages that Rust needs to handle
 */
export type StreamMessage =
  | { type: "text"; content: string }
  | { type: "tool_call"; toolName: string; toolInput: unknown }
  | {
      type: "result";
      isError: boolean;
      result?: string;
      errors?: string[];
      usage?: TokenUsage;
      totalCostUsd?: number;
      durationMs?: number;
    };

/**
 * Fetch options for op_fetch
 */
export interface FetchOptions {
  method: string;
  headers: Record<string, string>;
  body?: string;
}

/**
 * Fetch response from op_fetch
 */
export interface FetchResponse {
  status: number;
  body: string;
}

/**
 * Deno core ops interface (injected by Rust)
 */
declare global {
  // deno-lint-ignore no-var
  var Deno: {
    core: {
      ops: {
        op_suggest_command(args: SuggestCommandArgs): Promise<string>;
        op_read_scrollback(args: ReadScrollbackArgs): Promise<string>;
        op_fetch(url: string, options: FetchOptions): Promise<FetchResponse>;
      };
      print(msg: string): void;
    };
  };
}
