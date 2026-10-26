# Full release checklist

The numbered rows below match section 21 of the original brief. An implemented
API path or a passing mock test is not a passed real-account acceptance test.
The release stays a development prerelease until every mandatory row passes.

**0.2.0 evidence update:** one authorized real account passed Google browser
consent, Windows OS-vault/token refresh and synthetic draft/send/attachment/
read/star/label/archive/Trash/restore/search tests. Sender filter create/list/delete
also passed and its disposable rule was cleaned up. Two-account Linux acceptance
remains open. Rows below retain their original scope; the latest evidence in
docs/verification.md supersedes earlier statements that no account was connected.

New code implements inline replies, an embedded/pop-out composer, basic visual
formatting and hyperlinks, attachment previews/media playback, Linux sanitized
HTML, explicit image loading, stable animated row changes and server-side sender
filters. Full visual HTML draft re-editing, real contact photos and the other
remaining STATUS.md gaps are not claimed complete.

| # | Original acceptance criterion | Current evidence | What remains |
|---|---|---|---|
| 1 | Sign into two Gmail accounts | PKCE/loopback/token tests; Linux keyring; button-only sign-in; configured AppImage; External/In production registration with no test list | Verify live consent and restart with two Gmail accounts. Google verification remains pending; unverified warning and lifetime 100-user cap apply. |
| 2 | Both inboxes display correctly | Gmail list/metadata parsing and account isolation tests | Compare both real inboxes with Gmail, including Workspace if used. |
| 3 | Unified Inbox merges and sorts accounts | Transactional paging and cross-account sorting tests | Verify live ordering and mailbox changes across two accounts. |
| 4 | Unified All Mail without full-history downloads | Projected metadata requests, bounded pages, Spam/Trash exclusion tests | Inspect traffic/storage with large real mailboxes. |
| 5 | Source account is identifiable | Account identity retained in every key and visible row | Desktop review with two accounts, including keyboard/screen-reader access. |
| 6 | Clicking a conversation retrieves its messages | Thread metadata first; individual bodies only on expansion | Real large-thread/MIME interoperability and cancellation checks. |
| 7 | Compose/reply/reply-all/forward work | MIME/header/account tests and real Gmail endpoints | Real delivery/threading tests; polished rich editor and whole-conversation forwarding. |
| 8 | Drafts persist in Gmail across restarts | Debounce, create/update/conflict and attachment-preservation tests | Real restart/concurrent-client tests; better conflict recovery; large-draft editing strategy. |
| 9 | Delete moves the correct message to Trash | Correct message/thread/account API endpoints tested | Confirm changes in Gmail using disposable test mail. |
| 10 | Archive changes Gmail | Removes INBOX with previous-state undo tests | Confirm in Gmail, including mixed-state bulk operations. |
| 11 | Read/unread changes Gmail | Per-message and conversation remote operations tested | Confirm external persistence, undo and concurrent changes. |
| 12 | Labels and stars synchronise | Remote label CRUD/modify and star operations implemented | Real persistence; account-scoped bulk label safety and navigation review. |
| 13 | Search older mail without local indexing | Remote query/pagination tests, advanced builder/date validation/account subsets; no local index | Live old-mail searches and matching-term highlighting. |
| 14 | Attachments work without a historical archive | On-demand retrieval, bounded MIME and reopened-draft tests | Real send/download tests; save-all, safe previews, new inline images. |
| 15 | Large Drive attachments upload/share correctly | Resumable chunks, progress/cancel and recipient grants implemented | Real recipient access, interrupted upload recovery, partial-sharing cleanup and Picker. |
| 16 | Notifications while running/backgrounded | Linux D-Bus acceptance verifies hide/hold/restore/Quit and system Notify transport | Real incoming Gmail, click-to-open, Noctalia/DND, suspend/reconnect and burst tests. |
| 17 | Wallpaper updates Noctalia theme live | Validated template/palette and directory monitoring implemented | Verify installed Noctalia version, atomic replacement, light/dark and live wallpaper changes. |
| 18 | Cache respects its configured budget | Allocated-byte, zero-cache, LRU, TTL and account isolation tests pass | Filesystem/desktop acceptance under actual usage; refresh usage indicators. |
| 19 | Large mailboxes avoid unrestricted downloads | Synthetic 100,000-message paging test; bounded cache/UI buffers | Real giant-mailbox storage and memory profile, cancellation and quota behaviour. |
| 20 | No major control is a placeholder | Visible controls invoke code paths; limitations documented | End-to-end control audit, accessibility, error recovery and all functional gaps below. |

## Mandatory implementation and hardening work

- [ ] WYSIWYG rich-text editing with reliable undo/redo, lists, links and inline images.
- [x] Recipient chips/autocomplete and verified Send-as address choices;
  real alias delivery and two-account acceptance remain pending.
- [ ] Whole-conversation forwarding and safe attachment conveniences.
- [x] Draft conflict recovery preserving unsaved text in a separate Gmail draft;
  the newer server version is kept. Explicit field/body/attachment merging now
  opens a separate copy; live preservation and cleanup passed.
- [x] Account-subset filters and advanced search builder (live acceptance pending).
- [ ] Matching-term highlighting and remaining view preferences.
- [ ] Opt-in autostart/background behaviour, optional sound and account shortcuts.
- [ ] Custom accents, account identity polish and full accessible labels/navigation.
- [ ] Network/offline/account state, useful retry controls and partial-failure handling.
- [ ] Drive interruption/permission recovery and existing-file selection.
- [ ] Safe HTML layout rendering and explicit per-message/sender remote-image choices.
- [ ] Native Arch/CachyOS package test; Wayland, fractional scaling, clipboard and portals.
- [ ] Independent security/privacy review, MIME interoperability and sustained performance profile.

