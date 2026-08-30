#![cfg(unix)]

#[path = "terminal_snapshots/support.rs"]
mod terminal_snapshots;

use std::{os::unix::fs::PermissionsExt, path::Path, time::Duration};

use anyhow::Result;
use terminal_snapshots::{Scenario, Step};

fn assert_terminai(
  name: &str,
  guest: &str,
  steps: &[Step<'_>],
  interface: serde_json::Value,
  scrollback: bool,
) -> Result<()> {
  let temp = tempfile::tempdir()?;
  let config_dir = temp.path().join("terminai");
  std::fs::create_dir_all(&config_dir)?;
  let agent = temp.path().join("agent.sh");
  executable(
    &agent,
    "#!/bin/sh\nprintf '\\033[1;35magent-ready\\033[0m\\r\\n'\nsleep 30\n",
  )?;
  std::fs::write(
    config_dir.join("terminai.yaml"),
    serde_yaml::to_string(&serde_json::json!({
      "changelog": false,
      "interface": interface,
      "agent": {
        "kind": "custom",
        "command": agent,
        "uses-mcp": false,
        "uses-tool-cli": false
      }
    }))?,
  )?;
  let guest_path = temp.path().join("guest.sh");
  executable(&guest_path, guest)?;
  let command = format!(
    "XDG_CONFIG_HOME={} XDG_CACHE_HOME={} {} -- {}",
    quote(temp.path()),
    quote(&temp.path().join("cache")),
    quote(Path::new(env!("CARGO_BIN_EXE_terminai"))),
    quote(&guest_path),
  );
  let mut scenario =
    Scenario::new(&command, Path::new(env!("CARGO_MANIFEST_DIR")));
  scenario.scrollback = scrollback;
  scenario.steps = steps;
  scenario.timeout = Duration::from_secs(10);
  scenario.assert_snapshots(name)
}

fn executable(path: &Path, contents: &str) -> Result<()> {
  std::fs::write(path, contents)?;
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
  Ok(())
}

fn quote(path: &Path) -> String {
  format!("'{}'", path.to_string_lossy().replace('\'', "'\"'\"'"))
}

fn default_interface() -> serde_json::Value {
  serde_json::json!({ "terminal-sync": false })
}

#[test]
fn emulator_harness_preserves_formatting_and_scrollback() -> Result<()> {
  let steps = [
    Step::WaitFor(b"ready"),
    Step::Write(b"go\n"),
    Step::WaitFor(b"line-30"),
  ];
  let mut scenario = Scenario::new(
    "printf 'ready\\r\\n'; sleep 1; printf '\\033[1;38;2;255;128;0mformatted\\033[0m\\r\\n'; i=1; while [ $i -le 30 ]; do printf 'line-%02d\\r\\n' $i; i=$((i + 1)); done; sleep 30",
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
  );
  scenario.scrollback = true;
  scenario.steps = &steps;
  scenario.timeout = Duration::from_secs(5);
  scenario.assert_snapshots("emulator_harness")
}

#[test]
fn terminai_wrapped_command_happy_path() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"hello\n"),
    Step::WaitFor(b"guest:hello"),
  ];
  assert_terminai(
    "terminai_wrapped_command_happy_path",
    "#!/bin/sh\nprintf '\\033[1;32mguest-ready\\033[0m\\r\\n'\nIFS= read -r line\nprintf 'guest:%s\\r\\n' \"$line\"\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_bottom_resize_overlay() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"agent-ready"),
  ];
  assert_terminai(
    "terminai_bottom_resize_overlay",
    "#!/bin/sh\nprintf 'guest-ready\\r\\n'\nsleep 30\n",
    &steps,
    serde_json::json!({
      "terminal-sync": false,
      "chat-position": "bottom",
      "chat-height-percent": 50,
      "guest-display": "resize"
    }),
    false,
  )
}

#[test]
fn terminai_top_move_overlay() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-bottom"),
    Step::Write(b"\0"),
    Step::WaitFor(b"agent-ready"),
  ];
  assert_terminai(
    "terminai_top_move_overlay",
    "#!/bin/sh\nprintf 'guest-top\\r\\n\\r\\n\\r\\n\\r\\n\\r\\n\\r\\n\\r\\n\\r\\n\\r\\n\\r\\n\\r\\n\\r\\n\\r\\n\\r\\n\\r\\n\\r\\n\\r\\n\\r\\n\\r\\n\\r\\nguest-bottom\\r\\n'\nsleep 30\n",
    &steps,
    serde_json::json!({
      "terminal-sync": false,
      "chat-position": "top",
      "chat-height-percent": 50,
      "guest-display": "move"
    }),
    false,
  )
}

#[test]
fn terminai_fullscreen_overlay() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"agent-ready"),
  ];
  assert_terminai(
    "terminai_fullscreen_overlay",
    "#!/bin/sh\nprintf 'guest-ready\\r\\n'\nsleep 30\n",
    &steps,
    serde_json::json!({
      "terminal-sync": false,
      "chat-position": "fullscreen"
    }),
    false,
  )
}

#[test]
fn terminai_overlay_round_trip() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"agent-ready"),
    Step::Write(b"\0"),
    Step::Write(b"hello\n"),
    Step::WaitFor(b"guest:hello"),
  ];
  assert_terminai(
    "terminai_overlay_round_trip",
    "#!/bin/sh\nprintf 'guest-ready\\r\\n'\nIFS= read -r line\nprintf 'guest:%s\\r\\n' \"$line\"\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_native_scrollback_and_soft_wrap() -> Result<()> {
  let steps = [Step::WaitFor(b"scrollback-ready")];
  assert_terminai(
    "terminai_native_scrollback_and_soft_wrap",
    "#!/bin/sh\ni=1\nwhile [ $i -le 30 ]; do printf '\\033[36mhistory-%02d\\033[0m\\r\\n' \"$i\"; i=$((i + 1)); done\nprintf 'soft-wrap-abcdefghijklmnopqrstuvwxyz-ABCDEFGHIJKLMNOPQRSTUVWXYZ-0123456789-abcdefghijklmnopqrstuvwxyz\\r\\n'\nprintf 'scrollback-ready\\r\\n'\nsleep 30\n",
    &steps,
    default_interface(),
    true,
  )
}

#[test]
fn terminai_alternate_screen_round_trip() -> Result<()> {
  let steps = [
    Step::WaitFor(b"alternate-ready"),
    Step::Write(b"go\n"),
    Step::WaitFor(b"primary-restored"),
  ];
  assert_terminai(
    "terminai_alternate_screen_round_trip",
    "#!/bin/sh\nprintf 'primary-before\\r\\n'\nprintf '\\033[?1049halternate-ready\\r\\n'\nIFS= read -r _\nprintf '\\033[?1049lprimary-restored\\r\\n'\nsleep 30\n",
    &steps,
    default_interface(),
    true,
  )
}

#[test]
fn terminai_unicode_cell_boundaries() -> Result<()> {
  let steps = [Step::WaitFor(b"unicode-ready")];
  assert_terminai(
    "terminai_unicode_cell_boundaries",
    "#!/bin/sh\nprintf 'combining: e\\314\\201 | wide: \\344\\270\\255\\346\\226\\207 | emoji: \\360\\237\\221\\251\\342\\200\\215\\360\\237\\222\\273\\r\\nunicode-ready\\r\\n'\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}
