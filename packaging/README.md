# Native Arch / CachyOS package

The adjacent delivery includes `luma-mail-0.1.0.tar.gz`. Copy that source archive
beside this PKGBUILD, then run `makepkg -si`. The recipe is checksum-pinned and
builds/tests from the lockfile. It installs no daemon or autostart entry.

For AUR publication, first publish the source archive to a repository you own,
replace the local source filename with the real versioned HTTPS release URL,
regenerate checksums with `updpkgsums`, inspect dependencies, and generate `.SRCINFO`
with `makepkg --printsrcinfo`. No fictional repository/release URL is provided.
Google OAuth verification and a public privacy policy are separate release tasks.
