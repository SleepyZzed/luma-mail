# Implementation and verification status

0.5.0 is **early access**. It does not complete the original brief. Google sign-in
has no per-address test list, but its unverified warning and lifetime 100-user
cap remain. Branding and data-access verification were submitted on 26 October
2026 with an English demo and the homepage ownership appeal. Google Auth Platform
confirms both are under review. Submission is not approval; any assessment
decision and Google's final response remain gates.

## Implemented and exercised

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

## Remaining release work

- Two real Gmail accounts and restart in Linux Secret Service; account-isolated
  sorting/actions, incoming notification clicks and installed Noctalia/Wayland.
- User-tested automatic Drive upload works; recipient access, cancellation and
  partial-sharing recovery still need formal acceptance.
  Existing authorized file IDs work; Google Picker remains absent.
- Recipient chips, arbitrary sender photos beyond saved Google contacts,
  send-as aliases, richer lists and inline-image insertion in compose.
  Luma 0.3-generated rich drafts reopen visually; complex/older/third-party HTML
  remains in source mode. General visual HTML re-editing is incomplete.
- General conflict comparison/merging, editing attachments over 16 MiB combined,
  whole-conversation forwarding and persistent sender remote-image preferences.
- View preferences/search highlighting, accents, optional autostart/sound,
  keyboard/accessibility/HiDPI audit, native Arch acceptance and sustained profiling.
- Google's verification decision, any required assessment and external security review.

The 20 acceptance criteria remain tracked in RELEASE-CHECKLIST.md.
