# Upstream synchronization, 2026-11-15

Base: fork `7bac124` (v0.3.23).
Merged upstream: `4d2caf1`, including v0.2.4 and v0.2.5 and the Wayland fix after it.
The merge incorporates 71 upstream commits, including review and task records.

## Integration decisions

- Keep the fork's package name, version, app id, artwork, README, CI and
  release repository. Upstream's `v0.2.x` tags are not kept.
- Left out as upstream's alone, as before: `.github/workflows/website.yml`,
  `README_zh.md` and the user guides under `docs/en_us` and `docs/zh_cn`. Also
  left out: the NSIS installer, `packaging/windows/installer.nsi` and
  `scripts/windows-installer.ps1`. It packs upstream's five files under
  upstream's name, and the fork's zip also holds its `butler` folder; an
  installer for the fork is its own piece of work.
- Integrated: split panes, the table of shortcuts and Settings › Terminal ›
  Shortcuts, Clear, Toggle Full Screen, `ctl`, `-e PROGRAM`, the question
  before a multi-line paste and before a close that ends running programs, a
  selection drag that scrolls, the background picture's gradient, opacity and
  mask, `TERM_PROGRAM`, Ctrl-Insert and Shift-Insert on Windows, MSYS2's fish,
  Windows' default terminal, and the KWin and Omarchy Wayland fixes.
- A session holds panes as upstream's does: `panes`, `layout` and `focused`.
  It keeps the fork's own `id`, the number its hooks report under, in place of
  upstream's `EntityId`; rows, the menu and rename find a session by it.
- Only a session's first pane, `Session::main`, holds its agent. Its character,
  hooks, messages, resume line, prompt rail's restore, Butler's and the
  companion's snapshots, and the sidebar's folder groups read that pane. A
  split starts without the hook and message variables, so an agent started in
  a split is not the session's. Files, Git, the title and the keys follow the
  focused pane, as upstream's do.
- The drawer stays the fork's. Cmd-W closes the drawer when it holds the keys,
  else the focused pane, through upstream's question.
- The fork's commands join upstream's table, so Shortcuts lists and changes
  them: the session switcher, Jump to a waiting session, the drawer and the
  tea party. Two of upstream's own keys gave way to the fork's: Clear is
  Cmd-Shift-K, off macOS Ctrl-Shift-Alt-K, since the switcher holds Cmd-K and
  Ctrl-Shift-K; Split down off macOS is Ctrl-Shift-Alt-O, since the tea party
  holds Ctrl-Shift-O. The switcher's own Up and Down are bound before the
  table, which keeps what was bound before it.
- `ctl` is `ely-tea-party ctl`, and its door's file lies in the fork's own
  settings folder.
- The default terminal's class is the fork's own,
  `{759E171C-8EA2-4E3B-ABBD-CA924BD08D73}`, so the fork and ELYTerminal can
  both be installed. OpenConsole's and the proxy's classes are the bundled
  files' own and cannot differ: whichever of the two apps was made the default
  last serves them.
- A folder or `-e PROGRAM` named on the command line opens first and shown,
  and the saved sessions come back after it.
- The update check keeps the fork's own module and its six hours.
- `src/i18n/en.rs`, `zh.rs`, `workspace.rs`, `term/pointer.rs` and
  `term/shell/tests.rs` passed 500 lines: the shortcuts', paste's and close's
  words moved to `*_keys.rs`, the Terminal page's to `*_terminal.rs`, `tick` to
  `workspace/session.rs`, the drawer's `start` to `workspace/drawer.rs`, the
  link hover to `term/pointer/link.rs`, and the WSL test to
  `term/shell/tests/wsl.rs`.

## Known limits

- Only a session's first pane comes back at the next start; splits do not.
- When a session's first pane closes, the next oldest takes its place. It was
  started without the hook variables, so an agent in it is known by its
  process alone, as an unhooked agent is.
- With Ask before closing running sessions on, which is the default, Quit and the window's
  close ask while any agent runs. Settings › Terminal turns it off. An update's
  restart does not ask.
- A session Windows hands over runs already, so no hook of it reaches the app.

## Validation

- macOS ARM64: formatting, all-target Clippy with warnings denied, the
  500-line rule, 760 unit tests and the integration tests passed; two existing
  tests remain ignored. PowerShell's folder test fails here because `pwsh` is
  not installed on this Mac.
- The built app started under a temporary `HOME`: the keymap bound, the first
  pane and session opened and the main window showed. The run stopped there, at
  a key the `shot` example has no code for, so no picture was taken.
- Not verified by this merge: how split panes draw, alone and above a drawer;
  split panes beside a running agent; Windows and Linux compile and runtime,
  the default terminal's class among them; package installation, signing,
  release publication and remote CI.
