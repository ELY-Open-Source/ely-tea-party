# Ely Tea Party

A fork of [ELYTerminal](https://git.zacharyzhang.com/ZacharyZhang-NY/ELYTerminal) by Zachary Zhang, made with their permission, forked at `8bc94ed` (v0.1.1). The terminal, its engine and its design are theirs; this fork adds one opinion on top: **a terminal for running coding agents**. Each session running an agent shows it as an animated chibi character whose pose follows what the agent is doing, so a glance at the left column says who is working, who is thinking and who waits for you.

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
- **Hooks, for the real state.** `ely-tea-party hooks install` installs hooks for every agent it finds on `PATH`: Claude Code, Codex, Gemini, Antigravity, Grok, Kimi Code, dscode, and plugins for OpenCode, Pi and oh-my-pi (`hooks install codex kimi` picks some). Each file it changes is backed up first as `NAME.ely-tea-party.bak`, and running it again changes nothing. Each session carries `ELY_SESSION`, `ELY_SOCKET` and `ELY_HOOK`; a hook runs `$ELY_HOOK hook AGENT`, which reports to the app's socket, and does nothing in any other terminal. With hooks, a character shows what its agent does: reading, editing, running a command, waiting for you, done.
- **Poses.** Waiting: waving with a red "!". Working: turns every 15 s through the main pose, research, a special move and slacking off. Quiet 6 s while working: thinking. Idle: the character's leisure pose; after 10 min, asleep.
- **Cmd-Shift-U** (Ctrl-Shift-U off macOS) shows the first other session whose agent waits for you, else one that rang.
- Detection reads processes on macOS and Linux; on Windows sessions show no characters yet.

The art comes from cmux-todd-ui, a separate project that draws the same characters for cmux; `scripts/agents.py PATH/TO/cmux-todd-ui/sidebar/assets` turns its APNGs into the 112 px animated WebPs in `assets/agents`, which are built into the binary.

## Install

Each release on GitHub (Releases of `qiz029/ely-tea-party`) carries three builds.

- macOS 11 or later, Apple silicon: open `ely-tea-party-VERSION.dmg` and drag Ely Tea Party into Applications. The app is signed with Developer ID and notarized.
- Windows 10 or later, x86_64: unzip `ely-tea-party-VERSION-windows-x86_64.zip` and run `ely-tea-party.exe`. It is unsigned; if SmartScreen stops it, press More info, then Run anyway.
- Linux, x86_64, glibc 2.35 or later, with libxkbcommon, libxkbcommon-x11 and libxcb:

```sh
tar -xzf ely-tea-party-VERSION-linux-x86_64.tar.gz
cd ely-tea-party-VERSION-linux-x86_64
install -Dm755 ely-tea-party -t ~/.local/bin
install -Dm644 net.toddzheng.ely-tea-party.desktop -t ~/.local/share/applications
install -Dm644 net.toddzheng.ely-tea-party.svg -t ~/.local/share/icons/hicolor/scalable/apps
```

The desktop entry runs `ely-tea-party`, so `~/.local/bin` must be on `PATH`.

## Build and run

Rust 1.95.0, which `rust-toolchain.toml` pins; rustup installs it on the first build.

```sh
cargo run --release
```

Each platform's script builds its release in `target/release`: on macOS, `scripts/mac-app.sh` builds `Ely Tea Party.app`, with its icon, and a disk image of it; on Windows, `scripts/windows-zip.ps1` zips `ely-tea-party.exe`, its icon inside; on Linux, `scripts/linux-tar.sh` builds in Docker and packs the binary with its desktop entry and icon.

To release, dispatch the Release workflow on GitHub, which drafts `vVERSION` and puts the Linux and Windows builds on it. Then, on a Mac whose keychain holds a Developer ID Application identity, with an App Store Connect API key:

```sh
SIGN_IDENTITY="Developer ID Application: NAME (TEAMID)" \
NOTARY_KEY=~/.appstoreconnect/AuthKey_KEYID.p8 NOTARY_KEY_ID=KEYID NOTARY_ISSUER=ISSUER-UUID \
  scripts/mac-app.sh
scripts/release-asset.sh target/release/ely-tea-party-VERSION.dmg
```

On Linux, install the libraries gpui links first. On Debian and Ubuntu:

```sh
sudo apt-get install pkg-config libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libxcb1-dev libx11-xcb-dev libfontconfig-dev libfreetype-dev
```

The app writes its log to stderr. `RUST_LOG=ely_tea_party=debug` writes more; `trace` logs each paint. A Windows release build opens no console: `ely-tea-party.exe 2> log.txt` keeps its log.

## What it does

- The terminal runs on `alacritty_terminal`: 256 colors and truecolor, bold, dim, italic, five underline styles, wide characters and combining marks. Box drawing, block elements and powerline shapes are drawn as geometry, so lines join across cells. Keys follow xterm and the kitty keyboard protocol, and on Windows win32-input-mode, as Windows Terminal sends them, so herdr's shortcuts work; input methods compose at the cursor; programs can track the mouse; pastes are bracketed when a program asks; OSC 8 and plain links open with the link press below; inside herdr, herdr opens them, web links only, whole where its pane wraps them, and with Shift the terminal opens what it sees; find searches the scrollback.
- Shell integration for zsh, bash, fish and PowerShell: each prompt reports its folder and title, and OSC 133 marks each prompt, each command and its exit status.
- Ely peeks over the window's top edge and moves all the time: she breathes, sways and blinks. She follows your sessions: she hops when a program waits on you, shakes at an error, cheers a finish, reads along while a program works, and dozes when all is calm for long. Press her and she winks. Settings › Terminal › Ely turns her off.
- Sessions: new, close and switch. A bell marks a hidden session's row and makes no sound. A row's mark shows what its programs report through OSC 7501, OSC 9;4 or herdr: waiting on you, an error, work with its progress, done or idle, else a failed last command. The mark is 3 by 3 dots: work ripples and waiting dips while they last. Hover over the mark lists each report.
- Notifications: while the window is in the background or the session hidden, a session that starts waiting, finishes or fails, and a program's OSC 9, 777 or 99, post a system notification, one a session every 3 seconds at most; a press shows the session. On macOS only the app bundle posts them, not `cargo run`.
- Cmd-Up and Cmd-Down, Ctrl-Shift-Up and Ctrl-Shift-Down off macOS, jump to the prompt before or after the view's top.
- A button at each end of the title bar hides or shows its side column, and a drag on the border between a side column and the terminal resizes it; the window opens as it was left, at the size and widths it was left at. On Windows and Linux a gear beside the right one opens Settings.
- Files shows the session's folder as a tree with Git's marks. Inside herdr the folder is that of herdr's focused pane.
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
| Drawer (your own shell under the session) | Cmd-J | Ctrl-Shift-J |
| Tea party (every session at one table) | Cmd-Shift-O | Ctrl-Shift-O |
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
