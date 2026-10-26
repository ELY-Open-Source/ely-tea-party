# Upstream synchronization, 2026-10-25

Base: fork `dddd3e8` (v0.3.10).
Merged upstream: `cf0211b`, including v0.1.9 and subsequent nested-shell fixes.
The merge incorporates 103 upstream commits, including review and task records.

## Integration decisions

- Keep the fork's package name, version, app id, artwork and release repository.
- Integrate terminal fixes and the pinned alacritty fork, Chinese input fixes,
  wheel handling, process ownership and nested-shell prompt recognition.
- Integrate language selection, explicit start folders, and row drag/menu switches.
  The switches control the fork's existing rows and context menus.
- Keep agent hooks, animated characters, drawers, session restoration, Files
  actions, prompt rail and activity panel. Shared menus/settings are localized;
  some fork-specific interface labels retain their original language.
- Keep the fork's automatic installer and six-hour checks. Upstream's separate
  notification-only updater remains removed; its notification-specific fixes
  do not apply to the fork's title-bar update flow.
- Restore the first session through an explicit-folder launch, so WSL's new
  home flag cannot override the saved working directory.
- Keep the shell marker `aid=elyterminal` paired with upstream's parser.
- Adapt Linux tar, DEB, RPM and AppImage scripts and workflow paths to the fork.
  The Linux build baseline is Ubuntu 22.04 / glibc 2.35.

## Validation

- macOS ARM64: `scripts/check.sh` passed, including formatting, all-target
  Clippy with warnings denied, 386 unit tests, the font harness, and house rules.
  One existing test remains ignored.
- Linux ARM64 container: 389 unit tests and seven input-method integration tests
  passed; one existing test remains ignored.
- Linux tests used the merge before the final explicit-folder restoration
  adjustment; the final tree is checked by Linux all-target Clippy and macOS tests.
- Linux packaging shell syntax, ShellCheck and Python syntax passed.
- Fish was installed with Homebrew for local tests. PowerShell 7.6.9 was run
  from a temporary download on macOS and inside the disposable Linux container.
- Windows runtime, interactive GUI acceptance, generated package installation,
  signing, release publication and remote CI are not verified by this merge.

Upstream task and acceptance documents retain upstream's historical results;
those entries do not certify the fork's runtime or release artifacts.
