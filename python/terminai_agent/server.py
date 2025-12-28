"""FastAPI server for AG-UI protocol."""

import json
import logging
import socket
from http import HTTPStatus

import uvicorn
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse, Response
from pydantic import ValidationError
from pydantic_ai.ui import SSE_CONTENT_TYPE
from pydantic_ai.ui.ag_ui import AGUIAdapter

from .agent import TerminAIAgent
from .config import ProviderConfig
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
    async def run_agent(request: Request) -> Response:
        """AG-UI protocol endpoint for running the agent."""
        accept = request.headers.get("accept", SSE_CONTENT_TYPE)

        try:
            # Parse AG-UI RunAgentInput from request body
            run_input = AGUIAdapter.build_run_input(await request.body())
        except ValidationError as e:
            logger.error(f"Invalid AG-UI input: {e}")
            return Response(
                content=json.dumps(e.errors()),
                media_type="application/json",
                status_code=HTTPStatus.UNPROCESSABLE_ENTITY,
            )
        except Exception as e:
            logger.error(f"Error parsing request: {e}", exc_info=True)
            return Response(
                content=json.dumps({"error": str(e)}),
                media_type="application/json",
                status_code=HTTPStatus.BAD_REQUEST,
            )

        # Extract forwarded props for provider/model configuration
        try:
            forwarded_props = TerminAIForwardedProps(**run_input.forwarded_props)
            logger.info(
                f"Using provider: {forwarded_props.provider}, model: {forwarded_props.model}"
            )

            # Create provider config from forwarded props
            import os
            from .config import Provider

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
                Provider.OLLAMA: os.getenv("OLLAMA_BASE_URL", "http://localhost:11434"),
                Provider.OPENROUTER: "https://openrouter.ai/api/v1",
            }

            provider_config = ProviderConfig(
                provider=provider,
                model=forwarded_props.model,
                api_key_env=api_key_envs[provider],
                endpoint=endpoints.get(provider),
            )
        except Exception as e:
            logger.error(f"Invalid forwarded props: {e}", exc_info=True)
            # Fall back to environment configuration
            provider_config = ProviderConfig.from_env()
            logger.info(
                f"Falling back to env config: {provider_config.provider.value}/{provider_config.model}"
            )

        # Create the agent with the provider config
        agent = TerminAIAgent(provider_config)

        # Create AG-UI adapter
        adapter = AGUIAdapter(agent=agent.agent, run_input=run_input, accept=accept)

        # Run the agent and get event stream
        event_stream = adapter.run_stream()

        # Encode and return the response
        sse_event_stream = adapter.encode_stream(event_stream)
        return adapter.streaming_response(sse_event_stream)

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
