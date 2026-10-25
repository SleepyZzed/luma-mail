# Implementation and verification status

This is a substantial **development implementation**, not a claim that the full
project brief or all 20 acceptance criteria have passed. No user credentials
were available and no real Google account was connected or modified.

## Working code paths

| Area | Implemented | Verification boundary |
|---|---|---|
| Foundation | Rust workspace; native three-pane GTK4/libadwaita window; XDG settings; loading/error status; worker runtime | Windows and Ubuntu Linux build/native launch checked, including the bundled AppImage. Real keyring interaction remains manual. |
| Authentication | Default browser, loopback listener, S256 PKCE, state validation, scopes, per-account refresh serialization, Linux Secret Service, reconnect/revoke | OAuth URL, state and mocked refresh tested. Real consent, keyring prompts and revocation need Linux testing. |
| Mail access | Multiple enabled Gmail accounts, metadata projection, 25-row provider pages, transactional unified merge, account identity, one-body-per-expansion retrieval | Mocked response/search/pagination tests; real mailbox not connected. |
| Mail changes | Conversation and individual read/unread, stars, archive, inbox, Trash/restore, importance; labels CRUD/add/remove; cross-account dispatch; previous-state undo | Endpoint and semantic tests. UI waits for server confirmation; no speculative local success. Real persistence needs browser comparison. |
| Search and folders | Gmail operators, advanced builder with dates/flags/account subsets, Inbox/All Mail/Unread/Starred/Sent/Drafts/Archive/Trash, cached label navigation | Remote query, date validation and Spam/Trash exclusion tested. Search is date-oriented, not relevance-ranked. |
| Composition | Multiple native compose windows; To/Cc/Bcc validation; plain text and HTML source; markup insertion; reply/all and message forwarding with bounded attachments; signatures; undo/redo | MIME, headers and validation tested. Real send/reply threading needs Google testing. |
| Drafts | Gmail creation/update/delete/send; 2-second debounce; save state; message-ID conflict check; reopen with attachments fetched only on explicit edit, bounded to 16 MiB | Mocked create/conflict/attachment tests. Gmail lacks atomic draft CAS, so a residual cross-client race remains. |
| Attachments | Native picker/drop, file list/removal, MIME attachments up to 16 MiB combined, on-demand save | MIME/size tests. Received attachment responses capped; no historical archive. |
| Drive | Opt-in consent; 256 KiB resumable chunks; real progress; cancellation; existing authorized file IDs; explicit recipient read access | Implemented against documented API. Requires live upload/sharing verification; no mock claim of recipient access. |
| Desktop | Real system notification-service requests, bounded history polling, Refresh now / Ctrl+R, privacy settings; close-to-tray, Open/Quit menu, lost-tray fallback; mailto; launcher/icon | Dedup/baseline/close-policy/menu tests; Linux D-Bus acceptance verifies actual GTK hide/hold/restore, tray Quit and notification transport. Noctalia display and notification clicks remain manual. |
| Themes | System/dark/light/Noctalia modes, validated palette, parent-directory file monitor, contrast fallback, documented v5/v4 templates | Palette parsing/validation tested. Installed Noctalia unavailable on Windows; wallpaper hot reload needs Linux test. |
| Storage | Enforced allocated-byte cache, presets/custom limit, TTL, LRU, per-account clearing, clear-on-exit, disabled = zero contents | Budget/eviction/expiry tests and synthetic 100,000-message paging test. Real giant-mailbox acceptance remains manual. |
| Delivery | Public GitHub source, lockfile, license, desktop assets, Arch package recipe, Linux CI, setup/privacy/test/troubleshooting guides, published x86_64 AppImage prerelease | Full Linux release workflow passed; downloaded bundle checksum verified. Results in docs/verification.md. makepkg has not been run. |

## Still incomplete

These are real gaps, not hidden controls or placeholders:

- Full WYSIWYG rich-text editing, recipient chips/autocomplete, inline image
  insertion, contact integration and authorized send-as aliases.
- Editing drafts whose attachments exceed 16 MiB combined; those drafts are
  explicitly refused rather than stripped. A whole conversation cannot yet be
  forwarded in one operation; individual-message forwarding preserves attachments.
- Save-all attachments, attachment previews, granular image-loading consent,
  HTML layout rendering and per-sender remote-content preferences. Reader uses
  safe text conversion; it does not claim browser-equivalent rendering.
- Search term highlighting; user-selectable sort/datetime format;
  horizontal/hidden reading pane; conversation grouping preference; account
  avatars/custom colours; configurable custom accents.
- Opt-in standalone notification process, opt-in session autostart,
  notification sound and command palette.
- Permanent message deletion and the corresponding elevated OAuth scope.
- Google Picker for previously unapproved Drive files, upload session persistence
  across application restarts, and rollback of partial Drive sharing grants.
- Full accessibility/Wayland/HiDPI review, exhaustive MIME interoperability,
  external security review, restricted-scope verification, signed releases and
  Flatpak packaging.

The next release priorities are Linux/Google acceptance testing, draft
reconciliation, WYSIWYG composition, then
remaining desktop and accessibility work. Do not distribute this build as
production-ready or describe the entire brief as complete.

## Local checks

The final verification results are recorded in `docs/verification.md`. Tests use
local mock servers and synthetic data. The native preview is an actual GTK
launch with zero connected accounts, not fabricated mailbox contents.

## 0.1.1 development work

- Guided one-window OAuth setup/import, asynchronous browser launch, copy-link,
  retry and cancellation. That 0.1.1 preview had no bundled OAuth registration.

## 0.1.2 Google sign-in change

Main removes the JSON importer and runtime file override. Sign in with Google
uses the maintainer's Desktop client embedded at compile time, keeping browser
launch/retry/cancellation and the existing PKCE/Secret Service flow. Release
publication is gated on valid app registration values. Luma's Desktop client is
now created and the two GitHub build values are configured. Google remains in
External Testing mode, with limited test users and seven-day grants. Live consent,
keyring acceptance and public Google verification remain pending. The published
0.1.1 binary still uses the old flow until replaced with the configured 0.1.2 build.
- AppImage dconf module and host cursor-theme paths; session cursor overrides,
  live desktop cursor settings and diagnostics. Configuration transport passed
  Linux CI; the reporter's cursor artwork still needs another desktop check.
- Advanced search/account subsets and Ctrl+1…9 account switching.
- Retry failed page/message loads; cancel body requests on collapse/navigation.
- Preserve multi-page views, scroll position and bulk selection when background
  changes arrive; highlight Refresh to request an update.
- Retain sidebar label navigation across unread-count updates; map cross-account
  label actions by name and reject missing matches; prevent overlapping writes.
- Keep HTTP limits through body consumption and release slots during backoff;
  bounded notification-burst continuation; debounce settings text-field writes.

The [release checklist](RELEASE-CHECKLIST.md) remains the acceptance ledger.
