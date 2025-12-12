# Termin.AI rat-salsa Migration Design

## Overview

This document describes the migration of `src/bin/terminai.rs` from using direct ratatui widgets to the rat-salsa architecture with proper focus management for the AI modal components.

**Date:** 2025-12-11
**Status:** Design Phase

---

## Goals

1. **Adopt rat-salsa architecture**: Migrate from custom event loop to rat-salsa's `run_tui()` pattern
2. **Implement focus management**: Enable proper keyboard navigation between AI modal components (conversation view, input field, buttons)
3. **Improve AI modal UX**: Replace basic widgets with rat-salsa widgets that support proper focus/navigation
4. **Add scrollbar to conversation**: Display scrollbar for markdown conversation history (as in tui-markdown/markdown-reader)
5. **Maintain compatibility**: Keep existing shell integration, AI process, and terminal rendering working
6. **Preserve terminal behavior**: Terminal widget should NOT use focus management (focus only applies when AI modal is visible)

---

## Current Architecture Analysis

### Current Structure (terminai.rs)

```rust
struct App<'a> {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    shell: Shell,
    ai_process: Option<AIChatProcess>,
    ai_ui: AIChatUI<'a>,
    ai_visible: bool,
    last_total_rows: usize,
}

impl App {
    async fn run(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                Some(event) = self.shell.event_rx.recv() => { /* ... */ }
                _ = tokio::time::sleep(Duration::from_millis(16)) => {
                    // Poll keyboard events
                    // Handle routing: shell vs AI overlay
                    // Render
                }
            }
        }
    }

    fn render(&mut self) -> Result<()> {
        // Handle scrollback rendering
        // Render shell terminal
        // Render AI overlay if visible
    }
}
```

**Key aspects:**
- Custom tokio-based event loop with async shell event handling
- Manual event routing (keyboard -> shell OR AI)
- AI overlay rendered on top when `ai_visible` is true
- No focus management between components
- Uses tui_textarea for input (single widget, no composability)

### Current AI UI (src/ai_proc/ui.rs)

```rust
pub struct AIChatUI<'a> {
    input: TextArea<'a>,  // tui_textarea widget
}

impl AIChatUI {
    pub fn render(&mut self, process: &AIChatProcess, area: Rect, buf: &mut Buffer) {
        // Layout: [conversation (min 3), input (3), error (3)]
        // Render conversation as Paragraph with markdown
        // Render input as TextArea
        // Render approval popup if pending command
    }

    pub fn input_event(&mut self, event: impl Into<tui_textarea::Input>) {
        self.input.input(event);
    }
}
```

**Components in AI modal:**
1. **Conversation view**: Scrollable markdown display (currently no scrollbar)
2. **Input area**: Multi-line text input (tui_textarea)
3. **Error message**: Optional error display
4. **Approval popup**: Modal dialog for command approval (Y/N)

**Current limitations:**
- No focus management between components
- No scrollbar on conversation view
- Input is always "focused" when modal visible
- Cannot navigate to buttons or other controls with keyboard

---

## Target Architecture (rat-salsa)

### rat-salsa Event Loop Pattern

```rust
fn main() -> Result<()> {
    let mut global = Global::new();
    let mut state = AppState::default();

    run_tui(
        init,    // Initialize focus, etc.
        render,  // Render all widgets
        event,   // Handle events
        error,   // Handle errors
        &mut global,
        &mut state,
        RunConfig::default()?
            .poll(PollCrossterm)  // Crossterm events
            .poll(PollRendered)   // Post-render events
            .poll(PollCustom)     // Custom event source (shell events!)
    )
}
```

**Key components:**

1. **Global**: Application-wide state (implements `SalsaContext`)
   - Contains `SalsaAppContext` for framework features (focus, timers, tasks, queue)
   - Can hold theme, configuration, etc.

2. **AppState**: Application state with widget states
   - All widget states (TextInputState, ScrollState, etc.)
   - Application data (shell, AI process, etc.)

3. **Four Functions**:
   - `init()`: Initialize focus, set up initial state
   - `render()`: Render all widgets to buffer
   - `event()`: Handle application events, return Control<Event>
   - `error()`: Handle errors

4. **Event Flow**:
   ```
   Event Source → Event Handler → Widget.handle() → Control
                                                   ↓
                                    Changed/Unchanged/Continue/Quit
   ```

5. **Focus Management**:
   - `FocusBuilder::build_for(state)` in `init()`
   - `ctx.handle_focus(event)` in event handler (Tab/Shift-Tab/mouse)
   - `FocusBuilder::rebuild_for(state, focus)` after render
   - Widgets implement `HasFocus` trait

