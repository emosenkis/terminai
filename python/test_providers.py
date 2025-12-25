#!/usr/bin/env python3
"""Quick test of providers with real API/Ollama."""

import asyncio
import os
from terminai_llm import LLMClient, TerminalContext

async def test_ollama():
    """Test with Ollama (local, no API key needed)."""
    print("Testing Ollama provider (local)...")
    try:
        client = LLMClient(provider="ollama", model="functiongemma:latest")

        context = {
            "cwd": "/tmp",
            "history_lines": ["$ ls", "file.txt"],
            "last_exit_code": 0,
        }

        print("Sending test message to Ollama...")
        async for chunk in client.send_message_stream(
            "Say hello in one short sentence.",
            context,
            []
        ):
            print(chunk, end="", flush=True)

        print("\n✓ Ollama test passed!")
        return True

    except Exception as e:
        print(f"✗ Ollama test failed: {e}")
        return False

async def test_anthropic():
    """Test with Anthropic (uses API key, minimal usage)."""
    api_key = os.getenv("ANTHROPIC_API_KEY")
    if not api_key:
        print("⊘ Skipping Anthropic test - no API key")
        return None

    print("\nTesting Anthropic provider (with API key)...")
    try:
        client = LLMClient(provider="anthropic", model="claude-haiku-4-5")

        context = {
            "cwd": "/tmp",
            "history_lines": ["$ pwd", "/tmp"],
            "last_exit_code": 0,
        }

        print("Sending minimal test message to Anthropic...")
        response = ""
        async for chunk in client.send_message_stream(
            "Reply with just 'OK'",
            context,
            []
        ):
            response += chunk
            print(chunk, end="", flush=True)

        print(f"\n✓ Anthropic test passed! Response length: {len(response)} chars")
        return True

    except Exception as e:
        print(f"✗ Anthropic test failed: {e}")
        return False

async def main():
    print("=" * 60)
    print("Provider Testing")
    print("=" * 60)

    # Test Ollama first (free)
    ollama_ok = await test_ollama()

    # Test Anthropic with minimal usage
    anthropic_ok = await test_anthropic()

    print("\n" + "=" * 60)
    print("Results:")
    print(f"  Ollama:    {'✓ PASS' if ollama_ok else '✗ FAIL'}")
    print(f"  Anthropic: {'✓ PASS' if anthropic_ok else '✗ FAIL' if anthropic_ok is not None else '⊘ SKIP'}")
    print("=" * 60)

if __name__ == "__main__":
    asyncio.run(main())
