# Google review corrections — 28 October 2026

Google requested DNS-verifiable website ownership and a new comprehensive live
demonstration. This record describes corrections, not an approval or 1.0 release.

## Website and ownership

- Registered `luma-mail.duckdns.org` in the maintainer's account with approval.
- Pointed its IPv4 record at GitHub Pages and published `docs/CNAME`.
- Google Search Console successfully verified the **Domain property** through
  the provider's TXT record. Keep that record; a URL-prefix HTML verification
  alone does not meet this review request.
- Valid HTTPS serves the homepage, `/privacy.html`, `/permissions.html` and
  `/terms.html` without a certificate exception.
- Saved those URLs and `luma-mail.duckdns.org` in the existing Cloud project's
  branding. Cloud confirmed “Branding changes saved.”
- The website accurately describes the Qt/Rust Linux/Windows early-access app,
  bounded optional caching, the four scopes, optional-feature controls,
  revocation versus local disablement, and explicit signature-picture sharing.

DuckDNS credentials, OAuth tokens and private account information do not belong
in this repository. The DNS ownership proof is public and must remain at the
provider. The temporary encrypted DNS credential handoff was removed after use.

## Scope audit

Cloud data access matches `crates/core/src/auth.rs` exactly:

| Scope | Working use to demonstrate | Why narrower access is insufficient |
| --- | --- | --- |
| `gmail.modify` | Read/search, send/reply/forward, online drafts, labels, read/star/importance, archive and recoverable Trash | Read-only, metadata-only or send-only cannot provide combined mailbox operations; permanent deletion is not used. |
| `gmail.settings.basic` | Luma-owned server-side blocking filters and primary-address online signature | Mail modification cannot edit filters or sending signatures. No settings-sharing scope is requested. |
| `contacts.readonly` | Exact-email matching of a saved contact to a visible sender's photo | Gmail does not supply contact photos. No address-book write or mirror is needed. |
| `drive.file` | User-selected large uploads and explicitly confirmed app-created signature pictures | Restricted to files used with Luma; arbitrary Drive discovery/full-Drive access is unnecessary. |

The restricted-scope justification was corrected and saved to include signature
editing, confirmed recipient sharing, and explicitly confirmed public signature
picture copies. No scopes, OAuth clients or production-audience setting changed.

## New video acceptance checklist

The previous edited still-screen walkthrough is insufficient. Its old YouTube
URL must be replaced only once a real live recording passes these checks:

- [ ] Real current packaged application, with a separately isolated profile and
  zero mail-body cache; synthetic disposable messages/files only.
- [ ] Actual browser authorization, app identity/client, unverified-app warning,
  all requested services expanded and readable, and successful return to Luma.
- [ ] Gmail read/search and compose/save/reopen/send/reply/forward actions,
  including online Drafts and Sent/source-account evidence.
- [ ] Read/star/importance, label, archive, recoverable Trash and restore, with
  corresponding state visible in Gmail.
- [ ] Actual app Block/Unblock plus the corresponding Gmail filter and removal.
- [ ] Actual online signature edit plus its corresponding Gmail setting;
  restore the test signature afterwards.
- [ ] Real saved-contact fixture in Google Contacts and its matching photo in
  Luma. An own-account photo alone does not prove `contacts.readonly` use.
- [ ] Large synthetic attachment upload/progress, filename hyperlink, confirmed
  recipient sharing and the app-created Drive file in the source account.
- [ ] Signature picture confirmation/sharing, if used in the current app;
  publish only a non-personal synthetic or Luma-branded image.
- [ ] Explain optional Contacts/Drive controls, Luma-owned filter removal,
  per-message signature controls, account removal and Google-grant revocation.
  Local disablement does not revoke an already-issued Google grant or undo
  existing cloud files, sharing, signatures or filters.
- [ ] State no hosted mail backend, direct desktop-to-Google data flow, OS vault,
  optional bounded cache and no mailbox/contact mirror or AI training.
- [ ] Watch the encoded video; redact private avatars, unrelated mail, other
  account details and credentials without obscuring scopes or feature evidence.
- [ ] Upload unlisted, replace the Cloud demo URL, and reply to the original
  Google review email with the verified URLs, DNS proof method and new video.

Cuts for network waits and privacy redactions are acceptable if disclosed. Do
not substitute slides, mocks, fixture-only protocol output or screenshots for
live evidence of the requested user-facing features and source-account changes.

Until these checks and the direct email reply are complete, the review correction
is incomplete. Google approval, assessment applicability and the separate 1.0
acceptance gates remain unconfirmed.
