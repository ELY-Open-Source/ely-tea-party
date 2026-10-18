# Acceptance

`PRD.md` line by line, as of T23. Status: **Pass** when a test, a run or a review shows it; **Partial** when part of it was shown; **Open** when nothing has shown it yet. Reviews are in `tasks/reviews.md`; tests are named by module (`cargo test -- MODULE`).

What no check reached: no real KWin drew the blur; of Wayland desktops, only Hyprland, on the owner's Omarchy, has drawn it. On 2026-10-15 macOS gave `shot` the window's ground alone, for old and new builds alike, while the terminal it ran under lacked Screen Recording (`CGPreflightScreenCaptureAccess` false); that day's macOS checks read the app's log.

## 1. Product

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| One window: sessions left, terminal middle, Git or Files right | T11 | T11 review; a real X11 window shows the terminal and the right column | Pass |
| Every setting has a control in the settings window | T16–T18, T30, T33 | 17 of the 24 fields have a row; `show_sessions`, `show_panel` and `panel` are the main window's toggles and switch, `sessions_width` and `panel_width` its columns' borders, and `window_width` and `window_height` its size, which §6 keeps in the file | Pass |
| Silent: no sound, bell, notification but a session's, update check, telemetry, tip or onboarding | T10, T12, T40 | a bell is logged and marks a hidden row; the code holds no network call, and its one notification is a session's, through gpui's, which sets no sound on macOS and a silent toast on Windows; on Linux the notification daemon decides | Pass |
| Clean: no subtitle, tagline, logging option, status bar or badge without state | T11–T18 | reviews of the layout and settings tasks | Pass |
| Built-in themes, each with a preview, Catppuccin among them; a theme colors terminal and chrome | T03, T17 | `themes::tests`; T17 review | Pass |
| The background blurs on macOS, Windows, Wayland and X11 | T19–T21 | see §11 | Partial |

## 2. Foundation

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| One binary crate `elyterminal`, edition 2024, Rust 1.95 | T01 | `Cargo.toml` | Pass |
| Ely by pinned rev; gpui and gpui_platform from `gpui-pre` at Ely's version | T01 | `Cargo.toml`: Ely `a2cb531`, `gpui-pre` `=0.3.8` | Pass |
| `alacritty_terminal` 0.26 | T04 | `=0.26.0` | Pass |
| Logs through `log` to stderr, `RUST_LOG` sets the level, no log setting | T01 | `src/main.rs`; no such field in `Settings` | Pass |
| CI on Gitea Actions: macOS builds, tests and lints; Ubuntu and Windows build and test | T01, T23 | `.github/workflows/ci.yml`; run 926 on `65bb4d3`, see the CI notes | Pass |
| The repo, private until v0.1.1 and public since; no credential in any file or commit | T01, T26 | T22's review scanned the tree and its 398 reachable objects: 0 hits; T23's scans for the Gitea token and the runners' SSH details: 0 hits; before the owner made it public, a scan of all 91 commits' contents and messages found no token, SSH password, gateway, port, SSH user or private key | Pass |

## 3. Settings model and storage

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| One `settings.json` in `dirs::config_dir()/ELYTerminal` | T02 | `settings::store` tests | Pass |
| Every field has a default; a missing file or field means its default | T02 | `settings::tests`, `settings::store` tests | Pass |
| A bad file or value stops the start, naming file and field; never overwritten | T02 | `a_bad_file_is_an_error_naming_it`, `a_bad_value_is_an_error_naming_the_field` | Pass |
| Temp file in the same folder, then rename; one writer | T02 | T02 rounds 1–2 | Pass |

## 4. Themes

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| The 14 built-in themes | T03 | `assets/themes`, byte for byte with upstream `21f281d`; Ely's two | Pass |
| Background, foreground, cursor, cursor text, selection, 16 ANSI from upstream | T03 | `themes::tests` | Pass |
| A theme maps onto Ely's `Palette` through `Theme::set_palette` | T03, T19 | `the_chrome_wears_the_theme` | Pass |
| One theme, or a second for dark mode | T17 | T17 review | Pass |
| On Omarchy 4, an Omarchy theme wears the current theme and follows each switch; the default there; another pick stops following | T27 | `omarchys_ghostty_conf_reads_without_its_glass`, `omarchys_file_is_worn_again_once_it_reads`; on X11, as a user, with Omarchy v4.0.4's own renderer staging and swapping the folder as `omarchy-theme-set` does: with no theme saved the window wore Omarchy's, and through all 22 of its themes the terminal's ground matched each background within 1.5 s; Omarchy's glass lines changed nothing; a file without palette 7 was logged and the last theme stayed; a file swapped in unreadable was worn once made readable, its stamp unchanged; a folder unreadable for 2 s kept the theme; each failure was logged once while it lasted; a picked Catppuccin Mocha stayed through a switch, and with it worn a bad Omarchy file was logged and the start went on; with Omarchy's folder unreadable the start stopped, a theme saved or not, and with settings wearing it and no Omarchy too, each naming the file and the error; the first release build, on the owner's Omarchy 4.0.4 under Hyprland 0.56.2, wore the live theme, sepia-night, which is none of Omarchy's 22, and the release build from `aae7fba`, under Xvfb on Debian 13 with that theme's folder, painted its background | Pass |