---

## Design: New Architecture

### 1. State Structure

```rust
/// Global state (implements SalsaContext)
pub struct Global {
    ctx: SalsaAppContext<AppEvent, Error>,
    theme: SalsaTheme,
    // Could add config, etc.
}

impl SalsaContext<AppEvent, Error> for Global {
    fn set_salsa_ctx(&mut self, app_ctx: SalsaAppContext<AppEvent, Error>) {
        self.ctx = app_ctx;
    }
    fn salsa_ctx(&self) -> &SalsaAppContext<AppEvent, Error> {
        &self.ctx
    }
}

/// Application state
pub struct AppState {
    // Shell state
    shell: Shell,
    last_total_rows: usize,

    // AI state
    ai_process: Option<AIChatProcess>,
    ai_visible: bool,

    // AI UI widget states (only participate in focus when ai_visible)
    ai_conversation: ConversationState,
    ai_input: TextAreaState,  // rat-widget TextArea
    // Note: approval popup will be separate modal
}

/// Custom state for conversation display with scrollbar
pub struct ConversationState {
    focus: FocusFlag,
    area: Rect,
    scroll: ScrollState,  // Custom scroll state
}

impl HasFocus for ConversationState {
    fn build(&self, builder: &mut FocusBuilder) {
        builder.leaf_widget(self);
    }
    fn focus(&self) -> FocusFlag { self.focus.clone() }
    fn area(&self) -> Rect { self.area }
    fn navigable(&self) -> Navigation {
        // Allow navigation to conversation for scrolling
        Navigation::Regular
    }
}

/// Conditional focus implementation for AppState
impl HasFocus for AppState {
    fn build(&self, builder: &mut FocusBuilder) {
        if self.ai_visible {
            // Only add AI components to focus when modal is visible
            builder.widget(&self.ai_conversation);
            builder.widget(&self.ai_input);
            // Note: could add buttons here if we add them
        }
        // Terminal is NEVER in focus (events go directly to PTY)
    }

    fn focus(&self) -> FocusFlag {
        // Container focus - use first child's focus
        if self.ai_visible {
            self.ai_conversation.focus()
        } else {
            FocusFlag::default()  // No focus when terminal mode
        }
    }

    fn area(&self) -> Rect {
        // Full terminal area
        self.ai_conversation.area().union(self.ai_input.area())
    }

    fn navigable(&self) -> Navigation {
        if self.ai_visible {
            Navigation::Regular
        } else {
            Navigation::None  // No navigation in terminal mode
        }
    }
}
```

**Key design decisions:**

1. **Conditional focus**: Focus system only active when `ai_visible == true`
2. **Terminal not focusable**: Shell terminal is not a widget in the focus tree
3. **Conversation scrollable**: Conversation view participates in focus to allow keyboard scrolling
4. **Separate scroll state**: Custom `ScrollState` for conversation (like markdown-reader)

### 2. Event System

```rust
/// Application events
pub enum AppEvent {
    /// Crossterm event (keyboard, mouse, resize)
    Event(crossterm::event::Event),

    /// Post-render event (for focus rebuild)
    Rendered,

    /// Shell events
    ShellOutput,
    ShellTermReply(String),
    ShellExited(i32),
}

impl From<RenderedEvent> for AppEvent {
    fn from(_: RenderedEvent) -> Self {
        Self::Rendered
    }
}

impl From<crossterm::event::Event> for AppEvent {
    fn from(value: crossterm::event::Event) -> Self {
        Self::Event(value)
    }
}
```

**Event routing:**

```
┌─────────────────────────────────────────┐
│         run_tui() event loop            │
├─────────────────────────────────────────┤
│                                         │
│  PollCrossterm  → AppEvent::Event       │
│  PollRendered   → AppEvent::Rendered    │
│  PollCustom     → AppEvent::Shell*      │
│                                         │
└──────────────┬──────────────────────────┘
               ↓
       event() function
               ↓
      ┌────────┴─────────┐
      │                  │
  ai_visible?      shell events?
      │                  │
      ↓                  ↓
  AI modal       Terminal/Shell
  - Focus mgmt    - Direct PTY
  - Widget events - No focus
  - Try_flow!     - Raw events
```

**Event handler structure:**

