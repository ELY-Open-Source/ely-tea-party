# Upstream synchronization, 2026-10-30

Base: fork `bb9f73b` (v0.3.15).
Merged upstream: `6936b6f`, including v0.2.0 and its Windows zip fix.
The merge incorporates 155 upstream commits, including review and task records.

## Integration decisions

- Keep the fork's package name, version, app id, artwork, README and release
  repository. Upstream's `v0.2.0` tag is not fetched: the fork's own `v0.2.0`
  names its first release.
- Integrate kitty graphics (transmission, placements, animation, Unicode
  placeholders, SIMD decoding, textures freed by hidden sessions), kitty's
  clipboard protocol OSC 5522, packed idle history, the ASCII fast path,
  per-color row shaping, custom themes and a background image, the window's
  kept display and place, Mica on Windows 11, OpenConsole on Windows,
  PowerShell's `cd /e`, and the Wayland modifier fixes.
- Upstream moved the view's pty event handling into `term/view/events.rs`; the
  fork's folder tracking, Speed events and reply timing moved with it.
- A closed session, and a closed drawer, give their image textures back, as
  upstream's hidden sessions do; a hidden session's drawer keeps its own.
- Files keeps the fork's tree, keyboard navigation and its menu (new file or
  folder, reveal, trash). Upstream's Open, New session here and Copy path join
  that menu, and upstream's request-once fix applies to it. Settings ›
  Terminal › Files menu switches the whole menu; the fork defaults it on, where
  upstream defaults it off, so the menu users had stays.
- The Windows zip carries `conpty.dll`, `OpenConsole.exe` and their license
  beside the fork's executable and Butler bundle. `release-asset.sh` stays the
  fork's: upstream's version-less aliases point at upstream's repository.
- Upstream's `README_zh.md`, `docs/en_us` and `docs/zh_cn` are ELYTerminal's
  user guides and are left out. Upstream's `website/` stays as its record,
  like `tasks/`; it deploys nothing from this repository.

## Validation

- macOS ARM64: formatting, all-target Clippy with warnings denied, 530 unit
  tests, the hooks installer tests, the font harness and house rules passed;
  two existing tests remain ignored. PowerShell's folder test was skipped:
  `pwsh` is not installed on this Mac.
- `tap.rs` passed 500 lines with the fork's Speed scanner; the mode and speed
  reports moved into `tap/modes.rs` and `speed.rs`. Their Windows branches
  compile only on Windows and are left to CI.
- Windows and Linux runtime, interactive GUI acceptance, package installation,
  signing, release publication and remote CI are not verified by this merge.

Upstream task and acceptance documents retain upstream's historical results;
those entries do not certify the fork's runtime or release artifacts.
