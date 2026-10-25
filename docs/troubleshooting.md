# Troubleshooting

## HTML fallback on systems restricting namespaces

HTML previews require WebKit's namespace sandbox. Ubuntu 24.04 restricts
unprivileged user namespaces for third-party executables; Luma checks this and
shows readable text when its sandbox cannot start. Use your distro's native
WebKit/bubblewrap packages or have an administrator review a narrowly scoped
AppArmor userns profile for the actual bwrap executable. Do not disable WebKit's
sandbox or the OS-wide namespace restriction to view email. The official
[Ubuntu release notes](https://documentation.ubuntu.com/release-notes/24.04/)
explain the policy. Arch/CachyOS follow their own kernel/LSM configuration.

- **Sign-in unavailable in this build:** this is an unconfigured developer build.
  Install the maintainer's sign-in-enabled build. Account setup has no JSON
  import; the app registration is supplied during compilation by the maintainer.
- **Old import/setup screen:** 0.1.0 and 0.1.1 binaries predate button-only login.
  Update to the published configured 0.1.2 build. Old client JSON files and
  `LUMA_OAUTH_CLIENT` are ignored by the new code.
- **Browser launch fails:** use Open browser again or Copy sign-in link in the
  sign-in window. Closing it cancels the listener; an attempt expires in five
  minutes. A normal desktop URI handler/portal must be configured.
- **Wrong cursor in the AppImage:** the 0.1.1 bundle includes dconf settings
  support and keeps host/user cursor paths visible. About & Diagnostics shows
  the effective GTK cursor. Check `XCURSOR_THEME`, `XCURSOR_SIZE` and the normal
  desktop cursor preference if it differs; no app-specific cursor is installed.
- **Google warning / access blocked:** Luma is published for unverified early
  access, with no test-user list. Google's unverified-app warning and 100-user
  lifetime cap remain; Advanced Protection and Workspace policy may block it.
  Review the app and requested permissions before deciding whether to consent.
  Luma cannot override a Google or administrator block. If Google explicitly
  says only approved test users are allowed, report the exact error and build
  version to the maintainer; importing JSON is not a solution.
- **Keyring unavailable:** run and unlock GNOME Keyring or a KWallet Secret
  Service backend in your user D-Bus session. On Hyprland, configure your normal
  PAM/keyring integration. Luma will not save plaintext tokens.
- **Reconnect needed:** consent may have expired or been revoked. Use Settings
  → Account → Reconnect and choose the same email address.
- **Mailbox error:** Ctrl+R refetches current server state. Search is submitted
  with Enter. After a failed page request, refresh to reset the view.
- **Sent state uncertain:** interrupted sends are never replayed automatically.
  Inspect Gmail Sent/Drafts before creating another send attempt. The originating
  composer remains locked to prevent a repeated button press from sending twice.
- **Draft conflict:** preserve your editor contents and open the newer Gmail
  draft. Luma checks the server's message ID before updating, but Gmail does not
  provide an atomic compare-and-swap draft endpoint.
- **Large body / attachment error:** response limits protect memory. Use Gmail
  for unusually large bodies; use Drive for composed files above 16 MiB total.
- **Drive access denied:** enable Drive in the account settings and complete
  consent. `drive.file` does not grant arbitrary access to your entire Drive.
  Sharing may be denied by organization policy; no link is inserted on failure.
- **Theme fallback:** inspect `luma-mail/noctalia.toml`, confirm your Noctalia
  version/configuration, and use a palette with adequate text contrast.
- **No notifications:** keep Luma running, enable account notifications and check
  the desktop's notification permission / Do Not Disturb. This is polling, not
  Gmail push. Initial history baselines deliberately suppress old messages.
- **Check the notification daemon:** Settings → Notifications & Privacy → Send
  a system test notification uses the same desktop notification service as mail
  alerts. Press Refresh / Ctrl+R for an immediate Gmail history check. The app
  must remain running, including while hidden in the tray.
- **Close exits rather than hiding:** enable Close to system tray and start a
  StatusNotifier host before launching Luma; on Noctalia enable its tray widget.
  The safe fallback is to exit when no usable tray exists. Tray → Quit always
  exits completely; a pending composer save can keep the app open for attention.
- **Reset settings:** exit Luma, back up `settings.json`, then correct the invalid
  JSON or move that one file aside. Do not erase unrelated user directories.
