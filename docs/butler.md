# Optional Agent Butler

Settings → Agent Butler enables click-to-report on the whale mascot. It defaults off.
Keep the whale visible in Settings → Terminal. Click and drag are separate: a drag
moves her; a click opens a compact speech bubble containing only her reply.
The bubble stays open after completion; no timer hides it. Clicking the mascot
again keeps the same reply and never starts a duplicate request. Click elsewhere
inside the app (including Settings or another workspace) to dismiss it and cancel
any current run; clicking or scrolling inside the bubble keeps it open.
Opening it after dismissal reads fresh snapshots. Disabling the feature or closing the
workspace also cancels the run. Loading and failures appear as short messages.

When a session reports an error or waits for input/approval, the existing whale
animation indicates attention; no extra badge is overlaid. Clicking her immediately opens the first affected session (errors
before waiting sessions, then tab order) and asks Pi to explain those terminals
only, excluding healthy sessions. Session references in her reply are real internal hyperlinks, displayed as underlined names. Their destinations are `ely://session/<UUID>`, so duplicate names, renaming and reordering do not affect routing. Selection uses
stable session IDs and happens only on a click, never on asynchronous model output.
Each session gets a UUID at creation, retained when saved sessions are restored.
A duplicated tab gets a new UUID. Older saved sessions receive IDs on first load;
invalid empty or duplicate saved IDs are replaced. Right-click a tab and choose
Copy session link to copy its address. These are app-internal links; no OS URL
handler or browser integration is registered. Closed or unknown targets remain
plain labels, and never open another tab by matching its name.
Normal clicks with no attention signals still produce the general briefing.
Existing terminal/agent status flags route attention; explanations still come
from terminal snapshots, without reading hook payloads or Activity history.
There is no background LLM scan for arbitrary errors in otherwise unflagged output.

The bubble uses a curved speech tail, asymmetric corners, a soft blue gradient
and theme-aware shadows, without a header,
portrait, timestamp, status bar or action buttons. Replies are one conversational
paragraph, without headings, bullets or inline source numbers. A brief fade and
upward entrance respects reduced-motion preferences; streaming does not replay it.
Placement follows the mascot's actual edge and horizontal position. At the bottom,
the bubble opens above her with a downward tail, reserving her height. At the top,
it opens below her. Available space limits the scrollable reply; resizing or
moving the mascot updates the anchor without starting another model request.
Settings → Agent Butler → Agent persona provides a multiline personality editor, up to
2,000 characters. Save persona persists the tone and preferred form of address
for subsequent reports; Restore default clears the override. A blank field uses
the built-in cheeky DeepSeek whale-girl persona. The editor displays the default
as editable text; a saved custom persona is preserved. Persona settings do
not replace the read-only tool boundary or terminal-evidence rules.

Choose DeepSeek (default), OpenAI, Anthropic, Gemini, or OpenRouter, then pick a
model from the short dropdown list. There is no model-ID text field. The shared
`butler/models.json` lists 2–4 current models per provider, checked against Pi's
bundled catalog by tests. A previously saved model outside the list remains in
use until the user selects a listed model and applies it; the UI explains this
and does not silently replace the saved model. Apply saves provider and model. API keys are held
only in memory for this app run, separately per provider. An empty key uses
DEEPSEEK_API_KEY, OPENAI_API_KEY, ANTHROPIC_API_KEY, GEMINI_API_KEY, or
OPENROUTER_API_KEY from the application's launch environment. No credential is
written to settings or passed in process arguments. New provider credentials
must be applied before requesting a report.

## Runtime and evidence

The Rust application starts a bundled Node process only when a report is requested.
`@earendil-works/pi-agent-core` and `pi-ai` are pinned to 1.4.0. Node is pinned to
22.23.4; its platform archive is verified against a pinned SHA-256 before packaging.
The runtime is packaged with the app, so users do not need Node or npm.

The host takes frozen snapshots of up to 32 main terminal sessions, limiting the
set to attention targets when flagged sessions exist, without reading agent hooks
or Activity. Each snapshot includes at most 400 physical grid rows
and about 32 KB of text, with capture time, session identity, alternate-screen
and truncation flags. Wrapped rows are joined; wide and hidden cells are handled.
Reading does not depend on the displayed scroll position and does not type into
or switch terminals. Snapshotting runs off the UI thread under a bounded grid read.

