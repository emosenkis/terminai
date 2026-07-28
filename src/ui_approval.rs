use crate::agent_tools::PendingCommand;
use tui::{
  buffer::Buffer,
  layout::{Margin, Rect},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{
    Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, StatefulWidget, Widget, Wrap,
  },
};

const SUGGESTED_INPUT_BG: Color = Color::Indexed(237);
const CONTROL_INPUT_FG: Color = Color::Cyan;
const BUTTON_BG: Color = Color::Indexed(238);
const BUTTON_FOCUSED_BG: Color = Color::Indexed(220);
const APPROVE_LABEL: &str = " Approve (Y) ";
const DENY_LABEL: &str = " Deny (N) ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalAction {
  Approve,
  Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApprovalButtonAreas {
  pub approve: Rect,
  pub deny: Rect,
}

pub fn approval_modal_area(area: Rect) -> Rect {
  let width = area.width.saturating_sub(4).max(1);
  let height = area.height.saturating_sub(2).max(1);

  Rect {
    x: area.x + area.width.saturating_sub(width) / 2,
    y: area.y + area.height.saturating_sub(height) / 2,
    width,
    height,
  }
}

pub fn render_shell_input_approval(
  area: Rect,
  buf: &mut Buffer,
  pending: &PendingCommand,
) {
  render_shell_input_approval_with_state(
    area,
    buf,
    pending,
    0,
    ApprovalAction::Approve,
  );
}

pub fn render_shell_input_approval_with_scroll(
  area: Rect,
  buf: &mut Buffer,
  pending: &PendingCommand,
  scroll: usize,
) {
  render_shell_input_approval_with_state(
    area,
    buf,
    pending,
    scroll,
    ApprovalAction::Approve,
  );
}

pub fn render_shell_input_approval_with_state(
  area: Rect,
  buf: &mut Buffer,
  pending: &PendingCommand,
  scroll: usize,
  focus: ApprovalAction,
) {
  let approval_area = approval_modal_area(area);
  Clear.render(approval_area, buf);

  let block = Block::default()
    .borders(Borders::ALL)
    .title(" Shell Input Approval ")
    .style(Style::default().fg(Color::Yellow));
  let inner = block.inner(approval_area);
  block.render(approval_area, buf);

  let lines = approval_lines(pending);
  let viewport_rows = approval_viewport_height(area);
  let scroll = scroll.min(max_approval_scroll(lines.len(), viewport_rows));
  let content_area = approval_content_area(area);

  Paragraph::new(lines.clone())
    .style(Style::default().fg(Color::White))
    .wrap(Wrap { trim: false })
    .scroll((scroll as u16, 0))
    .render(content_area, buf);

  if lines.len() > viewport_rows {
    let mut scrollbar_state = ScrollbarState::new(lines.len())
      .position(scroll)
      .viewport_content_length(viewport_rows);
    Scrollbar::new(ScrollbarOrientation::VerticalRight).render(
      content_area.inner(Margin {
        vertical: 0,
        horizontal: 0,
      }),
      buf,
      &mut scrollbar_state,
    );
  }

  if inner.height > 0 {
    let buttons = approval_button_areas(area);
    Paragraph::new(APPROVE_LABEL)
      .style(button_style(focus == ApprovalAction::Approve))
      .render(buttons.approve, buf);
    Paragraph::new(DENY_LABEL)
      .style(button_style(focus == ApprovalAction::Deny))
      .render(buttons.deny, buf);
  }
}

pub fn approval_content_line_count(pending: &PendingCommand) -> usize {
  approval_lines(pending).len()
}

pub fn approval_viewport_height(area: Rect) -> usize {
  approval_content_area(area).height as usize
}

pub fn max_approval_scroll(
  content_lines: usize,
  viewport_rows: usize,
) -> usize {
  content_lines.saturating_sub(viewport_rows.max(1))
}

fn approval_lines(pending: &PendingCommand) -> Vec<Line<'static>> {
  let mut lines =
    vec![Line::from("The AI suggests shell input:"), Line::from("")];
  lines.extend(suggested_input_lines(&pending.command));
  lines.extend([
    Line::from(""),
    Line::from("Description:"),
    Line::from(
      pending
        .explanation
        .as_deref()
        .unwrap_or("No explanation provided.")
        .to_string(),
    ),
  ]);
  lines
}

pub fn approval_button_areas(area: Rect) -> ApprovalButtonAreas {
  let approval_area = approval_modal_area(area);
  let inner = Rect {
    x: approval_area.x.saturating_add(1),
    y: approval_area.y.saturating_add(1),
    width: approval_area.width.saturating_sub(2),
    height: approval_area.height.saturating_sub(2),
  };
  let approve_width = APPROVE_LABEL.len() as u16;
  let deny_width = DENY_LABEL.len() as u16;
  let gap = 2;
  let total_width =
    approve_width.saturating_add(gap).saturating_add(deny_width);
  let start_x = inner.x + inner.width.saturating_sub(total_width) / 2;
  let y = inner.y + inner.height.saturating_sub(1);

  ApprovalButtonAreas {
    approve: Rect {
      x: start_x,
      y,
      width: approve_width.min(inner.width),
      height: 1,
    },
    deny: Rect {
      x: start_x.saturating_add(approve_width).saturating_add(gap),
      y,
      width: deny_width.min(inner.width),
      height: 1,
    },
  }
}

pub fn approval_action_at(
  area: Rect,
  x: u16,
  y: u16,
) -> Option<ApprovalAction> {
  let buttons = approval_button_areas(area);
  if point_in_rect(x, y, buttons.approve) {
    Some(ApprovalAction::Approve)
  } else if point_in_rect(x, y, buttons.deny) {
    Some(ApprovalAction::Deny)
  } else {
    None
  }
}

