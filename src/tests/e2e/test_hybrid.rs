// E2E tests for the Hybrid Terminal System
//
// Tests the 4-mode state machine, output routing, shadow terminal,
// and buffer management for the hybrid terminal emulator.

use super::*;
use crossterm::event::{KeyCode, KeyModifiers};
use std::sync::Arc;
use termin::hybrid::{
  input::key_to_bytes,
  mode::{Mode, ModeManager},
  rendering::{ModalContent, ModalState},
  routing::{OutputBuffer, OutputRouter},
  terminal::{HostTerminalController, ShadowTerminal},
};
use termin::vt100::TermReplySender;
use tokio::sync::{Mutex, RwLock};

// Test reply sender for vt100
#[derive(Clone, Debug)]
struct TestReplySender;

impl TermReplySender for TestReplySender {
  fn reply(&self, _s: compact_str::CompactString) {}
}

// ============================================================================
// Mode Manager Tests
// ============================================================================

#[tokio::test]
async fn test_mode_transitions_passthrough_to_modal() {
  let mut mode_mgr = ModeManager::new();

  // Initial state: Passthrough
  assert_eq!(mode_mgr.current_mode(), Mode::Passthrough);
  assert!(!mode_mgr.is_modal_visible());
  assert!(!mode_mgr.is_guest_in_alt_buffer());

  // Show modal -> ModalWithBuffering
  let transition = mode_mgr.set_modal_visible(true);
  assert_eq!(mode_mgr.current_mode(), Mode::ModalWithBuffering);
  assert!(mode_mgr.is_modal_visible());
  assert!(transition.needs_host_buffer_switch);
  assert!(!transition.needs_buffer_replay);
}

#[tokio::test]
async fn test_mode_transitions_modal_to_passthrough() {
  let mut mode_mgr = ModeManager::new();

  // Go to modal state
  mode_mgr.set_modal_visible(true);
  assert_eq!(mode_mgr.current_mode(), Mode::ModalWithBuffering);

  // Hide modal -> back to Passthrough
  let transition = mode_mgr.set_modal_visible(false);
  assert_eq!(mode_mgr.current_mode(), Mode::Passthrough);
  assert!(!mode_mgr.is_modal_visible());
  assert!(transition.needs_host_buffer_switch);
  assert!(transition.needs_buffer_replay);
}

#[tokio::test]
async fn test_mode_transitions_guest_alt_buffer() {
  let mut mode_mgr = ModeManager::new();

  // Guest enters alt buffer -> GuestAltBuffer
  let transition = mode_mgr.set_guest_alt_buffer(true);
  assert_eq!(mode_mgr.current_mode(), Mode::GuestAltBuffer);
  assert!(mode_mgr.is_guest_in_alt_buffer());
  assert!(transition.needs_host_buffer_switch);

  // Guest leaves alt buffer -> back to Passthrough
  let transition = mode_mgr.set_guest_alt_buffer(false);
  assert_eq!(mode_mgr.current_mode(), Mode::Passthrough);
  assert!(!mode_mgr.is_guest_in_alt_buffer());
  assert!(transition.needs_host_buffer_switch);
}

#[tokio::test]
async fn test_mode_transitions_modal_guest_alt() {
  let mut mode_mgr = ModeManager::new();

  // Both modal and guest alt buffer -> ModalGuestAlt
  mode_mgr.set_modal_visible(true);
  mode_mgr.set_guest_alt_buffer(true);
  assert_eq!(mode_mgr.current_mode(), Mode::ModalGuestAlt);
  assert!(mode_mgr.is_modal_visible());
  assert!(mode_mgr.is_guest_in_alt_buffer());
}

// ============================================================================
// Shadow Terminal Tests
// ============================================================================

#[test]
fn test_shadow_terminal_basic_output() {
  let mut shadow = ShadowTerminal::new(80, 24, 1000, TestReplySender);

  // Process some text
  shadow.process(b"Hello, World!");

  // Verify content
  let content = shadow.visible_content();
  assert_eq!(content.size, (80, 24));
  assert!(!content.cells.is_empty());

  // First row should contain our text
  let first_row = &content.cells[0];
  let mut text = String::new();
  for cell in first_row {
    text.push_str(&cell.symbol().to_string());
  }
  assert!(text.trim_end().starts_with("Hello, World!"));
}

#[test]
fn test_shadow_terminal_alt_buffer_detection() {
  let mut shadow = ShadowTerminal::new(80, 24, 1000, TestReplySender);

  // Not in alt buffer initially
  assert!(!shadow.is_in_alt_buffer());

  // Enter alt buffer (ESC[?1049h)
  shadow.process(b"\x1b[?1049h");
  assert!(shadow.is_in_alt_buffer());

  // Leave alt buffer (ESC[?1049l)
  shadow.process(b"\x1b[?1049l");
  assert!(!shadow.is_in_alt_buffer());
}

#[test]
fn test_shadow_terminal_resize() {
  let mut shadow = ShadowTerminal::new(80, 24, 1000, TestReplySender);

  // Initial size
  assert_eq!(shadow.size(), (80, 24));

  // Resize
  shadow.resize(120, 40);
  assert_eq!(shadow.size(), (120, 40));

  let content = shadow.visible_content();
  assert_eq!(content.size, (120, 40));
}

