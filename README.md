# Ely Tea Party

A fork of [ELYTerminal](https://git.zacharyzhang.com/ZacharyZhang-NY/ELYTerminal) by Zachary Zhang, made with his permission, forked at `8bc94ed` (v0.1.1). The terminal, its engine and its design are his; this fork adds one opinion on top: **a terminal for running coding agents**. Each session running an agent shows it as an animated chibi character whose pose follows what the agent is doing, so a glance at the left column says who is working, who is thinking and who waits for you.

A terminal for macOS, Windows and Linux, in Rust on GPUI and Ely components. Sessions sit on the left, the terminal in the middle, Git or Files on the right. Every setting has a control in the settings window.

## Agent characters

| Agent | Character |
|---|---|
| `claude` | Claude学姐 |
| `codex` | GPT娘 |
| `dscode`, `deepseek` | 鲸鱼娘 |
| `grok` | Grok娘 |
| `kimi` | Kimi兔 |
| `gemini`, `antigravity`, `agy` | Gemini星 |
| `pi`, `omp` | Pi酱 |
| `opencode` | OpenCode娘 |

- **Zero setup.** Once a second each session's foreground job is read (its process group leader's command line, `node …/codex` and `npx @openai/codex` included). An agent there gets its character. Without hooks it works while it draws, idles when quiet, and waits for you after a bell.
- **Hooks, for the real state.** `ely-tea-party hooks install` adds hooks to `~/.claude/settings.json` (backed up first) and prints Codex's `notify` line. Each session carries `ELY_SESSION`, `ELY_SOCKET` and `ELY_HOOK`; the hook runs `$ELY_HOOK hook claude`, which reports to the app's socket, and does nothing in any other terminal. With hooks, a quiet working agent shows as thinking, a permission prompt as waiting, and Stop as idle.
- **Poses.** Waiting: waving with a red "!". Working: turns every 15 s through the main pose, research, a special move and slacking off. Quiet 6 s while working: thinking. Idle: the character's leisure pose; after 10 min, asleep.
- **Cmd-Shift-U** (Ctrl-Shift-U off macOS) shows the first other session whose agent waits for you, else one that rang.
- Detection reads processes on macOS and Linux; on Windows sessions show no characters yet.

The art comes from cmux-todd-ui, a separate project that draws the same characters for cmux; `scripts/agents.py PATH/TO/cmux-todd-ui/sidebar/assets` turns its APNGs into the 112 px animated WebPs in `assets/agents`, which are built into the binary.

## Install

No release yet: build from source, below. On macOS, `scripts/mac-app.sh` makes `Ely Tea Party.app` and a disk image; on Windows `scripts/windows-zip.ps1` a zip of `ely-tea-party.exe`; on Linux `scripts/linux-tar.sh` a tarball with `net.toddzheng.ely-tea-party.desktop` and its icon, whose desktop entry runs `ely-tea-party` from `PATH`.

## Build and run

Rust 1.95.0, which `rust-toolchain.toml` pins; rustup installs it on the first build.

```sh
cargo run --release
```

Each platform's script builds its release in `target/release`: on macOS, `scripts/mac-app.sh` builds `Ely Tea Party.app`, with its icon, and a disk image of it; on Windows, `scripts/windows-zip.ps1` zips `ely-tea-party.exe`, its icon inside; on Linux, `scripts/linux-tar.sh` builds in Docker and packs the binary with its desktop entry and icon.

On Linux, install the libraries gpui links first. On Debian and Ubuntu:

```sh
sudo apt-get install pkg-config libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libxcb1-dev libx11-xcb-dev libfontconfig-dev libfreetype-dev
```

The app writes its log to stderr. `RUST_LOG=ely_tea_party=debug` writes more; `trace` logs each paint. A Windows release build opens no console: `ely-tea-party.exe 2> log.txt` keeps its log.

## What it does

