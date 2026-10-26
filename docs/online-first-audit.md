# Online-first audit — 26 October 2026

The app still queries Gmail for lists, search, drafts and actions. It does not
mirror a mailbox, scan Sent for suggestions, or maintain a local search index.

## Request and UI work

- Provider pages contain 25 conversation IDs. Metadata projections omit body
  data; the view retains at most 1,000 rows. Opening a message fetches its body.
- Background history checks default to 60 seconds. Accounts poll with at most
  four concurrent jobs; checks cannot overlap. Unread counts and history run
  together. Existing bounded retry and per-account quota budgets still apply.
- New mail fetches metadata only for affected threads, with four concurrent
  requests. It updates existing widgets or inserts animated rows, preserving
  loaded pages, selection and the reader. Background checks do not activate the
  page-loading bar. A custom search requires explicit refresh so unrelated
  incoming mail cannot appear in its results.
- New replies restore the thread's unread state. A reply to the open thread
  offers an Open button instead of replacing the body being read.

## Disk and memory

- Disposable bodies have an allocation-aware budget, creation TTL, LRU eviction,
  a maximum of 1,024 entries, and zero-cache mode. Entry headers are at most
  8 KiB and payloads at most 8 MiB. Expiry/account scans do not deserialize bodies.
- A synthetic 1 MiB byte payload produced a 1,048,657-byte cache file, charged
  as 1,064,960 bytes on the Windows test filesystem. The old JSON byte-array
  representation of that payload alone was 4,194,305 bytes. This is a format
  comparison, not a general performance benchmark.
- New drafts autosave to Gmail; local draft autosaving is disabled. Legacy
  unsent recovery copies are preserved until saved to Gmail or discarded.
- Optional recent recipients are limited to 100 entries, 64 KiB and 90 days,
  scoped by account. Matching happens in memory; disconnect/clear removes them.
- Attachment previews use bounded memory. Linux audio/video uses sealed
  anonymous RAM files, retained only for playback. Drive uploads use 256 KiB
  chunks and no disk spool. User-selected saved files are outside the cache.
- Linux HTML uses ephemeral WebKit. GStreamer keeps a small codec inventory in
  the private runtime directory, outside the body budget (about 1 MiB in CI).
  It contains installed library metadata, no mail. Normal XDG runtime directories
  are temporary; GLib's fallback may persist this inventory without XDG_RUNTIME_DIR.
  Windows QA uses InPrivate WebView2, which
  still creates runtime profile metadata outside the body budget (about 1.46 MB
  logical size in an earlier synthetic run). The Windows renderer therefore
  cannot claim a zero-disk footprint. No mailbox content cache is enabled there.

## Evidence and limits

Automated tests cover projected metadata pages, a simulated 100,000-thread
mailbox needing only one page, cache budget/expiry/LRU/zero retention, corrupt
envelopes, recipient limits, and legacy recovery safety. Native GTK diagnostics
exercise field focus, Tab/Shift+Tab, filename links and composer Restore. Linux
CI additionally checks real HTML/media engines and the packaged AppImage.
Completed [Linux run 40249509620](https://github.com/SleepyZzed/luma-mail/actions/runs/40249509620)
passed native and AppImage HTML/media, composer navigation, GTK/D-Bus transport,
system cursor preferences and the zero-mail-retention check. Their inspected
newsletter snapshots matched byte-for-byte. This is a synthetic runtime check,
not sustained CPU/memory profiling on the user's desktop.

Earlier authorized 25-row live checks measured Inbox 0.93 s, Load more 0.70 s,
Archive 1.12 s and All Mail 0.88 s; these are observations, not guarantees. A
sustained two-account Linux/Noctalia run, memory/CPU profiling on the target
desktop, and Drive recipient-sharing acceptance remain required for 1.0.
