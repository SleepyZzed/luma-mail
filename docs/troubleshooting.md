# Troubleshooting

- **OAuth client missing:** import a Desktop app JSON in Settings. A Web client
  is unsuitable. `LUMA_OAUTH_CLIENT` overrides the normal imported file.
- **Access blocked / testing warning:** add the exact account as a test user in
  Google Cloud. Workspace administrators may restrict OAuth. Follow Google's
  verification process for distribution; Luma cannot bypass consent policies.
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
- **Reset settings:** exit Luma, back up `settings.json`, then correct the invalid
  JSON or move that one file aside. Do not erase unrelated user directories.
