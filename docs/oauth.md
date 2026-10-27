# Google sign-in and app registration

## Connecting an account

Click **Sign in with Google** in Luma and finish Google's consent in your normal
browser. Repeat for additional accounts. There is no JSON import, manual client
configuration or Google Cloud project creation in the installed application's
account flow. Passwords stay with Google. Access tokens stay in process memory;
refresh tokens are kept in Linux Secret Service or Windows Credential Manager.

The first connection offers Gmail plus optional Contacts, limited Drive and
basic Gmail settings together. Choose the features before opening the browser;
Google can also let you decline optional scopes. Luma checks the returned grants:
Gmail is required, and missing optional grants disable only their own features.
Settings can connect skipped features later without requiring separate initial
sign-ins for every feature. Existing accounts retain their previous grants.

If the browser cannot launch, use **Open browser again** or **Copy sign-in link**.
Complete the same request within five minutes. Closing the window cancels the
callback listener. The callback uses an ephemeral `127.0.0.1` port, a random
state token and S256 PKCE. No application server is involved.

## One-time setup for the app maintainer

Luma's current registration and English demonstration were submitted for branding
and data-access review on 26 October 2026. Google Auth Platform reports both are
under review. The homepage ownership appeal and client-only data-flow explanation
are included. This is not approval, an assessment waiver or a promise of cost;
Google's unverified warning and lifetime user cap remain until Google approves.

The maintainer must create Luma's app registration before distributing a working
build. This is not a task for each user. The repository does not invent or borrow
another application's client ID, and an unconfigured developer build reports
that sign-in is unavailable. Published releases are blocked without registration.

1. Create/select the Luma Google Cloud project and enable Gmail API. Enable Drive
   API if offering optional Drive attachments.
2. Configure Google Auth Platform branding, support/developer contacts and an
   External audience for consumer Gmail accounts. Supply public homepage,
   privacy-policy and terms URLs and their authorized domain. Testing requires
   named test users; publishing the app removes that list while unverified
   warnings and the lifetime user cap remain. Complete applicable verification
   for normal public distribution.
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

Optional sender blocking requests `gmail.settings.basic` alongside the account's
existing Gmail and optional Drive scopes. This creates Gmail filters sending
future mail from a chosen address to Trash; it does not use Gmail's private native
blocked-address mechanism. The app retains the filter ID for ownership-checked
Unblock. Google normalizes away redundant INBOX removal on a TRASH filter.

`https://www.googleapis.com/auth/gmail.modify` supports reading, labels, drafts,
sending, archive and Trash. Permanent deletion is absent, and Luma does not
request the broader `https://mail.google.com/` scope.

Drive is opt-in through Settings → Account → Enable Drive permission.
Reauthorization requests Gmail plus `https://www.googleapis.com/auth/drive.file`
together to preserve the account's required permissions during reconnection.
Previously granted scopes are included in Google's combined authorization.
Only app-created or app-authorized files can be accessed. A Google Picker is not
yet implemented. Upload sharing is separately confirmed, recipient-specific and
read-only; Luma never creates an `anyone` permission.

Luma's registration is **External / In production / unverified**, confirmed in
Google Auth Platform on 25 October 2026. Any eligible Google account can request
consent without being entered in a test list, within Google's **100 lifetime
user cap** for unapproved scopes. The console showed 1 / 100 at publication.
Removing users does not reset that cap. Google still shows its unverified-app
warning and may display the application domain instead of verified branding.
Account policies, Advanced Protection and Workspace administrators can block
unverified access. Luma cannot override those controls.

The existing configured 0.1.2 AppImage uses this registration. No JSON import,
personal Cloud project or replacement binary is needed for this audience
change. Complete consent in your browser; it is not a simulated login. The
seven-day Testing-mode expiry no longer applies to newly issued production
grants. Old grants can still require reconnecting, and production tokens can
expire or be revoked for Google's other documented reasons.

Gmail modify is a restricted scope. Broad distribution requires applicable
Google OAuth verification, an accurate consent screen and privacy policy, and
compliance with the Google API Services User Data Policy. A security assessment
may be required depending on how restricted data is handled. Local test success
does not establish Google approval.

## Optional saved-contact photos

