"""FastAPI server for AG-UI protocol with Claude Agent SDK."""

import json
import logging
import os
import socket
from http import HTTPStatus

import uvicorn
from ag_ui.core import EventType, RunAgentInput, RunErrorEvent
from ag_ui.encoder import EventEncoder
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse, StreamingResponse

from .agent import TerminalContext, create_agent_adapter
from .config import Provider, ProviderConfig
from .forwarded_props import TerminAIForwardedProps

logger = logging.getLogger(__name__)

# Global shared secret for authentication
_EXPECTED_SECRET: str | None = None


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

    @app.post("/")
    async def run_agent(request: Request) -> StreamingResponse:
        """AG-UI protocol endpoint for running the agent."""
        accept = request.headers.get("accept", "text/event-stream")

        try:
            # Parse AG-UI RunAgentInput from request body
            body = await request.body()
            run_input = RunAgentInput.model_validate_json(body)
        except Exception as e:
            logger.error(f"Error parsing request: {e}", exc_info=True)
            return StreamingResponse(
                iter([json.dumps({"error": str(e)})]),
                media_type="application/json",
                status_code=HTTPStatus.BAD_REQUEST,
            )

        # Extract forwarded props for provider/model configuration AND terminal context
        try:
            forwarded_props = TerminAIForwardedProps(**run_input.forwarded_props)
            logger.info(
                f"Using provider: {forwarded_props.provider}, model: {forwarded_props.model}"
            )

            # Create provider config from forwarded props
            provider = Provider(forwarded_props.provider.lower())

            # API key environment variables
            api_key_envs = {
                Provider.ANTHROPIC: "ANTHROPIC_API_KEY",
                Provider.OPENAI: "OPENAI_API_KEY",
                Provider.GEMINI: "GOOGLE_API_KEY",
                Provider.OLLAMA: None,  # Local server, no API key
                Provider.OPENROUTER: "OPENROUTER_API_KEY",
            }

            # Custom endpoints
            endpoints = {
                Provider.OLLAMA: os.getenv("OLLAMA_BASE_URL", "http://localhost:11434/v1"),
                Provider.OPENROUTER: "https://openrouter.ai/api/v1",
            }

            provider_config = ProviderConfig(
                provider=provider,
                model=forwarded_props.model,
                api_key_env=api_key_envs[provider],
                endpoint=endpoints.get(provider),
            )

            # Extract terminal context from forwarded props (if available)
            terminal_context = None
            if forwarded_props.cwd is not None:
                terminal_context = TerminalContext(
                    cwd=forwarded_props.cwd,
                    history_lines=forwarded_props.history_lines or [],
                    last_exit_code=forwarded_props.last_exit_code,
                    os_info=forwarded_props.os_info,
                    shell=forwarded_props.shell,
                )
                logger.info(f"Terminal context: cwd={terminal_context.cwd}")
            else:
                logger.warning("No terminal context in forwarded_props!")

        except Exception as e:
            logger.exception(f"Error parsing forwarded props: {e}")
            # Fall back to environment configuration
            provider_config = ProviderConfig.from_env()
            terminal_context = None
            logger.info(
                f"Falling back to env config: {provider_config.provider.value}/{provider_config.model}"
            )

        # Create the Claude SDK adapter with the provider config and terminal context
        adapter = create_agent_adapter(provider_config, terminal_context)

        logger.info(f"Created Claude SDK adapter with model: {provider_config.model}")

        # Create event encoder for AG-UI protocol
        encoder = EventEncoder(accept=accept)

        # Create streaming response
        async def event_stream():
            """Stream AG-UI events from Claude SDK adapter."""
            try:
                async for event in adapter.run(run_input):
                    yield encoder.encode(event)
            except Exception as e:
                logger.exception(f"Error during agent run: {e}")
                # Emit error event
                error_event = RunErrorEvent(
                    type=EventType.RUN_ERROR,
                    thread_id=run_input.thread_id or "unknown",
                    run_id=run_input.run_id or "unknown",
                    message=str(e),
                )
                yield encoder.encode(error_event)

        return StreamingResponse(
            event_stream(),
            media_type=encoder.get_content_type(),
            headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
        )

    return app


async def run_server(
    secret: str,
    host: str = "127.0.0.1",
    port_range: tuple[int, int] = (18080, 18199),
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
