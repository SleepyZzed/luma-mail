# Native Arch / CachyOS package

Run `makepkg -si` beside this PKGBUILD. The recipe downloads an immutable
GitHub source revision, verifies its SHA-256, and builds/tests from the lockfile.
It installs no daemon or autostart entry. The initial pin is the first published
0.1.1 implementation; advance `_commit`, `_srcdir` and `sha256sums` together after
reviewing later revisions. The AppImage release is built from its tagged commit.

For AUR publication, inspect dependencies and generate `.SRCINFO` with
`makepkg --printsrcinfo`. This recipe has not been tested with makepkg or
published to the AUR.
Google OAuth verification and a public privacy policy are separate release tasks.
