use std::{
  io::{Read, Write},
  path::Path,
  process::Command,
  sync::{
    Mutex,
    mpsc::{self, Receiver},
  },
  thread,
  time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use compact_str::CompactString;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use termin::vt100::{Parser, TermReplySender};

static SNAPSHOT_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Emulator {
  Internal,
  Tmux,
  Zellij,
  Ghostty,
}

impl Emulator {
  fn name(self) -> &'static str {
    match self {
      Self::Internal => "internal",
      Self::Tmux => "tmux",
      Self::Zellij => "zellij",
      Self::Ghostty => "ghostty-vt",
    }
  }
}

#[derive(Clone, Copy, Debug)]
pub enum Step<'a> {
  WaitFor(&'a [u8]),
  Write(&'a [u8]),
}

#[derive(Debug)]
pub struct Scenario<'a> {
  pub command: &'a str,
  pub cwd: &'a Path,
  pub size: (u16, u16),
  pub scrollback: bool,
  pub steps: &'a [Step<'a>],
  pub timeout: Duration,
}

impl<'a> Scenario<'a> {
  pub fn new(command: &'a str, cwd: &'a Path) -> Self {
    Self {
      command,
      cwd,
      size: (80, 24),
      scrollback: false,
      steps: &[],
      timeout: Duration::from_secs(10),
    }
  }

  pub fn assert_snapshots(&self, name: &str) -> Result<()> {
    // ponytail: serialize heavyweight emulator sessions; split by emulator if this becomes a bottleneck.
    let _guard = SNAPSHOT_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    let emulators = selected_emulators()?;
    let raw = emulators
      .iter()
      .any(|e| matches!(e, Emulator::Internal | Emulator::Ghostty))
      .then(|| capture_raw(self))
      .transpose()?;

    for emulator in emulators {
      let bytes = match emulator {
        Emulator::Internal => capture_internal(self, raw.as_ref().unwrap()),
        Emulator::Ghostty => capture_ghostty(self, raw.as_ref().unwrap())?,
        Emulator::Tmux => capture_tmux(self)?,
        Emulator::Zellij => capture_zellij(self)?,
      };
      let snapshot = String::from_utf8(bytes).with_context(|| {
        format!("{} emitted non-UTF-8 output", emulator.name())
      })?;
      let snapshot_name = format!("{name}__{}", emulator.name());
      insta::assert_snapshot!(snapshot_name, snapshot);
    }
    Ok(())
  }
}

fn selected_emulators() -> Result<Vec<Emulator>> {
  std::env::var("TERMINAI_SNAPSHOT_EMULATORS")
    .unwrap_or_else(|_| "internal".into())
    .split(',')
    .map(|name| match name.trim() {
      "internal" => Ok(Emulator::Internal),
      "tmux" => Ok(Emulator::Tmux),
      "zellij" => Ok(Emulator::Zellij),
      "ghostty" | "ghostty-vt" => Ok(Emulator::Ghostty),
      "" => bail!("TERMINAI_SNAPSHOT_EMULATORS contains an empty name"),
      name => bail!("unknown terminal emulator {name:?}"),
    })
    .collect()
}

#[derive(Clone)]
struct IgnoreReplies;

impl TermReplySender for IgnoreReplies {
  fn reply(&self, _: CompactString) {}
}

#[derive(Clone)]
struct LiveReplies(mpsc::Sender<CompactString>);

impl TermReplySender for LiveReplies {
  fn reply(&self, reply: CompactString) {
    let _ = self.0.send(reply);
  }
}

fn capture_internal(scenario: &Scenario<'_>, raw: &[u8]) -> Vec<u8> {
  let (cols, rows) = scenario.size;
  let mut parser = Parser::new(rows, cols, 100_000, IgnoreReplies);
  parser.process(raw);
  let screen = parser.screen();
  let rows: Box<dyn Iterator<Item = _>> = if scenario.scrollback {
    Box::new(screen.all_rows())
  } else {
    Box::new(screen.drawing_rows())
  };
  let mut output = Vec::new();
  for row in rows {
    row.write_contents_formatted(&mut output, 0, cols, 0, false, None, None);
    output.extend_from_slice(b"\x1b[m\r\n");
  }
  output
}

