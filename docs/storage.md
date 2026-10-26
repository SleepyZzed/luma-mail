# Storage and privacy

## Online-first invariants

- Gmail lists at most 25 conversation IDs per provider page. Metadata is fetched
  only for those IDs with a field projection that omits body `data`.
- Unified views keep one page buffered per enabled account and merge by recent
  activity; account + Gmail thread ID is the identity. At most 1,000 loaded rows
  are retained in one view. Narrow a Gmail search for older mail.
- Opening a conversation fetches its metadata. Expanding a message fetches that
  message, not every body in the conversation. Attachments with Gmail attachment
  IDs are requested for an opened preview or save. Small visible image thumbnails
  may load automatically within the limits below.
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
Each entry uses a small, bounded JSON header followed by opaque body bytes, rather
than a JSON array of individual bytes. Expiry scans read headers only, at most
once a minute; a requested entry's own expiry is always checked. At most 1,024
entries are retained. Older disposable cache formats are dropped, not migrated.

The budget excludes small essential configuration files, desktop-keyring
storage, app binaries/libraries (including the public OAuth registration), and
files explicitly saved to a user-selected location. It also excludes filesystem
journal, snapshots and filesystem-global metadata that the application cannot
control or reliably attribute. Cache usage shown by Luma includes its conservative
allowances; external `du` may report a slightly different number.

## Memory and large files

API JSON responses are capped at 8 MiB. Standard attachment downloads have a
36 MiB JSON cap; previews and explicit saves decode selected files in memory.
Small visible image thumbnails may fetch four files of 8 MiB or less within a
12 MiB source budget per opened message/composer and two global preview jobs.
Cards use 160x120 textures; opened images use at most 1280x960 pixels. Decoders
reject over 32 megapixels or 16,000-pixel dimensions and cap allocation at 128 MiB.
Explicit previews cap source files at 24 MiB. DOCX expands only word/document.xml
(up to 2 MiB), with at most 100,000 text bytes and no macros/relationships.
Audio/video uses an in-memory stream, no autoplay and one preview window.
Closing stops playback and releases its source; codec support varies by platform. Standard composed attachments are limited to 16 MiB combined and encoded
messages to 24 MiB, deliberately below Gmail's upper boundary after MIME overhead.
They are prepared in bounded memory on a worker thread. Very large or unusually
structured messages may fail with an explicit limit error rather than consume
unbounded memory. Drive uploads stream in 256 KiB chunks without a disk spool.
Cancellation stops at a chunk boundary; an in-flight request can take up to the
network timeout. An uploaded file may remain in Drive after sharing failure; no
link is inserted in that case and partial grants are not automatically revoked.

## Email safety

Linux HTML mail uses ephemeral, sandboxed WebKit. Sanitization preserves allowed
layout/CSS, tables, colours and links, removing scripts, forms, frames and active
content. CSP blocks direct network/file requests. Only bounded decoded PNG images
enter the renderer. External HTTPS images require Load images and can reveal an
open to their servers; no Google authorization, cookies, referrer or proxy
credentials are attached. Private-network addresses and redirects are rejected.
The themed reader adapts neutral HTML surfaces and text in memory, retaining brand
button colours and image pixels. Original colours restores the sender’s palette.
Changing theme or colour mode reuses prepared images: at most eight and 12 MiB
of encoded PNGs per open HTML message. There are no extra downloads or new disk
copies for theme changes. Inline images fit within 480 pixels of height; full
attachment previews remain on demand. There is no persistent WebKit content database. GStreamer keeps its codec
inventory (about 1 MiB on the CI host) under the private Linux runtime directory,
outside the body-cache budget. It records installed libraries and capabilities,
never message bodies or attachment bytes. With a normal XDG desktop runtime this
is temporary session data; GLib's fallback can persist it if XDG_RUNTIME_DIR is
unavailable. A blocked namespace sandbox falls back to readable text.
Windows developer builds use InPrivate WebView2, with mail served from memory,
sender scripting disabled, no-store responses and a disabled content/shader disk
cache. WebView2 still creates runtime profile metadata under
`cache/webview2-runtime`, outside the body-cache budget: a synthetic check on the
Windows QA host produced approximately 1.46 MB of logical runtime files. This is
not a zero-disk renderer. Linux uses ephemeral WebKit instead. Clicking a safe
HTTP/HTTPS/mailto link uses the system handler. No email scripts execute.
There is no analytics, telemetry or third-party mail backend.

OAuth access tokens live in memory, refresh tokens use Linux Secret Service with
an encrypted D-Bus session, and token structures are zeroized on drop. No private
message body or token logging is implemented. A locked/unavailable keyring blocks
login visibly. Settings and cache use private Unix directory/file permissions.

See [manual testing](testing.md) for storage and remote-persistence acceptance
checks. Passing mocked tests does not establish production security.

