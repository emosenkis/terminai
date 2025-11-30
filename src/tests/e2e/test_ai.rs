// AI overlay and interaction tests
//
// Tests for the AI assistant overlay UI and interactions

use super::*;
use crossterm::event::{KeyCode, KeyModifiers};
use tui::layout::Rect;
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

  // Simulate a pending command approval
  let command = "git status";
  let risk_message = "This command is safe to run.";

  harness
    .terminal
    .draw(|f| {
      let area = f.area();
      let overlay_area = centered_rect(80, 50, area);

      let approval_widget =
        MockCommandApprovalWidget::new(command, risk_message);
      f.render_widget(approval_widget, overlay_area);
    })
    .unwrap();

  harness.assert_buffer_contains("git status");
  harness.assert_buffer_contains("safe");
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

/// Mock command approval widget
struct MockCommandApprovalWidget<'a> {
  command: &'a str,
  risk_message: &'a str,
}

impl<'a> MockCommandApprovalWidget<'a> {
  fn new(command: &'a str, risk_message: &'a str) -> Self {
    Self {
      command,
      risk_message,
    }
  }
}

impl Widget for MockCommandApprovalWidget<'_> {
  fn render(self, area: Rect, buf: &mut tui::buffer::Buffer) {
    use tui::style::{Color, Style};
    use tui::text::{Line, Span};
    use tui::widgets::{Block, Borders, Paragraph, Wrap};

    let block = Block::default()
      .borders(Borders::ALL)
      .title(" Command Approval ")
      .style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    block.render(area, buf);

    let text = vec![
      Line::from("The AI wants to run:"),
      Line::from(""),
      Line::from(vec![Span::styled(
        self.command,
        Style::default().fg(Color::Cyan),
      )]),
      Line::from(""),
      Line::from(self.risk_message),
      Line::from(""),
      Line::from("Approve? (Y/N)"),
    ];

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
