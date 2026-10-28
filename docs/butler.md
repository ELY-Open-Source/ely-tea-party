# Whale task cards and optional AI explanations

Click the whale mascot to open local task cards. This works without enabling AI,
entering an API key, or starting the bundled Node runtime. Cards show each main
terminal's current status, folder, most recent hook-reported task and latest hook
activity when available. Errors and waiting sessions come first. Open terminal
jumps to the card's stable session UUID, including when session names are identical.

The cards distinguish reported errors, waiting, working, reported completion,
idle and unknown status. A reported completion is labelled unreviewed; idle or
silent output never counts as completed work. Hook messages remain attributed
as “Agent reports” or “Agent asks”. Missing hooks do not prevent opening cards;
they limit how much task detail is available. Drawer terminals are not included.

Opening the bubble selects the first attention target, preserving the existing
error-before-waiting behavior. Card states update locally while it is open.
Clicking the whale again keeps it open without requesting an explanation. A drag
moves her; the bubble follows her actual top/bottom edge and horizontal position.
Clicking outside the bubble dismisses it and cancels any running explanation.
Clicking or scrolling inside it keeps it open. Reduced-motion settings are respected.

## Task overview layout

The mascot opens a compact panel with a fixed title and local/AI footer. Sessions
are grouped into attention items and other activity, with character avatars,
short status badges and two-line previews. Details expands the task context
locally; the title or arrow opens its terminal. The close button dismisses the
panel. Full status meanings and folder paths are available on hover. AI usage
and interpretation continue to appear inside their originating task.

