# Implementation and verification status

0.8.0 is **early access**. It does not complete the original brief. Google sign-in
has no per-address test list, but its unverified warning and lifetime 100-user
cap remain. Branding and data-access verification were submitted on 26 October
2026 with an English demo and the homepage ownership appeal. Google Auth Platform
confirms both are under review. Submission is not approval; any assessment
decision and Google's final response remain gates.

## Implemented and exercised

- Linux visual signatures with fonts, colours, lists, links, tables and hosted
  pictures, backed online by Gmail primary Send-as settings. Provider-neutral
  storage interface; other providers and Gmail named-signature libraries are not
  implemented. Public local-picture Drive copies require explicit confirmation.
- Discoverable wrapped formatting toolbar; Ask/conditional Save on close/Manual
  draft modes. No typing or attachment autosaves. See docs/signatures.md.

- Recycled GtkListView metadata/selection and direct row identity, with batched
  page insertion. The 1,000-row native test covers scrolling, offscreen actions,
  selection and releasing/rebuilding widget bindings.
- Shared immutable body/attachment buffers, projected draft revision checks,
  resize-driven HTML measurement and background reader/list reclamation.
  Process-tree/long-soak tooling is documented in docs/performance.md.
- Published process-tree results miss the RAM targets with the default software
  renderer. An optional Cairo comparison improves fresh idle to 136.59 MiB PSS,
  but HTML still exceeds 200 MiB. Real GPU/long-soak acceptance remains pending;
  the defaults are unchanged. See docs/performance.md for RSS, CPU and limits.
- Supplied wordmark and standalone star SVG icon, with a small sidebar resource.

- Rebuilt icon/control visual layer, compact Subject and borderless disclosures;
  compositor-managed resizable Linux windows. Existing layouts and functions kept.
- Verified Send-as aliases with account routing, Reply-To and unavailable-alias guard.
- Field-level three-way conflict merging opens a separate copy with explicit
  body/attachment choices; the original Gmail version is preserved.
- Inline X/repeated Reply/scroll fixes, confirmed draft-only context deletion,
  and connected-account photo reuse within existing opt-in permissions.

- Genuine compact inline reply controls; previous mail is separate, read-only and
  optionally included in the outgoing reply. Rich mail includes a plain-text alternative.
- Slide-up opening and slide-down closing; expand/restore clips final-size content
  without stretching a compact screenshot. Reduced motion remains available.
- Send waits for an active explicit save/clipboard import. Saves reuse the same
  Gmail draft; successful sending is terminal and cancels late save callbacks.
- Removable recipient chips, recent completion, numbered/nested lists, and rich
  undo/redo preserving formatting, fonts and links in capped RAM only.
- Read-only draft conflict comparison, with an explicit separate-copy option.
- Profile lookups restart when an avatar becomes visible; live lookup status is
  available in Settings. Existing optional Drive permission can supply the
  connected account's own photo. No new scopes or arbitrary Gmail-photo lookup.

- Unified mail folders start collapsed, with animated expansion/collapse. Account
  groups remain mounted for opening/closing transitions and stable geometry.
- New non-inline composers start in the existing compact overlay. Expand, Restore,
  Back, inline replies and pop-out retain their established behavior.
- Contact photos normalize display-name From headers to bare addresses. Saved
  contacts can merge available Google profile photos, with bounded retries and
  a Settings retry action. Arbitrary Gmail sender photos are not exposed by Gmail.
- Save all attachments downloads sequentially to an explicitly selected folder,
  with completion progress, cancellation and collision-safe filenames. It keeps
  no attachment archive or upload/download spool.
- Whole-package/runtime storage reports accompany Linux rendering gates. Mesa shaders
  use private runtime storage with a 16 MiB driver limit; zero-cache tests leave no
  body files or other persistent XDG cache files on the CI host.

- Animated composer Back/compact/full-page transitions without main-window fullscreen changes.
- Explicit per-account folders, stable sidebar geometry and right-click Gmail label deletion.
- Image clipboard attachments without a disk spool; separate Gmail draft conflict recovery.
- Optional on-demand saved-contact photos with read-only permission and bounded RAM.
  Real incremental Contacts consent, exact self-contact lookup, public photo fetch/
  decode and list/reader avatars passed with a disposable contact, removed afterward.