## 5. Terminal engine

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| PTY through alacritty's event loop; OSC 7 and OSC 133 scanned on the reader | T04, T34 | `term::pty`, `term::tap` tests; round 3 failed at the cap, its fix stands on a red-green test | Pass |
| Shell integration for zsh, bash, fish, PowerShell; a shell without it reports nothing and the panels say so | T05 | `term::shell` tests on macOS and Linux | Pass |
| Prompt marks, OSC 133, from the shell integration and any program; each session keeps its last command's status | T34 | `term::marks` tests: each letter and option as Ghostty reads them; `term::tap` and `term::pty` tests: marks split across reads reach the session in order; `term::shell` tests: zsh, bash, fish and PowerShell send C, D;1, A and B around a failing command, and the user's prompt still shows the failure, a zsh theme's that sets PS1 in its own hook too, whose first prompt is marked before any command, and `$pipestatus` stays; bash's lone subshell ends with D;7, `$_` stays, a command runs under `extdebug` after a failure, a key binding marks nothing, the user's DEBUG trap runs, bash 3.2 sends no C, and bash says at start why it marks nothing while `promptvars` is off; PowerShell reads 1 for a cmdlet's failure under `-ErrorAction Ignore` after a success, a native code twice, a native failure's code under `$PSNativeCommandUseErrorActionPreference`, on a line that mixes a program and a cmdlet the cmdlet's 1, and through a call by variable the program's code; on macOS fish 4.9 marks itself and bash 3.2 marks no C, in the Linux container fish 4.0 gets B and bash 5.2 marks C through PS0, CI's Ubuntu runs fish 3.7, which the script marks whole; by hand in a pty, PowerShell under strict mode, and a cmdlet's failure after a native one's reads 1 | Pass |
| Jump to the prompt before or after the view's top, as Ghostty jumps | T35 | `term::prompts` tests: a mark becomes its prompt's numbered link, read as the session reads marks, whole when split across reads at every byte, and other sequences, `133;AB` among them, pass as they came; a prompt's cells carry its link through SGR 0 and a resize, its input none; a program's own link and the prompt's keep each other, a program's link inside a prompt carries the prompt's number, a prompt inside a link left open too, a target of 264 bytes in either order; controls after an OSC's closing ESC keep their line; a long escape of controls keeps the pty reading; a prompt over rows starts at its first, blank rows within it too; up and down land as Ghostty's `scrollPrompt`; `term::links`: a prompt's link is no target; `term::pty`: through the event loop the prompt's cell alone carries it; in the app on macOS, Cmd-Up once and twice put PROMPT-3 and PROMPT-2 at the top, and Cmd-Shift-Down and Cmd-Down came back to the screen's bottom; the review's fixtures of a prompt that starts with a link and of a prompt with a blank row jumped to PROMPT-3-FIRST, and its fixture of `133;AB` marks found no prompt; the second review's fixtures of a prompt all in a link and of a linked first row jumped to PROMPT-3-FIRST. The keys off macOS are bound, not run | Pass |
| Program status, OSC 7501 revision 0.4: reports checked as the spec checks them, records per terminal, a new prompt and a full reset, the support query | T36 | `term::status` tests: the spec's examples read as it means them; a malformed pair is skipped, the last of a key wins, unknown keys pass; a sequence over any limit, with bad base64, UTF-8 or a control character, or without a known state drops whole, a clear too, and an earlier pair of a key counts though the last wins; the sequence's length counts the controls its body skips; kind and progress count only with their states; records replace whole, clear with those under them, and past 256 the one updated longest ago goes; a new prompt keeps done and error alone. `term::tap`: a full reset found between bodies, and through DEL and high bytes in its escape. `term::pty`: a report, a query and a full reset reach the session in order through the event loop. In the app on macOS, a program that sent the query and then DA1 read the answer with every state and kind, then DA1's | Pass |
| Progress, OSC 9;4, as the root record until an OSC 7501 report; notifications, OSC 9, 777 and 99, read | T37 | `term::notify` tests: OSC 9 reads as Ghostty reads it, ConEmu's commands no notifications, 9;4's progress clamped; OSC 777's notify; OSC 99 gathers chunks by id, base64 split before encoding or after and a character across chunks among them, keeps the newlines base64 carries, reads options, close, query and alive, and drops what breaks its rules with what it gathered; its answers name the asker's id. `term::tap`: an OSC 99 body keeps its metadata and a full payload. `term::status`: 9;4 maps to the root record until a report, and again after a full reset. `term::pty`: OSC 9, 777, chunked 99 and 9;4 reach the session in order, and a full encoded chunk with the lines base64 carries reaches it as one notification. In the app on macOS, a program read OSC 99's query and alive answers before DA1's, and four notifications reached the workspace's log | Pass |
| Inside herdr, the folder of herdr's focused pane, read from its socket, on macOS, Linux and Windows | T29, T32 | `term::herdr` tests: the socket found as herdr 0.9 finds it, only the terminal client followed and a remote one refused, the snapshot read, a socket round trip, a process's path, arguments and variables; `the_foreground_is_the_program_s_own_group`; on X11 with herdr 0.9.5 run in the session: its start showed the focused pane's folder, a cd in the pane moved the panel, the focus moved to a pane in `/etc` and back moved it each time, and the client's end gave the panel back to bash's report, then `exec herdr` from that bash, with `HERDR_SOCKET_PATH` relative to its folder, was followed through the socket there; on macOS under `shot`, herdr 0.9.5 as the program and again by `exec` from `sh` with a relative socket, its pane's cd moved the panel; on Windows, `term::herdr` tests on the CI runner: the folder under `APPDATA` and `USERPROFILE`, the `herdr.exe` nearest under the shell, this test's name, arguments and variables read through the live process list and `sysinfo`, a round trip over a named pipe; no real herdr has run on Windows yet | Partial |
| Inside herdr, herdr's agent states are the session's records | T38 | `term::herdr` tests: each agent in a state herdr knows is its pane's record, its app, title and message from herdr's fields, unknown and new states passed by, and a snapshot without agents lists none. `term::status`: herdr's agents replace their own records alone, a program's `herdr/build` kept through an empty snapshot, a full one and herdr's leaving, since a program's id ends at a colon; the same agents change nothing, and the next snapshot puts them back after a full reset | Pass |
| A row shows its session's status, hover each record; a hidden session that starts waiting marks its row | T39 | `term::status` tests: blocked, then error, working, done and idle show, the record reported last in its state speaks; an app comes from the nearest ancestor that names one; typing sees done and error, herdr's aside; each change of the state a row shows is noted, a wait after work in one output among them. `sessions` tests: hover lists each record's state, app, title and message, ten at most, then a failed command's status; a program's text shows on one line without direction controls or invisible formatting, joiners kept, cut at 120 characters. In the app on macOS, `shot` showed each mark: the ring at 40%, the still loader, a shield over three records whose hover listed each, the root's app inherited, a check, a cross, a hollow circle, an alert after `133;D;1`, a hidden session's question mark with its title strong, and a done record gone after a typed key; after review, a hidden session that waited, worked and waited again in one output set its title strong, an open hover followed a record replaced under it and work from 20% to 80%, ten long records scrolled to the last within the window, and a selected row at 0% in Ely Dark showed its loader; after round 2, herdr's records replaced in the same state showed at once in an open hover, and a hidden session that waited and worked again in one output set its title strong | Pass |
| A row's mark moves: work ripples from the centre, progress fills the outer ring clockwise from the top, waiting stays lit and dips, a settled state draws its shape and holds; the window draws at the marks' steps only while one moves | T42 | `sessions::orb` tests: the ripple peaks at the centre, then every edge, then every corner, once every 15 steps; at 50% the first four ring dots are lit, the next pulses and the rest stay dim, and at 100% the centre pulses; waiting dips on the 1st, 2nd, 5th and 6th of 20 steps, every dot alike; a done check draws from its left end and holds after 5 steps, idle is a ring; work and waiting never stop. `sessions` test: the mark takes the most pressing look, a failed command as an error. In the app on macOS, at 70% glass in Ely Dark, the recorder's stills showed each look in one column: the ripple, 40% filled, waiting lit, a check, two crosses and a ring; two stills 0.4 s apart showed the ripple at two steps | Pass |
| System notifications on macOS, Windows and Linux: a session that starts waiting, finishes or fails, and OSC 9, 777 and 99, while the window is inactive or the session hidden, one a session every 3 s; a press shows the window and session | T40 | `term::reports` tests: a session posts once in 3 s, its own under one tag; a program's notifications go by their id and live until pressed, closed or expired, an expiry taking only its own post and none a later post holds; a session keeps its last 64; the text shows as text, and as markup's text on Linux; a session posts unless it is the one shown in an active window; waiting, done and error post, work and rest do not; a tag names the session that posted it, by itself, an id or a count. `term::titles`: OSC 0 and 2 set the title as vte splits it, 15 parts past the number at most, and as alacritty joins it; CSI 22 t and 23 t save it and bring it back; a full reset empties both. `term::tap`: the scanner finds CSI 22 t and 23 t as vte reads them; a title's body holds 65536 bytes, another's 4096. `term::pty`: titles, notifications and full resets reach the session in the output's order, one write and a write a sequence alike, a title of 5000 bytes and one of 17 parts among them. `term::notify`: the query, on Linux without close and `w`, alive, untracked and activation answers. In the app on macOS, outside a bundle, where gpui posts nothing and the log names each post: an OSC 9 posted and an OSC 777 within 3 s was dropped; an OSC 99 with `c=1` and `w=800` posted, its program heard `p=close;untracked`, alive listed it, and once it expired alive listed none; a closed one left alive empty; the query's answer came back; a command's end without its C posted nothing, and a failed one after its C posted; a notice after a title set, or after a full reset, in the same write named the session as split writes do, as did one after a full reset and 2 KB to 32 KB more of output, and a title of 17 parts or of 4100 bytes named the notice as it named the row, one write or two; a notice, alive and DA1 in one write came back as alive naming it, then DA1's answer; a notice set to expire whose session ended expired after it; a hidden session that waited and worked again in one output posted its wait; and a notice sent while the window was active and the session shown posted nothing, the next, once the window lost focus, posted. No system showed one yet: on macOS gpui posts only from the app bundle, which asks the owner's leave first, and Windows and Linux did not run it; a press needs the system's notification center, so the owner posts and presses one on each | Partial |
| Colors, styles, five underlines, wide characters, combining marks | T06 | `term::frame`, `term::ink`, `term::paint` tests | Pass |
| Box drawing, blocks and powerline shapes as geometry | T07 | `term::boxes` tests | Pass |
| JetBrains Mono, any installed font, Nerd Font icons | T06, T17 | `tests/fonts.rs` | Pass |
| Cursor shapes, blink by setting and DECSCUSR, hollow unfocused | T06 | T06 review | Pass |
| xterm keys, cursor mode, Option as Alt, kitty protocol, keypad | T08 | `term::keys` tests | Pass |
| IME: marked text at the cursor, commits to the program | T08 | T08 review | Pass |
| Selection by character, word, line; Shift extends; mouse reporting; wheel | T09 | `term::mouse`, `term::tap` tests | Pass |
| Copy, paste, bracketed paste, copy on select, OSC 52 | T09 | T09 review | Pass |
| OSC 8 and plain links on Cmd-press, Ctrl-press off macOS | T09 | `term::links` tests | Pass |
| Focus reporting, title, scrollback limit, find | T10 | `term::find` tests; round 3 failed at the cap, its fix stands on a red-green test | Pass |
| A bell makes no sound; the row marks it | T10, T12 | T12 review | Pass |

