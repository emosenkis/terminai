"""Mock OpenAI-compatible LLM server for testing.

Provides a minimal implementation of the OpenAI chat completions API
that echoes back user messages. Used for end-to-end testing without
requiring a real LLM.

Special Commands:
- Prompts containing "[TOOL:tool_name]" will trigger a tool call with that name
- Prompts containing "[TOOL:suggest_command|ls -la]" will call suggest_command with that command
"""

import json
import re
import time
from typing import Any, AsyncIterator

from fastapi import FastAPI
from fastapi.responses import StreamingResponse
from pydantic import BaseModel


class ChatMessage(BaseModel):
    """A chat message."""

    role: str
    content: str


class ChatCompletionRequest(BaseModel):
    """Chat completion request matching OpenAI API."""

    model: str
    messages: list[ChatMessage]
    stream: bool = False
    temperature: float | None = None
    max_tokens: int | None = None
    tools: list[dict[str, Any]] | None = None


def create_mock_llm_app() -> FastAPI:
    """Create a mock LLM server that implements OpenAI chat completions API.

    Returns:
        FastAPI app that echoes user messages
    """
    app = FastAPI(title="Mock LLM Server")

    @app.get("/health")
    async def health():
        """Health check."""
        return {"status": "healthy"}

    @app.post("/v1/chat/completions")
    async def chat_completions(request: ChatCompletionRequest):
        """Mock chat completions endpoint.

        Echoes back the last user message with a prefix, or returns a tool call
        if the message contains a [TOOL:...] directive.
        """
        # Get the last user message
        user_messages = [msg for msg in request.messages if msg.role == "user"]
        if not user_messages:
            response_text = "Hello! I'm a mock LLM."
            tool_call_directive = None
        else:
            last_message = user_messages[-1].content

            # Check for tool call directive: [TOOL:tool_name] or [TOOL:tool_name|arg1|arg2]
            tool_match = re.search(r'\[TOOL:([^\]]+)\]', last_message)
            if tool_match:
                tool_call_directive = tool_match.group(1)
                # Remove the directive from the message for echoing
                clean_message = re.sub(r'\[TOOL:[^\]]+\]', '', last_message).strip()
                response_text = f"Echo: {clean_message}" if clean_message else "Calling tool..."
            else:
                tool_call_directive = None
                response_text = f"Echo: {last_message}"

        # If tool call directive is present, return a tool call
        if tool_call_directive:
            parts = tool_call_directive.split('|')
            tool_name = parts[0]

            # Build tool call arguments based on the directive
            if tool_name == "suggest_command" and len(parts) > 1:
                tool_args = {
                    "command": parts[1],
                    "explanation": parts[2] if len(parts) > 2 else f"Execute {parts[1]}"
                }
            elif tool_name == "read_scrollback":
                tool_args = {
                    "num_lines": int(parts[1]) if len(parts) > 1 else 100
                }
            else:
                # Generic tool call with no specific args
                tool_args = {}

            tool_call_id = f"call_mock_{int(time.time())}"

            if request.stream:
                # Return streaming response with tool call
                async def generate_tool_call_stream() -> AsyncIterator[str]:
                    """Generate OpenAI-compatible streaming chunks with tool call."""
                    chunk_id = f"chatcmpl-mock-{int(time.time())}"

                    # First chunk: start the tool call
                    chunk = {
                        "id": chunk_id,
                        "object": "chat.completion.chunk",
                        "created": int(time.time()),
                        "model": request.model,
                        "choices": [
                            {
                                "index": 0,
                                "delta": {
                                    "tool_calls": [
                                        {
                                            "index": 0,
                                            "id": tool_call_id,
                                            "type": "function",
                                            "function": {
                                                "name": tool_name,
                                                "arguments": ""
                                            }
                                        }
                                    ]
                                },
                                "finish_reason": None,
                            }
                        ],
                    }
                    yield f"data: {json.dumps(chunk)}\n\n"

                    # Stream the arguments in chunks
                    args_str = json.dumps(tool_args)
                    chunk_size = 10
                    for i in range(0, len(args_str), chunk_size):
                        arg_chunk = args_str[i:i+chunk_size]
                        chunk = {
                            "id": chunk_id,
                            "object": "chat.completion.chunk",
                            "created": int(time.time()),
                            "model": request.model,
                            "choices": [
                                {
                                    "index": 0,
                                    "delta": {
                                        "tool_calls": [
                                            {
                                                "index": 0,
                                                "function": {
                                                    "arguments": arg_chunk
                                                }
                                            }
                                        ]
                                    },
                                    "finish_reason": None,
                                }
                            ],
                        }
                        yield f"data: {json.dumps(chunk)}\n\n"

                    # Final chunk with finish_reason
                    final_chunk = {
                        "id": chunk_id,
                        "object": "chat.completion.chunk",
                        "created": int(time.time()),
                        "model": request.model,
                        "choices": [
                            {
                                "index": 0,
                                "delta": {},
                                "finish_reason": "tool_calls",
                            }
                        ],
                    }
                    yield f"data: {json.dumps(final_chunk)}\n\n"
                    yield "data: [DONE]\n\n"

                return StreamingResponse(
                    generate_tool_call_stream(), media_type="text/event-stream"
                )

            # Non-streaming tool call response
            return {
                "id": f"chatcmpl-mock-{int(time.time())}",
                "object": "chat.completion",
                "created": int(time.time()),
                "model": request.model,
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": None,
                            "tool_calls": [
                                {
                                    "id": tool_call_id,
                                    "type": "function",
                                    "function": {
                                        "name": tool_name,
                                        "arguments": json.dumps(tool_args)
                                    }
                                }
                            ]
                        },
                        "finish_reason": "tool_calls",
                    }
                ],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 5,
                    "total_tokens": 15,
                },
            }

        # Regular text response (no tool call)
        if request.stream:
            # Return streaming response with Server-Sent Events
            async def generate_stream() -> AsyncIterator[str]:
                """Generate OpenAI-compatible streaming chunks."""
                chunk_id = f"chatcmpl-mock-{int(time.time())}"

                # Split response into words for streaming
                words = response_text.split()

                for i, word in enumerate(words):
                    chunk = {
                        "id": chunk_id,
                        "object": "chat.completion.chunk",
                        "created": int(time.time()),
                        "model": request.model,
                        "choices": [
                            {
                                "index": 0,
                                "delta": {"content": word + " "},
                                "finish_reason": None,
                            }
                        ],
                    }
                    yield f"data: {json.dumps(chunk)}\n\n"

                # Send final chunk with finish_reason
                final_chunk = {
                    "id": chunk_id,
                    "object": "chat.completion.chunk",
                    "created": int(time.time()),
                    "model": request.model,
                    "choices": [
                        {
                            "index": 0,
                            "delta": {},
                            "finish_reason": "stop",
                        }
                    ],
                }
                yield f"data: {json.dumps(final_chunk)}\n\n"
                yield "data: [DONE]\n\n"

            return StreamingResponse(
                generate_stream(), media_type="text/event-stream"
            )

        # Return non-streaming response
        return {
            "id": f"chatcmpl-mock-{int(time.time())}",
            "object": "chat.completion",
            "created": int(time.time()),
            "model": request.model,
            "choices": [
                {
                    "index": 0,
                    "message": {"role": "assistant", "content": response_text},
                    "finish_reason": "stop",
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15,
            },
        }

    return app


if __name__ == "__main__":
    import uvicorn

    app = create_mock_llm_app()
    uvicorn.run(app, host="127.0.0.1", port=11434)
