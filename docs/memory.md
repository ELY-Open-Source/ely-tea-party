# Whale memory

Settings → Agent Butler exposes **Enable local memory**, **Allow AI to organize
project memory**, and **Manage memory**. Both switches default off. Local memory
works without an API key; automatic organization requires the configured Butler
provider and key. It does not require occasional ideas to be enabled.

The native “她记得的事” window lets the owner save global expression preferences
or project notes, decisions and unresolved items, search within a project, inspect
source excerpts, correct a record, undo up to three corrections, forget a record,
or clear everything with a second confirmation. A companion bubble's “回应这句”
opens the same editor with its text attached as context. “别再提这个” marks that
suggestion rejected; silence never becomes acceptance or a global preference.

## Capture and evidence

A dedicated Rust thread owns SQLite and a bounded 32-operation queue. Hook
prompts, reported completions, failures and questions provide short events, each
at most 2,048 UTF-8 bytes. Full transcripts, file trees, embeddings and model
credentials are not stored. Obvious credential-bearing lines are filtered; this
is best-effort filtering, not a guarantee that arbitrary terminal text is safe.

Projects use the canonical Git common directory, falling back to the canonical
folder. Worktrees share their project's memory browser; automatic recall also
requires the same checkout. Renaming/moving the repository can change identity.
The database is `memory.sqlite` beside settings; debug/ELY_DEV builds use
`memory-dev.sqlite`. SQLite is local and unencrypted, with no cloud sync or
unlimited backup copies.

AI organization is a separate, opt-in call: up to four short source events,
500 output tokens, at most two proposed records, each at most 600 UTF-8 bytes.
Rust accepts only known record kinds and one to three real source IDs. Models
cannot write global preferences or execute SQL. Model records remain marked
“AI 整理的记录 · 尚未确认”; an agent reporting success is not independent proof.
A proposed decision additionally needs user-prompt evidence. Structured validation
cannot establish that a model's interpretation is correct; sources remain visible.

There is at most one pending job per project. While a main window is active and
visible, the organizer probes locally at most once per minute. It can start at
most two calls in a rolling hour, at least 30 minutes apart, persisted across
restarts. Failed attempts consume budget and await a new event instead of retrying
in a loop. A started call may complete after the window loses focus. Disabling
memory/organization cancels it. Local recording and cleanup do not depend on AI.

## Conservative recall

Ordinary requests carry no historical project records. Up to three explicitly
saved expression preferences may accompany a request, 200 bytes each. Manual
task explanations use these preferences but do not automatically search history.

An occasional idea may include at most two unresolved, AI-reported records only
when project, checkout and normalized current task match exactly, the record is
less than 30 days old, and it has not already been used for that session/task.
Each excerpt is at most 300 bytes. This intentionally misses loosely related
history rather than drawing another project or a vague similarity into a remark.
No model call decides whether to recall. Explicitly saved notes remain available
through the browser; they are not automatically injected into remarks.

Search is local FTS5 trigram search, with bounded substring matching for fewer
than three characters. It is literal search, not semantic search. A local bigram
comparison suppresses similar suggestions against the most recent 100 suggestions
in the same project over 30 days, including rejected ones; it does not promise
to catch differently worded ideas with the same meaning.

Corrections remove old source events and pending extraction for that project, so
the old statement cannot simply be extracted again. Corrections/deletions invalidate
in-flight context. Persisted revisions and job
state prevent an old consolidation result from writing back deleted or corrected
records. Forgetting also removes retained source events and memories derived from
the same events. Clear-all removes preferences, project memories, suggestions,
events and jobs, while retaining the short-lived spending ledger. A future new
report can create a new record; deletion is not a permanent topic blacklist.

## Storage limits

| Data | Limit / retention |
| --- | --- |
| Raw events | 7 days; 14,000 globally and 900 per project; 2 KiB each |
| Retained memories | 4,000 globally; 300 per project; 100 global preferences |
| Memory payload, sources and edit versions | 16 MiB admission budget |
| Corrections | Last three bodies per record |
| Suggestions | 30 days; 8 MiB / 4,096 rows, pruned in batches |
| Recall audit | 7 days / 4,096 rows |
| Projects / pending jobs | At most 256 projects / one job per project |
| Main SQLite file | 192 MiB hard page-count limit |
| Database + WAL + SHM | 256 MiB operating budget; checked before writes |

Cleanup runs on startup, during admission and on an idle worker tick every minute,
in bounded batches. It expires short-lived material and incrementally reclaims
free pages; it does not silently evict explicitly saved preferences. Once retained
memory is full, new writes fail visibly in the editor instead of discarding saved
facts. Automatic failures are logged and do not interrupt the terminal.

WAL checkpointing starts at 16 MiB. At 32 MiB, a blocked checkpoint pauses new
writes until the reader releases it. The total budget is an admission guard, not
an operating-system quota: one bounded transaction and SQLite overhead can cross
a threshold before the next check. `journal_size_limit` is not treated as a hard
WAL cap. No full VACUUM is scheduled, avoiding its temporary copy requirement.
Reads remain available when new writes are paused; permanent disk errors still
surface to the owner. Tests exercise reader-pinned WAL, SQLITE_FULL, event floods,
restart persistence, correction/undo/deletion, invalid proposals and zero recall
for unrelated tasks/projects.
