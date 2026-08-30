#[path = "terminal_snapshots/support.rs"]
mod terminal_snapshots;

use std::time::Duration;

use anyhow::Result;
use terminal_snapshots::{Scenario, Step};

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
