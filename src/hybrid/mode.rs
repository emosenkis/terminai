//! Mode management for the hybrid terminal system
//!
//! The hybrid terminal operates in four distinct modes based on two boolean states:
//! - Whether the modal is visible
//! - Whether the guest terminal is in alternate buffer
//!
//! These combine to create four operational modes:
//! - **Passthrough**: Direct output to host, shadow tracks state
//! - **GuestAltBuffer**: Full ratatui rendering (guest in alt buffer, no modal)
//! - **ModalWithBuffering**: Ratatui + output buffering for later replay
//! - **ModalGuestAlt**: Full ratatui rendering (modal + guest in alt buffer)

/// The current operational mode of the hybrid terminal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
  /// Direct passthrough to host main buffer, shadow tracks state
  /// - Host: Main buffer
  /// - Guest: Main buffer
  /// - Modal: Hidden
  /// - Buffering: No
  /// - Rendering: Passthrough (no ratatui)
  Passthrough,

  /// Guest requested alt buffer, we use ratatui exclusively
  /// - Host: Alt buffer
  /// - Guest: Alt buffer
  /// - Modal: Hidden
  /// - Buffering: No
  /// - Rendering: Ratatui (full screen)
  GuestAltBuffer,

  /// Modal visible while guest in main buffer - need buffering
  /// - Host: Alt buffer
  /// - Guest: Main buffer
  /// - Modal: Visible
  /// - Buffering: Yes
  /// - Rendering: Ratatui (terminal + modal overlay)
  ModalWithBuffering,

  /// Modal visible while guest in alt buffer - no buffering needed
  /// - Host: Alt buffer
  /// - Guest: Alt buffer
  /// - Modal: Visible
  /// - Buffering: No
  /// - Rendering: Ratatui (terminal + modal overlay)
  ModalGuestAlt,
}

/// Describes a transition between modes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeTransition {
  /// The mode we're transitioning from
  pub from: Mode,

  /// The mode we're transitioning to
  pub to: Mode,

  /// Whether we need to switch the host terminal buffer
  pub needs_host_buffer_switch: bool,

  /// Whether we need to replay buffered output
  pub needs_buffer_replay: bool,
}

/// Manages the current operational mode of the hybrid terminal
///
/// This component tracks the state and determines which mode we're in
/// based on the modal visibility and guest buffer state.
pub struct ModeManager {
  /// Whether the modal overlay is currently visible
  modal_visible: bool,

  /// Whether the guest terminal has switched to alternate buffer
  guest_in_alt_buffer: bool,

  /// Whether we've put the host terminal in alternate buffer
  host_in_alt_buffer: bool,
}

impl ModeManager {
  /// Create a new mode manager in initial state (Passthrough mode)
  pub fn new() -> Self {
    Self {
      modal_visible: false,
      guest_in_alt_buffer: false,
      host_in_alt_buffer: false,
    }
  }

  /// Get the current mode based on state
  pub fn current_mode(&self) -> Mode {
    match (self.modal_visible, self.guest_in_alt_buffer) {
      (false, false) => Mode::Passthrough,
      (false, true) => Mode::GuestAltBuffer,
      (true, false) => Mode::ModalWithBuffering,
      (true, true) => Mode::ModalGuestAlt,
    }
  }

  /// Check if we need the host in alternate buffer for current mode
  pub fn requires_host_alt_buffer(&self) -> bool {
    // Any mode except pure passthrough needs alt buffer
    self.current_mode() != Mode::Passthrough
  }

  /// Check if we need to buffer output for later replay
  pub fn requires_output_buffering(&self) -> bool {
    self.current_mode() == Mode::ModalWithBuffering
  }

  /// Check if we should render via ratatui
  pub fn requires_ratatui_rendering(&self) -> bool {
    self.current_mode() != Mode::Passthrough
  }

  /// Get whether modal is currently visible
  pub fn is_modal_visible(&self) -> bool {
    self.modal_visible
  }

  /// Get whether guest is in alternate buffer
  pub fn is_guest_in_alt_buffer(&self) -> bool {
    self.guest_in_alt_buffer
  }

