/**
 * Fallback Agent for Termin.AI
 *
 * This is used when the bundled TypeScript agent is not available.
 * It provides basic functionality for testing the Rust<->JS interop.
 */

// Initialize the terminai namespace
globalThis.terminai = {
  version: "0.0.0-fallback",
  isLoaded: true,
};

/**
 * Chat stream function - returns an array of messages
 * In fallback mode, this just returns an error explaining the situation.
 */
globalThis.chatStream = async function (options) {
  return [
    {
      type: "text",
      content:
        "Fallback agent: The TypeScript bundle is not available.\n\n" +
        "To build the TypeScript agent:\n" +
        "1. cd typescript\n" +
        "2. pnpm install\n" +
        "3. pnpm build\n" +
        "4. Rebuild the Rust project",
    },
    {
      type: "result",
      isError: true,
      errors: ["Fallback agent cannot process real LLM requests"],
      durationMs: 0,
    },
  ];
};

/**
 * Test agent function - verifies the runtime is working
 */
globalThis.testAgent = async function () {
  return "Fallback agent loaded successfully (TypeScript bundle not available)";
};

/**
 * Echo function - for testing basic interop
 */
globalThis.echo = function (message) {
  return "Echo: " + message;
};

/**
 * Add function - for testing numeric operations
 */
globalThis.add = function (a, b) {
  return a + b;
};

/**
 * Build system prompt - fallback version
 */
globalThis.buildSystemPrompt = function (context) {
  return "You are a helpful terminal assistant. (Fallback mode)";
};

// Log that fallback agent is loaded
try {
  if (
    typeof globalThis.Deno !== "undefined" &&
    globalThis.Deno.core &&
    typeof globalThis.Deno.core.print === "function"
  ) {
    globalThis.Deno.core.print(
      "[terminai-agent] Fallback agent initialized\n"
    );
  }
} catch (e) {
  // Ignore if print isn't available
}
