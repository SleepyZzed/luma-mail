# Google OAuth setup

Luma requires your own Google Cloud OAuth client for this development release.
It cannot authenticate before you finish Google's consent screen.

1. Create/select a Google Cloud project, enable Gmail API, and optionally enable
   Google Drive API if you intend to use Drive attachments.
2. Configure the Google Auth Platform consent screen. Choose the appropriate
   audience (Internal only for an eligible Workspace organization; otherwise
   External). In Testing mode, add each intended Gmail address as a test user.
3. Create an OAuth client with application type **Desktop app** and download its
   JSON. Import it in Luma Settings → Accounts. Alternatively place it at
   `$XDG_CONFIG_HOME/luma-mail/oauth-client.json`, using `~/.config` when unset,
   or set `LUMA_OAUTH_CLIENT` to a JSON path.
   Clicking Add Google account with no client opens a guided setup window with
   an import button; selecting a valid JSON continues into browser sign-in.
4. Add an account in Luma, choose the correct account in the default browser,
   and grant Gmail access. The callback listens only on `127.0.0.1` at an
   ephemeral port, validates a random state token and uses S256 PKCE.
5. Repeat for each account. Reconnect an expired/revoked account from Settings.
   Reconnecting verifies that the Google email address matches that account.

If Add only reopened Settings in v0.1.0-dev.1, the OAuth client was missing. That
build hid the useful error behind another Settings dialog. The guided flow in
0.1.1 explains this prerequisite. The JSON identifies your Google Cloud app; it
is not an exported Gmail account, a password file, or a service-account key.

If the default browser cannot be launched, use Open browser again or Copy sign-in
link in the sign-in window. Finish the same request within five minutes. Close
the window to cancel or start a fresh attempt after timeout.

The desktop client ID and client secret identify a public installed app. They
are not treated as a confidential user secret by Google. They are stored in the
imported configuration JSON; access tokens stay in process memory and refresh
tokens are kept in Secret Service. There is no plaintext-token fallback.

## Scopes

`https://www.googleapis.com/auth/gmail.modify` supports reading, labels, drafts,
sending, archive and Trash. Permanent message deletion is intentionally absent;
Luma never requests the broader `https://mail.google.com/` scope.

Drive is opt-in through Settings → Account → Enable Drive permission.
Reauthorization requests Gmail plus `https://www.googleapis.com/auth/drive.file`
together because installed apps do not support incremental authorization.
Only app-created or app-authorized files can be accessed. Pasting an arbitrary
Drive file ID does not bypass that restriction. A Google Picker is not yet
implemented. Upload sharing is separately confirmed, recipient-specific and
read-only; Luma never creates an `anyone` permission.

Google may expire refresh tokens for an External application in Testing mode
(commonly after seven days for these scopes), and Workspace admins may restrict
access. Handle this by reconnecting, not by saving passwords.

## Public distribution

Gmail modify is a restricted scope. Publishing a widely distributed client
requires Google's OAuth verification, an accurate consent screen, privacy
policy, and compliance with the Google API Services User Data Policy. A security
assessment may be required depending on how restricted data is handled. Local
testing success does not imply approval for public distribution. The developer
must resolve these requirements before publishing an official OAuth client.

Authoritative references checked during implementation:

- [Desktop OAuth and PKCE](https://developers.google.com/identity/protocols/oauth2/native-app)
- [Gmail scopes and verification](https://developers.google.com/workspace/gmail/api/auth/scopes)
- [Desktop loopback support](https://developers.google.com/identity/protocols/oauth2/resources/loopback-migration)
- [Drive uploads](https://developers.google.com/workspace/drive/api/guides/manage-uploads)
