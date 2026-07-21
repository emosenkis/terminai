#![allow(warnings)]
#![allow(clippy::all, clippy::cargo, clippy::nursery, clippy::pedantic)]

// Terminai library - exports modules for use in binaries

pub mod clipboard;
pub mod encode_term;
pub mod key;
pub mod mouse;
pub mod ui_approval;
pub mod ui_controls;
pub mod vt100;

// Terminai: AI assistant modules
pub mod agent_launcher;
pub mod agent_terminal;
pub mod agent_tools;
pub mod command;
pub mod env_loader;
pub mod mcp_host;
pub mod paths;
pub mod privacy;
pub mod scrollback;
pub mod shell;
pub mod shell_resolution;
pub mod terminai_config;
pub mod terminai_config_init;
pub mod terminai_init;
pub mod ui_layer;
