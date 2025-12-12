use rat_event::{HandleEvent, Regular};
use rat_text::text_area::{TextArea, TextAreaState};
use tui::{
  buffer::Buffer,
  layout::{Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Clear, Paragraph, StatefulWidget, Widget, Wrap},
};
use tui_markdown::from_str;

use super::chat_process::{AIChatProcess, MessageRole};
use crate::key::Key;

/// Render the AI chat interface
pub struct AIChatUI<'a> {
  input_state: TextAreaState,
  _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> AIChatUI<'a> {
  pub fn new() -> Self {
    Self {
      input_state: TextAreaState::default(),
      _phantom: std::marker::PhantomData,
    }
  }

  /// Render the full chat UI
  pub fn render(
    &mut self,
    process: &AIChatProcess,
    area: Rect,
    buf: &mut Buffer,
  ) {
    // Clear the entire area first to set background
    Clear.render(area, buf);

    // Determine if we need space for an error message
    let has_error = process.error_message().is_some();

    let chunks = if has_error {
      Layout::default()
        .direction(Direction::Vertical)
        .constraints(
          [
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(3),
          ]
          .as_ref(),
        )
        .split(area)
    } else {
      Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
        .split(area)
    };

    // Render conversation history
    self.render_conversation(process, chunks[0], buf);

    // Render input area
    self.render_input(process, chunks[1], buf);

    // Render error message if present
    if has_error {
      self.render_error(process, chunks[2], buf);
    }

    // Render pending command approval if any
    if let Some(pending) = process.pending_command() {
      self.render_approval_prompt(area, buf, pending);
    }
  }

  fn render_conversation(
    &self,
    process: &AIChatProcess,
    area: Rect,
    buf: &mut Buffer,
  ) {
    let messages: Vec<Line> = process
      .conversation()
      .iter()
      .flat_map(|msg| {
        let (prefix, style) = match msg.role {
          MessageRole::User => (
            "You: ",
            Style::default()
              .fg(Color::Cyan)
              .add_modifier(Modifier::BOLD),
          ),
          MessageRole::Assistant => (
            "AI: ",
            Style::default()
              .fg(Color::Green)
              .add_modifier(Modifier::BOLD),
          ),
          MessageRole::System => (
            "System: ",
            Style::default()
              .fg(Color::Yellow)
              .add_modifier(Modifier::BOLD),
          ),
        };

        let mut lines = Vec::new();

        // Add prefix line
        lines.push(Line::from(Span::styled(prefix, style)));

        // For assistant messages, render markdown; for others, use plain text
        if matches!(msg.role, MessageRole::Assistant) {
          // Use tui-markdown to parse and render markdown
          let md_text = from_str(&msg.content);
          lines.extend(md_text.lines.into_iter().map(Line::from));
        } else {
          // For user and system messages, use plain text
          lines.push(Line::from(Span::raw(&msg.content)));
        }

        // Add empty line between messages
        lines.push(Line::from(""));

        lines
      })
      .collect();

    let paragraph = Paragraph::new(messages)
      .block(
        Block::default()
          .borders(Borders::ALL)
          .title(" AI Assistant (↑↓ to scroll) ")
          .style(Style::default().bg(Color::Black)),
      )
      .wrap(Wrap { trim: false })
      .scroll((process.scroll_offset(), 0));

    paragraph.render(area, buf);
  }

  fn render_input(
    &mut self,
    process: &AIChatProcess,
    area: Rect,
    buf: &mut Buffer,
  ) {
    let title = if process.is_sending() {
      " Sending message... "
    } else {
      " Your Message (Ctrl+Space to toggle, Enter to send) "
    };

    let block = Block::default()
      .borders(Borders::ALL)
      .title(title)
      .style(Style::default().fg(Color::Cyan).bg(Color::Black));

    let widget = TextArea::new().block(block);
    StatefulWidget::render(widget, area, buf, &mut self.input_state);
  }

  fn render_error(
    &self,
    process: &AIChatProcess,
    area: Rect,
    buf: &mut Buffer,
  ) {
    if let Some(error_msg) = process.error_message() {
      let error_text = format!("⚠ Error: {}", error_msg);

      let paragraph = Paragraph::new(error_text)
        .block(
          Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Red).bg(Color::Black)),
        )
        .style(Style::default().fg(Color::Red))
        .wrap(Wrap { trim: false });

      paragraph.render(area, buf);
    }
  }

  fn render_approval_prompt(
    &self,
    area: Rect,
    buf: &mut Buffer,
    pending: &super::chat_process::PendingCommand,
  ) {
    // Create a centered popup for command approval
    let popup_area = centered_rect(60, 40, area);

    // Clear the popup area first
    Clear.render(popup_area, buf);

    // Render the popup border
    let border_block = Block::default().borders(Borders::ALL);
    border_block.render(popup_area, buf);

    let chunks = Layout::default()
      .direction(Direction::Vertical)
      .margin(1)
      .constraints(
        [
          Constraint::Length(3),
          Constraint::Length(3),
          Constraint::Length(2),
        ]
        .as_ref(),
      )
      .split(popup_area);

    // Risk level
    let risk_color = match pending.risk_level {
      crate::command::RiskLevel::Safe => Color::Green,
      crate::command::RiskLevel::Caution => Color::Yellow,
      crate::command::RiskLevel::Dangerous => Color::Red,
    };

    let risk_text = format!("Risk: {:?}", pending.risk_level);
    let risk_para =
      Paragraph::new(risk_text).style(Style::default().fg(risk_color));
    risk_para.render(chunks[0], buf);

    // Command
    let cmd_text = format!("Command: {}", pending.command);
    let cmd_para = Paragraph::new(cmd_text).wrap(Wrap { trim: false });
    cmd_para.render(chunks[1], buf);

    // Approval prompt
    let prompt_text = "Press 'y' to approve, 'n' to reject";
    let prompt_para = Paragraph::new(prompt_text).style(
      Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD),
    );
    prompt_para.render(chunks[2], buf);
  }

  pub fn get_input_value(&self) -> String {
    self.input_state.value().to_string()
  }

  pub fn input_event(&mut self, key: Key) {
    // Convert Key to tui::crossterm event for rat-text
    // Note: Key uses crossterm 0.29, need to create tui::crossterm::event (which is ratatui::crossterm)
    // This is a workaround for the crossterm version mismatch
    use tui::crossterm::event as ct_event;
    let event = ct_event::Event::Key(ct_event::KeyEvent::new(
      // These conversions work because KeyCode/KeyModifiers have same values across versions
      unsafe { std::mem::transmute(key.code()) },
      unsafe { std::mem::transmute(key.mods()) },
    ));
    // Use HandleEvent trait from rat-event
    let _ = self.input_state.handle(&event, Regular);
  }
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