- The terminal runs on `alacritty_terminal`: 256 colors and truecolor, bold, dim, italic, five underline styles, wide characters and combining marks. Box drawing, block elements and powerline shapes are drawn as geometry, so lines join across cells. Keys follow xterm and the kitty keyboard protocol; input methods compose at the cursor; programs can track the mouse; pastes are bracketed when a program asks; OSC 8 and plain links open with the link press below; find searches the scrollback.
- Shell integration for zsh, bash, fish and PowerShell: each prompt reports its folder and title.
- Sessions: new, close and switch. A bell marks a hidden session's row and makes no sound.
- A button at each end of the title bar hides or shows its side column; the window opens as it was left. On Windows and Linux a gear beside the right one opens Settings.
- Files shows the session's folder as a tree with Git's marks.
- Git shows the branch, stages, unstages, discards, commits, lists recent commits and opens diffs, all through the `git` command line.
- Themes: Ely Dark, Ely Light, Catppuccin Mocha, Macchiato, Frappé and Latte, Tokyo Night, Dracula, Nord, Gruvbox Dark, Rosé Pine, Solarized Light, One Light and Kanagawa Wave. A second theme can take over while the system is dark. On Omarchy 4, an Omarchy theme follows the system's theme as it switches, and is the default there.
- Background opacity and blur: NSVisualEffectView on macOS, acrylic on Windows, `ext_background_effect_v1` or `org_kde_kwin_blur` on Wayland, `_KDE_NET_WM_BLUR_BEHIND_REGION` on X11, which KWin reads. Blur shows only while the opacity is under 100%. Hyprland blurs only while its own `decoration:blur:enabled` is on, and Omarchy 4.0.4 turns it off; to turn it on there, add `hl.config({ decoration = { blur = { enabled = true } } })` to `~/.config/hypr/looknfeel.lua`.

## Keys

Settings › Terminal lists them all, as this platform binds them.

| | macOS | Windows and Linux |
|---|---|---|
| New session | Cmd-T | Ctrl-Shift-T |
| Close session | Cmd-W | Ctrl-Shift-W |
| Session 1 to 8, last | Cmd-1 to Cmd-8, Cmd-9 | Alt-1 to Alt-8, Alt-9 |
| Next, previous session | Ctrl-Tab, Ctrl-Shift-Tab | Ctrl-Tab, Ctrl-Shift-Tab |
| Sessions column | Cmd-B | Ctrl-Shift-B |
| Right column | Cmd-Alt-B | Ctrl-Shift-Alt-B |
| Copy, paste | Cmd-C, Cmd-V | Ctrl-Shift-C, Ctrl-Shift-V; on Windows Ctrl-V pastes too |
| Select all | Cmd-A | Ctrl-Shift-A |
| Find | Cmd-F | Ctrl-Shift-F |
| Bigger, smaller, reset size | Cmd-=, Cmd-minus, Cmd-0 | Ctrl-=, Ctrl-minus, Ctrl-0 |
| Open a link | Cmd-press | Ctrl-press |
| Settings | Cmd-, | Ctrl-, |
| Minimize | Cmd-M | |
| Quit | Cmd-Q | Ctrl-Shift-Q |

## Settings

The settings window saves each change at once to `settings.json` in the platform's config folder:

- macOS: `~/Library/Application Support/ely-tea-party/settings.json`
- Linux: `$XDG_CONFIG_HOME/ely-tea-party/settings.json`, else `~/.config/ely-tea-party/settings.json`
- Windows: `%APPDATA%\ely-tea-party\settings.json`

A file that does not parse, or holds a value out of range, stops the start with an error naming the file and the field. The app never overwrites it.

## Development

`scripts/check.sh` runs fmt, clippy, the tests and the 500-line rule. `AGENTS.md` lists the rest: the layout, the decisions and the tools.

## License

MIT, in `LICENSE`, as upstream ELYTerminal; the fork keeps its copyright notice. Box drawing and key encoding follow Ghostty's code, MIT, in `LICENSE-GHOSTTY`. The theme files come from iTerm2-Color-Schemes, MIT, with its license in `assets/themes`. JetBrains Mono and its Nerd Font build are under the SIL Open Font License, in `assets/fonts`.