Pi can only call `list_sessions`, `read_terminal`, and `search_terminal` against
those frozen snapshots. Tool output is untrusted evidence. The prompt requires
source attribution through host-provided session links, uncertainty for missing history, and a distinction between
reported and verified success. Capture time is not an output timestamp. TUI redraws
and trimmed scrollback cannot be reconstructed. Drawer terminals are not included.
The tools provide each session's URL and escaped Markdown link alongside its ID.
Pi copies those links; the bubble renders only labels, hiding incomplete URL
syntax while streaming and accepting only exact addresses of open sessions.
Only excerpts selected by the agent's tools are sent to the configured provider.
There is no background monitoring or persisted report history.

The run is limited to six model calls, 64,000 characters of tool output, and 120 seconds. Input and protocol output
are bounded. The host kills and reaps the helper on cancellation or timeout;
a helper also exits if its parent disappears. No provider-specific loop or Pi event type is exposed to the UI.

## Development

```
npm --prefix butler ci --ignore-scripts --no-audit --no-fund
npm --prefix butler test
npm --prefix butler run build
npm --prefix butler run smoke
cargo test --lib bundled_pi_validates -- --ignored
scripts/check.sh
```

`target/butler` contains the development runtime. Packagers copy it into the
macOS bundle, Windows zip, or Linux payload. The Node helper is signed separately
on macOS with its own JIT entitlements; the terminal receives no new entitlements.
Pi, Node, and dependency licenses accompany the runtime.

For isolated debug UI checks, set `ELY_TEST_CONFIG_DIR` to an absolute temporary
directory containing settings.json. Settings and session persistence use that
directory in debug builds only. The `shot` example accepts `to:pet` and dispatches
pet clicks through GPUI because the non-key mascot has no normal first responder.

Provider support is not a claim that every model or hosted gateway has been tested.
The automated tests use Pi's real loop with a scripted provider, and test the
bundled process without contacting a provider. Check actual provider behavior and
signed distribution artifacts separately before publishing a release.

Initial validation includes the twelve JavaScript tests, the macOS check gate,
Linux Rust and IME tests, and a live DeepSeek report from a synthetic terminal.
The live check exercised whale click, snapshot tools, Chinese streaming output,
session navigation, and helper exit. Other providers, Windows execution,
and notarized release delivery have not been exercised by that check.
The macOS development app passes strict recursive signature verification. Its
packaged Node/Pi runtime also completed a live DeepSeek tool loop and exited cleanly.
The persona editor was exercised through the UI: a multiline custom persona was
saved to isolated settings, then a live DeepSeek report used the requested address.
Attention navigation was checked with three isolated sessions: a build error, a
permission prompt, and a healthy shell. Clicking the whale switched from the
healthy shell to the error and sent only the two affected snapshots. Selecting
the second alert while Pi was running opened its session; report completion did
not switch back. This predates the replacement of alert buttons with text links.
The compact bubble was then checked with a live DeepSeek reply: only prose and
underlined session names remain, clicking the second name opens its session,
and outside clicks dismiss the bubble. Repeated mascot clicks now keep it open.
The compact-bubble gate passed 398 Rust tests (two ignored) and 12 JavaScript tests.
The edge-placement follow-up passed 400 Rust tests (two ignored). A live macOS
maximize check opened the reply above the bottom mascot, and restoring the window
moved an already-open bubble back to the top without requesting another report.
The persistent-reply follow-up checked repeated mascot clicks during and after a
live report: one model run, with the reply still visible afterward. Clicking
inside kept it open; a click in Settings dismissed it without consuming the click.

## Selection references

The curated model list was checked on 2026-10-25 against the official
[DeepSeek](https://api-docs.deepseek.com/quick_start/pricing),
[OpenAI](https://developers.openai.com/api/docs/models),
[Anthropic](https://platform.claude.com/docs/en/models/overview),
[Gemini](https://ai.google.dev/gemini-api/docs/models), and
[OpenRouter](https://openrouter.ai/deepseek/deepseek-v4.1-flash-1017) catalogs.
This is a release-maintained list, not a live model-discovery service.

The default persona is our own wording inspired by the community's clever,
snack-loving, cheeky whale characterization, including this
[community project](https://github.com/yunyunyunyunsuan/dsh-plugin-moments).
It is not an official DeepSeek character specification. Both the editor and
Pi runtime use `butler/persona.json` as the single source for the default text.

## Session-link validation

The address change passed 404 Rust tests (two ignored) and 14 JavaScript tests.
A live DeepSeek run with two restored tabs both named Same produced distinct
internal links. Clicking the first displayed link opened the second tab by its
UUID, with the bubble still visible. Both UUIDs survived another app launch.
The new copy-link menu entry was also checked in the isolated app.
