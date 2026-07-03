use crate::agent_tools::PendingCommand;
use tui::{
  buffer::Buffer,
  layout::Rect,
  style::{Color, Style},
  widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

const MODAL_MAX_WIDTH: u16 = 80;
const MODAL_HEIGHT: u16 = 12;
const TAB_DISPLAY: &str = "  ";

pub fn approval_modal_area(area: Rect) -> Rect {
  let width = area.width.saturating_sub(4).min(MODAL_MAX_WIDTH).max(1);
  let height = area.height.saturating_sub(2).min(MODAL_HEIGHT).max(1);

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
  let approval_area = approval_modal_area(area);
  Clear.render(approval_area, buf);

  let message = format!(
    "The AI suggests shell input:\n\n{}\n\n{}  Approve? (Y/N)",
    format_shell_input_for_display(&pending.command),
    pending
      .explanation
      .as_deref()
      .unwrap_or("No explanation provided.")
  );

  Paragraph::new(message)
    .block(
      Block::default()
        .borders(Borders::ALL)
        .title(" Shell Input Approval ")
        .style(Style::default().fg(Color::Yellow)),
    )
    .style(Style::default().fg(Color::White))
    .wrap(Wrap { trim: false })
    .render(approval_area, buf);
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
      output.push('\n');
    }
    Some('r') => {
      chars.next();
      output.push('\n');
    }
    Some('t') => {
      chars.next();
      output.push_str(TAB_DISPLAY);
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
    Some('\n') | Some('\r') => output.push('\n'),
    Some('\t') => output.push_str(TAB_DISPLAY),
    _ => {
      output.push_str("\\u");
      output.push_str(&digits);
    }
  }
}

fn append_char_for_display(ch: char, output: &mut String) {
  match ch {
    '\n' | '\r' => output.push('\n'),
    '\t' => output.push_str(TAB_DISPLAY),
    ch if ch.is_control() => {
      output.push_str(&format!("\\u{:04x}", ch as u32));
    }
    ch => output.push(ch),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn formats_literal_whitespace_escapes_as_whitespace() {
    assert_eq!(
      format_shell_input_for_display("printf a\\nb\\r\\tc"),
      "printf a\nb\n  c"
    );
  }

  #[test]
  fn formats_control_characters_as_escaped_unicode() {
    assert_eq!(
      format_shell_input_for_display("cancel \u{3} and esc \u{1b}"),
      "cancel \\u0003 and esc \\u001b"
    );
  }

  #[test]
  fn preserves_literal_non_whitespace_unicode_escapes() {
    assert_eq!(
      format_shell_input_for_display(
        "cancel \\u0003 and esc \\u001b char \\u0061"
      ),
      "cancel \\u0003 and esc \\u001b char \\u0061"
    );
  }
}
