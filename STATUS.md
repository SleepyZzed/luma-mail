# Implementation and verification status

0.2.2 is **early access**. It does not complete the original brief. Google sign-in
has no per-address test list, but its unverified warning and lifetime 100-user
cap remain. Formal verification is not submitted: homepage ownership propagation
and the final review video are still gates.

## Implemented and exercised

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
- Recipient chips, real contact photos (current avatars use initials),
  send-as aliases, richer lists and inline-image insertion in compose.
  Existing HTML drafts reopen as source; full visual HTML re-editing is incomplete.
- Draft conflict reconciliation, editing attachments over 16 MiB combined, save-all,
  whole-conversation forwarding and persistent sender remote-image preferences.
- View preferences/search highlighting, accents, optional autostart/sound,
  keyboard/accessibility/HiDPI audit, native Arch acceptance and sustained profiling.
- Google verification, final demo, any required assessment and external security review.

The 20 acceptance criteria remain tracked in RELEASE-CHECKLIST.md.
