/**
 * Main agent implementation using official Anthropic SDK
 *
 * This agent uses the official @anthropic-ai/sdk package to communicate
 * with Claude. The SDK is bundled by vite at build time.
 */

import Anthropic from "@anthropic-ai/sdk";
import type {
  ChatOptions,
  StreamMessage,
  TokenUsage,
} from "./types.ts";
import { buildSystemPrompt } from "./system_prompt.ts";
import { getToolDefinitions, executeTool } from "./custom_tools.ts";

const DEFAULT_MAX_TOKENS = 4096;
const MAX_TOOL_LOOPS = 10;

/**
 * Main chat stream function called from Rust
 *
 * This collects all messages and returns them as an array.
 *
 * @param options - Chat configuration including message, model, context, etc.
 * @returns Array of StreamMessage for Rust to consume
 */
export async function chatStream(options: ChatOptions): Promise<StreamMessage[]> {
  const messages: StreamMessage[] = [];
  const startTime = Date.now();
  let totalUsage: TokenUsage = {
    inputTokens: 0,
    outputTokens: 0,
  };

  try {
    // Validate API key
    const apiKey = options.apiKey;
    if (!apiKey) {
      throw new Error("API key is required for Anthropic provider");
    }

    // Create Anthropic client
    const client = new Anthropic({
      apiKey,
    });

    // Build system prompt with terminal context
    const systemPrompt =
      options.systemPrompt ?? buildSystemPrompt(options.terminalContext);

    // Initialize conversation with user message
    const conversationMessages: Anthropic.MessageParam[] = [
      {
        role: "user",
        content: options.message,
      },
    ];

    // Get tool definitions
    const tools = getToolDefinitions();

    // Main conversation loop (handles tool use)
    let loopCount = 0;
    while (loopCount < MAX_TOOL_LOOPS) {
      loopCount++;

      // Call Anthropic API
      const response = await client.messages.create({
        model: options.model,
        max_tokens: DEFAULT_MAX_TOKENS,
        system: systemPrompt,
        messages: conversationMessages,
        tools,
      });

      // Accumulate usage
      totalUsage.inputTokens += response.usage.input_tokens;
      totalUsage.outputTokens += response.usage.output_tokens;

      // Process response content
      let hasToolUse = false;
      const toolUseBlocks: Anthropic.ToolUseBlock[] = [];

      for (const block of response.content) {
        if (block.type === "text") {
          // Emit text content
          if (block.text) {
            messages.push({
              type: "text",
              content: block.text,
            });
          }
        } else if (block.type === "tool_use") {
          hasToolUse = true;
          toolUseBlocks.push(block);

          // Emit tool call message
          messages.push({
            type: "tool_call",
            toolName: block.name,
            toolInput: block.input,
          });
        }
      }

      // If there are tool uses, execute them and continue
      if (hasToolUse) {
        // Add assistant message to conversation
        conversationMessages.push({
          role: "assistant",
          content: response.content,
        });

        // Execute tools and collect results
        const toolResults: Anthropic.ToolResultBlockParam[] = [];
        for (const toolUse of toolUseBlocks) {
          const result = await executeTool(toolUse.name, toolUse.input);
          toolResults.push({
            type: "tool_result",
            tool_use_id: toolUse.id,
            content: result.content,
            is_error: result.isError,
          });
        }

        // Add tool results to conversation
        conversationMessages.push({
          role: "user",
          content: toolResults,
        });

        // Continue loop to get next response
        continue;
      }

      // No tool use, conversation complete
      break;
    }

    // Emit final result message
    const durationMs = Date.now() - startTime;
    messages.push({
      type: "result",
      isError: false,
      result: "Chat completed successfully",
      usage: totalUsage,
      durationMs,
    });
  } catch (error) {
    // Emit error result
    const durationMs = Date.now() - startTime;
    messages.push({
      type: "result",
      isError: true,
      errors: [(error as Error).message],
      usage: totalUsage,
      durationMs,
    });
  }

  return messages;
}

/**
 * Simple test function to verify the agent works
 * Can be called from Rust tests
 */
export async function testAgent(): Promise<string> {
  // This is a simple test that doesn't actually call the API
  // It just verifies that the JS environment is working
  return "Agent module loaded successfully with Anthropic SDK. Tools available: suggest_command, read_scrollback";
}

/**
 * Echo function for testing Rust<->JS interop
 */
export function echo(message: string): string {
  return `Echo: ${message}`;
}

/**
 * Add function for testing numeric operations
 */
export function add(a: number, b: number): number {
  return a + b;
}