## 6. Main window and layout

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| `ColumnShell` with three columns; blur when the setting is on | T11, T19 | `shot` names a `BlurredView` under the window | Pass |
| Columns toggle and keep their state | T11, T23 | a click on each title bar button hid its column and saved it, and a restart opened the window with both hidden (147 columns against 87); Cmd-B with the app active; Ctrl-Shift-B on a real X11 window | Pass |
| The window opens at the size it was left at; maximized or full screen, the size before stays; a kept size past the screen shrinks to it | T30 | `a_value_out_of_range_names_its_field`; on X11 under xfwm4: the first start opened at 1200 by 760 and wrote nothing; a resize to 900 by 600 was kept with one write; maximized by xfwm4's Alt+F10 the window spanned the 1400 by 900 screen while the file kept 900 by 600, and restored it was 900 by 600 again; maximized, minimized and restored, the file kept 900 by 600; resized to 950 by 650 and minimized at once, the file kept 900 by 600 while minimized and 950 by 650 once restored; the next start opened at 950 by 650 and wrote nothing; `WM_NORMAL_HINTS` names a 100 by 100 minimum, and a resize to 900 by 50 held at 900 by 100, kept so; a kept 3000 by 2000 opened at 1400 by 900, logged; on macOS, through gpui's own window: a kept 900 by 600 opened at 900 by 600, where a centered edge on a half point had grown it to 601, and a move wrote nothing; a resize to 923 by 617, covered by an opaque window within 150 ms and uncovered, was kept once shown, and the next start opened at 923 by 617; zoomed, minimized, restored, full screen and back, the file kept 923 by 617; a 100 by 100 window was kept; a kept 5000 by 4000 opened at the visible 1920 by 1113, logged; on Windows the owner dragged v0.1.2's window to a new size, closed it, and it opened again at that size; a mouse drag on macOS not yet shown | Pass |
| The sessions and right columns resize by a drag on their border toward the terminal, 160 to 960 pixels, kept in settings | T33 | Ely `5c2822c` and `005faef`: `shell::tests::border`: a border toward the filling column drags its width, before it by its right and after it by its left; a drag stops at each limit; a drag in a header moves no window in Windows', macOS' and Linux's style (the test window panics on a move); a drag sets off from the width drawn, which holds Windows' caption buttons inside the window; a short shell squeezes both columns, and a drag moves each border the pointer's way from the width kept, a drag with no sideways move changing none (Ely `0b4848b`): on macOS 900 and 900 kept in a 1200 window drew 572 each, and a drag of 20 from the left border saved 920 and moved it right, where review round 1 saw it jump to 435; on X11 900 and 900 in a 1380 window, a drag of 40 saved 940 and moved both borders right, and a drag with no sideways move kept them; a drag out and back to where it set off gives the width it had (Ely `da3fdca`): on macOS each border dragged 50 out and back kept 240 and 240 and came back to its pixel, where review round 2 saw 290 kept, and on X11 both settled at 240 with nothing written; a resizing column with no border toward a filling one fails; `a_value_out_of_range_names_its_field` (`panel_width` 1000); on macOS under `shot`, the sessions border dragged in the header and the right border in the body moved 50 points each, saved 290 and 290 once, and the next start opened at them; on X11 under xfwm4: a drag in the body set 300, one on the right border in the header set 300 and left the window at 0,0, drags stopped at 960 and 160, the next start opened at 160, and both borders paint the theme's border color, #37384A, as before; the line had painted half a pixel on macOS and none on X11 until Ely `005faef`; no real drag on Windows yet | Partial |
| The title shows the session's title, nothing else | T11, T40 | log: "the title is", per session shown; since T40 the title comes from the tap in the output's order, and the shell tests read each shell's title from it | Pass |
| The pointer over the terminal is the arrow | T26 | the terminal sets no pointer, so gpui's arrow shows; on X11 with the pointer over the grid, gpui asked for no I-beam ("text"), which it had asked for, and failed to load under Xvfb, before | Pass |
| The title bar is transparent | T11, T19 | T19 review | Pass |
| Every title bar control takes its press, Ely's window buttons on Windows and Linux too | T26 | Ely `cdc5ce8`: `window_controls_occlude_what_lies_under` (each style; before, the box under Windows' caption buttons took the press), `a_wobbling_press_on_a_slot_moves_no_window` (before, the window moved), `a_wobbling_press_on_a_column_title_moves_the_window`; on the owner's PC, window messages asked the app for Windows' hit test along the title bar: v0.1.0 answered caption from end to end, T26's build answers client over the toggles and +, caption over the titles, min, max and close over Ely's buttons, and a press on close closed the app; the owner, given v0.1.1's build and these controls to click, reported it works on Windows | Pass |