// ============================================================================
// Output Buffer Tests
// ============================================================================

#[tokio::test]
async fn test_output_buffer_append_and_take() {
  let buffer = Arc::new(Mutex::new(OutputBuffer::default()));

  // Append some data
  {
    let mut buf = buffer.lock().await;
    buf.append(b"Hello");
    buf.append(b" ");
    buf.append(b"World");
  }

  // Take data
  {
    let mut buf = buffer.lock().await;
    let data = buf.take();
    assert_eq!(data, b"Hello World");
  }

  // Should be empty now
  {
    let buf = buffer.lock().await;
    assert!(buf.is_empty());
  }
}

#[tokio::test]
async fn test_output_buffer_overflow() {
  let buffer = Arc::new(Mutex::new(OutputBuffer::new(100)));

  {
    let mut buf = buffer.lock().await;

    // Fill with 50 bytes
    buf.append(&vec![b'A'; 50]);
    assert!(!buf.has_overflow());

    // Add 60 more bytes (total 110, exceeds 100)
    buf.append(&vec![b'B'; 60]);
    assert!(buf.has_overflow());

    // Should keep the most recent data (within max_size)
    let data = buf.take();
    assert!(data.len() <= 100);
    // After take(), overflow flag is reset
    assert!(!buf.has_overflow());
  }
}

// ============================================================================
// Output Router Tests
// ============================================================================

#[tokio::test]
async fn test_output_router_passthrough_mode() {
  let mode_mgr = Arc::new(RwLock::new(ModeManager::new()));
  let shadow = Arc::new(RwLock::new(ShadowTerminal::new(
    80,
    24,
    1000,
    TestReplySender,
  )));
  let host = Arc::new(Mutex::new(HostTerminalController::new()));
  let buffer = Arc::new(Mutex::new(OutputBuffer::default()));

  let router = OutputRouter::new(
    Arc::clone(&mode_mgr),
    Arc::clone(&shadow),
    Arc::clone(&host),
    Arc::clone(&buffer),
  );

  // In Passthrough mode, output should not be buffered
  router.route_output(b"Hello").await.unwrap();

  // Buffer should be empty (output was passed through)
  let buf = buffer.lock().await;
  assert!(buf.is_empty());

  // Shadow should have processed it
  let shadow = shadow.read().await;
  let content = shadow.visible_content();
  let first_row = &content.cells[0];
  let mut text = String::new();
  for cell in first_row {
    text.push_str(&cell.symbol().to_string());
  }
  assert!(text.contains("Hello"));
}

#[tokio::test]
async fn test_output_router_modal_buffering_mode() {
  let mode_mgr = Arc::new(RwLock::new(ModeManager::new()));
  let shadow = Arc::new(RwLock::new(ShadowTerminal::new(
    80,
    24,
    1000,
    TestReplySender,
  )));
  let host = Arc::new(Mutex::new(HostTerminalController::new()));
  let buffer = Arc::new(Mutex::new(OutputBuffer::default()));

  let router = OutputRouter::new(
    Arc::clone(&mode_mgr),
    Arc::clone(&shadow),
    Arc::clone(&host),
    Arc::clone(&buffer),
  );

  // Enter modal mode (ModalWithBuffering)
  {
    let mut mgr = mode_mgr.write().await;
    mgr.set_modal_visible(true);
  }

  // Route output - should be buffered
  router.route_output(b"Buffered Data").await.unwrap();

  // Buffer should contain the data
  let buf = buffer.lock().await;
  assert!(!buf.is_empty());
}

// ============================================================================
// Modal Rendering Tests
// ============================================================================

#[test]
fn test_modal_state_text() {
  let modal = ModalState::text("Test Title", "Test Content");

  assert_eq!(modal.title, "Test Title");
  match &modal.content {
    ModalContent::Text(text) => {
      assert_eq!(text, "Test Content");
    }
    _ => panic!("Expected Text content"),
  }
}

#[test]
fn test_modal_state_list() {
  let items = vec!["Item 1".to_string(), "Item 2".to_string()];
  let modal = ModalState::list("List Title", items.clone());

  assert_eq!(modal.title, "List Title");
  match &modal.content {
    ModalContent::List {
      items: list_items, ..
    } => {
      assert_eq!(list_items, &items);
    }
    _ => panic!("Expected List content"),
  }
}

#[test]
fn test_modal_rendering() {
  let mut harness = TestHarness::new();

  // Create a simple modal
  let mut modal = ModalState::text("Test Modal", "This is a test");

  // Render the modal using terminal draw
  harness
    .terminal
    .draw(|frame| {
      let area = frame.area();
      modal.render(frame, area);
    })
    .unwrap();

  // Verify the modal is rendered
  harness.assert_buffer_contains("Test Modal");
  harness.assert_buffer_contains("This is a test");
}

// ============================================================================
// Keyboard Input Tests
// ============================================================================

