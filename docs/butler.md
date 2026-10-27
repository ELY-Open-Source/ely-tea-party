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
starts an explanation; opening cards, switching tabs, moving the mascot and
incoming terminal output never trigger a model request.

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
cached. Closing the workspace removes its cache. There is no persisted report history.

While a request runs, other Explain buttons are disabled. Dismissal, disabling AI,
or closing the workspace cancels the request and terminates its helper. An
explanation describes the excerpt captured at click time, not subsequent output;
new output does not automatically spend tokens to regenerate it. Model output
never changes the selected tab. Links in a reply accept only exact UUID addresses
of currently open sessions, so closed or unknown targets cannot open another tab
by matching its name.

## Provider and persona settings

Choose DeepSeek (default), OpenAI, Anthropic, Gemini or OpenRouter and a model from
the curated dropdown, then Apply. The bundled `butler/models.json` is validated
against Pi's catalog. An existing saved model outside the list remains in use
until the user deliberately selects and applies another model.

API keys are held only in memory for this app run, separately per provider. An
empty key uses DEEPSEEK_API_KEY, OPENAI_API_KEY, ANTHROPIC_API_KEY, GEMINI_API_KEY,
or OPENROUTER_API_KEY from the app's launch environment. No credential is written
to settings or included in process arguments. Missing credentials affect only AI
explanations; the local cards continue to work.

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

For isolated macOS UI checks, set `ELY_TEST_CONFIG_DIR` to an absolute temporary
directory with settings.json. It changes settings/session persistence only in
debug builds. The `shot` example accepts `to:pet` and sends clicks through GPUI.
Use synthetic terminal status reports to verify cards without any model request.