## 7. Sessions panel

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| Rows: title, then the folder, home as `~` | T12 | `sessions::tests` | Pass |
| New (+, Cmd-T), close (×, Cmd-W), select (press, Cmd-1…9, Ctrl-Tab) | T12 | 2026-10-15: + opened session 2, a press showed session 1 again | Pass |
| A program's end closes its row; the last row closes the window | T12 | T12 review | Pass |
| A bell in a hidden session marks its row | T12 | T12 review | Pass |

## 8. Right panel: Files

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| A switch at the top, centered below the title bar: Git, Files | T13, T26 | T13 review; a capture on X11 shows it centered below the title bar, the right toggle and Ely's window buttons in the bar above | Pass |
| The session's folder as a tree, folders first, Git colors | T13 | `files::tests` | Pass |
| Double press opens a folder, or a file in the default app | T13 | T13 review, with a logging `open` stub | Pass |
| Reloads on a new folder and at each prompt | T13 | log: "the tree shows", per folder reported | Pass |

## 9. Right panel: Git

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| Through the `git` command line | T14 | `git::tests` on temp repos | Pass |
| Branch, staged and unstaged, commit field and Commit, recent commits | T15 | `git::panel` tests; T15 review | Pass |
| Stage and unstage; a press opens the diff in `DiffViewer` | T15 | T15 review | Pass |
| Discard asks first | T15 | T15 review, in temp repos | Pass |
| Outside a repository it says so; a failed command shows its error | T15 | "Not a Git repository." on a real X11 window | Pass |
| Reloads at each prompt and on a folder change | T15 | log: "git panel: shows", per folder | Pass |

