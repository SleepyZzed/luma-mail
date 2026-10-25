# Google sign-in and app registration

## Connecting an account

Click **Sign in with Google** in Luma and finish Google's consent in your normal
browser. Repeat for additional accounts. There is no JSON import, manual client
configuration or Google Cloud project creation in the installed application's
account flow. Passwords stay with Google. Access tokens stay in process memory;
refresh tokens are kept in the Linux Secret Service keyring.

If the browser cannot launch, use **Open browser again** or **Copy sign-in link**.
Complete the same request within five minutes. Closing the window cancels the
callback listener. The callback uses an ephemeral `127.0.0.1` port, a random
state token and S256 PKCE. No application server is involved.

## One-time setup for the app maintainer

The maintainer must create Luma's app registration before distributing a working
build. This is not a task for each user. The repository does not invent or borrow
another application's client ID, and an unconfigured developer build reports
that sign-in is unavailable. Published releases are blocked without registration.

1. Create/select the Luma Google Cloud project and enable Gmail API. Enable Drive
   API if offering optional Drive attachments.
2. Configure Google Auth Platform branding, support/developer contacts and an
   External audience for consumer Gmail accounts. During private testing, add
   the intended account addresses as test users. Follow Google's requirements
   for production publication and applicable verification before broad release.
3. Create a client with application type **Desktop app**. A Web application or
   service-account client is unsuitable. Use its client ID and client secret as
   the build-time values `LUMA_GOOGLE_CLIENT_ID` and `LUMA_GOOGLE_CLIENT_SECRET`.
   Do not use a user's password, token or confidential server credential.
4. For GitHub releases, set both names as repository Actions secrets. The Linux
   build job supplies them to compilation, validates the pair in tests and uses
   the `require-google-client` feature to block publication if either is missing.
   Fork pull requests have no access to repository secrets and can still compile
   and test the deliberately unconfigured developer build.
5. Run the Linux workflow with publication enabled only after live consent and
   keyring tests. Every person installing that build uses the same app identity
   and separately authorizes their own account.

For a local distributable build, supply the two values in your build environment:

```sh
cargo test --locked --workspace
cargo build --locked --release -p luma-mail --features require-google-client
bash packaging/appimage.sh --require-google-client
```

`option_env!` embeds the registration at compile time; runtime environment
variables cannot replace it. Cargo tracks changes to these build-time values.
Installed desktop clients are public OAuth clients: the embedded ID and client
secret can be extracted from the binary and must not be treated as confidential.
Repository secrets reduce accidental disclosure in build logs; they do not make
the installed app's registration secret. User refresh tokens are never bundled.

## Migrating from earlier previews

The old `oauth-client.json` import and `LUMA_OAUTH_CLIENT` override have been
removed. Existing files are ignored and are not uploaded or erased. Refresh
tokens issued to a previous client cannot be reused with Luma's new client;
reconnect the same account from Settings to authorize the new registration.
New keyring records also identify the client that obtained consent. An unknown
or different client requires reconnecting before any refresh request is sent.

## Scopes and Google consent

`https://www.googleapis.com/auth/gmail.modify` supports reading, labels, drafts,
sending, archive and Trash. Permanent deletion is absent, and Luma does not
request the broader `https://mail.google.com/` scope.

Drive is opt-in through Settings → Account → Enable Drive permission.
Reauthorization requests Gmail plus `https://www.googleapis.com/auth/drive.file`
together because installed apps do not support incremental authorization.
Only app-created or app-authorized files can be accessed. A Google Picker is not
yet implemented. Upload sharing is separately confirmed, recipient-specific and
read-only; Luma never creates an `anyone` permission.

An External app in Testing mode is restricted to listed test users and these
Gmail grants/refresh tokens expire after seven days. Workspace administrators
may restrict access. Reconnect to renew an expired or revoked grant.

Gmail modify is a restricted scope. Broad distribution requires applicable
Google OAuth verification, an accurate consent screen and privacy policy, and
compliance with the Google API Services User Data Policy. A security assessment
may be required depending on how restricted data is handled. Local test success
does not establish Google approval.

## Public sign-in and costs

The test-user list is temporary. The full-release goal is Google's approved
production consent flow without adding each person manually. Completing testing
or embedding a client ID alone does not establish that approval. Preparation
includes a homepage/privacy policy on a verified domain, scope justification,
and a demonstration of real consent and the mail features that need each scope.

Luma connects directly from the user's desktop to Google and has no mail-handling
application server. Google's restricted-scope guidance ties annual independent
assessment to access through third-party servers. The expectation for this local
architecture must be confirmed with Google's review team before promising that
assessment is unnecessary. Google itself does not charge assessment fees, but
independent assessors may charge; total production costs are not guaranteed.

Standard Gmail API use is currently free within its standard quotas. Google has
announced planned charges for usage above daily thresholds, with notice before
they take effect. Review the current terms/quotas before a large public rollout.
Public sign-in, assessment applicability and any actual hosting/API costs remain
explicit release gates rather than reasons to keep a permanent test-user list.

References:

- [Desktop OAuth and PKCE](https://developers.google.com/identity/protocols/oauth2/native-app)
- [Gmail scopes](https://developers.google.com/workspace/gmail/api/auth/scopes)
- [Restricted scope verification](https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification)
- [Testing and production audiences](https://support.google.com/cloud/answer/15549945)
- [Assessment fees](https://support.google.com/cloud/answer/13463817)
- [Gmail API quotas and pricing](https://developers.google.com/workspace/gmail/api/reference/quota)
