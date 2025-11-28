// Termin.AI library - exports modules for use in binaries

// All modules (needed for dependencies)
pub mod app;
pub mod client;
pub mod clipboard;
pub mod config;
pub mod config_lua;
pub mod ctl;
pub mod encode_term;
pub mod error;
pub mod event;
pub mod host;
pub mod just;
pub mod kernel;
pub mod key;
pub mod keymap;
pub mod modal;
pub mod mouse;
pub mod package_json;
pub mod proc;
pub mod protocol;
pub mod server;
pub mod settings;
pub mod state;
pub mod term;
pub mod theme;
pub mod ui_keymap;
pub mod ui_procs;
pub mod ui_term;
pub mod ui_zoom_tip;
pub mod vt100;
pub mod widgets;
pub mod yaml_val;

// TERMIN.AI: AI assistant modules
pub mod ai_proc;
pub mod command;
pub mod env_loader;
pub mod llm;
pub mod privacy;
