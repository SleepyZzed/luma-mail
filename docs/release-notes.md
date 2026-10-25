Luma Mail is an online-first Rust / GTK4 / libadwaita Gmail client for Linux.

0.1.2 Google sign-in update:

- Sign in with Google opens your default browser directly. The app's Desktop
  OAuth registration is supplied by the maintainer at build time.
- Removed client JSON import controls and runtime configuration overrides.
  Connecting an account no longer requires a Google Cloud project per user.
- Release publication is blocked if the app registration is missing or invalid.
- Asynchronous browser launching, reopen/copy-link recovery and cancellation.
- Bundled dconf settings support and host cursor-theme paths; session cursor
  overrides and live configured desktop cursor preferences.
- Advanced search with dates, flags and any selected subset of enabled accounts;
  Ctrl+1…9 account switching.
- Retry failed pages/messages, cancel body fetches when collapsed or left behind,
  preserve loaded pages/bulk selection during background changes and highlight Refresh.
- Persistent label navigation, safe name-based cross-account label mapping,
  overlapping-write protection and debounced settings text edits.
- Network limits cover response bodies; quota backoff releases transfer slots.
  Notification bursts continue within a history page without discarding its tail.
- RELEASE-CHECKLIST.md maps every original acceptance criterion to remaining work.

**Public early-access update — 25 October 2026:** Luma's Google registration is
now **External / In production / unverified**. There is **no test-user list**:
eligible Gmail users can request consent without being added manually. Google's
unverified-app warning and **100 lifetime user cap** remain until verification;
account protection and Workspace policy can block access. This is an unfinished
early-access release, not a production-complete or Google-verified application.

The existing 0.1.2 AppImage already embeds this registration, so this is a
server-side audience change and release-documentation update; its binary and
SHA-256 checksum are unchanged. No JSON file or personal Cloud project is needed.
New production grants are not subject to Testing's seven-day lifetime; reconnect
if an older grant has expired. Public homepage, privacy policy and terms are at
[Luma Mail](https://sleepyzzed.github.io/luma-mail/). The protocol assessment and
remaining verification gates are in the [OAuth guide](https://github.com/SleepyZzed/luma-mail/blob/main/docs/oauth.md).

- Gmail accounts, remote search, paging, lazy body retrieval and server-backed mail actions.
- Gmail drafts, reply/forward, bounded attachments and Drive upload/sharing paths.
- System notification-daemon delivery, periodic Gmail history polling, Refresh now / Ctrl+R.
- Close to tray with Open and Quit actions where a StatusNotifier host is available.
- Enforced disposable cache and optional Noctalia palette integration.

Download the x86_64 AppImage, mark it executable and run it. Verify its checksum
against `SHA256SUMS`. Source archives are supplied automatically by GitHub.

Complete Google's consent after clicking Sign in with Google. An
unlocked Linux Secret Service keyring, session D-Bus, notification daemon and
optional tray host are required. User credentials are never bundled. Anyone
migrating from a personal client in an older preview must reconnect their account
to authorize Luma's new app registration.

Read `README.md`, `STATUS.md`, `docs/oauth.md`, `docs/noctalia.md` and
`docs/testing.md` before using real mail. Build/launch checks do not establish
real Gmail/Drive interoperability, correct Google OAuth configuration, or a full
Hyprland/Noctalia desktop acceptance pass. Notifications use polling, not push.