  /// Get whether host is in alternate buffer
  pub fn is_host_in_alt_buffer(&self) -> bool {
    self.host_in_alt_buffer
  }

  /// Set modal visibility and return the resulting transition
  pub fn set_modal_visible(&mut self, visible: bool) -> ModeTransition {
    let old_mode = self.current_mode();
    let old_requires_alt = self.requires_host_alt_buffer();

    self.modal_visible = visible;

    let new_mode = self.current_mode();
    let new_requires_alt = self.requires_host_alt_buffer();

    ModeTransition {
      from: old_mode,
      to: new_mode,
      needs_host_buffer_switch: old_requires_alt != new_requires_alt,
      needs_buffer_replay: old_mode == Mode::ModalWithBuffering && !visible,
    }
  }

  /// Set guest alt buffer state and return the resulting transition
  pub fn set_guest_alt_buffer(&mut self, in_alt: bool) -> ModeTransition {
    let old_mode = self.current_mode();
    let old_requires_alt = self.requires_host_alt_buffer();

    self.guest_in_alt_buffer = in_alt;

    let new_mode = self.current_mode();
    let new_requires_alt = self.requires_host_alt_buffer();

    ModeTransition {
      from: old_mode,
      to: new_mode,
      needs_host_buffer_switch: old_requires_alt != new_requires_alt,
      needs_buffer_replay: false, // Guest alt buffer changes don't trigger replay
    }
  }

  /// Update the host buffer state tracking
  ///
  /// This should be called after successfully switching the host buffer
  /// to keep our state tracking in sync.
  pub fn set_host_alt_buffer(&mut self, in_alt: bool) {
    self.host_in_alt_buffer = in_alt;
  }
}

impl Default for ModeManager {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_initial_state() {
    let mgr = ModeManager::new();
    assert_eq!(mgr.current_mode(), Mode::Passthrough);
    assert!(!mgr.requires_host_alt_buffer());
    assert!(!mgr.requires_output_buffering());
    assert!(!mgr.requires_ratatui_rendering());
  }

  #[test]
  fn test_modal_toggle() {
    let mut mgr = ModeManager::new();

    // Show modal: Passthrough -> ModalWithBuffering
    let transition = mgr.set_modal_visible(true);
    assert_eq!(transition.from, Mode::Passthrough);
    assert_eq!(transition.to, Mode::ModalWithBuffering);
    assert!(transition.needs_host_buffer_switch);
    assert!(!transition.needs_buffer_replay);

    // Hide modal: ModalWithBuffering -> Passthrough
    let transition = mgr.set_modal_visible(false);
    assert_eq!(transition.from, Mode::ModalWithBuffering);
    assert_eq!(transition.to, Mode::Passthrough);
    assert!(transition.needs_host_buffer_switch);
    assert!(transition.needs_buffer_replay);
  }

  #[test]
  fn test_guest_alt_buffer() {
    let mut mgr = ModeManager::new();

    // Guest enters alt: Passthrough -> GuestAltBuffer
    let transition = mgr.set_guest_alt_buffer(true);
    assert_eq!(transition.from, Mode::Passthrough);
    assert_eq!(transition.to, Mode::GuestAltBuffer);
    assert!(transition.needs_host_buffer_switch);
    assert!(!transition.needs_buffer_replay);

    // Guest leaves alt: GuestAltBuffer -> Passthrough
    let transition = mgr.set_guest_alt_buffer(false);
    assert_eq!(transition.from, Mode::GuestAltBuffer);
    assert_eq!(transition.to, Mode::Passthrough);
    assert!(transition.needs_host_buffer_switch);
    assert!(!transition.needs_buffer_replay);
  }

