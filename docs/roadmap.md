# Road to 1.0

Current frontend coverage and remaining acceptance are tracked in
[fluid-parity.md](fluid-parity.md). The 0.10 preview is GTK-free; the legacy source
is retained as the reference until full acceptance.

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

The custom Qt Quick frontend preserves the Rust provider/cache engine and now has
animated shared row/reader projection, adaptive panes, collapsed unified folders,
account disclosures, selection/context actions, remote search highlighting,
private HTML/media, inline/compact/full/pop-out composition, rich formatting,
Gmail draft/conflict/alias flows, online signatures and sign-in recovery.

Published 0.10.0-dev.1 (d4889b7) portable acceptance passes 272 Windows checks
and 269 Linux checks. Both
packages pass without SDK search paths and the Windows provenance is clean.
The latest local on-demand composer passes 272 native checks and destroys its
editor, document, previews, undo and popups after closing springs settle. New
composition and inline replies recreate current state; rapid reopening retains
its existing editor. Source preservation and draft policy are unchanged.

The exposed Windows full-process 60-second HTML audit passes ten lifecycle checks:
136.3 MiB private working set at baseline, 252.2 with HTML and 58.8 backgrounded;
private commit is recorded separately. Nine settled HTML CPU samples are 0.00%
of one core at the measurement resolution. Zero-cache leaves only 51 logical
bytes of settings and zero cache files. These are synthetic Windows measurements;
Linux/GPU, longer real sync and the 150/200-MiB goals remain open. The matching Linux
d4889b7 software-rendered PSS is 225.8 MiB idle/400.9 with HTML.

Remaining work:

- Finish inline-image authoring, whole-conversation forwarding and interrupted
  Drive sharing/upload recovery with the same bounded RAM/online-only model.
- Complete two-account Gmail, real Hyprland/GPU/Noctalia/notification/keyring,
  suspend/reconnect, fractional scaling and sustained mailbox/resource checks.
- Complete Google verification and the required security/privacy acceptance.
  The submitted review is still recorded as pending; no approval is inferred.

The 0.9 tag remains the reference until the full parity and target-desktop gates
pass. See ui-migration.md, fluid-validation.md and RELEASE-CHECKLIST.md for exact
wiring, evidence and the original twenty acceptance criteria.

## Fluid feedback and keyboard accessibility

- Center/pad button contents, preserving aligned navigation labels and compact rows.
- Give all app menus/dialogs explicit themed bounds, rounded surfaces and spring
  entrance/exit; create editor context menus only on demand and release them.
- Restore pointer-opened inbox actions and add Shift+F10/Menu-key equivalents,
  selection-aware editing actions and accessible text-editor names.
- Animate pop-out handoff using the same document/session and native host, retaining
  Windows decorations and Linux compositor control.
- Close outgoing inline replies on conversation changes through Ask/Manual/Save-on-close;
  keep unsaved text if Ask is declined and leave detached composition independent.

These advance the keyboard/focus and UI hardening priorities. Full accessibility,
live desktop acceptance and the remaining authoring/Drive/resource gates above
remain open.


The feedback milestone is published as [v0.10.0-dev.2](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.10.0-dev.2),
source a23a207. All fifteen portable groups pass (296 Linux/299 Windows).
The callback/feature audit and exact resource evidence are recorded; next code
priorities remain inline-image authoring, whole-conversation forwarding and Drive
interruption/permission/existing-file recovery. Target desktop, RAM, independent
security and Google approval remain release gates.
