//! Modal state and rendering
//!
//! This module defines the modal state and rendering logic for overlays
//! that appear on top of the terminal content.

use tui::{
  Frame,
  layout::Rect,
  style::{Color, Modifier, Style},
  widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

/// State for a modal overlay
pub struct ModalState {
  /// Modal title
  pub title: String,

  /// Modal content
  pub content: ModalContent,

  /// Modal style
  pub style: ModalStyle,
}

/// Visual style for the modal
#[derive(Clone)]
pub struct ModalStyle {
  /// Border color
  pub border_color: Color,

  /// Background color
  pub background_color: Color,

  /// Text color
  pub text_color: Color,

  /// Title color
  pub title_color: Color,
}

impl Default for ModalStyle {
  fn default() -> Self {
    Self {
      border_color: Color::Cyan,
      background_color: Color::Black,
      text_color: Color::White,
      title_color: Color::Yellow,
    }
  }
}

/// Content types for modals
pub enum ModalContent {
  /// Plain text content
  Text(String),

  /// List with selection
  List {
    /// List items
    items: Vec<String>,

    /// Currently selected index
    selected: usize,

    /// List state for rendering
    state: ListState,
  },

  /// Custom rendering function
  /// Note: Not fully supported due to trait object limitations
  Custom(String),
}

impl ModalState {
  /// Create a new text modal
  pub fn text(title: impl Into<String>, content: impl Into<String>) -> Self {
    Self {
      title: title.into(),
      content: ModalContent::Text(content.into()),
      style: ModalStyle::default(),
    }
  }

  /// Create a new list modal
  pub fn list(title: impl Into<String>, items: Vec<String>) -> Self {
    let mut state = ListState::default();
    if !items.is_empty() {
      state.select(Some(0));
    }

    Self {
      title: title.into(),
      content: ModalContent::List {
        items,
        selected: 0,
        state,
      },
      style: ModalStyle::default(),
    }
  }

  /// Set the modal style
  pub fn with_style(mut self, style: ModalStyle) -> Self {
    self.style = style;
    self
  }

  /// Render the modal to the given frame and area
  pub fn render(&mut self, frame: &mut Frame, area: Rect) {
    // Calculate centered modal area (80% width, 80% height)
    let modal_width = (area.width * 80 / 100).min(100);
    let modal_height = (area.height * 80 / 100).min(30);

    let modal_area = Rect {
      x: area.x + (area.width - modal_width) / 2,
      y: area.y + (area.height - modal_height) / 2,
      width: modal_width,
      height: modal_height,
    };

    // Render modal block with border
    let modal_block = Block::default()
      .title(self.title.as_str())
      .borders(Borders::ALL)
      .border_style(Style::default().fg(self.style.border_color))
      .style(Style::default().bg(self.style.background_color));

    let inner_area = modal_block.inner(modal_area);
    frame.render_widget(modal_block, modal_area);

    // Render content based on type
    match &mut self.content {
      ModalContent::Text(text) => {
        let paragraph = Paragraph::new(text.as_str())
          .style(Style::default().fg(self.style.text_color));
        frame.render_widget(paragraph, inner_area);
      }

      ModalContent::List { items, state, .. } => {
        let list_items: Vec<ListItem> = items
          .iter()
          .map(|item| ListItem::new(item.as_str()))
          .collect();

        let list = List::new(list_items)
          .highlight_style(
            Style::default()
              .fg(self.style.border_color)
              .add_modifier(Modifier::BOLD),
          )
          .highlight_symbol("> ");

        frame.render_stateful_widget(list, inner_area, state);
      }

      ModalContent::Custom(text) => {
        let paragraph = Paragraph::new(text.as_str())
          .style(Style::default().fg(self.style.text_color));
        frame.render_widget(paragraph, inner_area);
      }
    }
  }

  /// Move selection up (for list modals)
  pub fn select_previous(&mut self) {
    if let ModalContent::List {
      items: _,
      selected,
      state,
    } = &mut self.content
    {
      if *selected > 0 {
        *selected -= 1;
        state.select(Some(*selected));
      }
    }
  }

  /// Move selection down (for list modals)
  pub fn select_next(&mut self) {
    if let ModalContent::List {
      items,
      selected,
      state,
    } = &mut self.content
    {
      if *selected + 1 < items.len() {
        *selected += 1;
        state.select(Some(*selected));
      }
    }
  }

  /// Get the currently selected index (for list modals)
  pub fn selected_index(&self) -> Option<usize> {
    if let ModalContent::List { selected, .. } = &self.content {
      Some(*selected)
    } else {
      None
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_text_modal() {
    let modal = ModalState::text("Test", "Content");
    assert_eq!(modal.title, "Test");
    assert!(matches!(modal.content, ModalContent::Text(_)));
  }

  #[test]
  fn test_list_modal() {
    let items = vec!["Item 1".to_string(), "Item 2".to_string()];
    let modal = ModalState::list("Select", items);
    assert_eq!(modal.title, "Select");
    assert!(matches!(modal.content, ModalContent::List { .. }));
    assert_eq!(modal.selected_index(), Some(0));
  }

  #[test]
  fn test_list_navigation() {
    let items = vec![
      "Item 1".to_string(),
      "Item 2".to_string(),
      "Item 3".to_string(),
    ];
    let mut modal = ModalState::list("Select", items);

    assert_eq!(modal.selected_index(), Some(0));

    modal.select_next();
    assert_eq!(modal.selected_index(), Some(1));

    modal.select_next();
    assert_eq!(modal.selected_index(), Some(2));

    modal.select_next(); // Should stay at 2 (last item)
    assert_eq!(modal.selected_index(), Some(2));

    modal.select_previous();
    assert_eq!(modal.selected_index(), Some(1));

    modal.select_previous();
    assert_eq!(modal.selected_index(), Some(0));

    modal.select_previous(); // Should stay at 0 (first item)
    assert_eq!(modal.selected_index(), Some(0));
  }

  #[test]
  fn test_custom_style() {
    let style = ModalStyle {
      border_color: Color::Red,
      background_color: Color::Blue,
      text_color: Color::Green,
      title_color: Color::Magenta,
    };

    let modal = ModalState::text("Test", "Content").with_style(style);
    assert_eq!(modal.style.border_color, Color::Red);
    assert_eq!(modal.style.background_color, Color::Blue);
  }
}