```rust
pub fn event(
    event: &AppEvent,
    state: &mut AppState,
    ctx: &mut Global,
) -> Result<Control<AppEvent>, Error> {
    match event {
        AppEvent::Event(event) => {
            // Handle global hotkeys first
            match event {
                ct_event!(key press CONTROL-' ') => {
                    // Ctrl-Space: toggle AI modal
                    state.ai_visible = !state.ai_visible;
                    return Ok(Control::Changed);
                }
                ct_event!(key press Esc) if state.ai_visible => {
                    // ESC: close AI modal
                    state.ai_visible = false;
                    return Ok(Control::Changed);
                }
                ct_event!(resized) => {
                    // Handle resize
                    state.shell.resize(w, h)?;
                    return Ok(Control::Changed);
                }
                _ => {}
            }

            // Route events based on mode
            if state.ai_visible {
                // AI modal mode: focus management + widget events
                ctx.handle_focus(event);  // Handle Tab/Shift-Tab/mouse

                // Handle conversation scrolling
                try_flow!(handle_conversation_scroll(
                    &mut state.ai_conversation,
                    event
                ));

                // Handle input
                try_flow!(state.ai_input.handle(event, Regular));

                // Handle Enter to send message
                if matches!(event, ct_event!(key press Enter)) {
                    let msg = state.ai_input.text();
                    // Send to AI (need to handle async...)
                    // Clear input
                    state.ai_input.set_text("");
                    return Ok(Control::Changed);
                }
            } else {
                // Terminal mode: send all events to shell
                let key = Key::new(code, modifiers);
                state.shell.send_key(key)?;
            }
        }
        AppEvent::Rendered => {
            // Rebuild focus after render
            ctx.set_focus(FocusBuilder::rebuild_for(state, ctx.take_focus()));
        }
        AppEvent::ShellOutput => {
            // Check if immediate render needed (scrollback threshold)
            if should_render_scrollback(&state.shell, state.last_total_rows) {
                return Ok(Control::Changed);
            }
        }
        AppEvent::ShellTermReply(reply) => {
            state.shell.writer.write_all(reply.as_bytes())?;
            state.shell.writer.flush()?;
        }
        AppEvent::ShellExited(code) => {
            log::info!("Shell exited with code: {}", code);
            return Ok(Control::Quit);
        }
    }

    Ok(Control::Continue)
}
```

### 3. Render Function

```rust
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    state: &mut AppState,
    ctx: &mut Global,
) -> Result<(), Error> {
    // Handle scrollback rendering (push to native terminal)
    render_scrollback(area, buf, state)?;

    // Render current shell terminal (full screen)
    render_terminal(area, buf, state)?;

    // Render AI overlay if visible
    if state.ai_visible {
        render_ai_modal(area, buf, state, ctx)?;
    }

    Ok(())
}

fn render_ai_modal(
    area: Rect,
    buf: &mut Buffer,
    state: &mut AppState,
    ctx: &mut Global,
) -> Result<(), Error> {
    // Calculate overlay area (80% x 70%, centered)
    let overlay_area = centered_rect(80, 70, area);

    if let Some(ref ai_process) = state.ai_process {
        // Clear overlay area
        Clear.render(overlay_area, buf);

        // Layout: [conversation, input, error?]
        let has_error = ai_process.error_message().is_some();
        let chunks = if has_error {
            Layout::vertical([
                Constraint::Min(3),      // conversation
                Constraint::Length(3),   // input
                Constraint::Length(3),   // error
            ]).split(overlay_area)
        } else {
            Layout::vertical([
                Constraint::Min(3),      // conversation
                Constraint::Length(3),   // input
            ]).split(overlay_area)
        };

        // Render conversation with scrollbar
        render_conversation(chunks[0], buf, state, ai_process)?;

        // Render input
        render_input(chunks[1], buf, state, ai_process, ctx)?;

        // Render error if present
        if has_error {
            render_error(chunks[2], buf, ai_process)?;
        }

        // Render approval popup if pending command
        if let Some(pending) = ai_process.pending_command() {
            render_approval_popup(overlay_area, buf, pending)?;
        }
    } else {
        // Show "not configured" message
        render_not_configured(overlay_area, buf)?;
    }

    Ok(())
}

fn render_conversation(
    area: Rect,
    buf: &mut Buffer,
    state: &mut AppState,
    ai_process: &AIChatProcess,
) -> Result<(), Error> {
    // Split area: [content | scrollbar]
    let [content_area, scrollbar_area] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(1),
    ]).areas(area);

    // Save area for focus
    state.ai_conversation.area = content_area;

    // Build markdown text from conversation
    let messages: Vec<Line> = ai_process
        .conversation()
        .iter()
        .flat_map(|msg| {
            let (prefix, style) = match msg.role {
                MessageRole::User => ("You: ", Style::default().fg(Color::Cyan).bold()),
                MessageRole::Assistant => ("AI: ", Style::default().fg(Color::Green).bold()),
                MessageRole::System => ("System: ", Style::default().fg(Color::Yellow).bold()),
            };

            let mut lines = vec![Line::from(Span::styled(prefix, style))];

            // Render markdown for assistant messages
            if matches!(msg.role, MessageRole::Assistant) {
                let md_text = tui_markdown::from_str(&msg.content);
                lines.extend(md_text.lines);
            } else {
                lines.push(Line::from(&msg.content));
            }

            lines.push(Line::from(""));  // Empty line between messages
            lines
        })
        .collect();

    // Update scroll state
    let content_height = messages.len();
    state.ai_conversation.scroll.view_size = content_area.height as usize;
    state.ai_conversation.scroll.max = content_height;

    // Clamp scroll position
    let position = state.ai_conversation.scroll.position
        .min(content_height.saturating_sub(state.ai_conversation.scroll.view_size));

    // Render paragraph with scroll
    let paragraph = Paragraph::new(messages)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" AI Assistant ")
            .border_style(if state.ai_conversation.focus.get() {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }))
        .wrap(Wrap { trim: false })
        .scroll((position as u16, 0));

    paragraph.render(content_area, buf);

    // Render scrollbar
    let mut scrollbar_state: ScrollbarState = (&mut state.ai_conversation.scroll).into();
    Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .render(scrollbar_area, buf, &mut scrollbar_state);

    Ok(())
}

fn render_input(
    area: Rect,
    buf: &mut Buffer,
    state: &mut AppState,
    ai_process: &AIChatProcess,
    ctx: &mut Global,
) -> Result<(), Error> {
    let title = if ai_process.is_sending() {
        " Sending message... "
    } else {
        " Your Message (Enter to send) "
    };

    let border_style = if state.ai_input.focus().get() {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    TextArea::new()
        .block(Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style))
        .styles(ctx.theme.style(WidgetStyle::TEXT))
        .render(area, buf, &mut state.ai_input);

    // Set cursor if input has focus
    ctx.set_screen_cursor(state.ai_input.screen_cursor());

    Ok(())
}
```

