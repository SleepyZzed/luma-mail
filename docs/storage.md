# Storage and privacy

## Online-first invariants

- Gmail lists at most 25 conversation IDs per provider page. Metadata is fetched
  only for those IDs with a field projection that omits body `data`.
- Unified views keep one page buffered per enabled account and merge by recent
  activity; account + Gmail thread ID is the identity. At most 1,000 loaded rows
  are retained in one view. Narrow a Gmail search for older mail.
- Opening a conversation fetches its metadata. Expanding a message fetches that
  message, not every body in the conversation. Attachments with Gmail attachment
  IDs are requested only after an explicit save action.
- Search runs on Gmail. Restarting the app always fetches current list state.
- History notifications use one bounded page per account per poll. Initial login
  and expired history cursors establish a baseline without alerting old mail.
- Gmail All Mail includes sent and archived messages; Luma's Unified All Mail is
  an aggregate query excluding Spam and Trash. Unified Inbox only includes Inbox.
  Accounts remain separate mailboxes. No messages are copied between them.

## Cache budget

Only expanded message responses are disposable cache entries under
`$XDG_CACHE_HOME/luma-mail/bodies` (`~/.cache` fallback). Default 100 MiB, with
0/50/100/250/500 MiB and custom settings. Each entry expires after the configured
creation age (default 24 hours); reads update LRU order without extending expiry.
Disabling the cache removes its files. Clear-now, clear-on-exit and per-account
clearing are implemented. Cached bodies are private files, not encrypted at rest;
disable caching if that retention is unsuitable.

Files are charged using allocated filesystem blocks on Unix, plus an inode
allowance. Directory overhead is conservatively reserved. On the Windows test
host, 4 KiB-rounded file allocations plus extra overhead are used. The limit is
enforced before and after writes, including the serialized envelope overhead.
There is no SQLite database, WAL, disk image preview, temporary upload spool or
attachment cache. Cache writes are disposable and do not duplicate entries into
temporary files. Invalid/incomplete cache entries are removed on access.

The budget excludes small essential configuration files, desktop-keyring
storage, app binaries/libraries (including the public OAuth registration), and
files explicitly saved to a user-selected location. It also excludes filesystem
journal, snapshots and filesystem-global metadata that the application cannot
control or reliably attribute. Cache usage shown by Luma includes its conservative
allowances; external `du` may report a slightly different number.

## Memory and large files

API JSON responses are capped at 8 MiB. Standard attachment downloads have a
36 MiB JSON cap; a selected attachment is decoded in memory only after a save
choice. Standard composed attachments are limited to 16 MiB combined and encoded
messages to 24 MiB, deliberately below Gmail's upper boundary after MIME overhead.
They are prepared in bounded memory on a worker thread. Very large or unusually
structured messages may fail with an explicit limit error rather than consume
unbounded memory. Drive uploads stream in 256 KiB chunks without a disk spool.
Cancellation stops at a chunk boundary; an in-flight request can take up to the
network timeout. An uploaded file may remain in Drive after sharing failure; no
link is inserted in that case and partial grants are not automatically revoked.

## Email safety

No email JavaScript, CSS, local-file access, remote images, or browser engine is
used. HTML is converted to selectable plain text. Link destinations are visible
in converted text and are not automatically launched. The composer can send
plain text or HTML source with markup insertion controls. There is no analytics,
telemetry, third-party backend or automatic attachment download.

OAuth access tokens live in memory, refresh tokens use Linux Secret Service with
an encrypted D-Bus session, and token structures are zeroized on drop. No private
message body or token logging is implemented. A locked/unavailable keyring blocks
login visibly. Settings and cache use private Unix directory/file permissions.

See [manual testing](testing.md) for storage and remote-persistence acceptance
checks. Passing mocked tests does not establish production security.
