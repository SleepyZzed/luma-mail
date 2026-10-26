Luma Mail 0.3 early access — a cleaner interface and font-aware composer

- Frameless main window for Hyprland/tiling desktops. Refresh, Undo and Select
  move beside the mailbox title; Settings and the app menu stay in the sidebar.
  Optional window decorations, Close window and Quit remain available.
- Interface scaling from 75–200%, with Ctrl++/Ctrl+- and Ctrl+0 reset. Text,
  app controls and HTML previews scale without changing sent email font sizes.
- Refined easing/fades for mail arrivals, opening messages and composer changes.
  Reduced motion and desktop animation preferences are respected. No idle loops.
- Installed-system-font chooser, including installed Microsoft families; 8–72 pt
  sizes, selection/further-typing support and Reset font. No font downloads/copies.
- New Luma rich drafts reopen visually with supported fonts, lists and links.
  Complex, third-party and older HTML retains source mode to prevent silent loss.
- Density/scale changes update the visible UI without mailbox reloads. Existing
  cache budgets, Gmail-only drafts and on-demand attachment rules remain.

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
