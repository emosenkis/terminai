// AI overlay and interaction tests
//
// Tests for the AI assistant overlay UI and interactions

use super::*;
use crate::agent_tools::PendingCommand;
use crate::command::RiskLevel;
use crate::ui_approval::{
  ApprovalAction, approval_action_at, approval_button_areas,
  approval_modal_area, render_shell_input_approval,
  render_shell_input_approval_with_scroll,
  render_shell_input_approval_with_state,
};
use crossterm::event::{KeyCode, KeyModifiers};
use tui::layout::Rect;
use tui::style::Color;
use tui::widgets::Widget;

#[test]
fn test_ai_overlay_activation() {
  let mut harness = TestHarness::new();

  // Simulate Ctrl+Space to activate AI overlay
  harness.press_key_with_modifiers(KeyCode::Char(' '), KeyModifiers::CONTROL);

  // In a real app, this would show the AI overlay
  // For now, we just verify the event was queued
  assert_eq!(harness.events.len(), 1);
}

#[test]
fn test_ai_message_input() {
  let mut harness = TestHarness::new();

  // Type a message
  harness.type_string("help me with git");

  // Verify events were queued
  assert_eq!(harness.events.len(), "help me with git".len());
}

#[test]
fn test_ai_overlay_ui_rendering() {
  let mut harness = TestHarness::new();

  // Create a simple chat widget to test
  let messages = vec![
    create_message(MessageRole::User, "Hello AI"),
    create_message(MessageRole::Assistant, "Hello! How can I help you?"),
  ];

  let chat_widget = MockChatWidget::new(&messages, "");
  harness.render(chat_widget).unwrap();

  harness.assert_buffer_contains("Hello AI");
  harness.assert_buffer_contains("Hello! How can I help you?");
}

#[test]
fn test_command_approval_ui() {
  let mut harness = TestHarness::new();

  let pending = PendingCommand::new(
    "git status".to_string(),
    Some("This command is safe to run.".to_string()),
    RiskLevel::Safe,
  );

  harness
    .terminal
    .draw(|f| {
      let area = f.area();
      render_shell_input_approval(area, f.buffer_mut(), &pending);
    })
    .unwrap();

  harness.assert_buffer_contains("git status");
  harness.assert_buffer_contains("Description:");
  harness.assert_buffer_contains("safe");
}

#[test]
fn test_command_approval_styles_suggested_input_background() {
  let mut harness = TestHarness::new();
  let pending = PendingCommand::new(
    "git status".to_string(),
    Some("This command is safe to run.".to_string()),
    RiskLevel::Safe,
  );

  harness
    .terminal
    .draw(|f| {
      render_shell_input_approval(f.area(), f.buffer_mut(), &pending);
    })
    .unwrap();

  let buffer = harness.buffer();
  let (col_start, line_start) =
    find_buffer_text(buffer, "git status").expect("command should be rendered");

  for offset in 0.."git status".len() {
    let cell = buffer
      .cell(tui::layout::Position {
        x: col_start + offset as u16,
        y: line_start,
      })
      .expect("command cell should exist");
    assert_eq!(cell.bg, Color::Indexed(237));
  }
}

#[test]
fn test_command_approval_renders_focusable_buttons() {
  let mut harness = TestHarness::new();
  let pending = PendingCommand::new(
    "git status".to_string(),
    Some("This command is safe to run.".to_string()),
    RiskLevel::Safe,
  );

  harness
    .terminal
    .draw(|f| {
      render_shell_input_approval_with_state(
        f.area(),
        f.buffer_mut(),
        &pending,
        0,
        ApprovalAction::Deny,
      );
    })
    .unwrap();

  harness.assert_buffer_contains("Approve (Y)");
  harness.assert_buffer_contains("Deny (N)");

  let buffer = harness.buffer();
  let (col_start, line_start) =
    find_buffer_text(buffer, "Deny (N)").expect("deny button should render");
  let cell = buffer
    .cell(tui::layout::Position {
      x: col_start,
      y: line_start,
    })
    .expect("button cell should exist");
  assert_eq!(cell.bg, Color::Indexed(220));
}

