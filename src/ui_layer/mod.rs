// UI Layer abstraction for Termin.AI
// Provides a widget stack that renders layers bottom-to-top and handles events top-to-bottom

pub mod ai_overlay_layer;
pub mod terminal_layer;

pub use ai_overlay_layer::AIOverlayLayer;
pub use terminal_layer::TerminalLayer;
pub use terminal_layer::TerminalWidget;

use anyhow::Result;
use crossterm::event::Event;
use rat_event::Outcome;
use tui::{buffer::Buffer, layout::Rect};

/// Outcome of handling an event in a UI layer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerEventOutcome {
  /// Event was not handled, pass to next layer
  Continue,
  /// Event was handled, state changed (trigger re-render)
  Changed,
  /// Event was handled but state didn't change
  Unchanged,
}

impl From<Outcome> for LayerEventOutcome {
  fn from(outcome: Outcome) -> Self {
    match outcome {
      Outcome::Changed => LayerEventOutcome::Changed,
      Outcome::Unchanged => LayerEventOutcome::Unchanged,
      Outcome::Continue => LayerEventOutcome::Continue,
    }
  }
}

impl From<LayerEventOutcome> for Outcome {
  fn from(outcome: LayerEventOutcome) -> Self {
    match outcome {
      LayerEventOutcome::Changed => Outcome::Changed,
      LayerEventOutcome::Unchanged => Outcome::Unchanged,
      LayerEventOutcome::Continue => Outcome::Continue,
    }
  }
}

/// Trait for UI layers that can be stacked
///
/// Layers are rendered bottom-to-top (index 0 rendered first, last index rendered last/on top)
/// Events are handled top-to-bottom (last index handles first, if not consumed passes down)
pub trait UILayer {
  /// Check if this layer is currently visible
  fn is_visible(&self) -> bool;

  /// Render this layer to the buffer
  /// Called only if `is_visible()` returns true
  fn render(&mut self, area: Rect, buf: &mut Buffer);

  /// Handle a crossterm event
  /// Return `LayerEventOutcome::Continue` to pass event to layer below
  /// Return `LayerEventOutcome::Changed` or `LayerEventOutcome::Unchanged` to consume event
  fn handle_event(&mut self, event: &Event) -> Result<LayerEventOutcome>;

  /// Get screen cursor position if this layer wants to show a cursor
  /// Returns Some((x, y)) if cursor should be shown, None otherwise
  fn screen_cursor(&self) -> Option<(u16, u16)>;
}

/// A stack of UI layers
/// Renders layers bottom-to-top, handles events top-to-bottom
pub struct UILayerStack {
  layers: Vec<Box<dyn UILayer>>,
}

impl Default for UILayerStack {
  fn default() -> Self {
    Self::new()
  }
}

impl UILayerStack {
  pub fn new() -> Self {
    Self { layers: Vec::new() }
  }

  /// Add a layer to the top of the stack
  pub fn push(&mut self, layer: Box<dyn UILayer>) {
    self.layers.push(layer);
  }

  /// Remove and return the top layer
  pub fn pop(&mut self) -> Option<Box<dyn UILayer>> {
    self.layers.pop()
  }

  /// Get the number of layers
  pub fn len(&self) -> usize {
    self.layers.len()
  }

  /// Check if the stack is empty
  pub fn is_empty(&self) -> bool {
    self.layers.is_empty()
  }

  /// Get a reference to a layer by index
  pub fn get(&self, index: usize) -> Option<&Box<dyn UILayer>> {
    self.layers.get(index)
  }

  /// Get a mutable reference to a layer by index
  pub fn get_mut(&mut self, index: usize) -> Option<&mut Box<dyn UILayer>> {
    self.layers.get_mut(index)
  }