  #[test]
  fn test_modal_while_guest_in_alt() {
    let mut mgr = ModeManager::new();

    // Guest enters alt buffer first
    mgr.set_guest_alt_buffer(true);
    assert_eq!(mgr.current_mode(), Mode::GuestAltBuffer);

    // Show modal: GuestAltBuffer -> ModalGuestAlt
    let transition = mgr.set_modal_visible(true);
    assert_eq!(transition.from, Mode::GuestAltBuffer);
    assert_eq!(transition.to, Mode::ModalGuestAlt);
    assert!(!transition.needs_host_buffer_switch); // Already in alt
    assert!(!transition.needs_buffer_replay);

    // Hide modal: ModalGuestAlt -> GuestAltBuffer
    let transition = mgr.set_modal_visible(false);
    assert_eq!(transition.from, Mode::ModalGuestAlt);
    assert_eq!(transition.to, Mode::GuestAltBuffer);
    assert!(!transition.needs_host_buffer_switch); // Stay in alt
    assert!(!transition.needs_buffer_replay);
  }

  #[test]
  fn test_guest_alt_while_modal_visible() {
    let mut mgr = ModeManager::new();

    // Show modal first
    mgr.set_modal_visible(true);
    assert_eq!(mgr.current_mode(), Mode::ModalWithBuffering);

    // Guest enters alt: ModalWithBuffering -> ModalGuestAlt
    let transition = mgr.set_guest_alt_buffer(true);
    assert_eq!(transition.from, Mode::ModalWithBuffering);
    assert_eq!(transition.to, Mode::ModalGuestAlt);
    assert!(!transition.needs_host_buffer_switch); // Already in alt
    assert!(!transition.needs_buffer_replay);

    // Guest leaves alt: ModalGuestAlt -> ModalWithBuffering
    let transition = mgr.set_guest_alt_buffer(false);
    assert_eq!(transition.from, Mode::ModalGuestAlt);
    assert_eq!(transition.to, Mode::ModalWithBuffering);
    assert!(!transition.needs_host_buffer_switch); // Stay in alt
    assert!(!transition.needs_buffer_replay);
  }

  #[test]
  fn test_buffering_requirements() {
    let mut mgr = ModeManager::new();

    // Passthrough: no buffering
    assert!(!mgr.requires_output_buffering());

    // GuestAltBuffer: no buffering
    mgr.set_guest_alt_buffer(true);
    assert!(!mgr.requires_output_buffering());

    // Back to passthrough
    mgr.set_guest_alt_buffer(false);

    // ModalWithBuffering: yes buffering
    mgr.set_modal_visible(true);
    assert!(mgr.requires_output_buffering());

    // ModalGuestAlt: no buffering
    mgr.set_guest_alt_buffer(true);
    assert!(!mgr.requires_output_buffering());
  }

  #[test]
  fn test_host_buffer_requirements() {
    let mut mgr = ModeManager::new();

    // Passthrough: no alt buffer
    assert!(!mgr.requires_host_alt_buffer());

    // GuestAltBuffer: yes alt buffer
    mgr.set_guest_alt_buffer(true);
    assert!(mgr.requires_host_alt_buffer());

    // ModalGuestAlt: yes alt buffer
    mgr.set_modal_visible(true);
    assert!(mgr.requires_host_alt_buffer());

    // ModalWithBuffering: yes alt buffer
    mgr.set_guest_alt_buffer(false);
    assert!(mgr.requires_host_alt_buffer());

    // Passthrough: no alt buffer
    mgr.set_modal_visible(false);
    assert!(!mgr.requires_host_alt_buffer());
  }

  #[test]
  fn test_rendering_requirements() {
    let mut mgr = ModeManager::new();

    // Passthrough: no ratatui
    assert!(!mgr.requires_ratatui_rendering());

    // GuestAltBuffer: yes ratatui
    mgr.set_guest_alt_buffer(true);
    assert!(mgr.requires_ratatui_rendering());

    // ModalGuestAlt: yes ratatui
    mgr.set_modal_visible(true);
    assert!(mgr.requires_ratatui_rendering());

    // ModalWithBuffering: yes ratatui
    mgr.set_guest_alt_buffer(false);
    assert!(mgr.requires_ratatui_rendering());

    // Passthrough: no ratatui
    mgr.set_modal_visible(false);
    assert!(!mgr.requires_ratatui_rendering());
  }
}