**Key patterns:**

1. **Scrollbar integration**: Split layout for content + scrollbar (like markdown-reader)
2. **Focus indication**: Border color changes based on focus state
3. **Cursor management**: Only show cursor for focused input
4. **Markdown rendering**: Use tui-markdown for assistant messages

### 4. Custom Event Source for Shell

**Challenge**: Shell events come from async tokio channel, but rat-salsa uses sync polling.

**Solution**: Implement custom `PollEvent` for shell events:

```rust
use rat_salsa::poll::PollEvent;
use std::sync::Arc;
use parking_lot::Mutex;

pub struct PollShell {
    receiver: Arc<Mutex<mpsc::UnboundedReceiver<ShellEvent>>>,
}

impl PollShell {
    pub fn new(receiver: mpsc::UnboundedReceiver<ShellEvent>) -> Self {
        Self {
            receiver: Arc::new(Mutex::new(receiver)),
        }
    }
}

impl PollEvent<AppEvent> for PollShell {
    fn poll(&self, timeout: Duration) -> Result<Option<AppEvent>, Error> {
        // Try to receive shell event (non-blocking)
        let mut rx = self.receiver.lock();
        match rx.try_recv() {
            Ok(ShellEvent::Output) => Ok(Some(AppEvent::ShellOutput)),
            Ok(ShellEvent::TermReply(reply)) => Ok(Some(AppEvent::ShellTermReply(reply))),
            Ok(ShellEvent::Exited(code)) => Ok(Some(AppEvent::ShellExited(code))),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                // Shell process died
                Ok(Some(AppEvent::ShellExited(-1)))
            }
        }
    }
}
```

**Usage in main:**

```rust
let (shell, event_rx) = Shell::spawn(...)?;

run_tui(
    init,
    render,
    event,
    error,
    &mut global,
    &mut state,
    RunConfig::default()?
        .poll(PollCrossterm)
        .poll(PollRendered)
        .poll(PollShell::new(event_rx))  // Custom shell event source
)?;
```

### 5. Scroll State Implementation

