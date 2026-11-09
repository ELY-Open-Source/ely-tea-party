# Upstream synchronization, 2026-11-08

Base: fork `3b31440` (v0.3.18).
Merged upstream: `fdca49a`, including v0.2.1, v0.2.2 and v0.2.3.
The merge incorporates 109 upstream commits, including review and task records.

## Integration decisions

- Keep the fork's package name, version, app id, artwork, README and release
  repository. Upstream's `v0.2.x` tags are not kept: the fork's version numbers
  are its own.
- Left out as upstream's alone: `.github/workflows/website.yml`, which on a
  published release would deploy to upstream's terminal.elygpui.com with the
  org's secrets; `scripts/package-managers.sh`, `packaging/homebrew` and
  `packaging/winget`, which publish ELYTerminal; `README_zh.md` and the user
  guides under `docs/en_us` and `docs/zh_cn`. CI keeps the fork's jobs.
- Integrated: profiles beside +, a folder named on the command line, Explorer's
  menu, the interface's font and size, a CJK font, font weight, antialiasing,
  the background picture's size, position, repeat and scope, opacity down to
  0%, the cursor's glide, hover underlines, right click copies or pastes, the
  Windows mouse and paste fixes, and the rule thickness rounding.
- Files keeps the fork's tree. It takes upstream's polling each second while
  Files shows, the marks kept across a failed Git, and the pruning of folders
  that went away, on the tree's open set. Ely's `TreeDrag` cannot be built
  outside Ely, so a row drags the fork's `FileDrag`, which the terminal and
  drawers take, and types the path as upstream does.
- The right column keeps the fork's tabs and Activity. A right press on Files
  shows upstream's Choose folder row.
- Sessions keep the fork's drag; upstream's `workspace/reorder.rs` is left out.
- Links keep the fork's paths and file menu, on upstream's bounded token: a
  long line no longer scans whole. Paths are read inside one token.
- Right click pastes, when on, takes every right press, so on Windows, where it
  defaults on, the terminal's file menu needs it off.
- A folder named on the command line opens first and shown, and the saved
  sessions come back after it, so naming one loses none of them.
- A link argument, `elyteaparty://`, stays the handoff's and is not read as a
  folder.

## Known limits

- A profile's session comes back at the next start in its folder under the set
  shell, without the profile's program or arguments: the sessions file keeps
  neither.

## Validation

- macOS ARM64: formatting, all-target Clippy with warnings denied, 630 unit
  tests, the hooks installer tests and house rules passed; two existing tests
  remain ignored. PowerShell's folder test fails here because `pwsh` is not
  installed on this Mac.
- `zsh_keeps_one_prompt_when_the_line_typed_outgrows_the_screen`, upstream's
  test, unchanged, fails now and then here: once in 12 runs alone, and in some
  full runs. It finds a third prompt after a resize. The cause is not known.
- The built app started under a temporary `HOME` with a folder named on its
  command line and opened its first session there.
- Windows and Linux compile and runtime, the Windows-only tests that were
  renamed, interactive GUI acceptance, package installation, signing, release
  publication and remote CI are not verified by this merge.
