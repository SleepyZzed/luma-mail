Luma Mail is an online-first Rust / GTK4 / libadwaita Gmail client for Linux.

This is a **development preview**, not a production-complete implementation.

- Gmail accounts, remote search, paging, lazy body retrieval and server-backed mail actions.
- Gmail drafts, reply/forward, bounded attachments and Drive upload/sharing paths.
- System notification-daemon delivery, periodic Gmail history polling, Refresh now / Ctrl+R.
- Close to tray with Open and Quit actions where a StatusNotifier host is available.
- Enforced disposable cache and optional Noctalia palette integration.

Download the x86_64 AppImage, mark it executable and run it. Verify its checksum
against `SHA256SUMS`. Source archives are supplied automatically by GitHub.

You must configure a Google Desktop OAuth client and complete consent. An
unlocked Linux Secret Service keyring, session D-Bus, notification daemon and
optional tray host are required. No credentials are bundled.

Read `README.md`, `STATUS.md`, `docs/oauth.md`, `docs/noctalia.md` and
`docs/testing.md` before using real mail. Build/launch checks do not establish
real Gmail/Drive interoperability, correct Google OAuth configuration, or a full
Hyprland/Noctalia desktop acceptance pass. Notifications use polling, not push.
