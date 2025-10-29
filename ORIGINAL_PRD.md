# Product Requirements Document: Termin.AI
## Interactive Terminal LLM Wrapper

**Version:** 1.0
**Date:** 2025-10-23
**Status:** Planning

---

## Executive Summary

Termin.AI is a transparent shell wrapper that enables context-aware LLM assistance directly within terminal sessions. Users can seamlessly interact with their shell until they need AI help, at which point a key combination opens an overlay chat interface that has full visibility into terminal history and can execute commands with optional user approval.

---

## Problem Statement

Current terminal-based LLM integrations require:
- Switching contexts between terminal and separate AI chat applications
- Manually copying terminal output to provide context to LLMs
- Copy-pasting commands back from AI responses
- Loss of terminal state and history when seeking assistance

These friction points slow down developer workflows and reduce the effectiveness of AI assistance.

---

## Goals and Objectives

### Primary Goals
1. **Transparent Operation**: Function as a normal shell until AI assistance is explicitly invoked
2. **Context Awareness**: Automatically provide terminal history to the LLM without manual copying
3. **Seamless Command Execution**: Allow LLM to suggest and execute commands directly in the terminal
4. **Safety First**: Implement approval workflows to prevent unintended command execution
5. **Cross-Shell Support**: Work with bash, zsh, fish, and other common shells

### Success Metrics
- Zero perceivable latency during normal terminal operation
- <500ms response time for AI overlay activation
- 100% command approval rate for destructive operations
- Support for 95% of common shell configurations

---

## User Personas

### Primary: Software Developer/DevOps Engineer
- Works primarily in terminal environments
- Frequently needs to debug commands, understand errors, or find the right command syntax
- Values speed and efficiency in their workflow
- May work on remote systems via SSH

### Secondary: System Administrator
- Manages multiple systems and services
- Needs to quickly resolve issues and execute complex command sequences
- Requires careful review before executing system-modifying commands
- Values command history for auditing

---

## Core Features

### 1. Transparent Shell Wrapper
**Priority:** P0 (Critical)

**Description:**
The tool wraps the user's chosen shell (bash/zsh/fish/etc.) and operates as a pseudo-terminal (PTY) that passes through all I/O transparently.

**Requirements:**
- Must support bash, zsh, sh at minimum
- No perceivable latency during normal operation
- Preserve all shell features (tab completion, history, colors, etc.)
- Handle terminal resize events correctly
- Support unicode and multi-byte characters
- Maintain signal handling (Ctrl-C, Ctrl-Z, etc.)

**Acceptance Criteria:**
- User cannot distinguish wrapped shell from native shell during normal operation
- All standard terminal features work identically
- Terminal-based applications (vim, htop, etc.) function correctly

---

### 2. AI Overlay Activation
**Priority:** P0 (Critical)

**Description:**
A configurable key combination (default: Ctrl-Space) triggers an overlay chat interface over the terminal.

**Requirements:**
- Configurable activation key binding
- Must work even when applications are running
- Overlay should not interfere with terminal state
- Graceful handling if shell is busy (command running)
- Visual indicator when overlay is active

**Acceptance Criteria:**
- Overlay appears within 500ms of key press
- Terminal content remains visible beneath overlay
- Multiple activations toggle overlay visibility
- Works during both idle and active command execution

---

### 3. Context-Aware Chat Interface
**Priority:** P0 (Critical)

**Description:**
An overlay chat window that displays over the terminal, showing conversation with the LLM.

**Requirements:**
- Clean, readable UI with clear separation between user and AI messages
- Scroll through conversation history
- Markdown rendering for code blocks
- Syntax highlighting for code in responses
- Display terminal context that will be sent to LLM
- Show tokens/cost estimation
- Input box with multi-line support
- Keyboard shortcuts for common actions

**UI Layout:**
```
┌─────────────────────────────────────────────────────────────┐
│  Termin.AI Chat                                   [x] Close │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  [User] How do I find large files?                          │
│                                                               │
│  [AI] You can use the find command:                          │
│  ```bash                                                      │
│  find / -type f -size +100M -exec ls -lh {} \;             │
│  ```                                                          │
│  Would you like me to run this? [Yes] [No] [Modify]        │
│                                                               │
│  Context: Last 50 lines of terminal history included         │
│                                                               │
├─────────────────────────────────────────────────────────────┤
│  Your message: _                                             │
│  Ctrl-Enter to send | Esc to close | Ctrl-E to execute     │
└─────────────────────────────────────────────────────────────┘
```

