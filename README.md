# Luma Mail

<img src="data/luma-wordmark.png" alt="Luma Mail" width="320">

**[Download the 0.11 fluid interface preview](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.11.0-dev.2):** the replacement uses a custom Qt Quick scene with
shared-element spring transitions and the existing Rust engine. GTK-free Linux
and Windows packages pass 309/312 native portable checks. See the
[feature-parity audit](docs/fluid-parity.md), [interface wiring](docs/ui-migration.md), [build/packaging instructions](docs/fluid-packaging.md)
and [validation record](docs/fluid-validation.md). The 0.9 download below remains
the released reference while final parity and desktop acceptance continue.

The 0.9 reference uses Rust / GTK4 / libadwaita; both interfaces keep **online-first access**.
The mailbox stays on Google. Luma loads a page of conversation metadata, then
fetches individual message bodies only when you expand them. It never builds a
local mail mirror, search index, or attachment archive.

**Early access, not production-ready.** See [STATUS.md](STATUS.md) for
implemented features, limitations and the verification boundary. Real browser
consent, Windows keyring, synthetic self-mail operations and Gmail sender-filter
creation/removal have been tested with the maintainer's authorized account.
The 0.11 Windows engine passes 30 disposable two-account Gmail checks; target Linux
desktop acceptance remains outstanding.

The 0.11 preview adds direct Send, whole-conversation forwarding, inline-picture
authoring, native notification actions, opt-in image autoload, smoother opening
morphs and Wayland Vulkan startup selection. See the [current 1.0 audit](docs/v1-acceptance-0.11.md)
and [live two-account report](docs/validation/fluid-live-two-0.11.0-dev.2.json).

Preview 2 keeps HTML startup/resize on the active theme and expands its loading
pill into the measured content. Composer cards animate around a stable editor
layout; parked window-layout motion no longer skips the opening morph. Image
fetches can overlap without adding parallel full-resolution decoders or disk
storage. See [reader presentation and limits](docs/reader-loading-0.11.md).

The [full release checklist](RELEASE-CHECKLIST.md) maps all 20 original acceptance
criteria to implementation evidence and the remaining code/desktop checks.

