use crossterm::event::{Event, KeyCode, KeyEvent};
use tui::{
  Frame,
  prelude::{Margin, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Clear, Paragraph, Wrap},
};

use crate::{
  app::LoopAction, command::RiskLevel, event::AppEvent,
  kernel::kernel_message::ProcContext, llm::CommandSuggestion, state::State,
  theme::Theme,
};

use super::modal::Modal;

/// Modal dialog for displaying LLM command suggestions
pub struct CommandSuggestionModal {
  pc: ProcContext,
  suggestion: CommandSuggestion,
}

impl CommandSuggestionModal {
  pub fn new(pc: ProcContext, suggestion: CommandSuggestion) -> Self {
    Self { pc, suggestion }
  }

  /// Get color for risk level
  fn risk_color(&self) -> Color {
    match self.suggestion.risk_level {
      RiskLevel::Safe => Color::Green,
      RiskLevel::Caution => Color::Yellow,
      RiskLevel::Dangerous => Color::Red,
    }
  }

  /// Get risk level label
  fn risk_label(&self) -> &str {
    match self.suggestion.risk_level {
      RiskLevel::Safe => "SAFE",
      RiskLevel::Caution => "CAUTION",
      RiskLevel::Dangerous => "DANGEROUS",
    }
  }
}

impl Modal for CommandSuggestionModal {
  fn boxed(self) -> Box<dyn Modal> {
    Box::new(self)
  }

  fn handle_input(
    &mut self,
    _state: &mut State,
    loop_action: &mut LoopAction,
    event: &Event,
  ) -> bool {
    match event {
      Event::Key(KeyEvent {
        code: KeyCode::Enter,
        modifiers,
        ..
      }) if modifiers.is_empty() => {
        // User approved - execute the command
        self.pc.send_self_custom(AppEvent::CloseCurrentModal);
        self.pc.send_self_custom(AppEvent::ExecuteSuggestedCommand {
          command: self.suggestion.command.clone(),
        });
        loop_action.render();
        return true;
      }
      Event::Key(KeyEvent {
        code: KeyCode::Esc,
        modifiers,
        ..
      }) if modifiers.is_empty() => {
        // User dismissed
        self.pc.send_self_custom(AppEvent::CloseCurrentModal);
        loop_action.render();
        return true;
      }
      _ => (),
    }

    match event {
      Event::FocusGained => false,
      Event::FocusLost => false,
      // Block keys
      Event::Key(_) => true,
      // Block mouse
      Event::Mouse(_) => true,
      // Block paste
      Event::Paste(_) => true,
      Event::Resize(_, _) => false,
    }
  }

  fn get_size(&mut self, frame_area: Rect) -> (u16, u16) {
    // Calculate height based on content
    let command_lines = 1 + (self.suggestion.command.len() / 50);
    let explanation_lines = if let Some(ref exp) = self.suggestion.explanation {
      2 + (exp.len() / 50)
    } else {
      0
    };

    let height =
      (8 + command_lines + explanation_lines).min(frame_area.height as usize);
    let width = 70.min(frame_area.width as usize);

    (width as u16, height as u16)
  }

  fn render(&mut self, frame: &mut Frame) {
    let area = self.area(frame.area());
    let theme = Theme::default();

    let block = theme.pane(true);
    frame.render_widget(block, area);

    let inner = area.inner(Margin::new(1, 1));

    // Build content lines
    let mut lines = Vec::new();

    // Title
    lines.push(Line::from(vec![Span::styled(
      "Command Suggestion",
      Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(""));

    // Risk level badge
    lines.push(Line::from(vec![
      Span::raw("Risk: "),
      Span::styled(
        self.risk_label(),
        Style::default()
          .fg(self.risk_color())
          .add_modifier(Modifier::BOLD),
      ),
    ]));
    lines.push(Line::from(""));

    // Command
    lines.push(Line::from(vec![Span::styled(
      "Command:",
      Style::default().add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(vec![Span::styled(
      &self.suggestion.command,
      Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD),
    )]));

    // Explanation (if present)
    if let Some(ref explanation) = self.suggestion.explanation {
      lines.push(Line::from(""));
      lines.push(Line::from(vec![Span::styled(
        "Explanation:",
        Style::default().add_modifier(Modifier::BOLD),
      )]));
      lines.push(Line::from(Span::styled(
        explanation,
        Style::default().fg(Color::Gray),
      )));
    }

    // Instructions
    lines.push(Line::from(""));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
      Span::styled("<Enter>", Style::default().fg(Color::Green)),
      Span::raw(" - Execute    "),
      Span::styled("<Esc>", Style::default().fg(Color::Red)),
      Span::raw(" - Dismiss"),
    ]));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });

    frame.render_widget(Clear, inner);
    frame.render_widget(paragraph, inner);
  }
}
