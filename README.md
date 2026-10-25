# Luma Mail

A native Rust / GTK4 / libadwaita Gmail client built around **online-first access**.
The mailbox stays on Google. Luma loads a page of conversation metadata, then
fetches individual message bodies only when you expand them. It never builds a
local mail mirror, search index, or attachment archive.

**Development release, not production-ready.** See [STATUS.md](STATUS.md) for
implemented features, limitations and the verification boundary. No Google
account has been connected during development in this workspace.

The [full release checklist](RELEASE-CHECKLIST.md) maps all 20 original acceptance
criteria to implementation evidence and the remaining code/desktop checks.

Repository: [SleepyZzed/luma-mail](https://github.com/SleepyZzed/luma-mail).
[Download the x86_64 development AppImage](https://github.com/SleepyZzed/luma-mail/releases/tag/v0.1.1-dev.1)
and its SHA-256 checksum. Linux CI builds and launch-checks the bundle before
publishing it. See [AppImage packaging](docs/appimage.md).

Main now uses the maintainer's bundled Google app registration with **Sign in
with Google**. The older 0.1.1 binary still has the removed JSON import flow.
The next AppImage is gated on provisioning Luma's Google registration; an
unconfigured developer build cannot authenticate and is not published as a
sign-in-ready release.

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
Periodic polling defaults to 120 seconds and is configurable. Gmail push is not
claimed. No always-running daemon or autostart entry is installed.

## Daily-use controls

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

## Build on Arch Linux / CachyOS

```sh
sudo pacman -S --needed base-devel rust gtk4 libadwaita pkgconf gnome-keyring
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

## Connect accounts

1. Install a sign-in-enabled Luma build.
2. Click **Sign in with Google** and finish consent in your default browser.
   Luma supplies its own app registration; no JSON import or Google Cloud setup
   is needed by the person connecting an account.
3. Repeat for
   another account. A running, unlocked Secret Service keyring is required.
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