#[cfg(feature = "ghostty-snapshot-tests")]
fn capture_ghostty(scenario: &Scenario<'_>, raw: &[u8]) -> Result<Vec<u8>> {
  use libghostty_vt::{Terminal, TerminalOptions, fmt};

  let (cols, rows) = scenario.size;
  let mut terminal = Terminal::new(TerminalOptions {
    cols,
    rows,
    max_scrollback: 100_000,
  })?;
  terminal.vt_write(raw);

  let selection = if scenario.scrollback {
    Some(terminal.select_all()?.ok_or_else(|| {
      anyhow!("ghostty-vt terminal has no selectable content")
    })?)
  } else {
    None
  };
  let mut options = fmt::FormatterOptions::new()
    .with_format(fmt::Format::Vt)
    .with_cursor(!scenario.scrollback)
    .with_style(true);
  if let Some(selection) = &selection {
    options = options.with_selection(selection);
  }
  let mut formatter = fmt::Formatter::new(&terminal, options)?;
  Ok(formatter.format_alloc(None)?.to_vec())
}

#[cfg(not(feature = "ghostty-snapshot-tests"))]
fn capture_ghostty(_: &Scenario<'_>, _: &[u8]) -> Result<Vec<u8>> {
  bail!("ghostty-vt requires --features ghostty-snapshot-tests and Zig")
}

fn capture_raw(scenario: &Scenario<'_>) -> Result<Vec<u8>> {
  let (cols, rows) = scenario.size;
  let pair = native_pty_system().openpty(PtySize {
    rows,
    cols,
    pixel_width: 0,
    pixel_height: 0,
  })?;
  let mut command = CommandBuilder::new("sh");
  command.args(["-c", scenario.command]);
  command.cwd(scenario.cwd);
  command.env("TERM", "xterm-256color");
  command.env_remove("NO_COLOR");
  let mut child = pair.slave.spawn_command(command)?;
  drop(pair.slave);
  let mut reader = pair.master.try_clone_reader()?;
  let mut writer = pair.master.take_writer()?;
  let (send, recv) = mpsc::channel();
  let (reply_send, reply_recv) = mpsc::channel();
  let mut responder = Parser::new(rows, cols, 0, LiveReplies(reply_send));
  thread::spawn(move || {
    let mut chunk = [0; 8192];
    while let Ok(count) = reader.read(&mut chunk) {
      if count == 0 || send.send(chunk[..count].to_vec()).is_err() {
        break;
      }
    }
  });

  let mut result = Ok(Vec::new());
  for step in scenario.steps {
    let step_result = match step {
      Step::WaitFor(needle) => {
        wait_for_raw(&recv, needle, scenario.timeout, |chunk| {
          responder.process(chunk);
          for reply in reply_recv.try_iter() {
            writer.write_all(reply.as_bytes())?;
          }
          writer.flush().map_err(Into::into)
        })
      }
      Step::Write(bytes) => writer
        .write_all(bytes)
        .and_then(|_| writer.flush())
        .map(|_| Vec::new())
        .map_err(Into::into),
    };
    match step_result {
      Ok(output) => result.as_mut().unwrap().extend(output),
      Err(err) => {
        result = Err(err);
        break;
      }
    }
  }
  let _ = child.kill();
  let _ = child.wait();
  result
}

fn wait_for_raw(
  recv: &Receiver<Vec<u8>>,
  needle: &[u8],
  timeout: Duration,
  mut process: impl FnMut(&[u8]) -> Result<()>,
) -> Result<Vec<u8>> {
  let start = Instant::now();
  let mut output = Vec::new();
  while !contains(&output, needle) {
    let remaining = timeout.checked_sub(start.elapsed()).ok_or_else(|| {
      anyhow!(
        "timed out waiting for {:?}",
        String::from_utf8_lossy(needle)
      )
    })?;
    let chunk = recv.recv_timeout(remaining).with_context(|| {
      format!(
        "timed out waiting for {:?}; output was {:?}",
        String::from_utf8_lossy(needle),
        String::from_utf8_lossy(&output)
      )
    })?;
    process(&chunk)?;
    output.extend(chunk);
  }
  thread::sleep(Duration::from_millis(50));
  for chunk in recv.try_iter() {
    process(&chunk)?;
    output.extend(chunk);
  }
  Ok(output)
}

