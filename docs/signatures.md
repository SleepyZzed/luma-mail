# Online signatures

Open **Settings → Accounts → an account → Rich signature**, or use the Edit
button on the composer's Signature row. Both desktop versions offer installed fonts,
8–72 pt sizing, bold/italic/underline/strikeout, superscript/subscript, text and
highlight colours, bullets/numbering, indentation, alignment, line spacing,
hyperlinks, tables, pictures and undo/redo. Microsoft fonts appear when installed;
Luma downloads no fonts. Recipients use their installed font or a fallback.

**Save to Gmail** updates the primary address's Gmail Send-as signature. This also
changes what Gmail web uses. The app asks for optional basic Gmail settings
permission if not already granted for sender filters; it sends only the signature
field. A fresh projected read refuses an update if the server signature changed
since opening. This is a preflight check, not an atomic compare-and-swap: Gmail
provides no conditional revision primitive here. No mail is sent by saving a
signature. Closing with edits asks before discarding them.

Gmail's API exposes one signature per Send-as address, not Gmail web's full named
signature library/defaults. This integration edits primary addresses and reads
available alias signatures; it does not require domain-wide service-account
access to edit aliases. A plain signature from an older Luma version is offered
when Gmail has none, and its legacy setting is cleared after a successful online
save. Arbitrary Outlook/Word RTF import and exact document layout parity are not
claimed. Both desktop versions provide the visual editor; Linux uses ephemeral WebKit
and Windows uses a short-lived private WebView2 surface. Editing history is
bounded to 64 entries / 1 MiB of text snapshots, with shared image previews.

## Pictures

Insert a **public HTTPS image URL** to fetch a bounded anonymous preview in RAM.
The server sees the request's IP address and URL, without Google bearer tokens,
cookies or referrers. Existing pictures load only when you choose **Load signature
pictures**. The editor itself cannot make network requests or access local files.

Choosing a local picture offers an explicit confirmation before creating a
**public, unlisted, read-only Drive copy**. Anyone with its link can fetch it;
Google/recipient mail systems may fetch it to render the signature. Optional
`drive.file` access is limited to files Luma creates or the user authorizes.
Luma removes image metadata by decoding/re-encoding an optimized PNG (at most
768×576 pixels and 2 MiB). It uploads in 256 KiB chunks, with progress and Cancel,
then grants link access only after Drive confirms the complete upload. The
original local file is not changed. Restricted Workspace sharing policies can
prevent this and produce an error instead of an unusable signature picture.

Removing a picture from the signature does not delete its Drive copy: older sent
mail may still use that URL. Cancel/failure can leave a private or already-shared
copy; status text explains which, and it can be managed in Google Drive. The app
never silently deletes cloud files. Public image URLs can stop working or be
blocked by a recipient's privacy settings; hosted images cannot guarantee that
all recipients display them automatically.

## Composition and storage

Luma reads the selected sender's signature when composing, not during background
polls. The Signature checkbox includes/excludes it for this message; Reload reads
the latest server version. Signature HTML is separate from editable authored text
and quoted history. Reopening a Luma draft preserves that draft's signature, and
repeated Save/Send does not append another copy. A signature alone does not count
as the authored body required by **Save on close**.

HTML is capped at 64 KiB and eight hosted pictures. Local picture sources cap at 24 MiB; anonymous HTTPS previews cap at 4 MiB;
previews share the existing two-job decoder semaphore and a 12 MiB batch limit.
There is no signature database, persistent editor/browser profile, image spool
or additional local mail archive. Signature settings stay in Gmail; explicitly
saved message drafts naturally include their signature as outgoing mail. The
normal disposable expanded-body cache can include it when that message is read.
Closing the editor releases its view. The editor shares the existing ephemeral
HTML engine; it does not run an idle browser page or perpetual layout timer.

`SignatureStore` is a provider-neutral trait over identity, HTML, editability and
remote load/save. Only the Gmail adapter and optional Drive hosting are currently
implemented. Future providers can implement their own online store/hosting
without changing the editor's HTML model or introducing a local mirror.

API references: [Send-as get](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.settings.sendAs/get),
[patch](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.settings.sendAs/patch),
[Send-as update restrictions](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users.settings.sendAs/update),
[Drive resumable uploads](https://developers.google.com/workspace/drive/api/guides/manage-uploads).
