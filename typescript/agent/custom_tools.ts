/**
 * Custom tools for Termin.AI that call back into Rust
 */

import type Anthropic from "@anthropic-ai/sdk";
import type {
  ReadScrollbackArgs,
  SuggestCommandArgs,
} from "./types.ts";

/**
 * Tool: suggest_command
 * Suggests a shell command to execute in the terminal
 */
export async function suggestCommand(args: SuggestCommandArgs): Promise<string> {
  try {
    // Call into Rust to handle command suggestion
    const result = await globalThis.Deno.core.ops.op_suggest_command(args);
    return result;
  } catch (error) {
    throw new Error(`Error suggesting command: ${(error as Error).message}`);
  }
}

/**
 * Tool: read_scrollback
 * Reads the terminal scrollback history
 */
export async function readScrollback(args: ReadScrollbackArgs): Promise<string> {
  try {
    // Call into Rust to read scrollback
    const result = await globalThis.Deno.core.ops.op_read_scrollback(args);
    return result;
  } catch (error) {
    throw new Error(`Error reading scrollback: ${(error as Error).message}`);
  }
}

/**
 * Get tool definitions for Anthropic API
 */
export function getToolDefinitions(): Anthropic.Tool[] {
  return [
    {
      name: "suggest_command",
      description:
        "Suggest a shell command to execute in the terminal. The command will be shown to the user for approval before execution.",
      input_schema: {
        type: "object" as const,
        properties: {
          command: {
            type: "string",
            description: "The shell command to suggest (e.g., 'ls -la', 'git status')",
          },
          explanation: {
            type: "string",
            description: "Optional explanation of what the command does and why it's being suggested",
          },
        },
        required: ["command"],
      },
    },
    {
      name: "read_scrollback",
      description:
        "Read the terminal scrollback history to see recent command output and terminal activity.",
      input_schema: {
        type: "object" as const,
        properties: {
          numLines: {
            type: "number",
            description: "Number of lines to read from the scrollback buffer (default: 100, max: 1000)",
          },
        },
      },
    },
  ];
}

/**
 * Execute a tool by name with given input
 */
export async function executeTool(
  toolName: string,
  input: unknown,
): Promise<{ content: string; isError: boolean }> {
  try {
    switch (toolName) {
      case "suggest_command": {
        const args = input as SuggestCommandArgs;
        const result = await suggestCommand(args);
        return { content: result, isError: false };
      }
      case "read_scrollback": {
        const args = input as ReadScrollbackArgs;
        const result = await readScrollback(args);
        return { content: result, isError: false };
      }
      default:
        return {
          content: `Unknown tool: ${toolName}`,
          isError: true,
        };
    }
  } catch (error) {
    return {
      content: (error as Error).message,
      isError: true,
    };
  }
}