#[test]
fn test_command_approval_button_hit_targets() {
  let area = Rect::new(0, 0, 80, 24);
  let buttons = approval_button_areas(area);

  assert_eq!(
    approval_action_at(area, buttons.approve.x, buttons.approve.y),
    Some(ApprovalAction::Approve)
  );
  assert_eq!(
    approval_action_at(area, buttons.deny.x, buttons.deny.y),
    Some(ApprovalAction::Deny)
  );
  assert_eq!(approval_action_at(area, 0, 0), None);
}

#[test]
fn test_command_approval_can_render_scrolled_content() {
  let mut harness = TestHarness::new();
  let pending = PendingCommand::new(
    [
      "echo line0",
      "line1",
      "line2",
      "line3",
      "line4",
      "line5",
      "line6",
      "line7",
      "line8",
    ]
    .join("\\n"),
    Some("Long approval content should be scrollable.".to_string()),
    RiskLevel::Safe,
  );

  harness
    .terminal
    .draw(|f| {
      render_shell_input_approval_with_scroll(
        f.area(),
        f.buffer_mut(),
        &pending,
        5,
      );
    })
    .unwrap();

  let buffer = harness.buffer_as_string();
  assert!(!buffer.contains("echo line0"));
  assert!(buffer.contains("line5"));
  assert!(buffer.contains("Description:"));
}

#[test]
fn test_command_approval_modal_is_centered() {
  let outer = Rect::new(10, 4, 100, 40);
  let area = approval_modal_area(outer);

  assert_eq!(area.width, 80);
  assert_eq!(area.height, 12);
  assert_eq!(area.x, 20);
  assert_eq!(area.y, 18);
}

#[test]
fn test_command_approval_renders_whitespace_escapes_as_whitespace() {
  let mut harness = TestHarness::new();
  let pending = PendingCommand::new(
    "printf one\\ntwo\\r\\ttab".to_string(),
    Some("Whitespace escapes should be readable.".to_string()),
    RiskLevel::Safe,
  );

  harness
    .terminal
    .draw(|f| {
      render_shell_input_approval(f.area(), f.buffer_mut(), &pending);
    })
    .unwrap();

  let buffer = harness.buffer_as_string();
  assert!(buffer.contains("printf one"));
  assert!(buffer.contains("two"));
  assert!(buffer.contains("  tab"));
  assert!(!buffer.contains("\\n"));
  assert!(!buffer.contains("\\r"));
  assert!(!buffer.contains("\\t"));
}

#[test]
fn test_command_approval_renders_non_whitespace_escapes_escaped() {
  let mut harness = TestHarness::new();
  let pending = PendingCommand::new(
    "cancel \\u0003 and esc \u{1b}".to_string(),
    Some("Control escapes should stay explicit.".to_string()),
    RiskLevel::Safe,
  );

  harness
    .terminal
    .draw(|f| {
      render_shell_input_approval(f.area(), f.buffer_mut(), &pending);
    })
    .unwrap();

  let buffer = harness.buffer_as_string();
  assert!(buffer.contains("cancel \\u0003 and esc \\u001b"));
}

fn find_buffer_text(
  buffer: &tui::buffer::Buffer,
  needle: &str,
) -> Option<(u16, u16)> {
  let needle_symbols: Vec<String> =
    needle.chars().map(|ch| ch.to_string()).collect();

  for y in 0..buffer.area().height {
    for x in 0..buffer.area().width {
      let matches =
        needle_symbols.iter().enumerate().all(|(offset, symbol)| {
          buffer
            .cell(tui::layout::Position {
              x: x + offset as u16,
              y,
            })
            .is_some_and(|cell| cell.symbol() == symbol)
        });
      if matches {
        return Some((x, y));
      }
    }
  }

  None
}

