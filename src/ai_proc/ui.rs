use rat_event::{HandleEvent, Regular};
use rat_focus::FocusFlag;
use rat_text::text_area::{TextArea, TextAreaState};
use rat_widget::clipper::{Clipper, ClipperState};
use rat_widget::layout::GenericLayout;
use rat_widget::scrolled::{SCROLLBAR_VERTICAL, Scroll};
use tui::{
  buffer::Buffer,
  layout::{Constraint, Direction, Layout, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{
    Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, StatefulWidget, Widget, Wrap,
  },
};
use tui_markdown::from_str;

use super::chat_process::{AIChatProcess, MessageRole};
use crate::key::Key;

/// Render the AI chat interface
pub struct AIChatUI<'a> {
  input_state: TextAreaState,
  conversation_state: ClipperState<usize>, // One widget per message, indexed by message number
  _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> AIChatUI<'a> {
  pub fn new() -> Self {
    Self {
      input_state: TextAreaState::default(),
      conversation_state: ClipperState::new(),
      _phantom: std::marker::PhantomData,
    }
  }

  /// Get the input widget's focus flag
  pub fn input_focus(&self) -> &FocusFlag {
    &self.input_state.focus
  }

  /// Get the conversation widget's focus flag
  pub fn conversation_focus(&self) -> &FocusFlag {
    &self.conversation_state.container
  }

  /// Get the input state for focus building
  pub fn input_state(&self) -> &TextAreaState {
    &self.input_state
  }

  /// Get the conversation state for focus building and event handling
  pub fn conversation_state(&mut self) -> &mut ClipperState<usize> {
    &mut self.conversation_state
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

    // Layout without error bar - errors now shown as popup
    let chunks = Layout::default()
      .direction(Direction::Vertical)
      .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
      .split(area);

    // Render conversation history
    self.render_conversation(process, chunks[0], buf);

    // Render input area
    self.render_input(process, chunks[1], buf);

    // Render error dialog popup if present (rendered over everything else)
    if process.error_message().is_some() {
      self.render_error_dialog(process, area, buf);
    }

    // Render pending command approval if any (rendered over everything including error)
    if let Some(pending) = process.pending_command() {
      self.render_approval_prompt(area, buf, pending);
    }
  }

  fn render_conversation(
    &mut self,
    process: &AIChatProcess,
    area: Rect,
    buf: &mut Buffer,
  ) {
    // Build layout for clipper if needed
    let messages = process.conversation();
    let has_streaming = process.streaming_response().is_some();
    let total_widgets = messages.len() + if has_streaming { 1 } else { 0 };

    // Check if layout needs rebuild (message count changed)
    let needs_rebuild = {
      let layout = self.conversation_state.layout();
      layout.widget_len() != total_widgets
    };

    if needs_rebuild {
      let mut layout = GenericLayout::new();
      let mut y_offset = 0u16;

      // Add each message as a widget
      for (idx, msg) in messages.iter().enumerate() {
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

        // Calculate height needed for this message
        let width = area.width.saturating_sub(4) as usize; // Account for borders and scrollbar
        let prefix_lines = 1;
        let content_lines = if matches!(msg.role, MessageRole::Assistant) {
          // Markdown rendering
          let md_text = from_str(&msg.content);
          md_text.lines.len()
        } else {
          // Plain text, count wrapped lines
          msg.content.lines().count()
        };
        let separator_lines = 1;
        let height = (prefix_lines + content_lines + separator_lines) as u16;

        // Add widget to layout
        layout.add(
          idx,                                        // widget ID
          Rect::new(0, y_offset, area.width, height), // widget area
          None,                                       // no label
          Rect::default(),                            // no label area
        );

        y_offset += height;
      }

      // Add streaming response widget if present
      if has_streaming {
        let idx = messages.len();
        // Estimate height (will be approximate for streaming content)
        let height = 10; // Default height for streaming widget
        layout.add(
          idx,
          Rect::new(0, y_offset, area.width, height),
          None,
          Rect::default(),
        );
      }

      self.conversation_state.set_layout(layout);
    }

    // Use bright cyan border when focused, dim white when not
    let border_color = if self.conversation_state.container.get() {
      Color::Cyan
    } else {
      Color::DarkGray
    };

    let block = Block::default()
      .borders(Borders::ALL)
      .title(" AI Assistant ")
      .style(Style::default().fg(border_color).bg(Color::Black));

    let scroll = Scroll::new()
      .symbols(&SCROLLBAR_VERTICAL)
      .style(Style::default().fg(Color::DarkGray));

    let clipper = Clipper::new().block(block).vscroll(scroll);

    let mut clip_buf = clipper.into_buffer(area, &mut self.conversation_state);

    // Render each message widget (using render_widget for stateless widgets)
    for (idx, msg) in messages.iter().enumerate() {
      clip_buf.render_widget(idx, || {
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

        let mut lines = vec![Line::from(Span::styled(prefix, style))];

        if matches!(msg.role, MessageRole::Assistant) {
          let md_text = from_str(&msg.content);
          lines.extend(md_text.lines.into_iter().map(Line::from));
        } else {
          lines.push(Line::from(Span::raw(&msg.content)));
        }

        lines.push(Line::from("")); // Separator

        Paragraph::new(lines).wrap(Wrap { trim: false })
      });
    }

    // Render streaming response if present
    if let Some(streaming) = process.streaming_response() {
      let idx = messages.len();
      clip_buf.render_widget(idx, || {
        let prefix = "AI: ";
        let style = Style::default()
          .fg(Color::Green)
          .add_modifier(Modifier::BOLD);

        let mut lines = vec![Line::from(Span::styled(prefix, style))];

        let md_text = from_str(streaming);
        lines.extend(md_text.lines.into_iter().map(Line::from));

        lines.push(Line::from("")); // Separator
        lines.push(Line::from(Span::styled(
          "▌",
          Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        )));

        Paragraph::new(lines).wrap(Wrap { trim: false })
      });
    }

    // Finish rendering and copy to buffer
    clip_buf.finish(buf, &mut self.conversation_state);

    // Auto-scroll to bottom when at bottom (scroll_offset == 0)
    // Clipper uses standard scrolling where offset 0 = top
    // We want to be at the bottom by default
    if messages.len() > 0 || has_streaming {
      let max_offset = self.conversation_state.vscroll.max_offset();
      if self.conversation_state.vscroll.offset() < max_offset {
        // Not at bottom, keep current position
      } else {
        // At bottom, stay at bottom
        self.conversation_state.set_vertical_offset(max_offset);
      }
    }
  }

  fn render_input(
    &mut self,
    process: &AIChatProcess,
    area: Rect,
    buf: &mut Buffer,
  ) {
    let title = if process.is_sending() {
      " Sending message... (please wait) "
    } else {
      " Your Message (Ctrl+Space to toggle, Enter to send) "
    };

    // Use bright cyan border when focused, dim white when not
    let border_color = if self.input_state.focus.get() {
      Color::Cyan
    } else {
      Color::DarkGray
    };

    let block = Block::default()
      .borders(Borders::ALL)
      .title(title)
      .style(Style::default().fg(border_color).bg(Color::Black));

    let widget = TextArea::new().block(block);
    StatefulWidget::render(widget, area, buf, &mut self.input_state);
  }

  fn render_error_dialog(
    &self,
    process: &AIChatProcess,
    area: Rect,
    buf: &mut Buffer,
  ) {
    if let Some(error_msg) = process.error_message() {
      // Create a larger centered popup for error display (70% width, 60% height)
      let popup_area = centered_rect(70, 60, area);

      // Clear the popup area first
      Clear.render(popup_area, buf);

      // Render the popup border with title
      let border_block = Block::default()
        .borders(Borders::ALL)
        .title(" Error ")
        .style(Style::default().fg(Color::Red).bg(Color::Black));

      // Get inner area before rendering (Block doesn't implement Copy)
      let inner = border_block.inner(popup_area);
      border_block.render(popup_area, buf);
      let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
      let content_area = chunks[0];
      let scrollbar_area = chunks[1];

      // Split error message into lines and wrap
      let error_text = format!("⚠  {}", error_msg);
      let lines: Vec<Line> = error_text
        .lines()
        .flat_map(|line| {
          // Simple word wrapping
          let max_width = content_area.width.saturating_sub(2) as usize;
          if line.len() <= max_width {
            vec![Line::from(line.to_string())]
          } else {
            let mut wrapped_lines = Vec::new();
            let words: Vec<&str> = line.split_whitespace().collect();
            let mut current_line = String::new();
            for word in words {
              if current_line.len() + word.len() + 1 > max_width {
                if !current_line.is_empty() {
                  wrapped_lines.push(Line::from(current_line.clone()));
                  current_line.clear();
                }
              }
              if !current_line.is_empty() {
                current_line.push(' ');
              }
              current_line.push_str(word);
            }
            if !current_line.is_empty() {
              wrapped_lines.push(Line::from(current_line));
            }
            wrapped_lines
          }
        })
        .collect();

      // Render the error text with scrolling
      let paragraph = Paragraph::new(lines.clone())
        .style(Style::default().fg(Color::Red))
        .wrap(Wrap { trim: false })
        .scroll((process.error_scroll_offset(), 0));

      paragraph.render(content_area, buf);

      // Render scrollbar
      let content_height = lines.len();
      let view_height = content_area.height as usize;
      let scroll_offset = process.error_scroll_offset() as usize;
      let max_scroll = content_height.saturating_sub(view_height);

      let mut scrollbar_state =
        ScrollbarState::new(max_scroll).position(scroll_offset);
      Scrollbar::new(ScrollbarOrientation::VerticalRight).render(
        scrollbar_area,
        buf,
        &mut scrollbar_state,
      );

      // Render instructions at bottom
      let instructions_area = Rect {
        x: popup_area.x + 2,
        y: popup_area.bottom().saturating_sub(1),
        width: popup_area.width.saturating_sub(4),
        height: 1,
      };
      let instructions = Line::from(vec![
        Span::styled(
          "↑↓ scroll  ",
          Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
          "Esc dismiss",
          Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        ),
      ]);
      Paragraph::new(instructions).render(instructions_area, buf);
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

    // Risk level with emoji and description
    let (risk_emoji, risk_color, risk_description) = match pending.risk_level {
      crate::command::RiskLevel::Safe => {
        ("🟢", Color::Green, "Command appears safe to execute")
      }
      crate::command::RiskLevel::Caution => {
        ("🟡", Color::Yellow, "Review this command carefully")
      }
      crate::command::RiskLevel::Dangerous => {
        ("🔴", Color::Red, "This command could modify/delete data")
      }
    };

    let risk_text = format!(
      "{} {} - {}",
      risk_emoji,
      format!("{:?}", pending.risk_level).to_uppercase(),
      risk_description
    );
    let risk_para = Paragraph::new(risk_text)
      .style(Style::default().fg(risk_color).add_modifier(Modifier::BOLD));
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

  pub fn clear_input(&mut self) {
    self.input_state.clear();
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