fn drive_steps(
  scenario: &Scenario<'_>,
  mut wait: impl FnMut(&[u8], Duration) -> Result<Vec<u8>>,
  mut write: impl FnMut(&[u8]) -> Result<()>,
) -> Result<Vec<u8>> {
  let mut output = Vec::new();
  for step in scenario.steps {
    match step {
      Step::WaitFor(needle) => output.extend(wait(needle, scenario.timeout)?),
      Step::Write(bytes) => write(bytes)?,
    }
  }
  Ok(output)
}

fn capture_tmux(scenario: &Scenario<'_>) -> Result<Vec<u8>> {
  require_command("tmux")?;
  let id = unique_id();
  let socket = format!("terminai-snapshot-{id}");
  let target = format!("snapshot-{id}");
  let session = TmuxSession { socket, target };
  let (cols, rows) = scenario.size;
  session.run([
    "new-session",
    "-d",
    "-s",
    &session.target,
    "-c",
    scenario
      .cwd
      .to_str()
      .context("non-UTF-8 working directory")?,
    "-x",
    &cols.to_string(),
    "-y",
    &rows.to_string(),
    scenario.command,
  ])?;

  drive_steps(
    scenario,
    |needle, timeout| {
      wait_for_capture(timeout, needle, || {
        session.capture(false, scenario.scrollback)
      })
    },
    |bytes| {
      let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>();
      let mut args = vec!["send-keys", "-H", "-t", &session.target];
      args.extend(hex.iter().map(String::as_str));
      session.run(args).map(|_| ())
    },
  )?;
  session.capture(true, scenario.scrollback)
}

struct TmuxSession {
  socket: String,
  target: String,
}

impl TmuxSession {
  fn run<I, S>(&self, args: I) -> Result<Vec<u8>>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
  {
    checked(
      Command::new("tmux")
        .env_remove("NO_COLOR")
        .arg("-L")
        .arg(&self.socket)
        .args(args),
    )
  }

  fn capture(&self, ansi: bool, scrollback: bool) -> Result<Vec<u8>> {
    let mut args = vec!["capture-pane", "-p", "-N", "-t", &self.target];
    if ansi {
      args.push("-e");
    }
    if scrollback {
      args.extend(["-S", "-"]);
    }
    self.run(args)
  }
}

impl Drop for TmuxSession {
  fn drop(&mut self) {
    let _ = Command::new("tmux")
      .args(["-L", &self.socket, "kill-server"])
      .output();
  }
}

fn capture_zellij(scenario: &Scenario<'_>) -> Result<Vec<u8>> {
  require_command("zellij")?;
  let id = unique_id();
  let name = format!("terminai-snapshot-{id}");
  let layout = tempfile::Builder::new().suffix(".kdl").tempfile()?;
  let cwd = serde_json::to_string(&scenario.cwd.to_string_lossy())?;
  let command = serde_json::to_string(scenario.command)?;
  std::fs::write(
    layout.path(),
    format!(
      "layout {{ pane command=\"sh\" cwd={cwd} {{ args \"-c\" {command}; }}; }}"
    ),
  )?;
  let session = ZellijSession::start(name, layout.path(), scenario.size)?;
  let pane = session.first_pane_id()?;

  // ponytail: fail if Zellij does not honor the launch PTY instead of resizing through UI actions.
  let actual = session.pane_size(&pane)?;
  if actual != scenario.size {
    bail!(
      "Zellij pane is {actual:?}, but this scenario requires {:?}",
      scenario.size
    );
  }

  drive_steps(
    scenario,
    |needle, timeout| {
      wait_for_capture(timeout, needle, || {
        session.capture(&pane, false, scenario.scrollback)
      })
    },
    |bytes| {
      let values = bytes.iter().map(u8::to_string).collect::<Vec<_>>();
      checked(
        Command::new("zellij")
          .args([
            "--session",
            &session.name,
            "action",
            "write",
            "--pane-id",
            &pane,
          ])
          .args(values),
      )
      .map(|_| ())
    },
  )?;
  session.capture(&pane, true, scenario.scrollback)
}

struct ZellijSession {
  name: String,
  child: Box<dyn portable_pty::Child + Send + Sync>,
  _pty: Box<dyn portable_pty::MasterPty + Send>,
}

