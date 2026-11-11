# Task attention and result review

Sessions, Tea Party and the whale's task cards use the same task state. Their
attention order is error, waiting, reported completion awaiting review, then
working and other activity. Tea Party and task cards sort by that priority;
the session sidebar keeps the user's tab order. The attention shortcut uses the
same priority without rearranging sessions.

A successful hook-reported end starts **Done · Unseen**. Open the session's book
button, click its completed Tea Party card, or expand Details in the whale panel
to read it. This changes the state to **Viewed · Unreviewed**. Merely opening the
overview or selecting a terminal does not mark a result viewed or accepted.

**I reviewed this · Mark handled** acknowledges the result and removes it from
the attention count. **Reopen for review** returns it to that count. Neither
action verifies code or runs a command. Errors and unanswered questions cannot
be dismissed through the completion acknowledgment button.

The acknowledgment belongs to the recorded end event and stable session UUID.
A stale button cannot acknowledge a newer turn. Duplicate completion hooks do
not reset acknowledgment. When Restore sessions is enabled, review changes are
saved immediately with Activity; save failures are shown and retried by the
ordinary Activity saver. Closing the overview does not erase review state.

This iteration presents the newest turn per main terminal. Starting another
turn replaces the current card; older rounds remain in Activity. Drawer history
remains in Activity, outside this task queue. Native terminal status without a
matching hook completion retains the generic reported-done label and does not
offer a per-turn acknowledgment.

## Reading a result

The expanded card shows the captured closing reply and command events from that
round. New hook payloads preserve up to 8 KiB of closing text, including line
breaks, with an ellipsis when clipped. Older Activity files retain the short
excerpts originally captured. Agents that provide no closing text show an
explicit empty state; Ely does not invent a reply from terminal silence.

Command rows say returned, reported failure, or no completion reported. They are
hook events, not independent test results; command details remain the bounded
excerpts supplied by existing integrations. A returned shell tool does not
prove every command or test inside it passed. The Activity button opens the
source history with its latest round expanded.

**Workspace changes** opens the selected session's existing Git panel, including
its diff viewer. It shows the current checkout's entire diff. Shared directories
can contain edits by the user or other agents, so it is never labelled as this
task's exclusive changes. These actions make no model request and never commit,
push, approve an agent permission, or type into a terminal.

## Validation and remaining acceptance

Local checks on 2026-11-10:

- Offline Rust build, formatting, all-target Clippy with warnings denied, diff
  whitespace and repository file-size/comment rules passed.
- Regression tests cover shared attention priority, stable ordering while reading,
  viewing versus acknowledgment, stale actions, duplicate completion events,
  persisted review state, old Activity files, failure precedence, and bounded
  multiline reply capture. The three hooks-install integration tests passed.
- The final broad library run passed 654 tests and ignored two; one existing fish
  test failed only during temporary-directory cleanup (`DirectoryNotEmpty`). Its
  immediate isolated rerun passed. Four previously identified environment failures
  were excluded: two Unix-socket tests, a shared-memory test and missing PowerShell.
- Native window capture could not complete: macOS rejected its GUI/XPC service
  connections. The capture process was interrupted. No visual or click-through
  acceptance is claimed, and no release was published.

On a desktop with those permissions, check a completed, waiting and working
session together. Confirm all three surfaces agree; inspect a completed reply,
open its Git and Activity panels, acknowledge it, restart, then reopen it for
review. Start a new turn before using an old result action and confirm the new
turn remains unacknowledged. Include narrow windows and a long closing reply.

## Follow-up feasibility probes

Run `python3 scripts/probe-workspaces.py --self-test .` to exercise project identity
and worktree isolation in a disposable repository, then inspect this checkout.
The probe leaves the inspected repositories untouched and removes its fixture.

The probe checks that subfolders and symlinks resolve to one checkout, worktrees
share a canonical Git common directory but keep distinct checkout identities,
independent clones remain separate projects, detached HEAD is representable,
and editing a worktree does not alter the original checkout's file or index.
These checks passed locally on 2026-11-10. This is a feasibility probe, not a
shipped project-grouping or worktree-creation interface.

The machine has Herdr 0.8.0 (bundled API protocol 19), but no `hc` executable was
found on PATH or in `~/.local/bin`. Ely currently follows `session.snapshot` in
`src/term/herdr.rs`. New Herdr Core compatibility has not been established.
The sandbox also rejects Unix socket creation, so detach/reattach persistence
cannot be qualified here.

Before choosing a runtime backend, use an isolated core with one harmless long
command. Close and reopen its viewer; confirm the same process and output remain;
then test an app update, core loss, and explicit terminal close separately.
Record which events preserve the process and which only restore a shell.
Keep this independent from Ely's existing resume-command restoration.

Recommended next increment: group sessions by canonical Git common directory,
show branch and checkout separately, and offer an explicit isolated-task action
only after defining worktree naming, dirty-state handling and cleanup. Do not
attribute a shared working-tree diff to one agent or replace the terminal runtime
on the basis of the identity probe alone.