## 10. Settings window

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| Cmd-, opens it; categories left, rows right, each scrolling on its own; changes apply at once and save | T16, T26 | T16 review; on X11, eight wheel notches over the Terminal page carried it down to its Edit and View shortcuts while the category column and both headers stayed | Pass |
| On Windows and Linux, a gear in the title bar opens it too | T26 | on X11 under Xvfb with xfwm4, xdotool's click on the gear, just inside the right toggle, opened the 900×640 Settings window ("settings: the window opens"), after a click on + opened a session; sent through `App::dispatch_action`, the same click had opened nothing; the owner, given v0.1.1's build and the gear to click, reported it works on Windows | Pass |
| Terminal: Shell, Start in | T16, T26 | `settings::tests`; the General page is gone, and a capture on X11 shows its two rows leading Terminal | Pass |
| Appearance: theme grids, dark theme, font, size, line height, ligatures, cursor and preview, opacity, blur, padding | T17 | T17 review | Pass |
| On Omarchy, the Omarchy theme first in the grid | T27 | on X11 with Omarchy's theme folder, the Omarchy card leads the grid, chosen, over a preview in its colors | Pass |
| Terminal: scrollback, Option as Alt (macOS), copy on select | T18 | T18 review | Pass |
| Terminal: every shortcut, with a search | T26 | read from the keymap, the bindings that apply in the main window's terminal; on X11 the list showed this platform's keys (Ctrl+Shift+T, Alt+1 and on), and typing "paste" left Edit's Paste, Ctrl+Shift+V | Pass |