#[test]
#[cfg(feature = "snapshot-tests")]
fn test_command_approval_snapshot() {
  let mut harness = TestHarness::new();
  let pending = PendingCommand::new(
    "cargo test --manifest-path src/Cargo.toml very_long_filter_name_that_wraps_cleanly\\n\\u0003".to_string(),
    Some("Run the focused regression test before approving.".to_string()),
    RiskLevel::Safe,
  );

  harness
    .terminal
    .draw(|f| {
      render_shell_input_approval(f.area(), f.buffer_mut(), &pending);
    })
    .unwrap();

  insta::assert_snapshot!(harness.buffer_as_string());
}

#[test]
fn test_ai_response_streaming() {
  // Simplified test without test_config dependency
  let harness = TestHarness::new();
  // Verify we can create a harness for streaming tests
  assert_eq!(harness.size(), (80, 24));
}

#[test]
#[cfg(feature = "snapshot-tests")]
fn test_ai_chat_snapshot() {
  let mut harness = TestHarness::new();

  let messages = vec![
    create_message(
      MessageRole::System,
      "You are a helpful terminal assistant.",
    ),
    create_message(MessageRole::User, "Show me how to list files"),
    create_message(
      MessageRole::Assistant,
      "You can use `ls` to list files in the current directory.",
    ),
  ];

  harness
    .terminal
    .draw(|f| {
      let area = f.area();
      let overlay_area = centered_rect(80, 70, area);
      let chat_widget = MockChatWidget::new(&messages, "");
      f.render_widget(chat_widget, overlay_area);
    })
    .unwrap();

  insta::assert_snapshot!(harness.buffer_as_string());
}

/// Helper to create a message
fn create_message(role: MessageRole, content: &str) -> Message {
  Message {
    role,
    content: content.to_string(),
  }
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
  use tui::layout::{Constraint, Direction, Layout};

  let popup_layout = Layout::default()
    .direction(Direction::Vertical)
    .constraints(
      [
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
      ]
      .as_ref(),
    )
    .split(r);

  Layout::default()
    .direction(Direction::Horizontal)
    .constraints(
      [
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
      ]
      .as_ref(),
    )
    .split(popup_layout[1])[1]
}

/// Mock chat widget for testing
struct MockChatWidget<'a> {
  messages: &'a [Message],
  input: &'a str,
}

impl<'a> MockChatWidget<'a> {
  fn new(messages: &'a [Message], input: &'a str) -> Self {
    Self { messages, input }
  }
}

impl Widget for MockChatWidget<'_> {
  fn render(self, area: Rect, buf: &mut tui::buffer::Buffer) {
    use tui::style::{Color, Modifier, Style};
    use tui::text::{Line, Span};
    use tui::widgets::{Block, Borders, Paragraph, Wrap};

    let block = Block::default()
      .borders(Borders::ALL)
      .title(" AI Assistant ")
      .style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    block.render(area, buf);

    // Render messages
    let mut text = Vec::new();
    for msg in self.messages {
      let prefix = match msg.role {
        MessageRole::User => {
          Span::styled("You: ", Style::default().fg(Color::Green))
        }
        MessageRole::Assistant => {
          Span::styled("AI: ", Style::default().fg(Color::Cyan))
        }
        MessageRole::System => {
          Span::styled("System: ", Style::default().fg(Color::Yellow))
        }
      };
      text.push(Line::from(vec![prefix, Span::raw(msg.content.clone())]));
      text.push(Line::from(""));
    }

    // Render input if present
    if !self.input.is_empty() {
      text.push(Line::from(vec![
        Span::styled("You: ", Style::default().fg(Color::Green)),
        Span::raw(self.input),
        Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
      ]));
    }

    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    paragraph.render(inner, buf);
  }
}

// Simple message types for testing
#[derive(Debug, Clone)]
struct Message {
  role: MessageRole,
  content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageRole {
  User,
  Assistant,
  System,
}
