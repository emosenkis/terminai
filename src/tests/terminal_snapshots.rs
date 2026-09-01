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
  assert_terminai_with_agent(
    name,
    guest,
    "#!/bin/sh\nprintf '\\033[1;35magent-ready\\033[0m\\r\\n'\nsleep 30\n",
    steps,
    interface,
    scrollback,
  )
}

fn assert_terminai_with_agent(
  name: &str,
  guest: &str,
  agent_script: &str,
  steps: &[Step<'_>],
  interface: serde_json::Value,
  scrollback: bool,
) -> Result<()> {
  let temp = tempfile::tempdir()?;
  let config_dir = temp.path().join("terminai");
  std::fs::create_dir_all(&config_dir)?;
  let agent = temp.path().join("agent.sh");
  executable(&agent, agent_script)?;
  std::fs::write(
    config_dir.join("terminai.yaml"),
    serde_yaml::to_string(&serde_json::json!({
      "changelog": false,
      "interface": interface,
      "agent": { "preset": "snapshot" },
      "agent-presets": {
        "snapshot": {
          "command": agent,
          "uses-mcp": false,
          "uses-tool-cli": false
        }
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
  scenario.timeout = Duration::from_secs(30);
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

fn assert_agent_switch(name: &str, steps: &[Step<'_>]) -> Result<()> {
  let temp = tempfile::tempdir()?;
  let config_dir = temp.path().join("terminai");
  std::fs::create_dir_all(&config_dir)?;
  let first = temp.path().join("first-agent.sh");
  executable(
    &first,
    "#!/bin/sh\nprintf '\\033[35mold-session\\033[0m\\r\\n'\nsleep 2\nprintf 'old-still-running\\r\\n'\nsleep 30\n",
  )?;
  let second = temp.path().join("second-agent.sh");
  executable(
    &second,
    "#!/bin/sh\nprintf '\\033[32mnew-session\\033[0m\\r\\n'\nsleep 30\n",
  )?;
  std::fs::write(
    config_dir.join("terminai.yaml"),
    serde_yaml::to_string(&serde_json::json!({
      "changelog": false,
      "interface": default_interface(),
      "agent": { "preset": "snapshot" },
      "agent-presets": {
        "snapshot": {
          "command": first,
          "uses-mcp": false,
          "uses-tool-cli": false
        },
        "z-switch": {
          "command": second,
          "uses-mcp": false,
          "uses-tool-cli": false
        }
      }
    }))?,
  )?;
  let guest = temp.path().join("guest.sh");
  executable(&guest, "#!/bin/sh\nprintf 'guest-ready\\r\\n'\nsleep 30\n")?;
  let command = format!(
    "XDG_CONFIG_HOME={} XDG_CACHE_HOME={} {} -- {}",
    quote(temp.path()),
    quote(&temp.path().join("cache")),
    quote(Path::new(env!("CARGO_BIN_EXE_terminai"))),
    quote(&guest),
  );
  let mut scenario =
    Scenario::new(&command, Path::new(env!("CARGO_MANIFEST_DIR")));
  scenario.steps = steps;
  scenario.timeout = Duration::from_secs(10);
  scenario.assert_snapshots(name)
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
    Step::WaitFor(b"cursor-end>"),
  ];
  assert_terminai(
    "terminai_wrapped_command_happy_path",
    "#!/bin/sh\nprintf '\\033[1;32mguest-ready\\033[0m\\r\\n'\nIFS= read -r line\nprintf 'guest:%s\\r\\n\\033[36mstyled-stdout\\033[0m\\r\\n' \"$line\"\nprintf '\\033[31mstyled-stderr\\033[0m\\r\\n' >&2\nprintf 'cursor-end>'\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_suggested_input_approval_dialog() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"Approve (Y)"),
  ];
  assert_terminai_with_agent(
    "terminai_suggested_input_approval_dialog",
    "#!/bin/sh\nprintf 'guest-ready\\r\\n'\nsleep 30\n",
    "#!/bin/sh\nprintf 'agent-ready\\r\\n'\nsleep 1\nterminai tool suggest_input 'printf approved\\n' --explanation 'Print a marker.'\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_approves_suggested_input() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"Approve (Y)"),
    Step::Write(b"y"),
    Step::WaitFor(b"guest-done"),
  ];
  assert_terminai_with_agent(
    "terminai_approves_suggested_input",
    "#!/bin/sh\nprintf 'guest-ready\\r\\n'\nIFS= read -r line\nprintf 'guest:%s\\r\\nguest-done\\r\\n' \"$line\"\nsleep 30\n",
    "#!/bin/sh\nprintf 'agent-ready\\r\\n'\nsleep 1\nterminai tool suggest_input 'printf approved\\n' --explanation 'Print a marker.'\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_denies_suggested_input() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-prompt"),
    Step::Write(b"\0"),
    Step::WaitFor(b"Deny (N)"),
    Step::Write(b"n"),
    Step::WaitFor(b"denial-observed"),
  ];
  assert_terminai_with_agent(
    "terminai_denies_suggested_input",
    "#!/bin/sh\nprintf '\\033[32mguest-prompt>\\033[0m '\n(sleep 2; printf '\\r\\ndenial-observed\\r\\n') &\nIFS= read -r line\nprintf 'unexpected-input:%s\\r\\n' \"$line\"\nsleep 30\n",
    "#!/bin/sh\nprintf 'agent-ready\\r\\n'\nsleep 1\nterminai tool suggest_input 'printf denied\\n' --explanation 'This must not reach the guest.'\nsleep 30\n",
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
fn terminai_layout_mode_controls() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-bottom"),
    Step::Write(b"\0"),
    Step::WaitFor(b"agent-ready"),
    Step::Write(b"\x1b[20~"),
    Step::WaitFor(b"AI height: 50%"),
    Step::Write(b"+"),
    Step::Pause(Duration::from_millis(100)),
    Step::Write(b"-"),
    Step::Pause(Duration::from_millis(100)),
    Step::Write(b"p"),
    Step::Pause(Duration::from_millis(100)),
    Step::Write(b"g"),
    Step::Pause(Duration::from_millis(100)),
    Step::Write(b"f"),
    Step::Pause(Duration::from_millis(100)),
  ];
  assert_terminai(
    "terminai_layout_mode_controls",
    "#!/bin/sh\ni=1\nwhile [ $i -le 22 ]; do printf 'guest-%02d\\r\\n' \"$i\"; i=$((i + 1)); done\nprintf 'guest-bottom\\r\\n'\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_control_panel() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"agent-ready"),
    Step::Write(b"\x1b[21~"),
    Step::WaitFor(b"Terminai Controls"),
  ];
  assert_terminai(
    "terminai_control_panel",
    "#!/bin/sh\nprintf 'guest-ready\\r\\n'\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_auto_approval_confirmation() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"agent-ready"),
    Step::Write(b"\x1b[21~"),
    Step::WaitFor(b"Terminai Controls"),
    Step::Write(b"\r"),
    Step::WaitFor(b"Enable Auto-Approval?"),
    Step::Write(b"\x1b[C"),
    Step::Pause(Duration::from_millis(100)),
  ];
  assert_terminai(
    "terminai_auto_approval_confirmation",
    "#!/bin/sh\nprintf 'guest-ready\\r\\n'\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_agent_picker() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"agent-ready"),
    Step::Write(b"\x1b[21~"),
    Step::WaitFor(b"Terminai Controls"),
    Step::Write(b"\x1b[B\r"),
    Step::WaitFor(b"Switch Agent"),
  ];
  assert_terminai(
    "terminai_agent_picker",
    "#!/bin/sh\nprintf 'guest-ready\\r\\n'\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_clear_history_confirmation() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"agent-ready"),
    Step::Write(b"\x1b[21~"),
    Step::WaitFor(b"Terminai Controls"),
    Step::Write(b"\x1b[B\x1b[B\x1b[B\r"),
    Step::WaitFor(b"History?"),
    Step::Write(b"\x1b[C"),
    Step::Pause(Duration::from_millis(100)),
  ];
  assert_terminai(
    "terminai_clear_history_confirmation",
    "#!/bin/sh\nprintf 'guest-ready\\r\\n'\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_agent_exit_status() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"relaunch."),
  ];
  assert_terminai_with_agent(
    "terminai_agent_exit_status",
    "#!/bin/sh\nprintf 'guest-ready\\r\\n'\nsleep 30\n",
    "#!/bin/sh\nprintf '\\033[35magent-before-exit\\033[0m\\r\\n'\nexit 7\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_agent_relaunch() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"relaunch."),
    Step::Write(b"\r"),
    Step::WaitFor(b"agent-relaunched"),
  ];
  assert_terminai_with_agent(
    "terminai_agent_relaunch",
    "#!/bin/sh\nprintf 'guest-ready\\r\\n'\nsleep 30\n",
    "#!/bin/sh\nmarker=\"$XDG_CACHE_HOME/agent-launched-$PPID\"\nif [ -e \"$marker\" ]; then\n  printf '\\033[32magent-relaunched\\033[0m\\r\\n'\n  sleep 30\nelse\n  mkdir -p \"$XDG_CACHE_HOME\"\n  : > \"$marker\"\n  printf '\\033[35magent-before-exit\\033[0m\\r\\n'\n  exit 7\nfi\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_approval_text_encoding() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"Approve (Y)"),
  ];
  assert_terminai_with_agent(
    "terminai_approval_text_encoding",
    "#!/bin/sh\nprintf 'guest-ready\\r\\n'\nsleep 30\n",
    "#!/bin/sh\nprintf 'agent-ready\\r\\n'\nsleep 1\nterminai tool suggest_input 'space tab\\tline\\n\\u001b\\u0003é' --explanation 'Exercise encoded control input.'\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_approval_writes_exact_bytes() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"Approve (Y)"),
    Step::Write(b"y"),
    Step::WaitFor(b"bytes-done"),
  ];
  assert_terminai_with_agent(
    "terminai_approval_writes_exact_bytes",
    "#!/bin/sh\nprintf 'guest-ready\\r\\n'\nstty raw -echo\nbytes=$(dd bs=1 count=19 2>/dev/null | od -An -tx1 | tr -s ' ' | tr -d '\\n')\nprintf '\\r\\nbytes:%s\\r\\nbytes-done\\r\\n' \"$bytes\"\nsleep 30\n",
    "#!/bin/sh\nprintf 'agent-ready\\r\\n'\nsleep 1\nterminai tool suggest_input 'space tab\\tline\\n\\u001b\\u0003é' --explanation 'Exercise encoded control input.'\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_clears_ai_readable_history_only() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"before-has-old-history"),
    Step::Write(b"\x1b[21~"),
    Step::WaitFor(b"Terminai Controls"),
    Step::Write(b"\x1b[B\x1b[B\x1b[B\r"),
    Step::WaitFor(b"History?"),
    Step::Write(b"\x1b[C\r"),
    Step::WaitFor(b"after-history-cleared"),
  ];
  assert_terminai_with_agent(
    "terminai_clears_ai_readable_history_only",
    "#!/bin/sh\ni=1\nwhile [ $i -le 40 ]; do printf 'history-%02d\\r\\n' \"$i\"; i=$((i + 1)); done\nprintf 'guest-ready\\r\\n'\nsleep 30\n",
    "#!/bin/sh\nprintf 'agent-ready\\r\\n'\nbefore=$(terminai tool read_terminal --max-lines 100 2>&1)\ncase $before in *history-01*) printf 'before-has-old-history\\r\\n';; *) printf 'before-missing-old-history\\r\\n';; esac\nsleep 3\nafter=$(terminai tool read_terminal --max-lines 100 2>&1)\ncase $after in *history-01*) printf 'after-still-has-old-history\\r\\n';; *) printf '\\033[32mafter-history-cleared\\033[0m\\r\\n';; esac\nsleep 30\n",
    &steps,
    default_interface(),
    true,
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
    "#!/bin/sh\nprintf 'combining: e\\314\\201 | wide: \\344\\270\\255\\346\\226\\207 | emoji: \\360\\237\\221\\251\\342\\200\\215\\360\\237\\222\\273\\r\\nlast-column:'\ni=1\nwhile [ $i -le 67 ]; do printf x; i=$((i + 1)); done\nprintf '\\344\\270\\255\\r\\nunicode-ready\\r\\n'\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn emulator_osc_state_changes() -> Result<()> {
  let steps = [Step::WaitFor(b"osc-ready")];
  let mut scenario = Scenario::new(
    "i=1; while [ $i -le 30 ]; do printf 'old-%02d\\r\\n' \"$i\"; i=$((i + 1)); done; printf '\\033]1337;ClearScrollback\\007\\033[2J\\033[H\\033]7;file://localhost/tmp/terminai-osc\\007\\033]8;;https://example.test/terminai\\033\\\\\\033[4;34mlink-label\\033[0m\\033]8;;\\033\\\\\\r\\n\\033]4;1;rgb:ff/80/00\\007\\033[31mpalette-one\\033[0m\\r\\n'; i=1; while [ $i -le 30 ]; do printf 'new-%02d\\r\\n' \"$i\"; i=$((i + 1)); done; printf 'osc-ready\\r\\n'; sleep 30",
    Path::new(env!("CARGO_MANIFEST_DIR")),
  );
  scenario.scrollback = true;
  scenario.steps = &steps;
  scenario.timeout = Duration::from_secs(5);
  scenario.assert_snapshots("emulator_osc_state_changes")
}

#[test]
fn terminai_terminal_mode_input_after_overlay() -> Result<()> {
  let steps = [
    Step::WaitFor(b"input-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"agent-ready"),
    Step::Write(b"\0"),
    Step::Write(b"\x1b[A"),
    Step::Write(b"\x1b[200~pasted text\x1b[201~"),
    Step::Write(b"\x1b[I"),
    Step::Write(b"\x1b[<0;3;4M"),
    Step::WaitFor(b"input-done"),
  ];
  assert_terminai(
    "terminai_terminal_mode_input_after_overlay",
    "#!/bin/sh\nprintf '\\033[?1h\\033[?2004h\\033[?1004h\\033[?1000h\\033[?1006hinput-ready\\r\\n'\nstty raw -echo min 0 time 10\nbytes=$(while byte=$(dd bs=1 count=1 2>/dev/null); do [ -n \"$byte\" ] || break; printf %s \"$byte\"; done | od -An -tx1 | tr -s ' ' | tr -d '\\n')\nstty sane\nprintf '\\r\\ninput-bytes:%s\\r\\ninput-done\\r\\n' \"$bytes\"\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_rapid_output_during_overlay_changes() -> Result<()> {
  let steps = [
    Step::WaitFor(b"burst-05"),
    Step::Write(b"\0"),
    Step::WaitFor(b"agent-ready"),
    Step::Write(b"\x1b[20~"),
    Step::WaitFor(b"AI height: 50%"),
    Step::Write(b"+pg"),
    Step::Pause(Duration::from_millis(100)),
    Step::Write(b"\x1b[20~"),
    Step::Write(b"\0"),
    Step::WaitFor(b"burst-done"),
  ];
  assert_terminai(
    "terminai_rapid_output_during_overlay_changes",
    "#!/bin/sh\ni=1\nwhile [ $i -le 70 ]; do printf '\\033[3%dm burst-%02d \\033[0m\\r\\n' $((i % 7 + 1)) \"$i\"; i=$((i + 1)); sleep 0.03; done\nprintf 'burst-done\\r\\n'\nsleep 30\n",
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn terminai_synchronized_update_paths() -> Result<()> {
  let steps = [Step::WaitFor(b"sync-ready")];
  let guest = "#!/bin/sh\nprintf '\\033[2J\\033[H\\033[1;36msynchronized\\033[0m\\r\\n\\033[3;5H\\033[38;2;255;128;0mstyled-final\\033[0m\\r\\n\\033[5;1Hsync-ready'\nsleep 30\n";
  assert_terminai(
    "terminai_synchronized_updates_enabled",
    guest,
    &steps,
    serde_json::json!({ "terminal-sync": true }),
    false,
  )?;
  assert_terminai(
    "terminai_synchronized_updates_disabled",
    guest,
    &steps,
    default_interface(),
    false,
  )
}

#[test]
fn emulator_partial_and_malformed_escape_streams() -> Result<()> {
  let steps = [Step::WaitFor(b"parser-recovered")];
  let mut scenario = Scenario::new(
    "printf 'split-utf8: \\342'; sleep 0.02; printf '\\202\\254\\r\\n\\033['; sleep 0.02; printf '1;32msplit-csi\\033[0m\\r\\n\\033]8;;https://example.test/split'; sleep 0.02; printf '\\033\\\\\\033[4;34msplit-link\\033[0m\\033]8;;\\033\\\\\\r\\n\\377\\376invalid-bytes\\r\\n\\033[999999999999999999999m\\033[1;35mparser-recovered\\033[0m\\r\\n'; sleep 30",
    Path::new(env!("CARGO_MANIFEST_DIR")),
  );
  scenario.steps = &steps;
  scenario.timeout = Duration::from_secs(5);
  scenario.assert_snapshots("emulator_partial_and_malformed_escape_streams")
}

#[test]
fn terminai_cancels_agent_switch() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"old-session"),
    Step::Write(b"\x1b[21~"),
    Step::WaitFor(b"Terminai Controls"),
    Step::Write(b"\x1b[B\r"),
    Step::WaitFor(b"Switch Agent"),
    Step::Write(b"\x1b[B\r"),
    Step::WaitFor(b"z-switch?"),
    Step::Write(b"\x1b"),
    Step::WaitFor(b"old-still-running"),
  ];
  assert_agent_switch("terminai_cancels_agent_switch", &steps)
}

#[test]
fn terminai_confirms_agent_switch() -> Result<()> {
  let steps = [
    Step::WaitFor(b"guest-ready"),
    Step::Write(b"\0"),
    Step::WaitFor(b"old-session"),
    Step::Write(b"\x1b[21~"),
    Step::WaitFor(b"Terminai Controls"),
    Step::Write(b"\x1b[B\r"),
    Step::WaitFor(b"Switch Agent"),
    Step::Write(b"\x1b[B\r"),
    Step::WaitFor(b"z-switch?"),
    Step::Write(b"\x1b[C\r"),
    Step::WaitFor(b"new-session"),
  ];
  assert_agent_switch("terminai_confirms_agent_switch", &steps)
}
