"""FastAPI server for AG-UI protocol."""

import json
import logging
import socket
from typing import AsyncIterator

import uvicorn
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse, StreamingResponse
from pydantic import BaseModel

from .agent import TerminAIAgent
from .config import ProviderConfig

logger = logging.getLogger(__name__)

# Global shared secret for authentication
_EXPECTED_SECRET: str | None = None


# Request/Response models
class TerminalContext(BaseModel):
    """Terminal context for the agent."""

    history_lines: list[str]
    cwd: str
    last_exit_code: int | None = None


class ChatRequest(BaseModel):
    """Request to start a chat conversation."""

    message: str
    context: TerminalContext | None = None


class ChatResponse(BaseModel):
    """Response from a chat request."""

    response: str


def find_available_port(host: str, port_range: tuple[int, int]) -> int:
    """Find an available port in the given range.

    Args:
        host: Host address to bind to
        port_range: Tuple of (start_port, end_port) inclusive

    Returns:
        An available port number

    Raises:
        RuntimeError: If no ports are available in the range
    """
    start_port, end_port = port_range

    for port in range(start_port, end_port + 1):
        try:
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
                sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                sock.bind((host, port))
                return port
        except OSError:
            continue

    # Fallback: Let OS assign a port
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        sock.bind((host, 0))
        return sock.getsockname()[1]


def create_app() -> FastAPI:
    """Create and configure the FastAPI application."""
    app = FastAPI(
        title="Termin.AI Agent",
        description="AI assistant subprocess for Termin.AI",
        version="0.1.0",
    )

    @app.middleware("http")
    async def verify_secret(request: Request, call_next):
        """Verify the shared secret in request headers."""
        # Skip auth for health check
        if request.url.path == "/health":
            return await call_next(request)

        secret = request.headers.get("x-ag-ui-secret")
        if secret != _EXPECTED_SECRET:
            logger.warning(f"Invalid secret attempt from {request.client.host}")
            return JSONResponse(
                status_code=401,
                content={"error": "Invalid or missing secret"},
            )

        return await call_next(request)

    @app.get("/health")
    async def health_check():
        """Health check endpoint."""
        return {"status": "healthy", "service": "terminai-agent"}

    @app.get("/")
    async def root():
        """Root endpoint."""
        return {
            "service": "terminai-agent",
            "version": "0.1.0",
            "protocol": "ag-ui",
        }

    @app.post("/chat", response_model=ChatResponse)
    async def chat(request: ChatRequest) -> ChatResponse:
        """Non-streaming chat endpoint."""
        logger.info(f"Received chat request: {request.message[:50]}...")

        # Load provider config from environment
        provider_config = ProviderConfig.from_env()
        agent = TerminAIAgent(provider_config)

        # Build context for the agent
        context = None
        if request.context:
            from .agent import TerminalContext as AgentTerminalContext

            context = AgentTerminalContext(
                history_lines=request.context.history_lines,
                cwd=request.context.cwd,
                last_exit_code=request.context.last_exit_code,
            )

        # Get response from agent
        response_text = await agent.chat(request.message, context)

        return ChatResponse(response=response_text)

    @app.post("/chat/stream")
    async def chat_stream(request: ChatRequest) -> StreamingResponse:
        """Streaming chat endpoint using Server-Sent Events."""
        logger.info(f"Received streaming chat request: {request.message[:50]}...")

        async def event_generator() -> AsyncIterator[str]:
            """Generate SSE events from agent stream."""
            # Load provider config from environment
            provider_config = ProviderConfig.from_env()
            agent = TerminAIAgent(provider_config)

            # Build context for the agent
            context = None
            if request.context:
                from .agent import TerminalContext as AgentTerminalContext

                context = AgentTerminalContext(
                    history_lines=request.context.history_lines,
                    cwd=request.context.cwd,
                    last_exit_code=request.context.last_exit_code,
                )

            try:
                # Stream response chunks
                async for chunk in agent.chat_stream(request.message, context):
                    # Format as SSE: "data: {...}\n\n"
                    event = {"type": "text_chunk", "content": chunk}
                    yield f"data: {json.dumps(event)}\n\n"

                # Send completion event
                done_event = {"type": "done"}
                yield f"data: {json.dumps(done_event)}\n\n"

            except Exception as e:
                logger.error(f"Error in chat stream: {e}", exc_info=True)
                # Send error event
                error_event = {"type": "error", "message": str(e)}
                yield f"data: {json.dumps(error_event)}\n\n"

        return StreamingResponse(
            event_generator(),
            media_type="text/event-stream",
            headers={
                "Cache-Control": "no-cache",
                "Connection": "keep-alive",
            },
        )

    return app


async def run_server(
    secret: str,
    host: str = "127.0.0.1",
    port_range: tuple[int, int] = (18080, 18099),
) -> None:
    """Run the FastAPI server.

    Args:
        secret: Shared secret for authentication
        host: Host address to bind to
        port_range: Range of ports to try
    """
    global _EXPECTED_SECRET
    _EXPECTED_SECRET = secret

    # Find available port
    port = find_available_port(host, port_range)
    logger.info(f"Selected port: {port}")

    # Communicate port to parent process via stdout
    print(f"AG_UI_PORT={port}", flush=True)

    # Create app
    app = create_app()

    # Configure uvicorn
    config = uvicorn.Config(
        app,
        host=host,
        port=port,
        log_level="info",
        access_log=False,  # Reduce noise, we have our own logging
    )

    server = uvicorn.Server(config)

    logger.info(f"Starting server on {host}:{port}")
    await server.serve()
