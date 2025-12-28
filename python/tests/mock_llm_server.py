"""Mock OpenAI-compatible LLM server for testing.

Provides a minimal implementation of the OpenAI chat completions API
that echoes back user messages. Used for end-to-end testing without
requiring a real LLM.
"""

import json
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

        Echoes back the last user message with a prefix.
        """
        # Get the last user message
        user_messages = [msg for msg in request.messages if msg.role == "user"]
        if not user_messages:
            response_text = "Hello! I'm a mock LLM."
        else:
            last_message = user_messages[-1].content
            response_text = f"Echo: {last_message}"

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
