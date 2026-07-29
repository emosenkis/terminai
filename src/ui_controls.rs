use crate::changelog;
use crate::terminai_config::{ApprovalMode, ChatPosition, GuestDisplayMode};
use tui::{
  buffer::Buffer,
  layout::Rect,
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Clear, Paragraph, Widget},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlPanelItem {
  ApprovalMode,
  Agent,
  ClearHistory,
  Changelog,
  Fullscreen,
  Layout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutPanelItem {
  Height,
  Position,
  GuestDisplay,
  Fullscreen,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlModal {
  Panel {
    selected: usize,
  },
  Layout {
    selected: usize,
  },
  AgentPicker {
    agents: Vec<String>,
    selected: usize,
    error: Option<String>,
  },
  ConfirmAutoApproval {
    confirm: bool,
  },
  ConfirmClearHistory {
    confirm: bool,
  },
  ConfirmAgentSwitch {
    agent: String,
    confirm: bool,
  },
  Changelog {
    scroll: u16,
    lines: Vec<Line<'static>>,
  },
}

impl ControlModal {
  pub fn panel() -> Self {
    Self::Panel { selected: 0 }
  }

  pub fn agent_picker(agents: Vec<String>) -> Self {
    Self::AgentPicker {
      agents,
      selected: 0,
      error: None,
    }
  }

  pub fn layout() -> Self {
    Self::Layout { selected: 0 }
  }

  pub fn confirm_auto_approval() -> Self {
    Self::ConfirmAutoApproval { confirm: false }
  }

  pub fn confirm_clear_history() -> Self {
    Self::ConfirmClearHistory { confirm: false }
  }

  pub fn confirm_agent_switch(agent: String) -> Self {
    Self::ConfirmAgentSwitch {
      agent,
      confirm: false,
    }
  }

  pub fn changelog() -> Self {
    Self::changelog_since(None)
  }

  pub fn changelog_since(acknowledged: Option<&str>) -> Self {
    Self::Changelog {
      scroll: 0,
      lines: changelog::render_since(acknowledged),
    }
  }

  fn item_count(&self) -> usize {
    match self {
      Self::Panel { .. } => 6,
      Self::Layout { .. } => 4,
      Self::AgentPicker { agents, .. } => agents.len(),
      Self::ConfirmAutoApproval { .. }
      | Self::ConfirmClearHistory { .. }
      | Self::ConfirmAgentSwitch { .. } => 2,
      Self::Changelog { .. } => 0,
    }
  }

  pub fn selected(&self) -> usize {
    match self {
      Self::Panel { selected }
      | Self::Layout { selected }
      | Self::AgentPicker { selected, .. } => *selected,
      Self::ConfirmAutoApproval { confirm }
      | Self::ConfirmClearHistory { confirm }
      | Self::ConfirmAgentSwitch { confirm, .. } => usize::from(!*confirm),
      Self::Changelog { scroll, .. } => *scroll as usize,
    }
  }

  pub fn next(&mut self) {
    let count = self.item_count();
    if count == 0 {
      return;
    }
    match self {
      Self::Panel { selected }
      | Self::Layout { selected }
      | Self::AgentPicker { selected, .. } => {
        *selected = (*selected + 1) % count
      }
      Self::ConfirmAutoApproval { confirm }
      | Self::ConfirmClearHistory { confirm }
      | Self::ConfirmAgentSwitch { confirm, .. } => *confirm = !*confirm,
      Self::Changelog { .. } => {}
    }
  }

  pub fn previous(&mut self) {
    let count = self.item_count();
    if count == 0 {
      return;
    }
    match self {
      Self::Panel { selected }
      | Self::Layout { selected }
      | Self::AgentPicker { selected, .. } => {
        *selected = selected.checked_sub(1).unwrap_or(count - 1)
      }
      Self::ConfirmAutoApproval { confirm }
      | Self::ConfirmClearHistory { confirm }
      | Self::ConfirmAgentSwitch { confirm, .. } => *confirm = !*confirm,
      Self::Changelog { .. } => {}
    }
  }

  pub fn is_confirmed(&self) -> bool {
    match self {
      Self::ConfirmAutoApproval { confirm }
      | Self::ConfirmClearHistory { confirm }
      | Self::ConfirmAgentSwitch { confirm, .. } => *confirm,
      _ => false,
    }
  }

  pub fn panel_item(&self) -> Option<ControlPanelItem> {
    let Self::Panel { selected } = self else {
      return None;
    };
    Some(match selected {
      0 => ControlPanelItem::ApprovalMode,
      1 => ControlPanelItem::Agent,
      2 => ControlPanelItem::ClearHistory,
      3 => ControlPanelItem::Changelog,
      4 => ControlPanelItem::Fullscreen,
      _ => ControlPanelItem::Layout,
    })
  }

  pub fn layout_item(&self) -> Option<LayoutPanelItem> {
    let Self::Layout { selected } = self else {
      return None;
    };
    Some(match selected {
      0 => LayoutPanelItem::Height,
      1 => LayoutPanelItem::Position,
      2 => LayoutPanelItem::GuestDisplay,
      _ => LayoutPanelItem::Fullscreen,
    })
  }

  pub fn selected_agent(&self) -> Option<&str> {
    let Self::AgentPicker {
      agents, selected, ..
    } = self
    else {
      return None;
    };
    agents.get(*selected).map(String::as_str)
  }

  pub fn confirmed_agent(&self) -> Option<&str> {
    match self {
      Self::ConfirmAgentSwitch {
        agent,
        confirm: true,
      } => Some(agent),
      _ => None,
    }
  }

  pub fn set_error(&mut self, message: String) {
    if let Self::AgentPicker { error, .. } = self {
      *error = Some(message);
    }
  }

  pub fn scroll_changelog(&mut self, delta: i16, max: u16) {
    if let Self::Changelog { scroll, .. } = self {
      *scroll = if delta.is_negative() {
        scroll.saturating_sub(delta.unsigned_abs())
      } else {
        scroll.saturating_add(delta as u16)
      }
      .min(max);
    }
  }
}

fn modal_area(area: Rect, height: u16) -> Rect {
  let width = area.width.saturating_sub(4).min(58);
  let height = height.min(area.height.saturating_sub(2));
  Rect::new(
    area.x + area.width.saturating_sub(width) / 2,
    area.y + area.height.saturating_sub(height) / 2,
    width,
    height,
  )
}

fn selected_line(text: String, selected: bool) -> Line<'static> {
  Line::from(Span::styled(
    format!("{} {text}", if selected { '›' } else { ' ' }),
    if selected {
      Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
    } else {
      Style::default().fg(Color::White)
    },
  ))
}

pub fn render_control_modal(
  area: Rect,
  buf: &mut Buffer,
  modal: &ControlModal,
  approval_mode: ApprovalMode,
  active_agent: &str,
  chat_position: ChatPosition,
  chat_height_percent: u8,
  guest_display: GuestDisplayMode,
) {
  if let ControlModal::Changelog { scroll, lines } = modal {
    return render_changelog(area, buf, *scroll, lines);
  }

  let (title, mut lines, height) = match modal {
    ControlModal::Panel { selected } => {
      let mode = match approval_mode {
        ApprovalMode::AlwaysAsk => "Always ask",
        ApprovalMode::AutoApproval => "AUTO-APPROVAL (DANGEROUS)",
      };
      (
        " Terminai Controls ",
        vec![
          selected_line(format!("Approval mode: {mode}"), *selected == 0),
          selected_line(format!("Agent: {active_agent}"), *selected == 1),
          selected_line(
            "Clear AI-readable history".to_string(),
            *selected == 2,
          ),
          selected_line("Changelog".to_string(), *selected == 3),
          selected_line(
            format!(
              "Fullscreen: {}",
              if chat_position == ChatPosition::Fullscreen {
                "on"
              } else {
                "off"
              }
            ),
            *selected == 4,
          ),
          selected_line("Layout…".to_string(), *selected == 5),
          Line::from(""),
          Line::from("↑/↓ select  Enter open  Esc close"),
        ],
        11,
      )
    }
    ControlModal::Layout { selected } => (
      " Layout Mode ",
      vec![
        selected_line(
          format!("AI height: {chat_height_percent}%  (-/+)"),
          *selected == 0,
        ),
        selected_line(
          format!("Position: {chat_position:?}  (p)"),
          *selected == 1,
        ),
        selected_line(format!("Guest: {guest_display:?}  (g)"), *selected == 2),
        selected_line(
          format!(
            "Fullscreen: {}  (f)",
            if chat_position == ChatPosition::Fullscreen {
              "on"
            } else {
              "off"
            }
          ),
          *selected == 3,
        ),
        Line::from(""),
        Line::from("↑/↓ select  ←/→ change  Esc done"),
      ],
      10,
    ),
    ControlModal::AgentPicker {
      agents,
      selected,
      error,
    } => {
      let mut lines = agents
        .iter()
        .enumerate()
        .map(|(index, agent)| selected_line(agent.clone(), index == *selected))
        .collect::<Vec<_>>();
      if let Some(error) = error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
          error.clone(),
          Style::default().fg(Color::Red),
        )));
      }
      lines.push(Line::from(""));
      lines.push(Line::from("↑/↓ select  Enter switch  Esc cancel"));
      (" Switch Agent ", lines, (agents.len() as u16 + 7).min(18))
    }
    ControlModal::ConfirmAutoApproval { confirm } => (
      " Enable Auto-Approval? ",
      vec![
        Line::from(Span::styled(
          "DANGER: every AI suggestion will be sent immediately.",
          Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        )),
        Line::from("The command risk classifier will not be consulted."),
        Line::from(""),
        confirmation_line(*confirm),
      ],
      8,
    ),
    ControlModal::ConfirmClearHistory { confirm } => (
      " Clear AI-Readable History? ",
      vec![
        Line::from("Remove Terminai's internal shell scrollback?"),
        Line::from("The visible screen and native terminal history remain."),
        Line::from(""),
        confirmation_line(*confirm),
      ],
      8,
    ),
    ControlModal::ConfirmAgentSwitch { agent, confirm } => (
      " Switch Agent? ",
      vec![
        Line::from(format!("Terminate the current agent and launch {agent}?")),
        Line::from("The current agent conversation will be lost."),
        Line::from(""),
        confirmation_line(*confirm),
      ],
      8,
    ),
    ControlModal::Changelog { .. } => unreachable!(),
  };

  let modal_area = modal_area(area, height);
  Clear.render(modal_area, buf);
  let block = Block::default()
    .borders(Borders::ALL)
    .title(title)
    .style(Style::default().fg(Color::Cyan).bg(Color::Black));
  let inner = block.inner(modal_area);
  block.render(modal_area, buf);
  Paragraph::new(std::mem::take(&mut lines))
    .style(Style::default().bg(Color::Black))
    .render(inner, buf);
}