**Acceptance Criteria:**
- Overlay covers 60-80% of terminal by default (configurable)
- Maintains conversation history within session
- Code blocks are clearly distinguished and syntax-highlighted
- Responsive keyboard navigation

---

### 4. Terminal History Context
**Priority:** P0 (Critical)

**Description:**
Automatically capture and provide relevant terminal history to the LLM.

**Requirements:**
- Buffer terminal I/O (both input commands and output)
- Configurable history size (default: last 500 lines or 50KB)
- Privacy filtering to exclude sensitive data (passwords, API keys, etc.)
- Option to manually select specific context region
- Show user what context will be sent before API call
- Efficient storage to minimize memory overhead

**Context Format:**
```
Terminal Context (last 50 lines):
$ ls -la
total 128
drwxr-xr-x  5 user user  4096 Oct 23 10:30 .
drwxr-xr-x 12 user user  4096 Oct 20 15:22 ..
-rw-r--r--  1 user user  1234 Oct 23 10:30 error.log
...

Current directory: /home/user/project
Shell: zsh
Last exit code: 1
```

**Acceptance Criteria:**
- History buffer uses <10MB RAM for typical usage
- Sensitive patterns (AWS keys, passwords) automatically redacted
- User can see exactly what context is shared
- Context includes working directory, last exit code, environment variables (opt-in)

---

### 5. LLM Command Execution
**Priority:** P0 (Critical)

**Description:**
LLM can suggest commands and execute them in the terminal with user approval.

**Requirements:**
- Parse command suggestions from LLM responses
- Present commands with clear approval UI
- Three approval modes:
  - **Auto-approve safe commands** (ls, cat, grep, etc.)
  - **Always prompt** (destructive commands like rm, dd, mkfs)
  - **Manual approval** (user decides per-command)
- Execute approved commands in the actual shell session
- Stream command output back to chat interface
- Handle interactive commands (should warn/block)
- Allow editing commands before execution

**Safety Categories:**
- **Safe (Auto-approve):** ls, pwd, echo, cat, grep, find (read-only)
- **Caution (Prompt):** cp, mv, mkdir, touch (modifications)
- **Dangerous (Always confirm):** rm, dd, chmod, chown, sudo, curl|bash

**Acceptance Criteria:**
- Dangerous commands always require explicit approval
- User can modify command before execution
- Command output appears both in terminal and chat
- Failed commands show error codes and messages
- User can interrupt running commands

---

### 6. Multi-Provider LLM Support
**Priority:** P0 (Critical)

**Description:**
Support multiple LLM providers with easy configuration.

**Requirements:**
- Support OpenAI (GPT-4, GPT-3.5)
- Support Anthropic (Claude 3.5 Sonnet, Claude 3 Opus/Sonnet/Haiku)
- Support Google (Gemini Pro/Flash)
- Support local models via Ollama
- Easy provider switching in config
- Model selection per-provider
- API key management via environment variables or config file
- Streaming responses for better UX

**Configuration Example:**
```toml
[llm]
default_provider = "anthropic"
default_model = "claude-3-5-sonnet-20241022"

[llm.providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
models = ["claude-3-5-sonnet-20241022", "claude-3-opus-20240229"]

[llm.providers.openai]
api_key_env = "OPENAI_API_KEY"
models = ["gpt-4-turbo", "gpt-3.5-turbo"]

[llm.providers.ollama]
endpoint = "http://localhost:11434"
models = ["llama3.2", "codellama"]
```

**Acceptance Criteria:**
- Can switch providers without restarting
- API keys never logged or displayed
- Graceful error handling for API failures
- Support for provider-specific features (vision, function calling)

---

### 7. Configuration Management
**Priority:** P1 (High)

**Description:**
Flexible configuration system for user preferences.

