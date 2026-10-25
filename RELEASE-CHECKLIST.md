# Full release checklist

The numbered rows below match section 21 of the original brief. An implemented
API path or a passing mock test is not a passed real-account acceptance test.
The release stays a development prerelease until every mandatory row passes.

| # | Original acceptance criterion | Current evidence | What remains |
|---|---|---|---|
| 1 | Sign into two Gmail accounts | PKCE/loopback/token tests; encrypted Linux keyring implementation; guided setup regression checked | Import a valid Desktop OAuth client, then verify consent and restart with two real accounts. The first Linux report exposed missing-client onboarding; the guided import fix is implemented. |
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
- [ ] Recipient chips/autocomplete and authorised sending aliases.
- [ ] Whole-conversation forwarding and safe attachment conveniences.
- [ ] Draft conflict reconciliation and recovery that preserves unsaved text.
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

- [ ] Decide and provision the public Desktop OAuth client; this preview currently
  requires each tester to import their own. Never publish fabricated credentials.
- [ ] Resolve Google's scope verification, consent-screen/privacy-policy and any
  applicable assessment requirements before presenting an official public client.
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
- [ ] Publish a checked development AppImage for the next Linux acceptance pass.

See [STATUS.md](STATUS.md), [docs/testing.md](docs/testing.md) and
[docs/verification.md](docs/verification.md) for implementation details and test evidence.