```rust
/// Custom scroll state (from markdown-reader pattern)
#[derive(Debug, Default, Clone)]
pub struct ScrollState {
    pub position: usize,    // Current scroll position (line number)
    pub view_size: usize,   // Visible area height
    pub max: usize,         // Total content height
}

impl ScrollState {
    pub fn new(max: usize) -> Self {
        Self {
            position: 0,
            view_size: 0,
            max,
        }
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.position = self.position.saturating_sub(lines);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        let max_scroll = self.max.saturating_sub(self.view_size);
        self.position = (self.position + lines).min(max_scroll);
    }

    pub fn scroll_page_up(&mut self) {
        self.scroll_up(self.view_size.saturating_sub(2));
    }

    pub fn scroll_page_down(&mut self) {
        self.scroll_down(self.view_size.saturating_sub(2));
    }

    pub fn scroll_top(&mut self) {
        self.position = 0;
    }

    pub fn scroll_bottom(&mut self) {
        self.position = self.max.saturating_sub(self.view_size);
    }
}

impl From<&mut ScrollState> for ScrollbarState {
    fn from(state: &mut ScrollState) -> ScrollbarState {
        ScrollbarState::new(state.max.saturating_sub(state.view_size))
            .position(state.position)
    }
}

/// Handle scroll events for conversation
fn handle_conversation_scroll(
    conv: &mut ConversationState,
    event: &crossterm::event::Event,
) -> Outcome {
    match event {
        ct_event!(key press 'k') | ct_event!(key press Up) => {
            conv.scroll.scroll_up(1);
            Outcome::Changed
        }
        ct_event!(key press 'j') | ct_event!(key press Down) => {
            conv.scroll.scroll_down(1);
            Outcome::Changed
        }
        ct_event!(key press 'u') | ct_event!(key press PageUp) => {
            conv.scroll.scroll_page_up();
            Outcome::Changed
        }
        ct_event!(key press 'd') | ct_event!(key press PageDown) => {
            conv.scroll.scroll_page_down();
            Outcome::Changed
        }
        ct_event!(key press 'g') | ct_event!(key press Home) => {
            conv.scroll.scroll_top();
            Outcome::Changed
        }
        ct_event!(key press 'G') | ct_event!(key press End) => {
            conv.scroll.scroll_bottom();
            Outcome::Changed
        }
        _ => Outcome::Continue,
    }
}
```

### 6. Async AI Communication

**Challenge**: AI communication is async (uses tokio), but event handler is sync.

**Solution 1**: Use rat-salsa's `spawn_async()` for background tasks:

```rust
// In event handler
if matches!(event, ct_event!(key press Enter)) && state.ai_input.focus().get() {
    let msg = state.ai_input.text().to_string();
    if !msg.is_empty() {
        // Extract context
        let context = extract_context(&state.shell);

        // Spawn async task to send message
        let ai_process = state.ai_process.clone();  // Clone Arc?
        ctx.spawn_async(async move {
            if let Some(ref mut ai) = ai_process {
                ai.send_input_with_context(&msg, context).await?;
            }
            Ok(Control::Changed)
        });

        // Clear input immediately
        state.ai_input.set_text("");
        return Ok(Control::Changed);
    }
}
```

**Solution 2**: Queue message, handle in separate async context:

```rust
// Keep ai_process as Arc<Mutex<AIChatProcess>>
// In event handler: just queue message
ctx.queue(AppEvent::SendAIMessage(msg, context));

// In event handler:
AppEvent::SendAIMessage(msg, context) => {
    if let Some(ref ai) = state.ai_process {
        let ai = ai.clone();
        ctx.spawn_async(async move {
            let mut ai = ai.lock();
            ai.send_input_with_context(&msg, context).await?;
            Ok(Control::Changed)
        });
    }
}
```

**Preferred**: Solution 1 with proper Arc/Mutex wrapping of AIChatProcess.

---

## Implementation Plan

### Phase 1: Setup & Scaffolding

**Goal**: Get rat-salsa event loop running without breaking existing functionality.

**Tasks:**

1. **Add rat-salsa dependencies to Cargo.toml**:
   ```toml
   [dependencies]
   rat-salsa = { path = "../rat-salsa/rat-salsa" }
   rat-focus = { path = "../rat-salsa/rat-focus" }
   rat-event = { path = "../rat-salsa/rat-event" }
   rat-widget = { path = "../rat-salsa/rat-widget" }
   rat-text = { path = "../rat-salsa/rat-text" }
   rat-scrolled = { path = "../rat-salsa/rat-scrolled" }
   rat-theme4 = { path = "../rat-salsa/rat-theme4" }
   ```

2. **Create basic rat-salsa structure**:
   - Define `Global` struct implementing `SalsaContext`
   - Define `AppEvent` enum
   - Implement `init()`, `render()`, `event()`, `error()` functions
   - Replace `main()` with `run_tui()` call