**Requirements:**
- Config file locations: `~/.config/terminai/config.toml` or `./terminai.toml`
- Hot-reload configuration without restart
- Comprehensive settings:
  - Key bindings
  - LLM provider and model
  - Context buffer size
  - Command approval policies
  - Privacy filters
  - UI preferences (colors, size, position)
  - History retention

**Sample Configuration:**
```toml
[general]
shell = "auto"  # auto-detect or specify: bash, zsh, fish
log_level = "info"
history_file = "~/.terminai_history"

[ui]
activation_key = "ctrl-space"
overlay_height_percent = 70
overlay_width_percent = 80
theme = "dark"  # dark, light, auto

[context]
max_lines = 500
max_size_kb = 50
include_env_vars = false
redact_patterns = [
    "password=.*",
    "APIKEY=.*",
    "AWS_.*_KEY=.*",
]

[execution]
safe_commands = ["ls", "pwd", "echo", "cat", "grep", "find"]
dangerous_commands = ["rm", "dd", "mkfs", "chmod", "chown"]
default_approval = "prompt"  # prompt, auto, never
allow_sudo = false
timeout_seconds = 30

[llm]
default_provider = "anthropic"
default_model = "claude-3-5-sonnet-20241022"
stream_responses = true
max_tokens = 4096
temperature = 0.7
```

**Acceptance Criteria:**
- Invalid config shows helpful error messages
- Missing config uses sensible defaults
- Config validation on startup
- Changes apply immediately or on next activation

---

### 8. Session History and Persistence
**Priority:** P1 (High)

**Description:**
Maintain conversation history across terminal sessions.

**Requirements:**
- Save chat history to disk
- Associate history with shell session or working directory
- Search through past conversations
- Export conversations (markdown, JSON)
- Privacy mode to disable history
- Automatic cleanup of old sessions

**Acceptance Criteria:**
- History persists across terminal restarts
- Can recall previous conversations about similar issues
- Privacy mode leaves no traces
- Searchable conversation archive

---

## Non-Functional Requirements

### Performance
- **Startup Time:** <100ms overhead vs native shell
- **Normal Operation:** <1ms latency for keystroke pass-through
- **Overlay Activation:** <500ms to display
- **LLM Response:** Stream tokens as received
- **Memory Usage:** <50MB base, <10MB per 1000 lines of history

### Security
- Never log or display API keys
- Redact sensitive patterns from context by default
- Secure storage of configuration (600 permissions)
- No telemetry or analytics without explicit opt-in
- Command approval for dangerous operations
- Sandboxing for command execution (future)

### Reliability
- Graceful degradation if LLM API unavailable
- Never crash the user's shell session
- Auto-recovery from PTY errors
- Comprehensive error messages
- Fallback to native shell if wrapper fails

### Usability
- Zero-config quick start for common setups
- Helpful first-run tutorial
- Clear error messages with suggested fixes
- Comprehensive documentation
- Keyboard-first interface with optional mouse support

### Compatibility
- Linux (Ubuntu 20.04+, Fedora 35+, Arch)
- macOS (11.0+)
- Windows with WSL2
- Common shells: bash 4.0+, zsh 5.0+, fish 3.0+
- Terminal emulators: iTerm2, Alacritty, GNOME Terminal, Windows Terminal, kitty

---

## Technical Constraints

### Must Use
- PTY (Pseudo-Terminal) for shell wrapping
- Async I/O for performance
- Streaming for LLM responses

### Preferred Languages
- Rust (primary) - for performance, safety, and cross-platform support

### Deployment
- Single static binary
- Package managers: cargo, homebrew, apt/rpm (future)
- No external dependencies at runtime

---

## User Stories

### As a developer debugging an error:
```
1. I run a command that fails with a cryptic error
2. I press Ctrl-Space to open AI chat
3. I type "why did this fail?"
4. The AI sees my terminal history including the error
5. The AI explains the issue and suggests a fix
6. I approve the suggested command
7. The command runs and fixes the issue
8. I press Esc to close the chat and continue working
```