## 11. Background blur

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| macOS: `Blurred` (NSVisualEffectView) | T19, T23 | `shot` reads `BlurredView, blur 30` off the view's backdrop layer; the owner saw the desktop blur | Pass |
| Windows: `Blurred` (acrylic) | T19 | the same gpui call; on 2026-10-15 the owner saw the window blur on Windows | Pass |
| Wayland: `ext_background_effect_v1` beside gpui's `org_kde_kwin_blur` | T20, T28 | `blur::wayland` tests on a `wayland-server` compositor; on the owner's Omarchy 4.0.4, Hyprland 0.56.2 took `ext_background_effect_v1` from a T27 release build: the log named it and the pane's region, and the wallpaper behind the window blurred while Hyprland's blur was on and showed sharp while it was off; Plasma and GNOME not run | Partial |
| X11: `_KDE_NET_WM_BLUR_BEHIND_REGION`, removed when blur turns off | T21 | `blur::x11` test on Xvfb; the property on a real xfwm4 window; KWin not run | Partial |
| Neither protocol: transparency without blur; the log names the path | T20, T21 | the log lines in `src/blur` | Pass |

## 12. Menus and shortcuts

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| The macOS menu bar | T22 | `shot` lists every item with its key | Pass |
| The same shortcuts off macOS, Ctrl-Shift where they collide | T22 | xdotool on a real X11 window: select all, size, toggle, find, quit | Pass |
| On Windows Ctrl-V pastes too, as in Windows Terminal | T26 | the binding is Windows' alone; on a real X11 window, with a command on the clipboard through xclip, Ctrl-Shift-V pasted its 33 characters and the shell ran it, and Ctrl-V pasted nothing, as Linux keeps it for programs; the owner, given v0.1.1's build and Ctrl-V to press, reported it works on Windows | Pass |

