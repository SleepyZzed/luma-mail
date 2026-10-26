Luma Mail 0.5.0 early access — quieter navigation, compact compose and photo fixes

- Unified Mail starts collapsed. Its button animates folders open and closed.
  Account folders animate in place when switching accounts; the header stays fixed.
- New messages start in the existing compact composer. Expand/Restore, Back,
  inline replies and pop-out keep their previous layouts and behavior.
- Fixed profile-photo matching for `Display Name <email>` senders. Google Contacts
  can supply available profile photos for matching saved contacts. Failed lookups
  retry after transient errors; Settings offers Retry profile photos. Unmatched
  senders keep initials: Gmail does not expose every profile picture it displays.
- Save all attachments to a chosen folder with sequential bounded downloads,
  completed-file progress and cancellation. Existing files are never overwritten;
  duplicate names get numbered suffixes. Completed files remain after cancellation.
- Persistent navigation avoids rebuilding/reparenting every folder on unread or
  selection changes. Contact lookup scheduling resumes after its rate budget resets.
- Linux checks now publish separate package/runtime storage reports, including
  the extracted installation and files outside the configurable body cache.

Quit the old version from its tray menu before launching this replacement.
Sign-in remains the built-in Sign in with Google flow. No JSON import is needed.
Google branding and data-access review is submitted and pending; the unverified
warning and Google's lifetime user cap remain until approval.

All earlier mail, themes, scaling, clipboard, system notification, tray and Gmail
server-draft behavior remains. The online-first model is unchanged: remote
search, 25-row pages, bounded optional body cache, no local mailbox/index,
no new local drafts or attachment archive. Explicitly saved files are user files.
The AppImage and bundled rendering libraries are application installation size,
not mail storage; zero body-cache mode does not remove installed binaries.

This is early access. Two-account target Linux/Hyprland/Noctalia, sustained large
mailbox profiling, remaining editor/recovery features and Google's decision are
still gates before 1.0. See STATUS.md, RELEASE-CHECKLIST.md and docs/roadmap.md.