### As a sysadmin managing servers:
```
1. I need to find all large log files
2. I press Ctrl-Space and ask "find log files larger than 1GB"
3. The AI suggests a find command
4. I modify the command to exclude certain directories
5. I execute the modified command
6. I see the results and ask a follow-up about disk cleanup
7. The AI suggests a cleanup strategy
8. I manually execute the approved steps
```

### As a user learning new tools:
```
1. I want to use jq to parse JSON but don't remember syntax
2. I have a JSON file and need to extract specific fields
3. I activate the AI chat
4. I ask "help me parse this JSON to get the 'name' field"
5. The AI sees the JSON file content in my terminal
6. The AI provides the exact jq command
7. I run it and see the results
8. I save the command to my notes
```

---

## Out of Scope (v1.0)

- GUI/web interface (terminal-only for v1)
- Multi-user/collaborative sessions
- Recording/playback of sessions (beyond chat history)
- Built-in shell features (aliases, functions)
- Remote session support (SSH pass-through) - future feature
- Plugin system - future extensibility
- Custom LLM fine-tuning
- Voice input/output
- Screen sharing or session broadcasting

---

## Open Questions

1. **Q:** How should we handle password prompts from sudo?
   **A:** Block interactive commands by default, warn user, optionally allow with config flag

2. **Q:** What if multiple terminal windows are open?
   **A:** Each instance is independent with separate context/history

3. **Q:** How to handle very long command outputs (GB of logs)?
   **A:** Cap context at configurable size, offer to attach files instead

4. **Q:** Should we support SSH pass-through?
   **A:** Defer to v2.0, complex due to PTY nesting

5. **Q:** Rate limiting for LLM APIs?
   **A:** Show warning if approaching limits, allow configurable throttling

---

## Success Criteria

### Launch Criteria (v1.0)
- [ ] Works transparently with bash and zsh
- [ ] Supports Anthropic Claude and OpenAI GPT-4
- [ ] Command approval system prevents accidental destructive operations
- [ ] Less than 10 reported critical bugs in first month
- [ ] Positive feedback from 10+ beta testers
- [ ] Documentation complete (README, Config Guide, API Reference)

### Future Enhancements (v2.0+)
- SSH session support with local AI overlay
- Plugin architecture for custom commands
- Team sharing of conversation patterns
- Integration with developer tools (git, docker, k8s)
- Visual mode for selecting terminal regions for context
- Voice input via Whisper API
- Mobile companion app for monitoring

---

## Timeline

**Phase 1: Foundation (Weeks 1-2)**
- PTY wrapper implementation
- Terminal I/O handling
- Basic configuration system

**Phase 2: Core Features (Weeks 3-4)**
- Overlay UI with Ratatui
- Terminal history capture
- LLM integration (single provider)

**Phase 3: Safety & Polish (Weeks 5-6)**
- Command approval system
- Multi-provider support
- Configuration management
- Error handling and recovery

**Phase 4: Release (Week 7-8)**
- Documentation
- Testing across platforms
- Beta release
- Community feedback incorporation

---

## Risks and Mitigations

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| PTY handling breaks certain shells | High | Medium | Extensive testing, fallback mode |
| LLM API costs too high | Medium | Low | Local model support, rate limiting |
| Security vulnerability in command execution | Critical | Low | Careful sanitization, approval system |
| Performance overhead noticeable | Medium | Low | Benchmarking, optimization |
| Terminal compatibility issues | Medium | Medium | Support major terminals first |
| Complex key binding conflicts | Low | Medium | Configurable bindings, detection |

---

## Appendix

### Related Projects
- **Warp Terminal:** Commercial terminal with AI features (not open source)
- **GitHub Copilot CLI:** Command suggestions (requires separate tool)
- **ShellGPT:** CLI tool (not integrated into terminal)
- **Aider:** AI pair programming (editor-focused)

### Key Differentiators
1. Transparent integration (no workflow change)
2. Full terminal context awareness
3. Direct command execution
4. Overlay UI (no context switching)
5. Open source and self-hostable
6. Shell-agnostic design

### References
- [PTY Programming](https://docs.rs/portable-pty/)
- [Ratatui Documentation](https://ratatui.rs/)
- [Anthropic API](https://docs.anthropic.com/)
- [OpenAI API](https://platform.openai.com/docs/)
