# Linux AppImage and release workflow

The public repository is https://github.com/SleepyZzed/luma-mail. The Linux
workflow checks formatting, Clippy, mocked tests, a release build and a native
GTK launch, then creates and launch-checks an x86_64 AppImage. Successful builds
are saved as GitHub Actions artifacts. An explicit manual workflow run with
`publish=true` publishes a development prerelease after those checks succeed.

## Use a published bundle

Download the AppImage and `SHA256SUMS` from the repository's Releases page:

```sh
sha256sum --check SHA256SUMS
chmod +x Luma-Mail-0.1.0-x86_64.AppImage
./Luma-Mail-0.1.0-x86_64.AppImage
```

If your desktop lacks FUSE support, use:

```sh
APPIMAGE_EXTRACT_AND_RUN=1 ./Luma-Mail-0.1.0-x86_64.AppImage
```

The bundle includes GTK/libadwaita dependencies and application resources. It
still uses your user-session D-Bus, existing notification daemon, Secret Service
keyring and StatusNotifier tray host. Enable Noctalia's tray widget. It does not
bundle a second notification daemon or simulate received mail. Configure a Google
Desktop OAuth client before connecting accounts; no credentials are shipped.

It does not force X11 or override libadwaita's theme choice. Native Wayland and
wallpaper palette integration still require verification on your real desktop.
Minimum libc compatibility follows the Ubuntu 24.04 build base; Arch/CachyOS
normally provide a newer libc. This is not an all-distributions compatibility
claim. Arm64 bundles are not yet provided.

## Build locally on x86_64 Linux

Install the native build dependencies from README. On Ubuntu 24.04, install the
packages named in `.github/workflows/linux.yml`, including GTK development files,
GObject introspection, librsvg, patchelf and desktop-file-utils. Then run:

```sh
bash packaging/appimage.sh
```

Use a fresh `target/appimage/Luma.AppDir`. The script refuses to overlay an old
AppDir. Downloaded linuxdeploy is checksum-pinned; the GTK plugin is pinned to an
immutable upstream commit. If the upstream continuous binary changes, review
the new official release and update the checksum deliberately. The packaging
step removes the plugin's old forced-X11 / forced-GTK-theme hook lines so GTK4
and libadwaita retain their native backend and appearance behaviour.

No build workflow embeds the OAuth client JSON, refresh tokens, account settings,
private mail, runtime cache, or local development toolchains. GitHub's standard
release source archives accompany the binary. See STATUS.md for remaining gaps.