Permanent deletion is conditional on elevated permission and is not required
under the current `gmail.modify` scope. Optional Flatpak packaging and a command
palette are tracked separately from the 20 mandatory behaviours.

## Distribution gates

- [x] Provision Luma's Desktop OAuth client; the 0.1.2 release embeds it
  at build time. No runtime JSON import remains. Publication is gated on this.
- [ ] Resolve Google's scope verification, consent-screen/privacy-policy and any
  applicable assessment requirements before presenting an official public client.
- [x] Publish the External Google registration without a per-user test list for
  early access (25 October 2026). Console confirms In production, 1 / 100 users.
- [x] Submit the English demo and branding/data-access review package (26 October
  2026). Console confirms both under review; Google's approval is outstanding.
- [ ] Verify live consent with two accounts not dependent on a test-list entry;
  complete Google verification for warning-free, uncapped sign-in. Record
  Google's assessment decision and verification/hosting/API costs before
  calling this a full public release.
- [ ] All mandatory live acceptance checks recorded with build/version and results.
- [ ] Source/archive/AppImage checksums, repeatable build and clear upgrade/uninstall instructions.
- [ ] Signing/release ownership and native package verification.

## Current development batch

- [x] Keep network concurrency slots through body consumption/cancellation.
- [x] Release transfer slots during quota backoff so unrelated requests can proceed.
- [x] Resume notification bursts inside a history page instead of losing its tail.
- [x] Add regression tests for these three behaviours.
- [x] Replace recursive missing-OAuth Settings with one guided import/sign-in window.
- [x] Use asynchronous desktop browser launching, retry/copy-link and cancellation.
- [x] Bundle the dconf settings module and honour host/session cursor preferences.
- [x] Verify missing-client onboarding and bundled settings transport in Linux CI (run 40219055786).
- [x] Add advanced search/account subsets and Ctrl+1…9 account navigation.
- [x] Retain label navigation and protect account-scoped bulk label actions.
- [x] Retry failed reads, cancel unnecessary body requests and preserve busy views.
- [x] Debounce preference text edits and flush settings on full exit.
- [x] Publish a checked development AppImage for the next Linux acceptance pass: v0.1.1-dev.1.

## Button-only Google sign-in batch

- [x] Remove JSON import controls, file loading and runtime client overrides.
- [x] Embed the maintainer's Desktop app identity at compile time.
- [x] Gate release publication on a configured, validated app registration.
- [x] Create/configure the Google registration and repository build values.
- [ ] Verify live consent and keyring storage with two Gmail accounts.
- [x] Publish the configured 0.1.2 development AppImage for Linux acceptance: v0.1.2-dev.1.

See [STATUS.md](STATUS.md), [docs/testing.md](docs/testing.md) and
[docs/verification.md](docs/verification.md) for implementation details and test evidence.

## Public early-access sign-in

- [x] Publish repository-backed homepage, privacy policy and terms on GitHub Pages.
- [x] Save those URLs and the Pages domain in Google Auth Platform branding.
- [x] Publish External audience to In production; the test-user table is no longer applicable.
- [x] Assess IMAP/SMTP/JMAP without weakening the original online-first/Gmail criteria.
- [ ] Complete real-account Linux acceptance; configuration evidence is not an end-to-end login test.


## 0.3 presentation and rich-draft milestone

- [x] Move main-window title-bar actions into the mailbox/sidebar; default to a
  frameless window with optional decorations and accessible Quit/Close controls.
- [x] App scaling independent of compositor scaling and sent email font sizes.
- [x] Refined finite mail/composer transitions, reduced motion and system preferences.
- [x] Installed-font chooser, including installed Microsoft families; no font copies.
- [x] Visual restoration of new Luma rich drafts with supported font/list/link spans.
- [x] Density/scale changes avoid refetching the mailbox.
- [x] Pinned sidebar header/footer and Ctrl/Shift selection with loaded-only
  Select all; incompatible bulk actions hidden and guarded.
- [ ] Target Hyprland/fractional-scaling and two-account live acceptance.

The remaining priorities and distribution gates are ordered in docs/roadmap.md.


## 0.4 polish milestone

- [x] Animated composer Back, compact/full-page transitions and explicit pop-out/dock.
- [x] Explicit account folder scope and stable sidebar geometry for long names/labels.
- [x] Right-click user-label deletion and context-sensitive Inbox/Important actions.
- [x] Bounded clipboard-image attachments without a disk spool.
- [x] Opt-in saved-contact photos without a contacts/photo database.
- [x] Record 0.4 Linux/AppImage and live Contacts/draft-conflict checks (see docs/verification.md).
- [x] Complete the Google review video and submit eligible verification material (26 October; under review).

## 0.5 hardening milestone

- [x] Unified folders collapsed initially; animated unified/account disclosure.
- [x] Compact-first new compose with existing layout and expansion behavior.
- [x] Normalize named senders for available contact pictures, retry transient failures.
- [x] Save all attachments sequentially with cancellation and unique filenames.
- [x] Record final 0.5 package/runtime storage and Linux execution evidence (run 40363407935).
- [ ] Target Linux/two-account acceptance and Google approval remain 1.0 gates.

## 0.6 reply and editing milestone

- [x] Small inline reply controls; separate read-only quoted history.
- [x] Serialized autosave/send and late clipboard/file callback protection.
- [x] Recipient chips, numbered lists and bounded formatting-preserving undo/redo.
- [x] Read-only conflict comparison and separate-copy identity.
- [x] Real own-account photo retrieval with existing permission.
- [x] Real self-mail: repeated attachment saves reuse one draft; one delivered mail.
- [x] Record final 0.6 Linux/AppImage/runtime evidence (run 40379375832).
- [ ] Target Linux/two-account acceptance and Google approval remain required.