Repository: [SleepyZzed/luma-mail](https://github.com/SleepyZzed/luma-mail).
[Project homepage](https://lumamail.app/) ·
[Privacy policy](https://lumamail.app/privacy.html) ·
[Permissions](https://lumamail.app/permissions.html) ·
[Terms](https://lumamail.app/terms.html).
[Download the 0.11 early-access Linux AppImage or Windows x64 ZIP](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.11.0-dev.2)
and its SHA-256 checksum. Release publication requires both platform build gates.
Linux remains primary; both versions share the mail engine and features, with
a Windows title bar and platform-specific rendering/tray/keyring backends. See
[AppImage packaging](docs/appimage.md) and [Windows usage](docs/windows.md).

The 0.2.0 mail experience and validation are described in
[PR #1](https://github.com/SleepyZzed/luma-mail/pull/1) and
[the verification record](docs/verification.md). Native and packaged HTML/media,
composer navigation, notification transport and system cursor checks passed.
The 0.2.1 packaging correction addresses HTML-message crashes on systems without
the CI host's WebKit installation; see [PR #2](https://github.com/SleepyZzed/luma-mail/pull/2).

Release builds use the maintainer's bundled Google app registration with
**Sign in with Google**. Initial sign-in offers Gmail plus optional Contacts,
limited Drive access and Gmail settings in one consent flow. Declined optional
features stay disabled and can be connected later in Settings. The older 0.1.1 binary still has the removed JSON import flow.
Luma's Desktop registration is provisioned and supplied through the release
build. On 25 October 2026, the Google app was published as **External / In
production / unverified**. There is **no per-address test list**. Google's
unverified-app warning and **100 lifetime user cap** remain until verification;
Workspace or Advanced Protection policies may block access. This is not
uncapped or Google-approved distribution.
Unconfigured developer builds cannot authenticate and are blocked from release
publication. See [sign-in limits and protocol options](docs/oauth.md).

The 0.7 visual and editing milestone adds app-owned icons, refined controls,
verified Send-as aliases and explicit draft conflict merging. See the
[visual system](docs/visual-system.md) and [roadmap](docs/roadmap.md).

## Signatures and draft saving

Settings → Accounts → Rich signature opens the Linux visual editor: fonts,
colours, lists, alignment, links, tables and pictures. Signatures are saved online
in Gmail and included separately from editable message text. A local picture
requires explicit approval for a public optimized Drive copy. See
[signature controls, limits and privacy](docs/signatures.md).

Settings → Storage → Drafts offers **Ask on close** (default), **Save on close**
only with a valid recipient and authored body, or **Manual Save only**. Typing and
attaching files do not create automatic drafts. Explicit Save and Send still work;
closing without saving keeps any previously saved Gmail version unchanged.

## Tray, notifications and Refresh now

On Linux, closing the main window hides Luma in the system tray when a live
StatusNotifier host is available. Right-click the tray icon for **Open Luma
Mail** or **Quit**. Quit exits completely, after giving open composers their
normal save/close handling. Disable Close to system tray in Settings if you
prefer the window's close button to exit. If the tray host disappears, Luma
reveals its window; it never stays hidden without a usable tray.

Notifications go through GIO to the desktop notification service and your
configured notification daemon (such as Noctalia's). They are real system
notifications. Gmail history polling continues while the window is hidden.
Delivery and Do Not Disturb are controlled by your desktop. Settings includes a
clearly labelled **system test notification** action for verification.

The refresh toolbar button and **Ctrl+R** fetch the visible mailbox and request
an immediate new-mail polling check. Overlapping polling requests are prevented.
Periodic polling defaults to 60 seconds and is configurable. Quiet history checks
insert new conversations and update unread replies in place, without a loading
bar, replacing loaded pages or reopening the reader. Gmail push is not
claimed. No always-running daemon or autostart entry is installed.

## Interface and fonts

The main window is frameless by default for tiling desktops such as Hyprland.
Luma and Compose stay fixed above the independently scrolling folders/accounts.
Refresh, Undo and Select sit beside the mailbox title. Settings and the app menu
stay at the bottom of the sidebar; the menu includes Close window and Quit.
Optional window decorations are under Settings → Appearance.

Interface size offers 75–200%, independently of compositor/monitor scaling.
Ctrl++ / Ctrl+- step through sizes; Ctrl+0 resets to 100%. Compact density updates
visible rows without fetching Gmail. Reduced motion disables mail/composer
transitions; desktop animation preferences are respected too.

The composer font picker lists installed system families, including Microsoft
fonts when installed. Select text to change its family/size, or choose a font for
further typing. Interface scaling changes its on-screen appearance without
changing the size sent in the email. Fonts are not downloaded, copied, embedded
or bundled. Recipients need the same family or their mail client uses a fallback.
Luma 0.3's own rich drafts reopen visually with supported fonts, lists and links;
complex, third-party and older HTML drafts retain source mode to avoid data loss.
Drafts stay in Gmail. See [the v1 roadmap](docs/roadmap.md).

Saved-contact profile photos are optional in Account settings and request read-only
Google Contacts permission. Luma looks up visible senders only and retains at most
128 small photos in RAM for ten minutes, with two jobs and twelve new lookups per
minute. It does not sync the address book or write a photo cache. Google does not
expose every photo Gmail can display; unmatched senders keep initials.

## Daily-use controls

**Unified Inbox** combines Inbox messages from enabled accounts. **All Mail**
combines their Inbox, sent and archived mail, excluding Spam and Trash. Click an
account in the sidebar for that account's Inbox and its own folder group beneath it.
Account folders keep that account selected; unified views are a separate group.
Right-click a user label to delete it in Gmail after confirmation; mail is kept.

Mail actions update confirmed rows in place, with Undo and arrival/removal
transitions. A normal click opens one conversation; **Ctrl+click** toggles selection
and **Shift+click** selects a range. **Select** exposes Select all loaded, Clear,
the selection count and bulk actions; Ctrl+A while the list has focus selects
loaded conversations and Escape leaves selection mode. Select all never fetches
the rest of the mailbox. Right-click a selected row to act on that selection.
Reply, forward, draft editing and sender blocking are available only for one
conversation; incompatible bulk actions are hidden and guarded in their handlers.
Refresh and Load more fetch only
bounded metadata pages, with a rolling quota budget and batched reads.

Compose opens inside the main window; replies open inline below their message.
Expand or restore a compact composer inside the app; transitions keep the same
draft without changing the main window’s desktop fullscreen state. Pop out is a
separate action and can return to the main window. Bold, italic, underline, strike,
bullets and Insert link are available, with real upload-byte progress and a
separate Gmail confirmation stage. **Ctrl+Enter** sends.
New messages start in To. Tab and Shift+Tab traverse visible text fields; links
are blue and Ctrl+click opens them. Recent-recipient completion learns only from
successful sends in Luma (100 entries, 90 days, 64 KiB maximum). Drafts save to
Gmail through explicit Save or the selected close policy. Conflicts keep your edits and offer a separate Gmail draft, preserving the
newer server version. Pasting a raster image attaches it in memory with a preview,
subject to the retained attachment limits; it does not create temporary image files.
New local draft copies are disabled; any older unsent recovery copies stay
available until saved to Gmail or discarded. Restore returns to an inline reply
or a compact composer window. Large Drive attachments insert one filename link.

Attachments sit in a separate strip below message headers and above the body.
Image thumbnails are bounded; image, DOCX/text, audio and video previews load on
demand in memory. Audio/video playback depends on available codecs. DOCX previews
show extracted text, not original pagination. Composer attachments use the same
cards. Original files are saved only to a location you choose.

Linux HTML mail uses a sandboxed WebKit renderer with sanitized layout, tables,
colours and clickable links. External images remain blocked until **Load images**;
that explicit request contacts the sender's image servers. Embedded images are
decoded and resized first. Sender scripts and local-file/network access are blocked.
Neutral HTML backgrounds and text follow the active system/Luma/Noctalia theme.
**Original colours** restores the sender’s palette for that message. Theme changes
reuse prepared images without another network request. Table columns keep words
together, and inline images fit within 480 pixels of height; attachment previews
remain available for a larger view. Simple phone messages use the normal themed text body. Windows development
builds use an InPrivate WebView2 child for formatted HTML; its runtime metadata
is separate from the body-cache budget (see docs/storage.md).

**Block sender…** creates a Gmail filter for the receiving account, sending future
mail from that address to Gmail Trash while Luma is closed. Existing mail is kept.
It requests optional Gmail settings/filter permission on first use. Manage rules
under **Settings → account → Blocked senders**. Unblock removes only the exact rule
Luma created; edited filters must be reviewed in Gmail. This is a Gmail filter,
not an entry in Gmail's separate native blocked-address list. **Report spam**
and **Not spam** operate on existing selected mail and are available separately.

**Search options & accounts** builds Gmail queries for sender, recipient,
subject, dates, unread/starred status and attachments, and lets you choose any
subset of enabled accounts. It searches All Mail remotely. **Ctrl+1…9** switches
to an account's Inbox in your configured order.

Background changes preserve a multi-page view, scroll position or bulk selection;
Refresh is highlighted when an update is waiting. Failed pages and message bodies
have retry buttons. Collapsing a message or changing conversations cancels body
work that is no longer needed. Label navigation survives unread-count updates.
Cross-account label actions match label names in each account and report a
missing label without applying an unrelated ID.

## Build the 0.9 reference on Arch Linux / CachyOS

For the GTK-free 0.10 frontend, use the [Qt Quick build instructions](docs/fluid-packaging.md).

```sh
sudo pacman -S --needed base-devel rust gtk4 libadwaita webkitgtk-6.0 gst-plugins-good gst-libav pkgconf gnome-keyring
cargo build --locked
cargo run --locked -p luma-mail
```

GTK >= 4.12 and libadwaita >= 1.5 are required. Cargo resolves maintained
gtk4 0.11 / libadwaita 0.9 bindings as one compatible family. Use the current
stable Rust toolchain (Rust 1.92 or newer, as required by the locked GTK bindings).

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --release
```

The core can be tested independently: `cargo test --locked -p luma-core`.
On a Linux display session, `cargo run -p luma-mail -- --smoke-test` opens the
native empty state and exits after three seconds without authenticating.
For a sign-in-enabled distributable, the maintainer supplies the two build-time
Google app registration values and builds with `--features require-google-client`;
see [app registration](docs/oauth.md). A plain unconfigured source build remains
useful for UI/core development and cannot connect accounts.

## Connect accounts

1. Install a sign-in-enabled Luma build.
2. Click **Sign in with Google** and finish consent in your default browser.
   Luma supplies its own app registration; no JSON import or Google Cloud setup
   is needed by the person connecting an account.
3. Repeat for another account. A running, unlocked Secret Service keyring is required.
4. Select a folder or account. Search supports Gmail operators. Enter submits
   the query to Google; Load more fetches the next page.

No test credentials, fabricated inbox data, telemetry, servers, or scheduled
services are included. No account login is simulated.

The sign-in window has **Open browser again** and **Copy sign-in link** for a
desktop browser-launch failure. Closing it cancels the pending callback listener.
The maintainer configures the app once before building, following
[Google app registration](docs/oauth.md). User configuration files and the old
`LUMA_OAUTH_CLIENT` path are no longer read. People who connected an account
using their own client in an older preview must reconnect to grant Luma's new
registration access; existing refresh tokens belong to the previous client.

## Project structure

```text
crates/core/src/
  auth.rs           PKCE, browser loopback flow, encrypted Secret Service session
  model.rs          Account-scoped Gmail message / thread / label models
  provider.rs       Future-provider boundary; Gmail is the only implementation
  gmail.rs          Remote mail, label, draft and attachment operations
  http.rs           TLS, timeouts, concurrency and safe read retries
  pagination.rs     Bounded transactional merge of provider pages
  composer.rs       MIME, reply headers, validation, safe text rendering
  drive.rs          Chunked resumable upload and recipient-only sharing
  notifications.rs  Incremental history cursor and bounded deduplication
  cache.rs          Expiring, allocation-aware, LRU body cache
  config.rs         XDG paths and settings; no OAuth tokens
  theme.rs          Validated Noctalia palette and contrast checks
crates/desktop/src/  GTK main thread, asynchronous controllers, composer, settings
data/               Desktop entry, SVG icon, CSS and Noctalia template
packaging/          Native Arch package recipe
docs/               Setup, privacy, testing and troubleshooting
```

Tokio handles network and file operations; GTK updates stay on its main context.
The UI receives completions through one-shot channels, without a busy loop.
Read requests have bounded retries. Mutating requests are never automatically
replayed. Source account IDs accompany every remote operation.

## Installation

After `cargo build --release`:

```sh
install -Dm755 target/release/luma-mail "$HOME/.local/bin/luma-mail"
install -Dm644 data/io.github.lumamail.Luma.desktop "$HOME/.local/share/applications/io.github.lumamail.Luma.desktop"
install -Dm644 data/io.github.lumamail.Luma.svg "$HOME/.local/share/icons/hicolor/scalable/apps/io.github.lumamail.Luma.svg"
update-desktop-database "$HOME/.local/share/applications"
xdg-mime default io.github.lumamail.Luma.desktop x-scheme-handler/mailto
```

Ensure `~/.local/bin` is on PATH. No systemd service is installed. Tray → Quit
exits completely. A package source archive and a
`makepkg` recipe are also supplied with this delivery; see `packaging/README.md`.

For wallpaper themes, follow [Noctalia setup](docs/noctalia.md). For storage
details and exclusions, read [storage and privacy](docs/storage.md).

## Uninstall

Remove the installed executable, desktop entry and icon, or uninstall the native
package with `sudo pacman -R luma-mail`. Disconnect accounts in Settings first
to revoke Google grants and delete their keyring entries. If that is impossible,
revoke access from your Google Account security page and remove Luma entries
using your keyring manager. Optional user-data cleanup is limited to
`${XDG_CONFIG_HOME:-$HOME/.config}/luma-mail` and
`${XDG_CACHE_HOME:-$HOME/.cache}/luma-mail`. Explicitly saved attachments remain
where you selected them. Remove the Noctalia user-template entry if enabled.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE). Google, Gmail, Drive, GTK and Noctalia
names identify integrations and do not imply endorsement.