The visual hierarchy draws on [Linear Inbox](https://mobbin.com/screens/beb9d6b3-ec34-46d7-9332-320fcb32a338)
and [Delphi notifications](https://mobbin.com/screens/11350c17-180f-437c-a48d-384f653b477e)
references on Mobbin: restrained surfaces, compact rows and light separators.
The implementation uses Ely's native theme tokens and built-in character assets.

## Controlling model use

Settings → Agent Butler → Enable AI explanations is off by default. Enabling it
adds an Explain with AI button to each task card. Only that explicit button
starts a task explanation. Opening cards, switching tabs and moving the mascot
remain local. The separate, opt-in companion mode below can generate an idea
after a sustained period of watching a working agent.

Each explanation uses:

- One selected main terminal, never all open sessions.
- The most recent nonblank tail, at most 60 logical lines and 6,000 UTF-8 bytes
  including line separators. Empty excerpts do not reach a model.
- One model request, no model tool loop, reasoning disabled, an output limit of
  600 tokens, and provider retry count set to zero where supported.
- A short reply grounded in the excerpt. The model must distinguish claims from
  verified success and acknowledge missing context instead of requesting more text.

The excerpt limit is a byte limit, not a promise about input tokens or price.
Input also includes the fixed instructions, session metadata and configured
persona. Tokenization and billing depend on the selected provider/model. When the
provider returns usage, the card displays its reported total tokens; unavailable
usage is not displayed as zero. Output limits are passed to the provider.

Successful explanations are cached only in workspace memory for five minutes,
with at most eight entries. Matching requires the same excerpt, session identity,
metadata, status signal, model, provider, language and persona. Capture timestamps
are excluded. A repeated request for unchanged input reuses the answer without
starting Node or making an API call; the card explicitly labels the reused answer.
Reads do not extend expiry. Failures, truncated replies and cancelled runs are not
cached. Closing the workspace removes its cache. Explanation reports are not persisted. Optional [local whale memory](memory.md) stores separate bounded events, preferences and project records.

While a request runs, other Explain buttons are disabled. Dismissal, disabling AI,
or closing the workspace cancels the request and terminates its helper. An
explanation describes the excerpt captured at click time, not subsequent output;
new output does not automatically spend tokens to regenerate it. Model output
never changes the selected tab. Links in a reply accept only exact UUID addresses
of currently open sessions, so closed or unknown targets cannot open another tab
by matching its name.

## Occasional companion ideas

Settings → Agent Butler → Occasional AI ideas is a separate switch, off by
default. It uses the same provider, model and saved/environment credential
as task explanations, without requiring the manual-explanation switch. Enabling
it permits automatic requests containing a small excerpt from the current session.

The whale only speaks while her main window is active and visible, the same
session and project stay selected, and that session has a working agent. Any
error or waiting session suppresses suggestions. Open task cards, the party view
and an open drawer also suppress them. Typing (including IME), clicks, scrolling,
changing sessions/projects, hiding the mascot, leaving/minimizing the window,
or disabling the setting resets the quiet period and cancels pending ideas.
A gap in timer delivery after sleep also restarts the quiet period.

After a randomly selected 5–10 minute quiet period, she may make one request.
Across all windows in this app run, attempts are at least 20 minutes apart, with
at most three in a rolling hour. Failures and cancellations consume that budget;
recent identical input is suppressed even after the cooldown. No network polling
runs in the background; eligibility is checked locally. Budgets and the last 32
input fingerprints live in memory and reset when the app exits.

Each idea uses the current task reported by hooks, if available, and a short
terminal tail: at most 24 lines / 2,400 UTF-8 bytes total. Metadata and up to 500
persona characters are additional input. If local memory is enabled, up to three
explicit expression preferences and, only for an exactly matching unresolved task,
up to two historical excerpts may be added; see [memory limits](memory.md). There is one model call, no tools,
no automatic retry where the provider supports that option, and at most 160
output tokens. Bytes are not a token-price guarantee. The model is asked for
one natural, optional suggestion in 1–2 sentences, without canned openers, forced
whale jokes, invented project facts, or
claiming it will perform work. It does not inspect project files.

Only complete answers appear, in a small bubble that never takes focus or switches
sessions. The bubble shows reported token usage, disappears after 25 seconds or
when the owner resumes interaction, and offers Quiet for this window until the
window closes. AI failures stay out of the UI and retain the request cooldown.

## Provider and persona settings

Choose DeepSeek (default), OpenAI, Anthropic, Gemini or OpenRouter and a model from
the curated dropdown, then Apply. The bundled `butler/models.json` is validated
against Pi's catalog. An existing saved model outside the list remains in use
until the user deliberately selects and applies another model.

API keys are saved separately per provider in the system credential store
(macOS Keychain, Windows Credential Manager, or Linux Secret Service), then loaded
at launch. Apply with an empty key keeps the saved key, including when changing
models. Remove saved key deletes it; without a saved key the app uses
DEEPSEEK_API_KEY, OPENAI_API_KEY, ANTHROPIC_API_KEY, GEMINI_API_KEY, or
OPENROUTER_API_KEY from its launch environment. The settings page shows loading,
saved, missing and read-error states, and offers Reload key to retry access.
Failed writes are reported and do not replace the key in use. No credential is
written to settings or included in process arguments. Debug and dev app builds use
separate credential entries; ELY_TEST_CONFIG_DIR also isolates test entries in debug
builds. Missing credentials affect only AI explanations; local cards continue to work.

Agent persona supports up to 2,000 characters and controls tone and form of
address. Save persona persists it; Restore default clears the override. A blank
value uses the bundled whale persona from `butler/persona.json`. Personality does
not override the evidence rules. Terminal text and labels are untrusted data,
never instructions. The model receives no tools, filesystem access, or ability
to type into the terminal.

## Runtime and development

Rust captures the chosen terminal off the UI thread from a bounded frozen grid
snapshot, then compacts it before passing it to the helper. Wrapped grid rows are
joined and hidden cells are excluded; capture time is observation time, not an
output timestamp. TUI redraws and trimmed scrollback cannot be reconstructed.

The host and helper validate request limits. The helper uses pinned Pi 1.4.0 and
Node 22.23.4 bundled with the application. Users do not need a system Node or npm.
The host kills and reaps the helper on cancellation or its 120-second timeout;
the helper exits if its parent disappears. The Node runtime is signed separately
on macOS, with its own JIT entitlements. Licenses accompany the runtime.

```
npm --prefix butler ci --ignore-scripts --no-audit --no-fund
npm --prefix butler test
npm --prefix butler run build
npm --prefix butler run smoke
cargo test --lib bundled_pi_validates -- --ignored
scripts/check.sh
```

Tests exercise Pi's real loop with a scripted provider: a single request, no tool
loop, excerpt limits, cancellation, incomplete answers, persona and session links.
Rust tests cover status semantics, Unicode excerpt bounds, cache expiry and
invalidation, successful-result caching, cancellation and helper protocol checks.
These checks do not contact or establish compatibility with every live provider.

For native credential persistence checks, build `cargo build --example butler_credentials`.
Set `ELY_TEST_CONFIG_DIR` to one unused absolute temporary directory, then run
`target/debug/examples/butler_credentials` with `write`, `read`, `replace`,
`read-replaced`, `remove`, and `missing`, in that order as separate processes.
This uses synthetic keys for two providers, checks restart isolation and replacement,
and removes the test entries. It requires access to the system credential store;
unit tests alone do not establish native keychain persistence.

For isolated macOS UI checks, set `ELY_TEST_CONFIG_DIR` to an absolute temporary
directory with settings.json. It changes settings/session persistence only in
debug builds. The `shot` example accepts `to:pet` and sends clicks through GPUI.
Use synthetic terminal status reports to verify cards without any model request.