## 13. Acceptance

| PRD line | Evidence | Status |
|---|---|---|
| Each task ticked with a passing review | each task is ticked only after its review, as `tasks/reviews.md` records; T04 and T10 reached the three-round cap without a passing round, their last fixes standing on red-green tests | Partial |
| `scripts/check.sh` passes | macOS: 135 tests, clippy, fmt, 500 lines; Linux through `scripts/linux.sh`: 138 tests, clippy, on aarch64 in Colima and on x86_64 in WSL's Docker | Pass |
| CI green on macOS, Ubuntu and Windows | run 955 on `43ea5b1`, v0.1.1's commit: all three jobs pass; see the CI notes below | Pass |

## 14. Icon and the macOS app

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| The icon: Ely, a girl with long pink hair, holding a terminal | T24 | `assets/icons`, drawn at 16 to 1024 px and legible at 32; the owner saw it | Pass |
| macOS: the rounded square; Windows and Linux: the figure alone, flat | T24 | `elyterminal-macos.svg` on Apple's grid; `elyterminal.svg`, which the Linux tarball carries and the Windows `.ico` renders | Pass |
| `scripts/mac-app.sh` builds the app, signed ad hoc, or for a release with Developer ID and notarized, and its disk image | T24, T26, T31, T41, T43 | its run: `codesign --verify --strict` passes on the app and on its copy in the image; the `.icns` holds 16 to 1024 px; the image verifies and mounts with the app beside Applications; LaunchServices opens the app as `com.zacharyzhang.elyterminal`; for v0.1.1 the Release workflow (run 959) ran it on `macos-signing`: notarization Accepted, the ticket stapled, and on this Mac the downloaded image and its app pass `codesign --verify --strict` under the Developer ID and Gatekeeper's `accepted, source=Notarized Developer ID`; the owner opened it; for v0.1.2 the Release workflow (run 1001) ran it again: the downloaded image's stapled ticket validates and Gatekeeper accepts it as `Notarized Developer ID`, and its app, version 0.1.2, passes `codesign --verify --strict --deep` under the Developer ID with the hardened runtime and is accepted the same way; for v0.1.3 the Release workflow (run 1157) ran it again, and its image and app, version 0.1.3, check the same way, as does the image rebuilt for T43 (run 1181); without the key the script stops before building | Pass |

## 15. Release

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| A Gitea release, `vVERSION`, with the macOS disk image, a Windows zip and a Linux tarball | T25, T26, T27, T31, T41, T43 | `v0.1.3` rebuilt for T43, at `eb96109`: the zip and the tarball from their scripts on the owner's PC and its WSL, the disk image from the Release workflow (run 1181), signed and notarized, each in place of the first build of its name, once CI run 1180 on that commit passed on macOS, Ubuntu and Windows; the tag moved from `7569f00` to `eb96109`, and the release's target with it; the notes give the new SHA-256 and say the builds were replaced; each file downloaded back anonymously matches its SHA-256, each binary holds the moving marks' log line and none the old ring's element id, and the tag's source archive holds `src/sessions/orb.rs`. The first `v0.1.3` builds, at `7569f00`: the zip and the tarball from their scripts on the owner's PC and its WSL, the disk image from the Release workflow (run 1157), signed and notarized; published once CI run 1156 on that commit passed on macOS, Ubuntu and Windows; each file downloaded back anonymously matches the SHA-256 in the release notes, and the tag named `7569f00`. `v0.1.2` at `4364802`: the zip and the tarball from their scripts on the owner's PC and its WSL, the disk image from the Release workflow (run 1001), signed and notarized; published once CI run 1000 on that commit passed on macOS, Ubuntu and Windows; each file downloaded back anonymously matches the SHA-256 in the release notes, and the tag names `4364802`. `v0.1.1` at `43ea5b1`: the zip from its script on the owner's PC, the disk image from the Release workflow, signed and notarized; the Linux tarball, rebuilt for T27 from `aae7fba` by its script on the owner's PC under Omarchy, took the first one's place, and the notes say so; each downloaded back anonymously, once the repository was public, matches the SHA-256 in the release notes. `v0.1.0` at `0075667`, its three files built from that commit by their scripts and checked again: the disk image verifies and its app passes `codesign --verify --strict`; the zip's executable has the icon and no C runtime DLL; the tarball opens its window on Ubuntu 22.04; each file downloaded back matches the SHA-256 in the release notes | Pass |
| The Windows executable carries the icon | T25, T31, T41 | its resources: icon group 1 with the seven images of `elyterminal.ico`, 16 to 256 px, and gpui's manifest (PerMonitorV2); a GUI program, importing no C runtime DLL; not yet seen on a Windows screen; v0.1.2's and v0.1.3's executables the same | Pass |
| The Linux tarball carries the icon and a desktop entry; the window names itself `com.zacharyzhang.elyterminal` | T25, T27, T41 | on Ubuntu 22.04: `desktop-file-validate` reports nothing, every library resolves, glibc 2.35; under Xvfb `WM_CLASS` reads `com.zacharyzhang.elyterminal` twice, and under headless sway the window's `app_id` reads the same; v0.1.2's passes the same checks on Ubuntu 22.04, run in the owner's WSL, `WM_CLASS` too; T27's rebuild passes the same checks on Ubuntu 22.04, whose llvmpipe (LLVM 15) leaves the window black in a capture, the first tarball's too, while Debian 13's paints it; v0.1.3's holds the same three files, its desktop entry names `com.zacharyzhang.elyterminal`, and its binary asks for glibc 2.35 at most | Pass |
| Each build comes from its script | T25, T27, T31, T41, T43 | `scripts/mac-app.sh` on macOS, `scripts/windows-zip.ps1` on the owner's Windows PC, `scripts/linux-tar.sh` in the owner's WSL, and for T27 on the owner's PC under Omarchy; v0.1.2's and v0.1.3's three the same way, and v0.1.3's again for T43, the disk image's on `macos-signing` | Pass |

