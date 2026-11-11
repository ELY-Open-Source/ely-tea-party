# Local session messaging

The app binary contains both the GUI and the CLI. Ely prepends the running binary's directory to each terminal's PATH and sets `ELY_CLI` to its absolute path. Supported shell integrations restore that PATH entry after startup files load. Child coding agents inherit it; no global shell configuration or separate CLI package is installed.

## Enable an agent

Open Settings → Terminal → Agent messaging integration and install the entry for Claude Code or Codex. The same actions are available from an Ely terminal:

```sh
ely-tea-party skills install claude
ely-tea-party skills install codex
ely-tea-party skills status
```

The standard personal skill locations are `~/.claude/skills/ely-session-messaging` for Claude Code and `~/.agents/skills/ely-session-messaging` for Codex, following their [Claude Code](https://code.claude.com/docs/en/skills#choose-where-skills-load) and [Codex](https://developers.openai.com/codex/skills) documentation. Custom skill roots are not configured by this installer. Other agents can call the CLI directly and read `msg guide`.

The skill contains a small bootstrap that reads `"$ELY_CLI" msg guide`. In PowerShell use `& $env:ELY_CLI msg guide`. Complete instructions and their versions come from the running app, not a second downloaded skill package. If an agent has not discovered the installed entry, restart it or explicitly ask it to read the guide.

## Send and receive

```sh
ely-tea-party msg guide
ely-tea-party msg whoami
ely-tea-party msg list --agent codex --folder shop
ely-tea-party msg list --query "login"
ely-tea-party msg send ADDRESS "The login response type is ready in src/api/types.ts"
ely-tea-party msg inbox
ely-tea-party msg reply MESSAGE_ID "I have updated the frontend type"
ely-tea-party msg ack MESSAGE_ID
ely-tea-party msg inbox --all
```

Copy an exact ADDRESS from `list`. The directory includes the agent, full working folder, title, task state and unread count across this app's open windows. Same-folder peers sort first. Filters combine with AND. A title or folder is a search term, not a routing address.

An address binds the app instance, runtime session and observed agent incarnation. Changing title or folder preserves it; observed agent restarts or native conversation changes replace it. Detection follows foreground-process and hook information, so these addresses are not a cryptographic identity for the underlying model. Install the normal agent hooks for better lifecycle reporting. Main terminal sessions are the messaging scope; drawers and WSL do not receive separate messaging identities in this version.

`send` returning `stored` means the message was committed to the mailbox. It does not mean the receiver read it. Reading `inbox` does not acknowledge mail; `ack` does. `reply` retains the original message ID and requires that original sender instance still be live. No text or Enter key is injected into a terminal. An idle agent is not automatically awakened; recipients explicitly check their inbox when coordinating.

The CLI prints a request key before submitting. After an uncertain response, retry the same operation with `--key UUID`. An identical retry returns its original receipt, including after the recipient closes. Reusing a key with different content fails. Messages are at most 32 KiB; inbox returns the latest 100 records, with acknowledged records hidden unless `--all` is supplied.

Mail is committed to `messages.sqlite3` in Ely's local data directory. App restarts use a new address namespace: old mail stays stored and is never reassigned. This version does not offer an archive browser for past app instances.

## Updates and ownership

Each managed entry has an ownership manifest containing its version, SHA-256 content hash and an interrupted-update marker. Startup and “Check and sync” update only existing managed installations. New installations require the install action. Writes use a temporary file, atomic replacement and read-back verification. A per-entry file lock serializes Ely installers.

An entry edited by the user, a symlinked entry, an invalid manifest or an entry from a newer installer is preserved and reported. The app never claims that a running agent has refreshed its in-memory instructions. To resolve a modified entry, move the custom entry aside or restore the managed content, then install again. `skills uninstall AGENT` removes only verified managed files and preserves unrelated files in the directory.

CLI requests include the protocol version and a per-session credential. Unix uses a private local socket; Windows uses an authenticated loopback connection. Credentials stay in process memory and child environments, not settings or skill files. Unsupported methods, arguments, protocol versions and retired destinations fail before message storage. Mail operations run off the GUI thread. The service lives inside Ely and ends with it.

The release bundle manifest pins the bootstrap and full-guide hashes with their versions; tests verify these against the embedded resources. When instructions change, update the corresponding version and manifest together. Keep command meaning compatible within a protocol version; incompatible changes require a protocol version bump. A CLI/app mismatch fails explicitly rather than opening another app.

## Validation boundaries

Tests cover routing, replies, explicit acknowledgment, retries, wrong-session access, identity changes, mailbox persistence, skill ownership and interrupted upgrades, CLI subprocess behavior, and PATH restoration in real zsh/bash/fish PTYs.

Debug builds accept an absolute `ELY_TEST_SKILL_HOME` for isolated installer tests. Windows resolves the normal home through Known Folders, so overriding `HOME` or `USERPROFILE` alone does not isolate these tests. Release builds ignore this test override.

On macOS, build the debug binary, then run `python3 scripts/check-messaging.py` from the repository. It opens the actual GUI with two restored zsh PTYs in an isolated home and config directory. Controlled peers call the bundled CLI over the real socket and verify discovery, delivery, replies, acknowledgments, duplicate retries, credentials, protocol rejection, and retired addresses. It also verifies startup skill upgrades, preservation of user edits, and socket-directory removal on normal exit. JSON results and logs remain in the temporary evidence directory printed by the script. It does not run coding models or change personal agent installations.

This runtime acceptance passed on macOS on 2026-11-11. The settings window's install, sync and remove actions were also exercised against an isolated home, including preserving a modified entry and inspecting the rendered status. Windows/PowerShell behavior and two coding models autonomously discovering and using the skill remain unverified.
