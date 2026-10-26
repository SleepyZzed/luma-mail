Luma Mail 0.2.2 early access — HTML layout and theme fixes

- Newsletter tables keep complete words in narrow columns, preserve box sizing
  and no longer repeat the hidden HTML title above the message.
- Neutral HTML surfaces and text follow the active system/Luma/Noctalia palette.
  Original colours restores the sender’s palette for an individual message.
  Brand button colours and image pixels retain their colours.
- Inline images fit within the reader width and 480 pixels of height. Attachment
  cards still provide larger previews and saving.
- Theme/colour changes reuse bounded prepared images in memory, without fetching
  them again. External images still require the explicit Load images action.
- Preserves 0.2.1’s sandboxed AppImage WebKit fix and online-first storage model.

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

One authorized real account passed browser consent, Windows vault/token refresh,
synthetic self-mail operations and Gmail filter creation/removal. Two-account Linux/
Noctalia, Drive recipient access and sustained mailbox checks remain. Real contact
photos and full HTML draft visual re-editing are incomplete. See STATUS and the
release checklist. Notifications use configurable polling, not Gmail push.