Account settings offers saved-contact profile photos through the People API and
`https://www.googleapis.com/auth/contacts.readonly`. It is requested only when
enabled, alongside existing Gmail/Drive/filter permissions. Search projects only
email addresses and photo URLs, matches exact sender addresses, and runs for visible
avatars. It does not sync or store an address book. Google-hosted photo downloads
carry no Gmail/People bearer token, cookie or referrer. Other senders use initials.
When profile photos and Drive are both enabled, the existing Drive grant can
also read the connected account's own projected email/photo fields. It does not
look up other senders or add another scope.
The maintainer must enable People API and include the implemented optional scope
and demonstration in verification. See storage.md for memory/rate limits.

## Public sign-in and costs

The test-user list has been removed by publishing the Google registration for
unverified early access. This is separate from verification and does not remove
Google's warning or cap. The full-release goal remains verified, uncapped
consent, backed by domain ownership, scope justification and a demonstration of
real consent and the mail features that need each scope. Public project pages
are hosted at [sleepyzzed.github.io/luma-mail](https://sleepyzzed.github.io/luma-mail/),
with linked privacy and terms information also saved in this repository.

Luma connects directly from the user's desktop to Google and has no mail-handling
application server. Google's restricted-scope guidance ties annual independent
assessment to access through third-party servers. The expectation for this local
architecture must be confirmed with Google's review team before promising that
assessment is unnecessary. Google itself does not charge assessment fees, but
independent assessors may charge; total production costs are not guaranteed.

Standard Gmail API use is currently free within its standard quotas. Google has
announced planned charges for usage above daily thresholds, with notice before
they take effect. Review the current terms/quotas before a large public rollout.
Uncapped sign-in, assessment applicability and any actual hosting/API costs
remain full-release gates. No payment or paid hosting is needed for the current
GitHub Pages site; any future billable service requires a separate decision.

## IMAP, SMTP and JMAP assessment

The original brief requires Gmail first, server-side Gmail semantics, native
thread IDs, Drive attachments, bounded on-demand access and browser OAuth.
Changing the mail protocol does not remove Google's authorization rules.

| Option | Sign-in consequence | Decision against the original criteria |
|---|---|---|
| Gmail API + Luma's Desktop OAuth client | Browser consent; unverified production removes the test list, subject to warning/cap | Keep this path: existing Gmail labels, search, threading, drafts and on-demand data access remain intact. |
| Gmail IMAP + SMTP with OAuth | Same Google app registration/verification; requires the broader `https://mail.google.com/` scope | No sign-in shortcut. Migration would add protocol/state work and require revalidating Gmail semantics. |
| Gmail IMAP + SMTP with an app password | Requires 2-Step Verification and manual per-user app-password creation; unavailable for some protected/managed accounts | Does not meet the requested button-only Google consent flow. Not added as a replacement. |
| JMAP | Requires a JMAP server or a Gmail bridge; Google's documented Gmail interfaces provide no native JMAP endpoint | Suitable for a future provider. A bridge still needs Gmail authentication and may introduce a server handling restricted data. Not a fix for Gmail consent. |

Provider abstraction remains available for future complete IMAP/JMAP providers.
No partial provider or mailbox mirror was introduced for this early-access
change. The 20 original acceptance criteria remain tracked independently in
[RELEASE-CHECKLIST.md](../RELEASE-CHECKLIST.md).

References:

- [Desktop OAuth and PKCE](https://developers.google.com/identity/protocols/oauth2/native-app)
- [Gmail scopes](https://developers.google.com/workspace/gmail/api/auth/scopes)
- [Restricted scope verification](https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification)
- [Testing and production audiences](https://support.google.com/cloud/answer/15549945)
- [Published unverified app state](https://developers.google.com/identity/protocols/oauth2/production-readiness/overview)
- [IMAP/SMTP OAuth and scope](https://developers.google.com/workspace/gmail/imap/xoauth2-protocol)
- [Google app-password requirements](https://support.google.com/mail/answer/185833)
- [JMAP protocol and implementations](https://jmap.io/software/)
- [Assessment fees](https://support.google.com/cloud/answer/13463817)
- [Gmail API quotas and pricing](https://developers.google.com/workspace/gmail/api/reference/quota)
