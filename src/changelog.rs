use tui::{
  style::{Color, Modifier, Style},
  text::{Line, Span},
};

const SOURCE: &str = include_str!("../CHANGELOG.md");

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Version([u64; 3]);

impl Version {
  fn parse(value: &str) -> Option<Self> {
    let mut parts = value.trim().trim_start_matches('v').split('.');
    let version = Self([
      parts.next()?.parse().ok()?,
      parts.next()?.parse().ok()?,
      parts.next()?.parse().ok()?,
    ]);
    parts.next().is_none().then_some(version)
  }
}

#[derive(Debug)]
struct Release<'a> {
  version: Version,
  version_text: &'a str,
  lines: Vec<Line<'a>>,
}

pub fn version_is_newer(current: &str, acknowledged: Option<&str>) -> bool {
  let Some(current) = Version::parse(current) else {
    return false;
  };
  acknowledged
    .and_then(Version::parse)
    .is_none_or(|acknowledged| current > acknowledged)
}

pub fn render_since(acknowledged: Option<&str>) -> Vec<Line<'static>> {
  let acknowledged = acknowledged.and_then(Version::parse);
  parse(SOURCE)
    .expect("CHANGELOG.md must use the supported changelog syntax")
    .into_iter()
    .filter(|release| {
      acknowledged.is_none_or(|acknowledged| release.version > acknowledged)
    })
    .flat_map(|release| release.lines)
    .collect()
}

fn parse(input: &str) -> Result<Vec<Release<'_>>, String> {
  let mut releases: Vec<Release<'_>> = Vec::new();
  let mut continuing_item = false;

  for (index, line) in input.lines().enumerate() {
    let line_number = index + 1;
    if let Some(heading) = line.strip_prefix("## ") {
      let Some((version_text, date)) = heading.split_once(" - ") else {
        return Err(format!(
          "line {line_number}: expected `## VERSION - YYYY-MM-DD`"
        ));
      };
      let Some(version) = Version::parse(version_text) else {
        return Err(format!("line {line_number}: invalid version"));
      };
      if !valid_date(date) {
        return Err(format!("line {line_number}: invalid date"));
      }
      releases.push(Release {
        version,
        version_text,
        lines: vec![Line::from(vec![
          Span::styled(
            version_text,
            Style::default()
              .fg(Color::Cyan)
              .add_modifier(Modifier::BOLD),
          ),
          Span::styled(
            format!(" — {date}"),
            Style::default().fg(Color::DarkGray),
          ),
        ])],
      });
      continuing_item = false;
      continue;
    }

    let Some(release) = releases.last_mut() else {
      return Err(format!("line {line_number}: expected a release heading"));
    };
    let rendered = if line.is_empty() {
      continuing_item = false;
      Line::default()
    } else if let Some(text) = line.strip_prefix("- ") {
      continuing_item = true;
      let mut spans =
        vec![Span::styled("• ", Style::default().fg(Color::Cyan))];
      spans.extend(parse_inline(text, line_number)?);
      Line::from(spans)
    } else if let Some(text) = line.strip_prefix("  ") {
      if !continuing_item {
        return Err(format!(
          "line {line_number}: continuation must follow a list item"
        ));
      }
      let mut spans = vec![Span::raw("  ")];
      spans.extend(parse_inline(text, line_number)?);
      Line::from(spans)
    } else if line.starts_with(char::is_whitespace)
      || line.starts_with('#')
      || line.starts_with("---")
      || is_ordered_list_item(line)
    {
      return Err(format!("line {line_number}: unsupported block syntax"));
    } else {
      continuing_item = false;
      Line::from(parse_inline(line, line_number)?)
    };
    release.lines.push(rendered);
  }

  if releases.is_empty() {
    return Err("expected at least one release".to_string());
  }
  if releases
    .windows(2)
    .any(|pair| pair[0].version <= pair[1].version)
  {
    return Err("release headings must be newest first".to_string());
  }
  Ok(releases)
}

fn valid_date(date: &str) -> bool {
  date.len() == 10
    && date.bytes().enumerate().all(|(index, byte)| {
      if matches!(index, 4 | 7) {
        byte == b'-'
      } else {
        byte.is_ascii_digit()
      }
    })
}

fn is_ordered_list_item(line: &str) -> bool {
  let marker = line
    .trim_start_matches(|ch: char| ch.is_ascii_digit())
    .as_bytes();
  marker.len() < line.len()
    && (marker.starts_with(b". ") || marker.starts_with(b") "))
}

fn parse_inline(
  input: &str,
  line_number: usize,
) -> Result<Vec<Span<'_>>, String> {
  let mut spans = Vec::new();
  let mut rest = input;
  while let Some((text, after_tick)) = rest.split_once('`') {
    validate_plain(text, line_number)?;
    if !text.is_empty() {
      spans.push(Span::raw(text));
    }
    let Some((code, after_code)) = after_tick.split_once('`') else {
      return Err(format!("line {line_number}: unclosed inline code"));
    };
    if code.is_empty() {
      return Err(format!("line {line_number}: inline code cannot be empty"));
    }
    spans.push(Span::styled(code, Style::default().fg(Color::Yellow)));
    rest = after_code;
  }
  validate_plain(rest, line_number)?;
  if !rest.is_empty() {
    spans.push(Span::raw(rest));
  }
  Ok(spans)
}

fn validate_plain(input: &str, line_number: usize) -> Result<(), String> {
  if input.chars().any(|ch| {
    matches!(
      ch,
      '*' | '_' | '[' | ']' | '<' | '>' | '#' | '!' | '\\' | '|' | '~'
    )
  }) {
    return Err(format!(
      "line {line_number}: unsupported inline Markdown syntax"
    ));
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn embedded_changelog_is_valid() {
    let changelog = parse(SOURCE).unwrap();
    assert_eq!(changelog[0].version_text, env!("CARGO_PKG_VERSION"));

    let newest = changelog[0].version_text;
    let previous = changelog[1].version_text;
    let rendered = render_since(Some(previous));
    let text = rendered
      .iter()
      .map(|line| line.to_string())
      .collect::<Vec<_>>()
      .join("\n");
    assert!(text.contains(newest));
    assert!(!text.contains(previous));
  }
}
