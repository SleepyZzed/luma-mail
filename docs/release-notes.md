Luma Mail 0.4.0 early access — composer motion, account folders and contact photos

- Full-page Back keeps the outgoing composer alive for the return animation.
  Restore uses a compact composer inside the app; expanding/contracting animates
  its bounds without changing the main window’s fullscreen state. Pop-out remains
  separate, with a return-to-main control. Reduced motion is respected.
- Per-account Inbox, All Mail, unread/starred, Sent, Drafts, Archive, Junk/Spam and
  Trash appear below the selected account. The heading includes its email address.
  Long names/labels and account changes do not shift the sidebar out of view.
- Right-click a user label to delete it in Gmail, after confirmation. Messages stay.
- Move to Inbox is hidden/guarded in Inbox; Important toggles to Unimportant when
  all selected conversations are important, with consistent mixed-selection rules.
- Paste an image into compose to attach it with a preview. Images stay in memory,
  with existing decoder/MIME limits; no clipboard temp files or attachment archive.
- Draft conflicts preserve edits and offer a separate Gmail draft, keeping the
  newer server version. Sending remains blocked until the conflict is resolved.
- Optional saved-contact profile photos use read-only Google Contacts permission.
  Only visible senders are searched: two jobs, twelve new lookups/minute, 128 small
  decoded photos in RAM and ten-minute expiry. There is no address-book sync or
  disk photo cache. Google does not expose every photo Gmail itself can display;
  unmatched/unavailable senders retain initials. Enable it in Account settings.
- All 0.3 interface scaling, installed-font composition, rich draft restoration,
  pinned sidebar and safe Ctrl/Shift/Select-all behavior remains.

Quit the previous version from its tray menu before launching the replacement.
Accounts and settings stay in their normal locations. All 0.2 mail features remain:

- Faster bounded metadata pages with batches and a rolling account quota budget.
- Confirmed archive/Trash/star/label updates in place, Undo, arrival/removal
  transitions, loading indicators, stable selection and right-click actions.
- Main-window composer, inline replies, top Reply and optional pop-out; basic
  visual formatting, hyperlinks, attachment cards and real upload/send progress.
- Image/DOCX/text previews and in-app audio/video without an attachment archive.
- Linux HTML newsletter layouts, safe embedded images and explicit Load images.
  Systems blocking WebKit's sandbox use a readable-text fallback.
- Gmail-side Block/Unblock filters, optional settings permission, separate
  Report spam/Not spam actions and a Spam folder.
- System cursor settings, notification-daemon delivery, close-to-tray/Open/Quit,
  manual refresh and the existing bounded-cache controls.
- Quiet 60-second history checks update new mail and unread replies in place;
  automatic checks have no page-loading bar. Simple phone mail uses themed text.
- Compact composer Restore, initial To focus, Tab/Shift+Tab text-field traversal,
  bounded recent-recipient completion and blue Ctrl+click links.
- Full-content newsletter height, Settings stacking fixes and one filename
  hyperlink per Drive attachment. Drafts autosave to Gmail; new local copies
  are disabled. Existing unsent recovery copies are kept until saved/discarded.

Sign in with Google opens your browser. No OAuth JSON or personal Cloud project
is needed. There is no test-user list; Google's unverified warning and lifetime
100-user cap remain. Account/Workspace policy can block access. This is not 1.0.

Download the x86_64 AppImage and SHA256SUMS, verify, mark executable and launch.
A session D-Bus, unlocked Secret Service keyring, notification daemon and optional
StatusNotifier tray host are required. WebKit and media codecs make the application
bundle larger; mailbox storage remains bounded. GitHub provides source archives.

One authorized real account passed browser consent (including optional Contacts),
Windows vault/token refresh, synthetic self-mail drafts/send, a 17 MiB Drive upload
and self-delivered filename link, saved-contact photo display, and Gmail filter
creation/removal with cleanup. Two-account Linux/Noctalia, other-recipient Drive
access and sustained mailbox checks remain. Arbitrary sender photos and general
HTML draft visual re-editing remain incomplete. See STATUS and the
release checklist. Notifications use configurable polling, not Gmail push.
