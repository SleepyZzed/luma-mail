# Fluid interface preview packages

This branch replaces GTK with a custom Qt Quick interface and the existing
online-first Rust engine. Preview packages are development checkpoints, not a
1.0 or completed live-Gmail/Hyprland/resource acceptance claim. Linux remains the
priority; Windows retains its native title bar. All cache limits stay unchanged.

## Windows

Extract the whole ZIP to a writable application folder and launch
`bin/luma-mail.exe`. Keep `bin/luma-engine.exe`, the shared Qt runtime, QML modules,
plugins, `luma_webview.dll` and the pinned WebView2 loader together. Windows 10/11
x64 and the installed Microsoft Edge WebView2 Evergreen runtime are required.
This package does not install a development toolchain or a browser runtime.

The package includes replaceable shared libraries, notices, exact source-package
links and per-file hashes in `THIRD-PARTY.json`. Luma uses installed system fonts;
no Microsoft fonts are redistributed. Source code is available on the matching
GitHub commit and in the separately supplied source archive.

## Linux

The x86_64 AppImage is built against Ubuntu 24.04 and includes its Qt Quick,
Wayland/X11, WebEngine helper/resources and media dependencies. Make it executable
and run it normally. The launcher retains system cursor paths and selects native
Wayland when available; the compositor owns window edges and positioning. Native
notifications use the desktop D-Bus notification daemon. Keep Chromium's sandbox
enabled. Its kernel/desktop requirements must be verified on the target desktop.

Build with `packaging/fluid-appimage.sh --binary <Qt-host> --engine <registered-engine>
--qt-notices <matching-notices-folder> --version <preview-version>`. Generate the
notices folder with `packaging/qt-source-notices.py --version 6.11.2 --output <folder>`.
That build-time operation streams matching source archives from Qt, retains only
licence/attribution files and records full source URLs and hashes. Source archives
are not installed. The build also retains distro runtime notices and the Rust
dependency/standard-library notices and exact source references.

Both CI packages use a registered release engine and run the native synthetic
scene suite after developer DLL/QML/Qt search paths are removed. Reports and
checksums accompany the artifacts. These packages remain previews until the live
Gmail/Hyprland/resource gates pass.

## Storage and privacy

Application binaries and shared frameworks are fixed installation bytes, separate
from email caching. No account data, emails, local drafts, attachment archive or
photo database is bundled. Drafts and signatures remain online. Runtime newsletter
profiles are private/ephemeral; attachment previews and edits are bounded in RAM.
Existing cache settings remain authoritative, including zero cache, allocated-byte
accounting, TTL/LRU, per-account clear and exit cleanup. User-selected downloads
are explicit exports and remain the user's files.

Full-process RAM, installed/runtime allocation, two-account Gmail and real
Hyprland/notification/Noctalia checks remain release gates. See
[validation](fluid-validation.md), [storage](storage.md), and
[migration](ui-migration.md). Never interpret a portable preview as Google
verification approval or a promise that private Gmail profile photos are exposed.

## Build

Build Qt Quick with Qt 6.8+ and the existing CMake target, build `luma-engine` and
(on Windows) `luma-webview` with Cargo, then use `packaging/fluid-windows.py`.
Production engines require the maintainer's build-time Google registration and
`--features require-google-client`; end users never import an OAuth JSON file.
The packager requires the MSYS2 ownership database and `windeployqt`, stages the
pinned WebView2 loader, resolves native dependency closure and rejects GTK runtime
files. Package/runtime size reports are separate from the email-cache budget.

Qt deployment follows its [official deployment API](https://doc.qt.io/qt-6.8/qt-generate-deploy-qml-app-script.html)
and [WebEngine deployment requirements](https://doc.qt.io/qt-6.8/qtwebengine-deploying.html).
