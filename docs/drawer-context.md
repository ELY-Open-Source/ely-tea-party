# Send drawer output to an agent

Open a session's shell drawer with Cmd+J on macOS or Ctrl+Shift+J elsewhere.
When an agent is running in the terminal above it, the drawer's bottom bar
offers **Send to Agent**.

Click **Send to Agent** to paste recent output directly into the agent's input
and focus it. Review the pasted text there and press Enter to submit.

The capture is limited to 400 terminal rows and 32 KB. Soft-wrapped rows are
joined before common credentials, authentication headers, cookies, private keys,
email addresses and home directory names are redacted locally. Review sensitive
information that the automatic redaction may miss before submitting.

Pasting requires an active agent and bracketed paste. If the paste fails, the
drawer shows a message next to the button.
