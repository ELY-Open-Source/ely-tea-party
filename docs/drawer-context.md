# Send drawer output to an agent

Open a session's shell drawer with Cmd+J on macOS or Ctrl+Shift+J elsewhere.
When an agent is running in the terminal above it, the drawer's bottom bar
offers **Send to Agent**.

The button captures recent output, limited to 400 terminal rows and 32 KB,
and opens an editable preview. Soft-wrapped rows are joined before common
credentials, authentication headers, cookies, private keys, email addresses
and home directory names are redacted locally. Older output may be omitted.
Detection is heuristic: review the preview for sensitive information it missed.
The preview is not saved to disk or copied to the system clipboard by this action.

Choose **Insert into Agent** to paste the preview into that session's agent
input. Press Enter in the agent to submit it. Edits containing recognized
sensitive values are redacted again and shown for another review before insertion.

Insertion requires the original session and agent to remain active and the
terminal to support bracketed paste. If the destination changes, return to
the original session or open a new preview. If bracketed paste is unavailable,
wait until the agent's input is ready and retry; no text is sent on failure.
