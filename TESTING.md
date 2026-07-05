# Testing Guide for Terminai

This document describes the e2e testing infrastructure for Terminai using ratatui's TestBackend.

## Overview

The e2e test harness allows you to test Terminai's UI components without requiring an actual terminal. It uses ratatui's `TestBackend` to render UI widgets to an in-memory buffer that can be inspected and verified.

## Test Infrastructure

### Location

- **Test harness**: `src/tests/e2e/mod.rs`
- **UI tests**: `src/tests/e2e/test_ui.rs`
- **VT100 tests**: `src/tests/e2e/test_vt100.rs`
- **AI overlay tests**: `src/tests/e2e/test_ai.rs`

### Dependencies

The test infrastructure uses:
- **ratatui TestBackend**: For rendering UI to memory
- **insta**: For snapshot testing (optional, enabled with `snapshot-tests` feature)
- **tempfile**: For creating temporary files/directories in tests

Add these to your `Cargo.toml` under `[dev-dependencies]`:

```toml
[dev-dependencies]
insta = { version = "1.34", features = ["yaml"] }
tempfile = "3.8"
```

## Test Harness API

### Creating a Test Harness

```rust
use super::*;

#[test]
fn my_test() {
    // Create with default size (80x24)
    let harness = TestHarness::new();

    // Or with custom size
    let config = TestAppConfig::new().with_terminal_size(120, 40);
    let harness = TestHarness::with_config(config);
}
```

### Rendering Widgets

```rust
#[test]
fn test_widget_rendering() {
    let mut harness = TestHarness::new();

    // Create a widget
    let widget = Paragraph::new("Hello, World!")
        .block(Block::default().borders(Borders::ALL));

    // Render it
    harness.render(widget).unwrap();

    // Verify the content
    harness.assert_buffer_contains("Hello, World!");
}
```

### Simulating User Input

```rust
#[test]
fn test_user_input() {
    let mut harness = TestHarness::new();

    // Type a string
    harness.type_string("hello world");

    // Press specific keys
    harness.press_key(KeyCode::Enter);

    // Press keys with modifiers
    harness.press_key_with_modifiers(KeyCode::Char('c'), KeyModifiers::CONTROL);

    // Add delays
    harness.wait(Duration::from_millis(100));
}
```

### Assertions

```rust
#[test]
fn test_assertions() {
    let mut harness = TestHarness::new();

    // ... render something ...

    // Assert buffer contains text
    harness.assert_buffer_contains("Expected text");

    // Assert buffer matches specific lines
    harness.assert_buffer_lines(vec!["Line 1", "Line 2", "Line 3"]);

    // Get buffer as string for custom assertions
    let buffer_str = harness.buffer_as_string();
    assert!(buffer_str.contains("something"));
}
```

## Running Tests

```bash
# Run all e2e tests
cargo test e2e

# Run specific test file
cargo test --test e2e_tests

# Run with output
cargo test e2e -- --nocapture

# Run specific test
cargo test e2e::test_ui::test_basic_rendering
```

## Snapshot Testing

Snapshot tests are useful for verifying complex UI layouts. They're disabled by default and require the `snapshot-tests` feature.

### Enabling Snapshot Tests

```bash
# Run tests with snapshot feature
cargo test --features snapshot-tests e2e
```

### Writing Snapshot Tests

```rust
#[test]
#[cfg(feature = "snapshot-tests")]
fn test_ui_snapshot() {
    let mut harness = TestHarness::new();

    let widget = Paragraph::new("Snapshot Test Content")
        .block(Block::default().borders(Borders::ALL).title("Snapshot"));

    harness.render(widget).unwrap();

    // Create/verify snapshot
    insta::assert_snapshot!(harness.buffer_as_string());
}
```

### Reviewing Snapshots

```bash
# Review and accept new snapshots
cargo insta review

# Accept all snapshots
cargo insta accept
```

## Testing VT100 Terminal Emulation

The `test_vt100.rs` file contains tests for the VT100 terminal emulator:

```rust
#[test]
fn test_vt100_basic_text() {
    let mut parser = vt100::Parser::new(24, 80, 1000, TestReplySender);

    // Process terminal output
    parser.process(b"Hello, World!");

    // Verify the screen
    let screen = parser.screen();
    assert_eq!(screen.size().rows, 24);
    assert_eq!(screen.size().cols, 80);
}
```

## Testing AI Overlay

The `test_ai.rs` file contains tests for the AI assistant overlay:

```rust
#[test]
fn test_ai_overlay_activation() {
    let mut harness = TestHarness::new();

    // Simulate Ctrl+Space to activate AI
    harness.press_key_with_modifiers(KeyCode::Char(' '), KeyModifiers::CONTROL);

    // Verify the event was queued
    assert_eq!(harness.events.len(), 1);
}
```

## Best Practices

### 1. Use Consistent Terminal Sizes

For reproducible tests, use consistent terminal sizes (default is 80x24):

```rust
let harness = TestHarness::new(); // Always 80x24
```

### 2. Test One Thing at a Time

Keep tests focused on a single behavior:

```rust
#[test]
fn test_border_rendering() {
    // Test ONLY border rendering, not content
}

#[test]
fn test_content_rendering() {
    // Test ONLY content, not borders
}
```

### 3. Use Descriptive Test Names

```rust
#[test]
fn test_ai_overlay_shows_conversation_history() {
    // Clear what this tests
}
```

### 4. Clean Up Test Data

For tests that create files or state:

```rust
#[test]
fn test_with_temp_files() {
    let temp_dir = tempfile::tempdir().unwrap();

    // ... use temp_dir ...

    // Automatically cleaned up when temp_dir drops
}
```

### 5. Document Complex Assertions

```rust
#[test]
fn test_complex_layout() {
    // ... setup ...

    // The overlay should be centered and take up 80% width
    harness.assert_buffer_contains("Overlay Title");
}
```

## Troubleshooting

### Tests Pass Locally But Fail in CI

- Ensure consistent terminal sizes
- Avoid timing-dependent tests
- Use snapshot tests for complex layouts

### Buffer Doesn't Contain Expected Text

- Check for whitespace differences
- Verify the widget was actually rendered
- Print the buffer for debugging:
  ```rust
  println!("{}", harness.buffer_as_string());
  ```

### Snapshot Tests Keep Failing

- Review the diff carefully using `cargo insta review`
- Ensure terminal size is consistent
- Check for platform-specific rendering differences

## Examples

See the existing tests in `src/tests/e2e/` for complete examples:

- **Basic UI**: `test_ui.rs` - Simple widget rendering
- **VT100**: `test_vt100.rs` - Terminal emulation testing
- **AI Overlay**: `test_ai.rs` - Complex overlay interactions

## Future Enhancements

Planned improvements to the test infrastructure:

1. **Shell Integration Tests**: Test actual shell interaction
2. **Command Execution Tests**: Verify command injection works correctly
3. **Performance Tests**: Measure rendering performance
4. **Accessibility Tests**: Verify keyboard navigation
5. **Integration with Actual Terminal**: Test in real terminal when needed

## References

- [Ratatui TestBackend Documentation](https://docs.rs/ratatui/latest/ratatui/backend/struct.TestBackend.html)
- [Ratatui Testing Guide](https://ratatui.rs/recipes/testing/snapshots/)
- [Insta Snapshot Testing](https://insta.rs/)

---

Last updated: 2025-11-29
