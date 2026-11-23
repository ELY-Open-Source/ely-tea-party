# Upstream synchronization, 2026-11-22

Base: fork `91b5823` (v0.3.24).
Merged upstream: `0828078` (v0.2.6), 24 commits, including review and task records.

## Integration decisions

- Keep the fork's package name, version, app id, README, CI and release
  repository, as before. Left out as upstream's alone: `README_zh.md`, the user
  guides under `docs/en_us` and `docs/zh_cn`, and the Homebrew cask.
- Left out: upstream's self-update, `src/updates.rs`, `src/updates/` and
  `examples/update.rs`, with its `sha2` dependency and `Leave::Restart`. The
  fork's own `src/update.rs` already downloads from the download store, checks
  the SHA-256 and signature and swaps the app on macOS, and opens the release's
  page on Windows and Linux. Upstream's Windows zip and Linux AppImage, tarball,
  DEB and RPM installs are a separate piece of work for the fork, against its
  own `latest.json`.
- Integrated: a drag on a split's line resizes its panes, Move pane swaps the
  focused pane with its neighbor (Cmd-Alt-arrows, Ctrl-Shift-Alt-arrows off
  macOS), the edge cells' grounds reaching the terminal's bottom and right
  edges (inside the fork's paint layer), Cmd-Q deferred past its window's
  update, the macOS login shell counted as the session's own, a folder dropped
  on the macOS icon opening a session there, Copy's Ctrl key going on to the
  program with nothing selected, Peek low, and the busy and shell tests.
- A folder dropped on the icon and a skin link both come through
  `on_open_urls`. The first folder handed over before the app starts names the
  first session's folder, as `-e` or a folder argument does. Later folders open a
  session in the main window. Links still go to the market.
- Named session groups (#110) work in both sidebar styles. Characters shows
  each block of rows under its group's header. Compact shows the ungrouped
  rows' folders first, then each group's header with its own folders under it,
  and a folder collapses by its group and path. A drag stays inside its group,
  and in Compact inside its folder too. Cmd-1 to Cmd-9 and Next and Previous
  session follow the rows' order. A row's menu keeps Copy link and adds New
  group, Move to, and Remove from group. A group header's menu renames or
  ungroups. A new session joins the shown one's group, or the group of the row
  it lands after.
- Session restore keeps the fork's `sessions.json` and its flow: the agent's
  resume line, the link id, the trinket, the steps, and the asked folder
  opening first with the saved sessions behind it. Upstream's `saved.rs`,
  `Keeps` and settle timer are left out. The file gains optional fields, so a
  file saved before still reads. `groups` holds the groups' ids and names.
  Each place may name its `group` and, while it holds more than its first pane,
  its `panes`: the layout with each pane's program and folder, the first pane's
  place in it, and the focused one's. A split's pane starts without hook
  variables, as one opened by hand. A pane `-e` started is not saved, and a
  pane whose folder went stays closed. Closing the window saves at once,
  through the fork's `keep`.
- Ely follows the session shown, as upstream decided, with that session's
  agent counting too: her mood is the more pressing of the session's mark and
  its agent's look. Peek low applies while Ely is the mascot; the whale girl
  keeps her own motion.
- `src/workspace/restore.rs`, `app.rs`, `keymap.rs` and `workspace.rs` passed
  500 lines. The saved layout moved to `workspace/restore/layout.rs`, the
  dropped folder to `app/dropped.rs`, the shortcuts' check to
  `keymap/check.rs`, and `Workspace::shown` to `workspace/order.rs`.

## Validation

- macOS ARM64: formatting, all-target Clippy with warnings denied, the
  500-line and comment rules, 770 unit tests and the integration tests passed.
  Two existing tests stay ignored. PowerShell's folder test fails because
  `pwsh` is not installed on this Mac.
- Not verified by this merge: how groups, split resizing and Move pane look and
  behave in the running app; restoring splits and groups across a restart; a
  folder dropped on the Dock icon; Windows and Linux compilation and runtime;
  remote CI.
