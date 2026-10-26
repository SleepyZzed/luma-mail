# Road to 1.0

The original 20 acceptance criteria in RELEASE-CHECKLIST.md remain mandatory.
0.3 is an early-access milestone, not a completed 1.0 release.

## This milestone

- Frameless Hyprland-friendly navigation; optional compositor decorations.
- Fixed Luma/Compose header and sidebar footer; Ctrl/Shift selection with safe
  bulk actions and loaded-only Select all.
- App UI scaling, reduced motion, refined transitions and local density updates.
- System-font composition and visual reopening of new Luma-generated rich drafts.
- No mailbox mirror/index, font downloads or new local draft/attachment archive.

## Next implementation priorities

1. Draft conflict reconciliation preserving both versions, full rich-text undo/redo,
   recipient chips and authorised send-as aliases. Keep complex HTML intact until
   its editor can represent it without silently losing content.
2. Attachment save-all, inline image insertion and whole-conversation forwarding;
   interrupted Drive upload/sharing recovery and existing-file selection.
3. Search highlighting, accessible keyboard/focus review, account identity polish
   and useful offline/partial-failure recovery.

## Required acceptance and distribution gates

- Two real Gmail accounts on the target Linux desktop: login/restart/keyring,
  account-isolated actions and sorting, incoming notifications/click-to-open,
  Noctalia, suspend/reconnect, fractional scaling, clipboard and native package.
- Confirm delivery, draft conflicts/restart, Drive recipient access and cancelled
  sharing with disposable fixtures; mocked API tests are not live acceptance.
- Sustained giant-mailbox traffic/memory/storage profiling. Cache growth must stay
  within the configured allocation budget, independent of mailbox size. Preserve
  zero-cache mode, remote search, bounded pages and on-demand attachment retrieval.
- Complete Google verification and any required assessment, final demo and external
  security/privacy review before claiming warning-free, uncapped public sign-in.
- Record these results with exact build IDs before tagging 1.0.
