# Road to 1.0

The original 20 acceptance criteria in RELEASE-CHECKLIST.md remain mandatory.
0.9 is an early-access milestone, not a completed 1.0 release.

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
- Serialized explicit draft saves/send, terminal sent state and late callback guards.
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

## 0.8 online signatures and draft policy

- Visible, individually wrapped composer formatting controls on Linux.
- Ask on close by default, conditional Save on close, or Manual Save only;
  typing and attachment callbacks never start background saves.
- Provider-neutral online signature model and Gmail primary Send-as editing.
- Visual rich formatting, installed fonts, links, tables and hosted pictures;
  public local-picture copies require confirmation, progress and cancellation.
- Draft signatures stay separate from authored text/quote and are not duplicated.
- Bounded ephemeral editing and previews; no new persistent signature/image store.

## 0.9 cross-platform editing milestone

- Windows x64 portable package, native tray/notification transport and keyring,
  title-bar controls, visual signatures and memory-only media playback.
- One initial Google consent flow with selectable optional features, actual-scope
  enablement and Gmail-only fallback after optional permission denials.
- Native composer colours/highlights, custom colours, alignment, line spacing,
  indent, superscript/subscript, code, bounded find/replace and source preservation.
- Complete closed-composer release, keyboard/popover teardown and event-driven
  Windows HTML layout/IPC. No new mailbox, attachment, signature or draft store.

## Next build — custom interface replacement

The user has requested replacing the GTK frontend after 0.9, with a distinctive
shared interface, fluid morphing transitions and a right-side sliding composer.
Keep the Rust engine and all existing functionality, Linux priority, Noctalia v5
colour integration, Windows title-bar controls and the same resource/storage
constraints. See ui-redesign.md. No framework migration is part of 0.9.

## Remaining 1.0 implementation priorities

1. Attribute remaining RAM/CPU to individual processes and renderer allocations;
   measure longer settled HTML phases and real GPU frame timing. The default
   0.7 CI run misses the RAM goals; Cairo improves fresh idle but HTML remains
   above 200 MiB. Preserve accurate rendering and smooth interaction while
   investigating lower-cost defaults; see performance.md for measured results.
2. Inline image insertion and whole-conversation forwarding;
   interrupted Drive upload/sharing recovery and existing-file selection.
3. Search highlighting, accessible keyboard/focus review, account identity polish
   and useful offline/partial-failure recovery.
4. Keep complex external HTML intact until its editor can represent it without
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

## Fluid replacement progress — 28 October

Advanced remote search/account subsets, native mailto/single-instance activation,
three-way draft comparison, per-message actions, stable account delegates,
keyboard shortcuts and adjustable panes are now implemented in the replacement.
Save/send reconciles the affected remote conversation IDs without a view reload.
Windows process-tree auditing caught a reentrant duplicate HTML-controller leak;
the replacement now guards construction and explicitly releases inactive readers.
The migration retains the original GTK release until full signature/editor and
remaining parity, packaging, resource and target-desktop gates pass. See
ui-migration.md and fluid-validation.md for current evidence and open work.

The native online signature editor is implemented and Windows-tested, including
font/colour/list/table/picture controls, source protection, public-image upload
approval and coordinated close/quit policies. No signature/image archive was
added. Whole-conversation forwarding, remaining layout/editor parity, packaged
Linux/Windows acceptance and resource hardening still gate the replacement.
The full Linux software-rendered baseline is 330.8 MiB PSS; resource targets are
not met. Lazy browser-profile lifetime and detailed process attribution are being
measured next rather than treating the lower Windows private-working-set result
as a Linux result.


Long conversations now have bounded 32-header views with explicit earlier/later/
latest navigation, lazy collapsed-body views and preserved reading positions when
new replies arrive. A 2,503-message synthetic test verifies all headers remain
accessible, and native checks verify bounded delegates and latest-message reveal.
The resource profiler now retains failed-run counters rather than losing them;
Linux software-renderer diagnostics and real GPU measurements remain open gates.


The c4b07ec Linux run passed all functional scenes. Its controlled Chromium-software
resource test measured 258.2 MiB PSS at idle, 452.8 with HTML and 405.5 after release/
background, with near-zero settled CPU samples. The RAM targets are still unmet.
Per-process data identifies the Qt host and persistent browser framework helpers;
this is not an email-cache growth issue. Actual GPU/Hyprland profiling is required.


The replacement now restores a real pop-out/dock composer with one retained RAM
session, spring-completion cleanup, saved window size, compact inbox density and
unified account badges. Windows native acceptance has 219 passing scene checks.
Its measured private working set is 128.8 MiB baseline, 251.3 with HTML and 98.6
while backgrounded after graphics release; private commit is reported separately.
Linux PSS and full-package/live-account gates remain open. Next parity work includes
sign-in recovery/cancellation, remaining editing/forwarding flows and distribution.


Sign-in recovery/onboarding is implemented in the replacement, including browser
retry/copy/cancel, official-URL validation and credential-commit/quit fencing.
Windows native acceptance now has 233 passing scene checks. Both platforms passed
all migration checks at 063ab88; its Linux software-rendered PSS is 247.4 MiB idle,
410.8 with HTML and 331.1 backgrounded. Resource goals and real desktop/Google
acceptance remain open. Remaining parity includes file drops and read/undo recovery;
inline images, whole-conversation forwarding and packaging remain 1.0 priorities.