fn render_changelog(
  area: Rect,
  buf: &mut Buffer,
  scroll: u16,
  lines: &[Line<'static>],
) {
  let modal_area = changelog_area(area);
  let block = Block::default()
    .borders(Borders::ALL)
    .title(" Changelog — ↑/↓ scroll, Esc close ")
    .style(Style::default().fg(Color::Cyan).bg(Color::Black));
  let inner = block.inner(modal_area);
  Clear.render(modal_area, buf);
  block.render(modal_area, buf);
  Paragraph::new(lines.to_vec())
    .style(Style::default().fg(Color::White).bg(Color::Black))
    .wrap(tui::widgets::Wrap { trim: false })
    .scroll((scroll, 0))
    .render(inner, buf);
}

fn changelog_area(area: Rect) -> Rect {
  let width = area.width.min(100);
  let height = 40.min(area.height.saturating_mul(80) / 100).max(1);
  Rect::new(
    area.x + area.width.saturating_sub(width) / 2,
    area.y + area.height.saturating_sub(height) / 2,
    width,
    height,
  )
}

pub fn changelog_max_scroll(area: Rect, modal: &ControlModal) -> u16 {
  let ControlModal::Changelog { lines, .. } = modal else {
    return 0;
  };
  let inner = changelog_area(area).inner(tui::layout::Margin::new(1, 1));
  Paragraph::new(lines.clone())
    .wrap(tui::widgets::Wrap { trim: false })
    .line_count(inner.width)
    .saturating_sub(inner.height as usize)
    .min(u16::MAX as usize) as u16
}

fn confirmation_line(confirm: bool) -> Line<'static> {
  Line::from(vec![
    Span::styled(
      " Confirm ",
      if confirm {
        Style::default().fg(Color::Black).bg(Color::Red)
      } else {
        Style::default().fg(Color::White)
      },
    ),
    Span::raw("  "),
    Span::styled(
      " Cancel ",
      if confirm {
        Style::default().fg(Color::White)
      } else {
        Style::default().fg(Color::Black).bg(Color::Cyan)
      },
    ),
  ])
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::terminai_config::ApprovalMode;
  use tui::{buffer::Buffer, layout::Rect};

  #[test]
  fn panel_and_picker_selection_wrap() {
    let mut panel = ControlModal::panel();
    panel.previous();
    assert_eq!(panel.selected(), 5);
    panel.next();
    assert_eq!(panel.selected(), 0);

    let mut layout = ControlModal::layout();
    layout.previous();
    assert_eq!(layout.layout_item(), Some(LayoutPanelItem::Fullscreen));

    let mut picker = ControlModal::agent_picker(vec!["a".into(), "b".into()]);
    picker.previous();
    assert_eq!(picker.selected(), 1);
  }

  #[test]
  fn dangerous_confirmations_default_to_cancel() {
    for modal in [
      ControlModal::confirm_auto_approval(),
      ControlModal::confirm_clear_history(),
      ControlModal::confirm_agent_switch("claude".into()),
    ] {
      assert!(!modal.is_confirmed());
    }
  }

  #[test]
  fn control_panel_renders_current_runtime_state() {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 60, 16));
    render_control_modal(
      buffer.area,
      &mut buffer,
      &ControlModal::panel(),
      ApprovalMode::AutoApproval,
      "codex",
      ChatPosition::Bottom,
      50,
      GuestDisplayMode::Resize,
    );

    let rendered = buffer
      .content
      .chunks(buffer.area.width as usize)
      .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
      .collect::<Vec<_>>()
      .join("\n");
    assert!(rendered.contains("Terminai Controls"));
    assert!(rendered.contains("AUTO-APPROVAL (DANGEROUS)"));
    assert!(rendered.contains("codex"));
    assert!(rendered.contains("Clear AI-readable history"));
    assert!(rendered.contains("Changelog"));
    assert!(rendered.contains("Fullscreen: off"));
    assert!(rendered.contains("Layout"));
  }
}