3. **Implement `PollShell` custom event source**:
   - Create sync wrapper for shell event receiver
   - Implement `PollEvent` trait
   - Register in RunConfig

4. **Migrate shell rendering**:
   - Move terminal rendering logic to `render()` function
   - Keep scrollback handling working
   - Ensure cursor positioning works

5. **Test basic functionality**:
   - Shell launches and displays correctly
   - Keyboard input goes to shell
   - Terminal resizing works
   - Shell exit closes application

**Acceptance criteria:**
- Application runs with rat-salsa event loop
- Terminal works exactly as before
- No AI modal yet (always hidden)
- Build, lint, test pass

**Estimated effort**: 4-6 hours

### Phase 2: AI Modal Basic Migration

**Goal**: Get AI modal rendering with rat-salsa widgets (no focus management yet).

**Tasks:**

1. **Create AppState structure**:
   - Move App fields to AppState
   - Add widget states (ConversationState, TextAreaState)
   - Keep AI process integration

2. **Migrate AI modal rendering**:
   - Move AI overlay rendering to `render_ai_modal()`
   - Use Layout for conversation/input/error areas
   - Keep existing tui-markdown rendering

3. **Add Ctrl-Space toggle**:
   - Handle in event() function
   - Toggle `state.ai_visible` flag
   - Return `Control::Changed`

4. **Add ESC to close**:
   - Handle in event() when ai_visible
   - Set ai_visible = false

5. **Basic event routing**:
   - Route to shell when !ai_visible
   - Route to AI when ai_visible (stub for now)

**Acceptance criteria:**
- Ctrl-Space toggles AI modal
- ESC closes AI modal
- Modal displays (even without focus management)
- Build, lint, test pass

**Estimated effort**: 3-4 hours

### Phase 3: Replace Input Widget

**Goal**: Replace tui_textarea with rat-widget TextArea.

**Tasks:**

1. **Add TextAreaState to AppState**:
   ```rust
   ai_input: TextAreaState,
   ```

2. **Update render_input()**:
   - Use rat-widget TextArea widget
   - Apply theme styles
   - Set block with title
   - Handle focus border color

3. **Handle TextArea events**:
   - Call `state.ai_input.handle(event, Regular)` in event()
   - Use `try_flow!` macro
   - Handle Enter key separately for sending message

4. **Get input text**:
   - Use `state.ai_input.text()` instead of TextArea::lines()
   - Clear with `state.ai_input.set_text("")`

5. **Set cursor position**:
   - Use `ctx.set_screen_cursor(state.ai_input.screen_cursor())`

**Acceptance criteria:**
- Input works with rat-widget TextArea
- Text entry, editing, multiline work
- Enter sends message (existing logic)
- Cursor appears in input
- Build, lint, test pass

**Estimated effort**: 2-3 hours

### Phase 4: Add Scrollbar to Conversation

**Goal**: Add scrollbar to conversation view using markdown-reader pattern.

**Tasks:**

1. **Create ConversationState**:
   ```rust
   pub struct ConversationState {
       pub focus: FocusFlag,
       pub area: Rect,
       pub scroll: ScrollState,
   }
   ```

2. **Create ScrollState** (copy from markdown-reader):
   - position, view_size, max fields
   - scroll_up(), scroll_down(), etc. methods
   - `From<&mut ScrollState> for ScrollbarState`

3. **Update render_conversation()**:
   - Split layout: [content | scrollbar]
   - Update scroll.view_size from area height
   - Update scroll.max from content height
   - Clamp scroll position
   - Render Paragraph with .scroll()
   - Render Scrollbar separately

4. **Handle scroll events** (stub for now):
   - Create `handle_conversation_scroll()` function
   - Handle k/j, Up/Down, PageUp/PageDown, Home/End
   - Return Outcome::Changed when scrolled

5. **Integrate in event()**:
   - Call handle_conversation_scroll() when ai_visible
   - Use try_flow!() macro

**Acceptance criteria:**
- Scrollbar appears next to conversation
- Scrollbar position reflects content scroll
- Scroll events work (k/j/arrows/page/home/end)
- Build, lint, test pass

**Estimated effort**: 3-4 hours

### Phase 5: Implement Focus Management

**Goal**: Enable proper focus navigation between conversation and input.

**Tasks:**

1. **Implement HasFocus for ConversationState**:
   ```rust
   impl HasFocus for ConversationState {
       fn build(&self, builder: &mut FocusBuilder) {
           builder.leaf_widget(self);
       }
       fn focus(&self) -> FocusFlag { self.focus.clone() }
       fn area(&self) -> Rect { self.area }
       fn navigable(&self) -> Navigation { Navigation::Regular }
   }
   ```