- Frameless tiling layout, 75–200% app scaling, optional decorations, reduced
  motion, installed-font composition and visual restoration of new Luma rich drafts.
- Fixed Luma/Compose header and Settings/menu footer around scrolling navigation.
- Ctrl/Shift selection, loaded-only Select all/Clear and shared bulk actions;
  single-conversation actions are hidden and handler-guarded for bulk selections.
- Native GTK4/libadwaita; account Inbox, Unified Inbox and All Mail; remote search,
  25-thread metadata pages, batched reads and a shared rolling account quota budget.
- Server-confirmed row updates for archive, Trash, read/star/labels, previous-state
  Undo, stable reader/scroll, right-click menus and explicit bulk selection.
- Main-window composer, inline replies, top Reply, expand/pop-out; basic visual
  formatting and hyperlinks, Gmail drafts/conflict protection and real byte progress.
- Separate attachment cards in received mail and compose; bounded image thumbnails,
  image/text/DOCX previews and in-memory audio/video playback. DOCX extracts text.
- Sanitized HTML layout in ephemeral, sandboxed WebKit on Linux. Scripts, forms,
  direct network/file loads are blocked. External images require Load images.
  A namespace probe falls back to readable text if the OS blocks the sandbox.
- Optional Gmail sender filters and ownership-checked Unblock; Report spam/Not spam
  and Spam navigation. Future-mail filters work while Luma is closed.
- Native notification-daemon requests, history polling, Refresh, close-to-tray with
  Open/Quit and lost-tray recovery. No extra daemon/autostart is installed.
- Bounded body cache, zero-cache mode, TTL/LRU and clearing; no local history index
  or attachment archive. Noctalia palette monitoring.

Real authorized Windows tests completed browser consent, PKCE/loopback, OS-vault
storage/reload, token refresh and synthetic self-mail draft create/edit/reload/send,
inline photo/DOCX/audio attachments, read/star/label/archive/Trash/restore/search.
Gmail filter create/list/delete and cleanup passed. Unrelated mail was not changed.
The 0.4 review session also saved/reopened/sent a Gmail draft (removed after send)
and uploaded a 17 MiB synthetic file to Drive, then delivered its filename link to
the same account. Other-recipient access/cancellation remain acceptance gates.
A 25-row test measured Inbox 0.93s, Load more 0.70s, Archive 1.12s and All Mail 0.88s.
These are observations, not latency guarantees.

Linux CI checks compilation/tests, actual GTK/D-Bus close/restore/Quit, native
notification transport, AppImage launch and cursor-settings transport. HTML/media
diagnostics additionally launch the real engines; see docs/verification.md for
executed results rather than assuming runtime success from compilation.

0.6 passed Linux release, native GTK/clipboard and native/bundled X11/Wayland
HTML/media checks, including isolated-system-WebKit and read-only AppImage runs.
The actual AppImage checksum and full installation/runtime storage breakdown
are recorded in docs/verification.md. A real native Windows compose + inline
reply test confirmed two delivered messages and no remaining matching drafts.
The own-account picture appeared in the actual sidebar/list/reader.

## Remaining release work

- Two real Gmail accounts and restart in Linux Secret Service; account-isolated
  sorting/actions, incoming notification clicks and installed Noctalia/Wayland.
- User-tested automatic Drive upload works; recipient access, cancellation and
  partial-sharing recovery still need formal acceptance.
  Existing authorized file IDs work; Google Picker remains absent.
- Arbitrary sender photos beyond Google's supported APIs remain unavailable.
  Inline-image insertion and general HTML editing remain open; alias delivery
  across two real accounts still needs acceptance.
  Luma 0.3-generated rich drafts reopen visually; complex/older/third-party HTML
  remains in source mode. General visual HTML re-editing is incomplete.
- Editing attachments over 16 MiB combined,
  whole-conversation forwarding and persistent sender remote-image preferences.
- View preferences/search highlighting, accents, optional autostart/sound,
  keyboard/accessibility/HiDPI audit, native Arch acceptance and sustained profiling.
- Google's verification decision, any required assessment and external security review.

The 20 acceptance criteria remain tracked in RELEASE-CHECKLIST.md.