## CI notes

On 2026-10-15 every job first failed on its runner, not on the code. With the owner's leave, each runner was put right:

- macOS: fish and PowerShell were missing, so the two shell tests failed, as they should without their shells. Homebrew installed both.
- Windows: rustup's `.cargo\bin` held `rustup.exe` alone, without the `cargo.exe` and `rustc.exe` proxies, so the toolchain step stopped at `rustc: command not found`. rustup's installer, run again, put them back.
- Ubuntu: no runner carried `ubuntu-24.04`, so 71 runs waited; they were cancelled, and a runner for this repository alone runs the job in Docker, in a VM of its own. Its first build was killed linking gpui's test binaries with debuginfo in 8 GB, so the Linux and Windows jobs build without debuginfo, as `scripts/linux.sh` does.

A clippy run for `x86_64-pc-windows-msvc` on macOS, with stand-ins for the MSVC assembler and `llvm-rc`, found `testlog` unused on Windows; it now builds only beside the Unix tests that use it.

Run 907 then failed the PowerShell test on macOS: it typed its probe right after the first folder report, which pwsh 7.6.6 sends before PSReadLine reads its first key, and the line was echoed, never run. The shell tests now type a line only once the line editor reads raw keys.

Run 926, on `65bb4d3`, passed all three jobs: macOS ran `scripts/check.sh` (135 tests, the fonts check, clippy, fmt, the 500-line rule) and the build; Ubuntu ran 138 tests and the build; Windows ran 113 tests and the build, without the Unix-only pty and shell tests.

After run 928 cargo failed on the Windows PC: `Swatinem/rust-cache`'s post step had deleted `rustup` from the host's `~/.cargo/bin`, on the Mac mini too, leaving the proxies pointing at nothing. The cache step now runs only in the Linux container. On the hosts, `rustup toolchain install --no-self-update` reads `rust-toolchain.toml` in place of dtolnay's action, which ran `rustup default`, so CI leaves their default toolchain alone. Run 934, on `0075667`, passed all three jobs, each on 1.95.0 "overridden by rust-toolchain.toml".

## The owner's checks

On 2026-10-15 the owner ran the app on macOS and found three faults, now fixed:

- The desktop showed through sharp: on macOS 27 gpui's `Selection` material builds no blur; the window's view now takes `underWindowBackground`, radius 30.
- The debug build lagged; the release build does not.
- No button hid the side columns; each now has one at its end of the title bar, its state saved.

On 2026-10-16 the owner ran it on Omarchy 4.0.4 under Hyprland:

- No window blurred: Omarchy turns Hyprland's own blur off, and Hyprland then blurs nothing; T28 says how to turn it on.
- The v0.1.1 Linux tarball built from `aae7fba`, installed by hand, wore the Omarchy theme.

On 2026-10-16 the owner ran v0.1.2 on Windows: dragged to a new size, closed and opened again, the window opened at that size.