2. **Implement HasFocus for AppState** (conditional):
   ```rust
   impl HasFocus for AppState {
       fn build(&self, builder: &mut FocusBuilder) {
           if self.ai_visible {
               builder.widget(&self.ai_conversation);
               builder.widget(&self.ai_input);
           }
       }
       // ... other methods
   }
   ```

3. **Initialize focus in init()**:
   ```rust
   pub fn init(state: &mut AppState, ctx: &mut Global) -> Result<(), Error> {
       ctx.set_focus(FocusBuilder::build_for(state));
       if state.ai_visible {
           ctx.focus().first();  // Focus first widget
       }
       Ok(())
   }
   ```

4. **Handle focus in event()**:
   ```rust
   if state.ai_visible {
       ctx.handle_focus(event);  // Tab/Shift-Tab/mouse
       // ... widget events
   }
   ```

5. **Rebuild focus after render**:
   ```rust
   AppEvent::Rendered => {
       ctx.set_focus(FocusBuilder::rebuild_for(state, ctx.take_focus()));
   }
   ```

6. **Update border colors based on focus**:
   - In render_conversation(): Check `state.ai_conversation.focus.get()`
   - In render_input(): Check `state.ai_input.focus().get()`
   - Set border_style to cyan when focused

7. **Handle focus state changes**:
   - When toggling AI modal, rebuild focus
   - When closing AI modal, clear focus

**Acceptance criteria:**
- Tab navigates from conversation to input
- Shift-Tab navigates backward
- Border color indicates focus
- Scroll keys work when conversation focused
- Text entry works when input focused
- Focus resets when toggling modal
- Build, lint, test pass

**Estimated effort**: 4-5 hours

### Phase 6: Polish & Integration

**Goal**: Ensure all existing features work with new architecture.

**Tasks:**

1. **Fix async AI message sending**:
   - Wrap AIChatProcess in Arc<Mutex> if needed
   - Use ctx.spawn_async() for sending messages
   - Handle success/error states

2. **Fix command approval**:
   - Ensure approval popup still works
   - Handle Y/N keys in event handler
   - Inject command into shell PTY

3. **Test scrollback rendering**:
   - Ensure multi-screen scrollback still works
   - Test with fast-scrolling output
   - Verify native terminal scrollback integration

4. **Test all keybindings**:
   - Ctrl-Space toggle
   - ESC close
   - Tab/Shift-Tab focus
   - Scroll keys (k/j, arrows, page, home/end)
   - Enter to send message
   - Y/N for approval

5. **Test error handling**:
   - Shell crash/exit
   - AI API errors
   - Terminal resize during various operations

6. **Code cleanup**:
   - Remove unused imports
   - Add documentation comments
   - Ensure consistent error handling
   - Run clippy and fix warnings

7. **Run full test suite**:
   - `cargo build --bin terminai`
   - `cargo clippy --bin terminai`
   - `cargo test`
   - Manual testing in various terminals

**Acceptance criteria:**
- All existing features work
- All tests pass
- No clippy warnings
- Code is clean and documented
- Ready for commit

**Estimated effort**: 3-4 hours

---

## Testing Strategy

### Unit Tests

- **ScrollState**: Test scroll methods (up/down/page/top/bottom)
- **PollShell**: Test event conversion from shell events
- **Focus logic**: Test conditional focus building

### Integration Tests

- **Event routing**: Verify events go to correct handler based on mode
- **Focus navigation**: Tab/Shift-Tab cycle through widgets
- **Scroll synchronization**: Scrollbar reflects content position

### Manual Testing

- **Terminal compatibility**: Test in multiple terminal emulators
- **Shell compatibility**: Test with bash, zsh, fish
- **Performance**: Verify no latency increase
- **Visual**: Check rendering, colors, borders, cursor

---

## Migration Risks & Mitigations

### Risk 1: Breaking Shell Integration

**Risk**: Shell PTY handling might break during event loop migration.

**Mitigation**:
- Phase 1 focuses only on shell (no AI yet)
- Keep shell event handling as close to original as possible
- Test thoroughly before moving to AI modal

### Risk 2: Async/Sync Impedance

**Risk**: AI process uses async/await, rat-salsa event handler is sync.

**Mitigation**:
- Use rat-salsa's spawn_async() for async operations
- Wrap AI process in Arc<Mutex> if needed
- Queue events for async operations

### Risk 3: Focus Complexity

**Risk**: Conditional focus (only when modal visible) might be tricky.

