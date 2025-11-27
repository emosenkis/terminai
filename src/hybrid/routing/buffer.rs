//! Output buffering for modal display
//!
//! This module provides buffering for terminal output that occurs while
//! the modal is visible. The buffered output is replayed when the modal
//! is closed so that it appears in the host's main buffer.

/// Buffers terminal output for later replay to host
///
/// This is used when the modal is visible and the guest is in main buffer.
/// We need to buffer the output so we can replay it to the host's main
/// buffer when the modal closes.
pub struct OutputBuffer {
  /// Raw bytes captured during modal display
  data: Vec<u8>,

  /// Maximum buffer size before we start dropping old data
  max_size: usize,

  /// Whether we've dropped data due to overflow
  overflow: bool,
}

impl OutputBuffer {
  /// Create a new output buffer with the given maximum size
  ///
  /// The default recommended size is 1MB (1024 * 1024 bytes)
  pub fn new(max_size: usize) -> Self {
    Self {
      data: Vec::with_capacity(max_size.min(4096)),
      max_size,
      overflow: false,
    }
  }

  /// Create a new output buffer with default size (1MB)
  pub fn default() -> Self {
    Self::new(1024 * 1024)
  }

  /// Append data to the buffer
  ///
  /// If this would exceed max_size, older data is dropped to make room.
  /// The strategy is to keep the most recent data, as this ensures the
  /// final screen state is accurate.
  pub fn append(&mut self, data: &[u8]) {
    if data.is_empty() {
      return;
    }

    // Check if we need to make room
    if self.data.len() + data.len() > self.max_size {
      let overflow_amount = (self.data.len() + data.len()) - self.max_size;

      if overflow_amount >= self.data.len() {
        // New data is larger than buffer, just keep the tail of new data
        self.data.clear();
        let start = data.len().saturating_sub(self.max_size);
        self.data.extend_from_slice(&data[start..]);
      } else {
        // Drop data from the beginning
        self.data.drain(0..overflow_amount);
        self.data.extend_from_slice(data);
      }

      self.overflow = true;
    } else {
      self.data.extend_from_slice(data);
    }
  }

  /// Take all buffered data, clearing the buffer
  ///
  /// This returns the raw bytes that should be replayed to the host.
  pub fn take(&mut self) -> Vec<u8> {
    self.overflow = false;
    std::mem::take(&mut self.data)
  }

  /// Clear the buffer without returning the data
  pub fn clear(&mut self) {
    self.data.clear();
    self.overflow = false;
  }

  /// Check if overflow has occurred
  ///
  /// This indicates that some data was lost due to buffer size limits.
  pub fn has_overflow(&self) -> bool {
    self.overflow
  }

  /// Get the current buffer size in bytes
  pub fn len(&self) -> usize {
    self.data.len()
  }

  /// Check if the buffer is empty
  pub fn is_empty(&self) -> bool {
    self.data.is_empty()
  }
}

/// Enhanced output buffer with intelligent overflow handling
///
/// This version maintains checkpoints of terminal state to allow better
/// recovery after overflow. This is more complex but provides better
/// user experience when large amounts of output occur during modal display.
#[allow(dead_code)]
pub struct SmartOutputBuffer {
  /// The basic buffer
  buffer: OutputBuffer,

  /// Checkpoints for recovery (position in buffer, screen state marker)
  checkpoints: Vec<usize>,
}

#[allow(dead_code)]
impl SmartOutputBuffer {
  /// Create a new smart output buffer
  pub fn new(max_size: usize) -> Self {
    Self {
      buffer: OutputBuffer::new(max_size),
      checkpoints: Vec::new(),
    }
  }

  /// Append data with checkpoint tracking
  pub fn append(&mut self, data: &[u8]) {
    // Create checkpoint every 64KB of data
    if self.buffer.len() % (64 * 1024) < data.len() {
      self.checkpoints.push(self.buffer.len());
    }

    self.buffer.append(data);

    // Clean up checkpoints that got dropped due to overflow
    if self.buffer.has_overflow() {
      self.checkpoints.clear();
    }
  }

  /// Take the buffered data
  pub fn take(&mut self) -> Vec<u8> {
    self.checkpoints.clear();
    self.buffer.take()
  }

  /// Clear the buffer
  pub fn clear(&mut self) {
    self.checkpoints.clear();
    self.buffer.clear();
  }

  /// Check for overflow
  pub fn has_overflow(&self) -> bool {
    self.buffer.has_overflow()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_empty_buffer() {
    let buffer = OutputBuffer::new(1024);
    assert_eq!(buffer.len(), 0);
    assert!(buffer.is_empty());
    assert!(!buffer.has_overflow());
  }

  #[test]
  fn test_append_within_limit() {
    let mut buffer = OutputBuffer::new(100);

    buffer.append(b"Hello, ");
    assert_eq!(buffer.len(), 7);
    assert!(!buffer.has_overflow());

    buffer.append(b"World!");
    assert_eq!(buffer.len(), 13);
    assert!(!buffer.has_overflow());

    let data = buffer.take();
    assert_eq!(data, b"Hello, World!");
    assert_eq!(buffer.len(), 0);
    assert!(!buffer.has_overflow());
  }

  #[test]
  fn test_overflow_keeps_recent_data() {
    let mut buffer = OutputBuffer::new(10);

    // Add more than capacity
    buffer.append(b"0123456789");
    assert!(!buffer.has_overflow());

    buffer.append(b"ABCDEFGHIJ");
    assert!(buffer.has_overflow());

    // Should keep most recent 10 bytes
    let data = buffer.take();
    assert_eq!(data.len(), 10);
    assert_eq!(data, b"ABCDEFGHIJ");
  }

  #[test]
  fn test_large_append_overflow() {
    let mut buffer = OutputBuffer::new(10);

    buffer.append(b"small");
    assert_eq!(buffer.len(), 5);

    // Append data larger than buffer
    buffer.append(b"THIS IS VERY LONG DATA");
    assert!(buffer.has_overflow());

    // Should keep last 10 bytes of the long data
    let data = buffer.take();
    assert_eq!(data.len(), 10);
    assert_eq!(data, b" LONG DATA");
  }

  #[test]
  fn test_clear() {
    let mut buffer = OutputBuffer::new(100);
    buffer.append(b"Some data");
    assert_eq!(buffer.len(), 9);

    buffer.clear();
    assert_eq!(buffer.len(), 0);
    assert!(buffer.is_empty());
  }

  #[test]
  fn test_multiple_take() {
    let mut buffer = OutputBuffer::new(100);
    buffer.append(b"Test");

    let data1 = buffer.take();
    assert_eq!(data1, b"Test");

    // After take, buffer should be empty
    let data2 = buffer.take();
    assert_eq!(data2, b"");
  }

  #[test]
  fn test_smart_buffer() {
    let mut buffer = SmartOutputBuffer::new(1024);
    buffer.append(b"Test data");
    assert!(!buffer.has_overflow());

    let data = buffer.take();
    assert_eq!(data, b"Test data");
  }

  #[test]
  fn test_smart_buffer_overflow() {
    let mut buffer = SmartOutputBuffer::new(10);
    buffer.append(b"12345");
    buffer.append(b"67890ABCDE");

    assert!(buffer.has_overflow());

    let data = buffer.take();
    assert_eq!(data.len(), 10);
  }
}