Sender-block settings hold chosen addresses and filter IDs, bounded by Gmail's
1,000-filter limit. Server filters survive disconnect/uninstall; unblock them
first or remove them in Gmail Settings. These settings are outside the cache.

## Drafts and recipient suggestions

Drafts autosave to Gmail. Automatic local draft copies are disabled, including
when upgrading from a development build that enabled them. Existing unsent
recovery files are preserved and shown only when present; saving that revision
to Gmail or explicitly discarding it retires its local copy. They are never
silently deleted on upgrade. No new local body archive is maintained.

Optional recent recipients retain at most 100 addresses across accounts in a
64 KiB configuration file, expire after 90 days, and are cleared on disconnect or
on request. They are learned only after successful sends in Luma, not by scanning
Sent, contacts or historical mail. Prefix matching happens in memory while typing.


## Interface scaling and composer fonts

Appearance adds only small numeric/boolean preferences to settings.json. Scaling,
density and animation changes do not fetch mailbox pages. Font lists use installed
system metadata; Luma does not download, copy or embed font files. Outgoing font
family/size spans are part of the Gmail draft/message, not a local font database.
Visual draft restoration is bounded to 2 MiB source, 512 KiB text, 4096 spans and
32 markup levels, and runs in memory without a browser or local draft store.
Unsupported markup stays intact in source mode. GTK/font libraries can use normal
system font metadata caches; these contain no mail or attachment content.

Bulk selection stores only keys from the at-most-1000 loaded conversation rows.
Select all does not fetch pages or persist a selection, mailbox index or bodies.


## Contact photos and clipboard attachments

Optional Contacts access is used only to match visible sender addresses to saved
contact photos. It does not download or mirror the address book. Requests are
limited to two jobs and twelve new lookups/minute; Google’s search cache is warmed
with an empty query. Search responses project email addresses and photos only.
Photo downloads are HTTPS-only on Google’s image host, at most 2 MiB, without
OAuth credentials/cookies/referrers. Existing pixel/allocation guards apply.
At most 128 96×96 RGBA textures (about 4.5 MiB) remain in RAM across accounts,
with ten-minute expiry and account separation. Eviction also releases avatar
widget references. No photo files, contacts index or body-cache entries are added.

Pasted raster images are read in bounded chunks, validated on a worker, and kept
as in-memory MIME attachments (16 MiB combined retained/pasted limit, at most 64
retained/pasted files). They autosave with the Gmail draft. No temp-image spool is
created; local file attachments can use Drive for larger transfers. Composer
movement retains one short-lived render snapshot in memory, released after its
280 ms transition. It adds no persistent store or idle animation loop.

## Whole-application audit and explicit Save all

The configurable limit is a **mail-body cache budget**, not a limit on installed
application binaries. The AppImage bundles GTK, WebKit and media libraries to
avoid dependence on the build host's installation. Its compressed download and
extracted installation are reported separately from growing user data.
`packaging/audit-storage.py --tree LABEL PATH` reads filesystem metadata without
reading file contents or following symbolic links; hard links are counted once.
Unix reports use allocated blocks, including directory/link allocation. Windows
reports are explicitly estimates. Shared OS keyring/font caches, filesystem
journals/snapshots and compositor resources cannot be reliably attributed here.
Development compiler/dependency/build directories are not installed client data.

Each Linux renderer test records configuration, app cache, private runtime and
other XDG cache usage. The read-only AppImage test also records compressed and
extracted package sizes. Reports are included in CI/release artifacts. These
synthetic runs are evidence about tested paths, not sustained real-mailbox proof.

Save all keeps at most one selected attachment download in flight and decodes at
most 26 MiB for a file. It writes only to a user-chosen folder, with safe basenames
and create-new semantics; existing files and symlinks are not overwritten.
Duplicate names get numbered suffixes. Cancellation stops after an in-flight
request returns or at a 256 KiB write boundary, removes an incomplete new file,
and keeps completed saves. It never stages an archive or adds body-cache entries.

Photo failures have a short transient retry deadline rather than a ten-minute
negative match. The rolling lookup budget schedules a wakeup for pending visible
senders. Clearing/disabling invalidates old completions. All these states and
navigation/composer animation snapshots remain in RAM only.

Mesa graphics shader caches are now directed to the app's private Linux runtime
folder with a nominal 16 MiB driver limit, separate from the body budget. Luma
disables Mesa's uncapped single-file mode for its own process only. It does not
change desktop settings or other applications. Normal XDG runtime storage is
temporary; its fallback can persist. See [Mesa's cache settings](https://docs.mesa3d.org/envvars.html).
The limit concerns supported Mesa drivers, not a universal quota on every GPU
vendor's implementation. Runtime reports include top-level metadata breakdowns
so non-mail library files are visible rather than hidden in a zero-cache claim.
