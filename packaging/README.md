# Native Arch / CachyOS package

Run `makepkg -si` beside this PKGBUILD. The recipe downloads an immutable
GitHub source revision, verifies its SHA-256, and builds/tests from the lockfile.
It installs no daemon or autostart entry. The current pin includes button-only
Google sign-in in 0.1.2; advance `_commit`, `_srcdir` and `sha256sums` together after
reviewing later revisions. The AppImage release is built from its tagged commit.

The package builder must supply Luma's `LUMA_GOOGLE_CLIENT_ID` and
`LUMA_GOOGLE_CLIENT_SECRET` build-time values from its Desktop app registration.
The required-client feature prevents packaging an unusable sign-in build. This
is a maintainer/packager step; people installing a configured binary only click
Sign in with Google. See [app registration](../docs/oauth.md).

For AUR publication, inspect dependencies and generate `.SRCINFO` with
`makepkg --printsrcinfo`. This recipe has not been tested with makepkg or
published to the AUR.
Google OAuth verification and a public privacy policy are separate release tasks.
