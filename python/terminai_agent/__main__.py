"""Entry point for terminai-agent subprocess."""

import argparse
import asyncio
import logging
import sys
from pathlib import Path

from terminai_agent.server import run_server


def setup_logging() -> None:
    """Configure logging to stderr."""
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
        stream=sys.stderr,
    )


def main() -> None:
    """Main entry point for the agent subprocess."""
    parser = argparse.ArgumentParser(description="Termin.AI Agent Subprocess")
    parser.add_argument(
        "--secret",
        required=True,
        help="Shared secret for authentication",
    )
    parser.add_argument(
        "--port-range-start",
        type=int,
        default=18080,
        help="Start of port range to try",
    )
    parser.add_argument(
        "--port-range-end",
        type=int,
        default=18099,
        help="End of port range to try",
    )
    parser.add_argument(
        "--host",
        default="127.0.0.1",
        help="Host to bind to (default: 127.0.0.1)",
    )

    args = parser.parse_args()

    setup_logging()
    logger = logging.getLogger(__name__)

    logger.info("Starting Termin.AI agent subprocess")
    logger.info(f"Port range: {args.port_range_start}-{args.port_range_end}")

    try:
        asyncio.run(
            run_server(
                secret=args.secret,
                host=args.host,
                port_range=(args.port_range_start, args.port_range_end),
            )
        )
    except KeyboardInterrupt:
        logger.info("Received interrupt signal, shutting down")
        sys.exit(0)
    except Exception as e:
        logger.exception(f"Fatal error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
