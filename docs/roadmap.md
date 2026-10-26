# Road to 1.0

The original 20 acceptance criteria in RELEASE-CHECKLIST.md remain mandatory.
0.7 is an early-access milestone, not a completed 1.0 release.

## This milestone

- Frameless Hyprland-friendly navigation; optional compositor decorations.
- Fixed Luma/Compose header and sidebar footer; Ctrl/Shift selection with safe
  bulk actions and loaded-only Select all.
- App UI scaling, reduced motion, refined transitions and local density updates.
- System-font composition and visual reopening of new Luma-generated rich drafts.
- No mailbox mirror/index, font downloads or new local draft/attachment archive.

## 0.4 hardening milestone

- Animated Back and compact/full-page composition without changing desktop fullscreen.
- Explicit account folders, stable navigation, label deletion and context-aware actions.
- Clipboard-image attachments in memory and separate Gmail drafts for conflicts.
- Optional saved-contact photos with bounded lookup/RAM, no contacts/photo mirror.

## 0.5 hardening milestone

- Collapsed unified folders and persistent animated account disclosures.
- Compact-first composition, preserving existing expand/restore behavior.
- Correct display-name matching and retry handling for available contact photos.
- Sequential attachment Save all with progress, cancellation and safe filenames.
- Separate whole-package/runtime allocation reports alongside zero-cache gates.

## 0.6 reply and editing milestone

- Small inline replies and separate read-only quoted history.
- Native-size slide/reveal composer transitions without glyph stretching.
- Serialized attachment autosaves/send, terminal sent state and late callback guards.
- Recipient chips, numbered lists and formatting-preserving bounded rich undo/redo.
- Read-only conflict comparison and distinct identity for intentionally separate copies.
- Visible-avatar lookup scheduling and own-account photo using existing Drive access.

## 0.7 editing and visual milestone

- Field-level three-way draft comparison; explicit body/attachment conflicts open
  a separate merged draft and preserve the original Gmail version.
- Verified Send-as addresses from Gmail, with account-isolated routing, Reply-To
  headers, unavailable-alias guards and matching inline/full From controls.
- Saved inline replies close correctly; repeated Reply keeps the editor inline,
  preserves its text and reveals it in the reader.
- Draft-only context deletion, with confirmation and changed-draft checks.
- App-owned vector icons, quieter disclosures, stable input focus and compact
  subject fields; resizable borderless Linux windows. See visual-system.md.
- Available own-account photos can be reused across opted-in connected accounts.

- Recycled GtkListView rows, direct identity lookup, batched page insertion,
  shared immutable payloads and event-driven HTML measurement.
- Hidden reader/list reclamation and complete process-tree profiling; real
  sustained sync/desktop acceptance still required. See performance.md.
- User-supplied Luma wordmark and a standalone SVG star mascot for the app.

## Next implementation priorities

1. Inline image insertion and whole-conversation forwarding;
   interrupted Drive upload/sharing recovery and existing-file selection.
2. Search highlighting, accessible keyboard/focus review, account identity polish
   and useful offline/partial-failure recovery.
3. Keep complex external HTML intact until its editor can represent it without
   silently losing content. Complete alias delivery and target-desktop acceptance.

## Required acceptance and distribution gates

- Two real Gmail accounts on the target Linux desktop: login/restart/keyring,
  account-isolated actions and sorting, incoming notifications/click-to-open,
  Noctalia, suspend/reconnect, fractional scaling, clipboard and native package.
- Confirm delivery, draft conflicts/restart, Drive recipient access and cancelled
  sharing with disposable fixtures; mocked API tests are not live acceptance.
- Sustained giant-mailbox traffic/memory/storage profiling. Cache growth must stay
  within the configured allocation budget, independent of mailbox size. Preserve
  zero-cache mode, remote search, bounded pages and on-demand attachment retrieval.
- Complete Google verification and any required assessment, and external
  security/privacy review before claiming warning-free, uncapped public sign-in.
  The English demo and formal request were submitted on 26 October 2026; Google
  reports branding and data access under review.
- Record these results with exact build IDs before tagging 1.0.

## After 1.0

Only after Gmail 1.0 is polished and accepted, add complete providers behind the
existing provider abstraction. IMAP/SMTP/JMAP/Outlook must keep remote search,
on-demand message/attachment access and enforced cache budgets; no offline mirror.