fn approval_content_area(area: Rect) -> Rect {
  let approval_area = approval_modal_area(area);
  let inner = Rect {
    x: approval_area.x.saturating_add(1),
    y: approval_area.y.saturating_add(1),
    width: approval_area.width.saturating_sub(2),
    height: approval_area.height.saturating_sub(2),
  };

  Rect {
    height: inner.height.saturating_sub(2),
    ..inner
  }
}

fn button_style(focused: bool) -> Style {
  if focused {
    Style::default().fg(Color::Black).bg(BUTTON_FOCUSED_BG)
  } else {
    Style::default().fg(Color::White).bg(BUTTON_BG)
  }
}

fn point_in_rect(x: u16, y: u16, area: Rect) -> bool {
  x >= area.x
    && x < area.x.saturating_add(area.width)
    && y >= area.y
    && y < area.y.saturating_add(area.height)
}

fn suggested_input_lines(input: &str) -> Vec<Line<'static>> {
  let command = format_shell_input_for_display(input);
  command.split('\n').map(styled_input_line).collect()
}

fn styled_input_line(line: &str) -> Line<'static> {
  let base = Style::default().bg(SUGGESTED_INPUT_BG);
  let control = base.fg(CONTROL_INPUT_FG).add_modifier(Modifier::BOLD);
  let mut spans = Vec::new();
  let mut rest = line;

  while let Some(start) = rest.find('<') {
    let (plain, tail) = rest.split_at(start);
    spans.push(Span::styled(plain.to_string(), base));
    if let Some(end) = tail.find('>') {
      let (token, after) = tail.split_at(end + 1);
      spans.push(Span::styled(
        token.to_string(),
        if is_control_token(token) {
          control
        } else {
          base
        },
      ));
      rest = after;
    } else {
      rest = tail;
      break;
    }
  }
  spans.push(Span::styled(rest.to_string(), base));
  Line::from(spans)
}

fn is_control_token(token: &str) -> bool {
  matches!(token, "<Nul>" | "<Enter>" | "<Tab>" | "<BS>" | "<Esc>")
    || (token.starts_with("<C-") && token.chars().count() == 5)
    || (token.starts_with("<U+") && token.ends_with('>'))
}

pub fn format_shell_input_for_display(input: &str) -> String {
  let mut chars = input.chars().peekable();
  let mut output = String::new();

  while let Some(ch) = chars.next() {
    if ch == '\\' {
      append_backslash_escape(&mut chars, &mut output);
    } else {
      append_char_for_display(ch, &mut output);
    }
  }

  output
}

fn append_backslash_escape<I>(
  chars: &mut std::iter::Peekable<I>,
  output: &mut String,
) where
  I: Iterator<Item = char>,
{
  match chars.peek().copied() {
    Some('n') => {
      chars.next();
      output.push_str("<Enter>\n");
    }
    Some('r') => {
      chars.next();
      output.push_str("<Enter>\n");
    }
    Some('t') => {
      chars.next();
      output.push_str("<Tab>");
    }
    Some('b') => {
      chars.next();
      output.push_str("<BS>");
    }
    Some('u') => {
      chars.next();
      append_unicode_escape(chars, output);
    }
    Some(other) => {
      chars.next();
      output.push('\\');
      output.push(other);
    }
    None => output.push('\\'),
  }
}

fn append_unicode_escape<I>(
  chars: &mut std::iter::Peekable<I>,
  output: &mut String,
) where
  I: Iterator<Item = char>,
{
  let mut digits = String::new();
  for _ in 0..4 {
    match chars.peek().copied() {
      Some(ch) if ch.is_ascii_hexdigit() => {
        digits.push(ch.to_ascii_lowercase());
        chars.next();
      }
      _ => {
        output.push_str("\\u");
        output.push_str(&digits);
        return;
      }
    }
  }

  match u32::from_str_radix(&digits, 16)
    .ok()
    .and_then(char::from_u32)
  {
    Some(ch) if ch.is_control() => append_control_key(ch, output),
    _ => {
      output.push_str("\\u");
      output.push_str(&digits);
    }
  }
}

fn append_char_for_display(ch: char, output: &mut String) {
  if ch.is_control() {
    append_control_key(ch, output);
  } else {
    output.push(ch);
  }
}

fn append_control_key(ch: char, output: &mut String) {
  output.push_str(&control_key_name(ch));
  if matches!(ch, '\r' | '\n') {
    output.push('\n');
  }
}

fn control_key_name(ch: char) -> String {
  match ch {
    '\0' => "<Nul>".into(),
    '\r' | '\n' => "<Enter>".into(),
    '\t' => "<Tab>".into(),
    '\u{8}' | '\u{7f}' => "<BS>".into(),
    '\u{1b}' => "<Esc>".into(),
    '\u{1}'..='\u{1a}' => {
      format!("<C-{}>", (b'a' + ch as u8 - 1) as char)
    }
    _ => format!("<U+{:04X}>", ch as u32),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn formats_literal_whitespace_escapes_as_whitespace() {
    assert_eq!(
      format_shell_input_for_display("printf a\\nb\\r\\tc"),
      "printf a<Enter>\nb<Enter>\n<Tab>c"
    );
  }

  #[test]
  fn formats_control_characters_as_shortcut_keys() {
    assert_eq!(
      format_shell_input_for_display("cancel \u{3} and esc \u{1b}"),
      "cancel <C-c> and esc <Esc>"
    );
  }

  #[test]
  fn formats_control_unicode_escapes_as_shortcut_keys() {
    assert_eq!(
      format_shell_input_for_display(
        "cancel \\u0003 and esc \\u001b char \\u0061"
      ),
      "cancel <C-c> and esc <Esc> char \\u0061"
    );
  }
}
