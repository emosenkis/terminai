/**
 * Termin.AI TypeScript Agent Module
 *
 * This module provides the main entry point for the Termin.AI agent.
 * All exported functions are exposed to the Rust runtime via globalThis.
 *
 * This code is bundled by vite and executed in an embedded deno_core runtime.
 */

// Import fetch polyfill FIRST - this provides fetch API using our Rust ops
import "./fetch_polyfill.ts";

// Now import the Anthropic SDK shim (which now has fetch available)
import "@anthropic-ai/sdk/shims/web";

import { chatStream, testAgent, echo, add } from "./agent.ts";
import { suggestCommand, readScrollback, executeTool, getToolDefinitions } from "./custom_tools.ts";
import { buildSystemPrompt } from "./system_prompt.ts";
import type {
  ChatOptions,
  ReadScrollbackArgs,
  StreamMessage,
  SuggestCommandArgs,
  TerminalContext,
  ToolResult,
} from "./types.ts";

// Re-export types for documentation
export type {
  ChatOptions,
  ReadScrollbackArgs,
  StreamMessage,
  SuggestCommandArgs,
  TerminalContext,
  ToolResult,
};

// Re-export functions
export {
  chatStream,
  testAgent,
  echo,
  add,
  suggestCommand,
  readScrollback,
  executeTool,
  getToolDefinitions,
  buildSystemPrompt,
};

/**
 * Initialize the module and expose all functions to globalThis
 * This is called automatically when the module is loaded.
 */
function initializeAgent(): void {
  // Store a reference to our module on globalThis so Rust can access it
  const terminai = {
    // Main agent functions
    chatStream,
    testAgent,
    
    // Test/interop functions
    echo,
    add,
    
    // Tool functions (can be called directly for testing)
    suggestCommand,
    readScrollback,
    executeTool,
    
    // Utility functions
    buildSystemPrompt,
    getToolDefinitions,
    
    // Version info
    version: "1.0.0",
    
    // Check if module is loaded
    isLoaded: true,
  };

  // Expose on globalThis
  (globalThis as unknown as { terminai: typeof terminai }).terminai = terminai;
  
  // Also expose key functions directly for convenience
  (globalThis as unknown as { chatStream: typeof chatStream }).chatStream = chatStream;
  (globalThis as unknown as { testAgent: typeof testAgent }).testAgent = testAgent;
  (globalThis as unknown as { echo: typeof echo }).echo = echo;
  (globalThis as unknown as { add: typeof add }).add = add;
}

// Initialize on module load
initializeAgent();

// Log that module is ready
try {
  if (typeof globalThis.Deno?.core?.print === "function") {
    globalThis.Deno.core.print("[terminai-agent] Module initialized with Anthropic SDK\n");
  }
} catch {
  // Ignore if print isn't available
}
