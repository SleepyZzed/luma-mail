# Windows x64

Download the Windows ZIP and checksum from the release, extract the whole folder,
and run **luma-mail.exe**. No administrator access, Rust or MSYS2 installation is
needed. Windows 10/11 x64 and the Microsoft Edge WebView2 Evergreen Runtime are
required. [Microsoft's runtime download](https://developer.microsoft.com/microsoft-edge/webview2/)
is provided for machines without it. The binary is currently unsigned.

Linux remains primary. The UI, mail features, cache/draft policies, OAuth and
provider engine are shared. Windows retains its title bar with minimise, maximise
and close. OS-specific adapters use Explorer tray, Windows shell notifications,
Credential Manager, WebView2, and GTK's in-memory GStreamer playback backend.
The native title-bar Close obeys Close to tray; right-click the star to Open/Quit.
Windows notifications obey OS settings; the native API transport is not an
in-app imitation. Native notification click/delivery still needs desktop acceptance.

Initial Google sign-in offers Gmail and optional Contacts, limited Drive and
basic Gmail settings together. Uncheck optional features or decline them on the
Google consent screen; Gmail remains usable. Settings can reconnect skipped
features. Google verification status is independent of the Windows package.

Preferences live in the per-user luma-mail config directory; tokens use the OS
vault. The existing optional bounded mail cache is shared in design, with a zero
budget disabling retained mail, attachment, thumbnail and photo cache files.
No mailbox mirror, search index, local drafts or contact database is added.
WebView2 runs InPrivate with documents served from memory and scripts/network
navigation disabled. Browser, image-loader, codec and font metadata use a private
session directory in TEMP; these are not a mail archive. Interrupted processes
can leave disposable runtime metadata, which is reported separately from the
mail-cache budget. Explicit Save/download writes the user's selected destination.

## Reproduce the build

The Windows workflow installs UCRT64 Rust, GTK4/libadwaita, selected GStreamer
codecs and librsvg through MSYS2. Use the same environment, then run:

```powershell
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --release -p luma-mail --features require-google-client
python packaging/webview2-loader.py --binary target/release/luma-mail.exe
python packaging/windows.py --prefix C:/msys64/ucrt64 --binary target/release/luma-mail.exe --output dist/Luma-Mail-0.9.0-windows-x86_64 --version 0.9.0
```

The required app registration is supplied by release CI secrets. No user's OAuth
JSON or password is included. Native debug acceptance uses test-windows.ps1 with
synthetic, isolated data; the relocated package is launched with MSYS2 removed
from PATH, including HTML and memory-only media checks. Both platform gates must
pass before publishing. The package includes replaceable runtime DLLs, licence
notices and an exact version/source inventory. The WebView2 SDK loader is hash
pinned to the official Microsoft package; the shared Evergreen browser runtime
is not duplicated in the ZIP. Whole-package size and cache/runtime allocation
are measured separately; the development toolchain is never distributed.
