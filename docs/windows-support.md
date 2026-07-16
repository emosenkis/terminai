# Windows support

Terminai has experimental Windows builds for 64-bit Windows 10 version 1809 or
later and Windows 11 when run in a current Windows Terminal. Windows is not yet
a qualified or supported platform; required CI and human QA remain in progress.
It uses ConPTY through `portable-pty` and keeps Windows Terminal's
normal-screen native scrollback.

Supported wrapped shells are PowerShell 7 (`pwsh.exe`), inbox Windows
PowerShell (`powershell.exe`), and `cmd.exe`. PowerShell 7 is preferred. Their
prompts are bootstrapped to report live working-directory changes through OSC
7. An explicitly supplied unknown command uses the generic PTY path; it starts
in the initial CWD and may report later CWD changes only if it emits OSC 7.

Choose the shell in this order: `terminai -- command args`, `TERMINAI_SHELL`,
the `shell.command`/`shell.args` config section, a detected parent shell, then
`pwsh.exe`, `powershell.exe`, and `cmd.exe`. `TERMINAI_SHELL` is an executable
name/path only. Shell configuration is a default-shell selector, not a script
launcher. PowerShell execution flags (`-Command`, `-File`, `-EncodedCommand`)
and cmd execution flags (`/C`, `/K`) are rejected in shell configuration
because they prevent Terminai's CWD bootstrap.

On Windows, configuration and `terminai.env` live in
`%APPDATA%\terminai`; logs and cache live in `%LOCALAPPDATA%\terminai`.

Deferred environments: VS Code's integrated terminal, legacy Console Host,
Git Bash/MSYS2, Cygwin, WSL guests, ARM64, and Windows versions without
ConPTY. They are not qualified for this release.

## Automated coverage limitation

The `deactivate_overlay_keeps_pending_command_for_reopen` unit test is gated
off on Windows. Its fixture starts `/bin/sh` and performs Crossterm mouse
capture cleanup, which requires an initialized console mode that GitHub's
non-interactive Windows test process does not provide.

Consequently, Windows CI does not directly assert that closing the AI overlay
preserves a pending approval for reopening. The ConPTY cmd and PowerShell smoke
tests, key encoding tests, and the remaining platform-neutral overlay and
approval tests continue to run on Windows. Verify the omitted close/reopen
approval flow during Windows Terminal human QA before Windows support is
qualified.

## Legacy cleanup record

The production Terminai library intentionally excludes the inherited mprocs
application rooted at `src/main.rs`: its app/client/config/daemon/process/UI
stack is not compiled by the Terminai binary. The shared modules retained by
Terminai are the VT100 parser, key/mouse encoding, shell PTY, and clipboard.
After this release stabilizes, remove the excluded source and dependencies in
a dedicated cleanup change.