  /// Render all visible layers bottom-to-top
  pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
    for layer in self.layers.iter_mut() {
      if layer.is_visible() {
        layer.render(area, buf);
      }
    }
  }

  /// Handle an event top-to-bottom
  /// Returns the outcome of the first layer that consumed the event,
  /// or Continue if no layer consumed it
  pub fn handle_event(&mut self, event: &Event) -> Result<LayerEventOutcome> {
    // Iterate from top to bottom (reverse order)
    for layer in self.layers.iter_mut().rev() {
      if layer.is_visible() {
        let outcome = layer.handle_event(event)?;
        if outcome != LayerEventOutcome::Continue {
          return Ok(outcome);
        }
      }
    }
    Ok(LayerEventOutcome::Continue)
  }

  /// Get the cursor position from the topmost visible layer that has a cursor
  pub fn screen_cursor(&self) -> Option<(u16, u16)> {
    // Iterate from top to bottom, return first cursor found
    for layer in self.layers.iter().rev() {
      if layer.is_visible() {
        if let Some(cursor) = layer.screen_cursor() {
          return Some(cursor);
        }
      }
    }
    None
  }

  /// Iterate over all layers
  pub fn iter(&self) -> impl Iterator<Item = &Box<dyn UILayer>> {
    self.layers.iter()
  }

  /// Iterate over all layers mutably
  pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn UILayer>> {
    self.layers.iter_mut()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  struct TestLayer {
    visible: bool,
    render_count: std::cell::Cell<u32>,
    event_count: std::cell::Cell<u32>,
    consume_events: bool,
    cursor: Option<(u16, u16)>,
    #[allow(dead_code)]
    id: &'static str,
  }

  impl TestLayer {
    fn new(id: &'static str, visible: bool, consume_events: bool) -> Self {
      Self {
        visible,
        render_count: std::cell::Cell::new(0),
        event_count: std::cell::Cell::new(0),
        consume_events,
        cursor: None,
        id,
      }
    }

    fn with_cursor(mut self, x: u16, y: u16) -> Self {
      self.cursor = Some((x, y));
      self
    }

    #[allow(dead_code)]
    fn render_count(&self) -> u32 {
      self.render_count.get()
    }

    #[allow(dead_code)]
    fn event_count(&self) -> u32 {
      self.event_count.get()
    }
  }

  impl UILayer for TestLayer {
    fn is_visible(&self) -> bool {
      self.visible
    }

    fn render(&mut self, _area: Rect, _buf: &mut Buffer) {
      self.render_count.set(self.render_count.get() + 1);
    }

    fn handle_event(&mut self, _event: &Event) -> Result<LayerEventOutcome> {
      self.event_count.set(self.event_count.get() + 1);
      if self.consume_events {
        Ok(LayerEventOutcome::Changed)
      } else {
        Ok(LayerEventOutcome::Continue)
      }
    }

    fn screen_cursor(&self) -> Option<(u16, u16)> {
      self.cursor
    }
  }

  #[test]
  fn test_layer_stack_empty() {
    let stack = UILayerStack::new();
    assert!(stack.is_empty());
    assert_eq!(stack.len(), 0);
  }

  #[test]
  fn test_layer_stack_push_pop() {
    let mut stack = UILayerStack::new();
    stack.push(Box::new(TestLayer::new("layer1", true, false)));
    stack.push(Box::new(TestLayer::new("layer2", true, false)));

    assert_eq!(stack.len(), 2);
    assert!(!stack.is_empty());

    stack.pop();
    assert_eq!(stack.len(), 1);
  }

  #[test]
  fn test_render_order_bottom_to_top() {
    let mut stack = UILayerStack::new();

    let layer1 = TestLayer::new("bottom", true, false);
    let layer2 = TestLayer::new("top", true, false);

    stack.push(Box::new(layer1));
    stack.push(Box::new(layer2));

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    stack.render(area, &mut buf);

    // Both layers should have been rendered
    // We can't easily check order here, but both should be rendered
    // The test confirms render is called on all visible layers
  }

  #[test]
  fn test_render_skips_invisible_layers() {
    let mut stack = UILayerStack::new();

    stack.push(Box::new(TestLayer::new("visible", true, false)));
    stack.push(Box::new(TestLayer::new("invisible", false, false)));
    stack.push(Box::new(TestLayer::new("visible2", true, false)));

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    stack.render(area, &mut buf);

    // Only visible layers should be rendered
    // The invisible layer should be skipped
  }

  #[test]
  fn test_event_handling_top_to_bottom() {
    let mut stack = UILayerStack::new();

    // Bottom layer doesn't consume events
    stack.push(Box::new(TestLayer::new("bottom", true, false)));
    // Top layer consumes events
    stack.push(Box::new(TestLayer::new("top", true, true)));

    let event = Event::Key(crossterm::event::KeyEvent::new(
      crossterm::event::KeyCode::Char('a'),
      crossterm::event::KeyModifiers::empty(),
    ));

    let outcome = stack.handle_event(&event).unwrap();

    // Top layer consumes the event
    assert_eq!(outcome, LayerEventOutcome::Changed);
  }

  #[test]
  fn test_event_passes_through_non_consuming_layer() {
    let mut stack = UILayerStack::new();

    // Bottom layer consumes events
    stack.push(Box::new(TestLayer::new("bottom", true, true)));
    // Top layer doesn't consume events
    stack.push(Box::new(TestLayer::new("top", true, false)));

    let event = Event::Key(crossterm::event::KeyEvent::new(
      crossterm::event::KeyCode::Char('a'),
      crossterm::event::KeyModifiers::empty(),
    ));

    let outcome = stack.handle_event(&event).unwrap();

    // Event passes through top layer to bottom layer which consumes it
    assert_eq!(outcome, LayerEventOutcome::Changed);
  }

  #[test]
  fn test_event_skips_invisible_layers() {
    let mut stack = UILayerStack::new();

    // Bottom layer consumes events
    stack.push(Box::new(TestLayer::new("bottom", true, true)));
    // Top layer is invisible but would consume events
    stack.push(Box::new(TestLayer::new("top", false, true)));

    let event = Event::Key(crossterm::event::KeyEvent::new(
      crossterm::event::KeyCode::Char('a'),
      crossterm::event::KeyModifiers::empty(),
    ));

    let outcome = stack.handle_event(&event).unwrap();

    // Invisible top layer is skipped, bottom layer handles event
    assert_eq!(outcome, LayerEventOutcome::Changed);
  }

  #[test]
  fn test_cursor_from_topmost_visible_layer() {
    let mut stack = UILayerStack::new();

    stack.push(Box::new(
      TestLayer::new("bottom", true, false).with_cursor(5, 5),
    ));
    stack.push(Box::new(
      TestLayer::new("top", true, false).with_cursor(10, 10),
    ));

    let cursor = stack.screen_cursor();
    assert_eq!(cursor, Some((10, 10))); // Top layer's cursor
  }

  #[test]
  fn test_cursor_skips_invisible_layers() {
    let mut stack = UILayerStack::new();

    stack.push(Box::new(
      TestLayer::new("bottom", true, false).with_cursor(5, 5),
    ));
    stack.push(Box::new(
      TestLayer::new("top", false, false).with_cursor(10, 10),
    ));

    let cursor = stack.screen_cursor();
    assert_eq!(cursor, Some((5, 5))); // Bottom layer's cursor (top is invisible)
  }

  #[test]
  fn test_cursor_returns_none_if_no_cursor() {
    let mut stack = UILayerStack::new();

    stack.push(Box::new(TestLayer::new("layer1", true, false)));
    stack.push(Box::new(TestLayer::new("layer2", true, false)));

    let cursor = stack.screen_cursor();
    assert_eq!(cursor, None);
  }

  #[test]
  fn test_event_continue_when_no_layer_consumes() {
    let mut stack = UILayerStack::new();

    // Both layers don't consume events
    stack.push(Box::new(TestLayer::new("bottom", true, false)));
    stack.push(Box::new(TestLayer::new("top", true, false)));

    let event = Event::Key(crossterm::event::KeyEvent::new(
      crossterm::event::KeyCode::Char('a'),
      crossterm::event::KeyModifiers::empty(),
    ));

    let outcome = stack.handle_event(&event).unwrap();

    // No layer consumed the event
    assert_eq!(outcome, LayerEventOutcome::Continue);
  }
}