#[test]
fn test_key_to_bytes_basic_chars() {
  use crossterm::event::{KeyEvent, KeyEventKind};

  let key = KeyEvent {
    code: KeyCode::Char('a'),
    modifiers: KeyModifiers::NONE,
    kind: KeyEventKind::Press,
    state: crossterm::event::KeyEventState::empty(),
  };

  let bytes = key_to_bytes(key);
  assert_eq!(bytes, vec![b'a']);
}

#[test]
fn test_key_to_bytes_enter() {
  use crossterm::event::{KeyEvent, KeyEventKind};

  let key = KeyEvent {
    code: KeyCode::Enter,
    modifiers: KeyModifiers::NONE,
    kind: KeyEventKind::Press,
    state: crossterm::event::KeyEventState::empty(),
  };

  let bytes = key_to_bytes(key);
  assert_eq!(bytes, vec![b'\r']);
}

#[test]
fn test_key_to_bytes_ctrl() {
  use crossterm::event::{KeyEvent, KeyEventKind};

  let key = KeyEvent {
    code: KeyCode::Char('c'),
    modifiers: KeyModifiers::CONTROL,
    kind: KeyEventKind::Press,
    state: crossterm::event::KeyEventState::empty(),
  };

  let bytes = key_to_bytes(key);
  // Ctrl-C is 0x03
  assert_eq!(bytes, vec![0x03]);
}

// ============================================================================
// Integration Tests
// ============================================================================

#[tokio::test]
async fn test_full_mode_cycle() {
  let mut mode_mgr = ModeManager::new();

  // Start in Passthrough
  assert_eq!(mode_mgr.current_mode(), Mode::Passthrough);

  // Guest enters alt buffer
  mode_mgr.set_guest_alt_buffer(true);
  assert_eq!(mode_mgr.current_mode(), Mode::GuestAltBuffer);

  // Show modal (now in ModalGuestAlt)
  mode_mgr.set_modal_visible(true);
  assert_eq!(mode_mgr.current_mode(), Mode::ModalGuestAlt);

  // Hide modal (back to GuestAltBuffer)
  mode_mgr.set_modal_visible(false);
  assert_eq!(mode_mgr.current_mode(), Mode::GuestAltBuffer);

  // Guest leaves alt buffer (back to Passthrough)
  mode_mgr.set_guest_alt_buffer(false);
  assert_eq!(mode_mgr.current_mode(), Mode::Passthrough);
}

#[tokio::test]
async fn test_shadow_terminal_with_router() {
  let mode_mgr = Arc::new(RwLock::new(ModeManager::new()));
  let shadow = Arc::new(RwLock::new(ShadowTerminal::new(
    80,
    24,
    1000,
    TestReplySender,
  )));
  let host = Arc::new(Mutex::new(HostTerminalController::new()));
  let buffer = Arc::new(Mutex::new(OutputBuffer::default()));

  let router = OutputRouter::new(
    Arc::clone(&mode_mgr),
    Arc::clone(&shadow),
    Arc::clone(&host),
    Arc::clone(&buffer),
  );

  // Send output in Passthrough mode
  router.route_output(b"Line 1\r\n").await.unwrap();
  router.route_output(b"Line 2\r\n").await.unwrap();

  // Verify shadow terminal has the content
  let shadow_guard = shadow.read().await;
  let content = shadow_guard.visible_content();

  // Extract text from first two rows
  let row0: String = content.cells[0]
    .iter()
    .map(|cell| cell.symbol().to_string())
    .collect::<Vec<_>>()
    .join("");
  let row1: String = content.cells[1]
    .iter()
    .map(|cell| cell.symbol().to_string())
    .collect::<Vec<_>>()
    .join("");

  assert!(row0.contains("Line 1"));
  assert!(row1.contains("Line 2"));
}

#[tokio::test]
async fn test_buffer_replay_on_modal_close() {
  let mode_mgr = Arc::new(RwLock::new(ModeManager::new()));
  let shadow = Arc::new(RwLock::new(ShadowTerminal::new(
    80,
    24,
    1000,
    TestReplySender,
  )));
  let host = Arc::new(Mutex::new(HostTerminalController::new()));
  let buffer = Arc::new(Mutex::new(OutputBuffer::default()));

  let router = OutputRouter::new(
    Arc::clone(&mode_mgr),
    Arc::clone(&shadow),
    Arc::clone(&host),
    Arc::clone(&buffer),
  );

  // Enter modal mode
  let transition = {
    let mut mgr = mode_mgr.write().await;
    mgr.set_modal_visible(true)
  };
  router.synchronize_host_buffer(&transition).await.unwrap();

  // Send output while in modal (should be buffered)
  router.route_output(b"Buffered output").await.unwrap();

  // Verify buffer is not empty
  {
    let buf = buffer.lock().await;
    assert!(!buf.is_empty());
  }

  // Exit modal mode
  let transition = {
    let mut mgr = mode_mgr.write().await;
    mgr.set_modal_visible(false)
  };
  router.synchronize_host_buffer(&transition).await.unwrap();

  // Buffer should now be empty (replayed)
  {
    let buf = buffer.lock().await;
    assert!(buf.is_empty());
  }
}