**Mitigation**:
- Start with always-visible modal for Phase 5 testing
- Add conditional logic once basic focus works
- Use FocusBuilder::default() when no focus needed

### Risk 4: Scrollback Rendering

**Risk**: Complex scrollback rendering might not work with rat-salsa.

**Mitigation**:
- Keep existing scrollback logic in Phase 1
- Use terminal.draw() directly if needed (rat-salsa allows this)
- Test with fast-scrolling output early

### Risk 5: Performance

**Risk**: rat-salsa overhead might increase latency.

**Mitigation**:
- Benchmark keystroke latency before and after
- Use Release build for realistic testing
- Profile if performance degrades

---

## Success Criteria

### Functional Requirements

- ✅ Shell works exactly as before (transparent pass-through)
- ✅ Ctrl-Space toggles AI modal
- ✅ ESC closes AI modal
- ✅ AI modal has proper focus management (Tab/Shift-Tab)
- ✅ Conversation view is scrollable with keyboard
- ✅ Scrollbar shows scroll position
- ✅ Input field accepts text and sends on Enter
- ✅ Focus is indicated by border color
- ✅ Command approval still works
- ✅ All existing AI features work

### Non-Functional Requirements

- ✅ Build succeeds: `cargo build --bin terminai`
- ✅ Lint passes: `cargo clippy --bin terminai`
- ✅ Tests pass: `cargo test`
- ✅ No performance degradation (<1ms latency increase)
- ✅ Code is clean and documented
- ✅ No regressions in shell handling

### User Experience

- ✅ Smooth focus indication (visual feedback)
- ✅ Intuitive navigation (Tab works as expected)
- ✅ Scrollbar provides orientation in long conversations
- ✅ No flicker or visual artifacts
- ✅ Consistent with existing terminal behavior

---

## Open Questions

### Q1: Should conversation view be focusable?

**Options:**
- A) Yes (allows keyboard scrolling, needs focus)
- B) No (scroll always works, no focus needed)

**Decision**: **A** - Make conversation focusable. This provides clear visual feedback about which component has focus and allows scroll bindings to be context-specific.

### Q2: How to handle Enter in input?

**Options:**
- A) Always sends message (single-line behavior)
- B) Shift-Enter for newline, Enter sends (chat app pattern)
- C) Ctrl-Enter sends, Enter for newline (code editor pattern)

**Decision**: **B** - Shift-Enter for newline, Enter sends. This is most familiar from chat applications.

### Q3: Should we add buttons (Send, Cancel)?

**Options:**
- A) Yes (more discoverable, adds to focus cycle)
- B) No (keyboard-first, keep it simple)

**Decision**: **B** - No buttons for now. Keep keyboard-first design. Can add later if user feedback requests it.

### Q4: How to wrap AIChatProcess for async?

**Options:**
- A) Arc<Mutex<AIChatProcess>>
- B) Arc<RwLock<AIChatProcess>>
- C) Keep as is, use ctx.spawn_async() with cloned references

**Decision**: TBD - Need to review AIChatProcess API. Likely **C** if process has interior mutability, otherwise **A**.

### Q5: Should we handle mouse clicks on conversation?

**Options:**
- A) Yes (scroll on mouse wheel, maybe select text?)
- B) No (keyboard-only for now)

**Decision**: **A** - Yes, but only mouse wheel for scrolling. Text selection would be complex and can be deferred.

---

## References

### Code Locations

- **Current terminai.rs**: `/var/home/eitan/projects/termin.ai/src/bin/terminai.rs`
- **Current AI UI**: `/var/home/eitan/projects/termin.ai/src/ai_proc/ui.rs`
- **rat-salsa examples**: `/var/home/eitan/projects/termin.ai/rat-salsa/rat-salsa/examples/`
- **markdown-reader**: `/var/home/eitan/projects/termin.ai/tui-markdown/markdown-reader/src/`

### Key Examples

- **textinput.rs**: Basic text input with focus
- **composite.rs**: Composite widget with focus
- **markdown-reader/app.rs**: Scrollbar with paragraph (lines 144-183)

### Documentation

- **rat-salsa docs**: See examples and source comments
- **Focus system**: `/var/home/eitan/projects/termin.ai/rat-salsa/rat-focus/`
- **Event handling**: `/var/home/eitan/projects/termin.ai/rat-salsa/rat-event/`

---

## Change Log

### 2025-12-11 - Initial Design

- Created initial design document
- Defined architecture and migration plan
- Outlined 6 phases of implementation
- Identified risks and mitigations
- Set success criteria

---

**Status**: Ready for implementation review and approval.