impl ZellijSession {
  fn start(name: String, layout: &Path, size: (u16, u16)) -> Result<Self> {
    let (cols, rows) = size;
    let pair = native_pty_system().openpty(PtySize {
      rows,
      cols,
      pixel_width: 0,
      pixel_height: 0,
    })?;
    let mut command = CommandBuilder::new("zellij");
    command.env("TERM", "xterm-256color");
    command.env_remove("NO_COLOR");
    command.args([
      "--new-session-with-layout",
      layout.to_str().context("non-UTF-8 layout path")?,
      "options",
      "--session-name",
      &name,
      "--pane-frames",
      "false",
    ]);
    let child = pair.slave.spawn_command(command)?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader()?;
    thread::spawn(move || {
      let _ = std::io::copy(&mut reader, &mut std::io::sink());
    });
    Ok(Self {
      name,
      child,
      _pty: pair.master,
    })
  }

  fn command(&self) -> Command {
    let mut command = Command::new("zellij");
    command.args(["--session", &self.name]);
    command
  }

  fn first_pane_id(&self) -> Result<String> {
    let start = Instant::now();
    loop {
      let output = self
        .command()
        .args(["action", "list-panes", "--json"])
        .output()?;
      if output.status.success() {
        let panes: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        if let Some(id) = panes
          .as_array()
          .and_then(|panes| {
            panes
              .iter()
              .find(|pane| pane["is_plugin"].as_bool() == Some(false))
          })
          .and_then(|pane| pane["id"].as_u64())
        {
          return Ok(format!("terminal_{id}"));
        }
      }
      if start.elapsed() >= Duration::from_secs(5) {
        bail!(
          "Zellij session has no terminal pane: {}",
          String::from_utf8_lossy(&output.stderr)
        );
      }
      thread::sleep(Duration::from_millis(20));
    }
  }

  fn pane_size(&self, pane_id: &str) -> Result<(u16, u16)> {
    let panes: serde_json::Value =
      serde_json::from_slice(&checked(self.command().args([
        "action",
        "list-panes",
        "--json",
      ]))?)?;
    let pane = panes
      .as_array()
      .and_then(|panes| {
        panes.iter().find(|pane| {
          pane["is_plugin"].as_bool() == Some(false)
            && pane["id"].as_u64()
              == pane_id
                .strip_prefix("terminal_")
                .and_then(|id| id.parse().ok())
        })
      })
      .with_context(|| format!("Zellij pane {pane_id} disappeared: {panes}"))?;
    Ok((
      pane["pane_content_columns"]
        .as_u64()
        .context("missing pane width")?
        .try_into()?,
      pane["pane_content_rows"]
        .as_u64()
        .context("missing pane height")?
        .try_into()?,
    ))
  }

  fn capture(
    &self,
    pane: &str,
    ansi: bool,
    scrollback: bool,
  ) -> Result<Vec<u8>> {
    let mut command = self.command();
    command.args(["action", "dump-screen", "--pane-id", pane]);
    if ansi {
      command.arg("--ansi");
    }
    if scrollback {
      command.arg("--full");
    }
    checked(&mut command)
  }
}

impl Drop for ZellijSession {
  fn drop(&mut self) {
    let _ = Command::new("zellij")
      .args(["delete-session", "--force", &self.name])
      .output();
    let _ = self.child.kill();
  }
}

fn wait_for_capture(
  timeout: Duration,
  needle: &[u8],
  mut capture: impl FnMut() -> Result<Vec<u8>>,
) -> Result<Vec<u8>> {
  let start = Instant::now();
  loop {
    let output = capture()?;
    if contains(&output, needle) {
      thread::sleep(Duration::from_millis(50));
      return capture();
    }
    if start.elapsed() >= timeout {
      bail!(
        "timed out waiting for {:?}",
        String::from_utf8_lossy(needle)
      );
    }
    thread::sleep(Duration::from_millis(20));
  }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
  needle.is_empty()
    || haystack
      .windows(needle.len())
      .any(|window| window == needle)
}

fn checked(command: &mut Command) -> Result<Vec<u8>> {
  let debug = format!("{command:?}");
  let output = command
    .output()
    .with_context(|| format!("failed to run {debug}"))?;
  if !output.status.success() {
    bail!(
      "{debug} failed: {}",
      String::from_utf8_lossy(&output.stderr)
    );
  }
  Ok(output.stdout)
}

fn require_command(name: &str) -> Result<()> {
  which::which(name)
    .with_context(|| format!("{name} was selected but is not installed"))
    .map(|_| ())
}

fn unique_id() -> String {
  format!(
    "{}-{}",
    std::process::id(),
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_nanos()
  )
}
