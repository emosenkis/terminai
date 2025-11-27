# Hybrid Terminal Implementation Progress

**Started:** 2025-11-27
**Status:** In Progress

## Overview

Implementing a hybrid terminal system that:
- Passes through guest terminal output directly to host's main buffer (native scrollback)
- Maintains a shadow ratatui-compatible terminal emulator for modal display
- Switches to alternate buffer when showing modals
- Handles buffer transitions correctly for both host and guest alternate buffer states

## Implementation Plan Checklist

### Phase 1: Core State Management
- [ ] `src/hybrid/mode.rs` - ModeManager and Mode enum
  - [ ] Mode enum (Passthrough, GuestAltBuffer, ModalWithBuffering, ModalGuestAlt)
  - [ ] ModeManager state tracking
  - [ ] Mode transition logic
  - [ ] Unit tests for mode transitions

### Phase 2: Terminal Components
- [ ] `src/hybrid/terminal/mod.rs` - Terminal module structure
- [ ] `src/hybrid/terminal/shadow.rs` - ShadowTerminal (wraps vt100)
  - [ ] Vt100Wrapper integration
  - [ ] Event detection (alt buffer switches)
  - [ ] Screen content extraction
  - [ ] Unit tests for vt100 integration
- [ ] `src/hybrid/terminal/host_controller.rs` - HostTerminalController
  - [ ] Alternate buffer enter/leave
  - [ ] Raw output writing
  - [ ] State tracking
- [ ] `src/hybrid/terminal/content.rs` - TerminalContent struct
  - [ ] Cell grid structure
  - [ ] Cursor state
  - [ ] vt100 to ratatui cell mapping

### Phase 3: Output Routing
- [ ] `src/hybrid/routing/mod.rs` - Routing module structure
- [ ] `src/hybrid/routing/output_router.rs` - OutputRouter
  - [ ] Mode-based routing logic
  - [ ] Terminal event handling
  - [ ] Host buffer synchronization
  - [ ] Integration tests
- [ ] `src/hybrid/routing/buffer.rs` - OutputBuffer
  - [ ] Basic buffering
  - [ ] Overflow handling
  - [ ] Smart buffer with checkpoints
  - [ ] Buffer replay logic

### Phase 4: Rendering
- [ ] `src/hybrid/rendering/mod.rs` - Rendering module structure
- [ ] `src/hybrid/rendering/ratatui_renderer.rs` - RatatuiRenderer
  - [ ] Terminal content rendering
  - [ ] Modal overlay rendering
  - [ ] Frame composition
- [ ] `src/hybrid/rendering/modal.rs` - ModalState and ModalContent
  - [ ] Modal state structure
  - [ ] Content variants (Text, List, Custom)
  - [ ] Rendering logic
- [ ] `src/hybrid/rendering/cell_mapping.rs` - vt100 to ratatui conversions
  - [ ] Color mapping
  - [ ] Style mapping
  - [ ] Cell conversion

### Phase 5: Event Loop
- [ ] `src/hybrid/event_loop.rs` - HybridTerminal main event loop
  - [ ] PTY output handling
  - [ ] Application event handling
  - [ ] Mode transitions
  - [ ] Rendering coordination
- [ ] `src/hybrid/input.rs` - Input handling
  - [ ] Key event to bytes conversion
  - [ ] Modal input handling
  - [ ] Mode-aware input routing

### Phase 6: Integration
- [ ] `src/hybrid/mod.rs` - Module exports and public API
- [ ] `src/hybrid/lib.rs` - Public library interface
- [ ] Integration with existing app.rs
- [ ] Update binary to use hybrid terminal

### Phase 7: Testing
- [ ] Unit tests for all components
- [ ] Integration tests for mode transitions
- [ ] Buffer replay tests
- [ ] End-to-end tests
- [ ] Manual testing with various applications

### Phase 8: Documentation
- [ ] API documentation
- [ ] Architecture documentation
- [ ] State diagram documentation
- [ ] Usage examples

## Current Progress

### Completed
- ✅ Phase 1: ModeManager and Mode enum
  - Mode enum with 4 operational modes
  - ModeTransition struct for state changes
  - Comprehensive unit tests (all passing)

- ✅ Phase 2: Terminal Components
  - TerminalContent struct for ratatui rendering
  - ShadowTerminal wrapping vt100 parser
  - HostTerminalController for alt buffer management
  - vt100 to ratatui cell mapping

- ✅ Phase 3: Output Routing
  - OutputBuffer with overflow handling
  - SmartOutputBuffer with checkpoints
  - OutputRouter with mode-based routing
  - Host buffer synchronization logic

- ✅ Phase 4: Rendering Components
  - RatatuiRenderer for terminal + modal rendering
  - ModalState with Text, List, and Custom content types
  - Modal styling and navigation support

- ✅ Phase 5: Event Loop and Input
  - HybridTerminal main orchestrator
  - Event loop with PTY output, app events, and keyboard input
  - Key to bytes conversion for terminal sequences
  - Modal input handling

- ✅ Build successful with all components integrated

### In Progress
- Documentation and testing

### Blocked
- None

## Notes

- Following design document: See task description for complete architecture
- Using existing vt100 module from mprocs
- Integrating with existing ratatui rendering
- Maintaining compatibility with existing AI modal system

## Next Steps

1. Create module directory structure
2. Implement ModeManager (Phase 1)
3. Implement ShadowTerminal (Phase 2)
4. Implement OutputRouter (Phase 3)
