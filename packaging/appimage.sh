#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
if [[ $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
  echo 'AppImage packaging requires x86_64 Linux.' >&2
  exit 1
fi
build_root="$(pwd)/target/appimage"
mkdir -p "$build_root/tools" dist
appdir="$build_root/Luma.AppDir"
if [[ -e "$appdir" ]]; then
  echo "Use a fresh target/appimage/Luma.AppDir for a reproducible bundle." >&2
  exit 1
fi
tool="$build_root/tools/linuxdeploy-x86_64.AppImage"
plugin="$build_root/tools/linuxdeploy-plugin-gtk.sh"
curl --fail --location --retry 3 --output "$tool" https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
printf '%s  %s\n' 8aea8da0f7f7039d2a2cecb14657d752a222a5e1d3825caeef186c82f751cdd1 "$tool" | sha256sum --check
curl --fail --location --retry 3 --output "$plugin" https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/7a3fbc31a9e5075073ff8790f26effbac5f84453/linuxdeploy-plugin-gtk.sh
# The plugin is pinned to an immutable upstream commit, and its output hook is
# adjusted below so GTK chooses Wayland and libadwaita manages its own theme.
chmod +x "$tool" "$plugin"
cargo build --locked --release -p luma-mail
install -Dm755 target/release/luma-mail "$appdir/usr/bin/luma-mail"
install -Dm644 data/io.github.lumamail.Luma.desktop "$appdir/usr/share/applications/io.github.lumamail.Luma.desktop"
install -Dm644 data/io.github.lumamail.Luma.svg "$appdir/usr/share/icons/hicolor/scalable/apps/io.github.lumamail.Luma.svg"
install -Dm644 data/noctalia/luma.toml.in "$appdir/usr/share/luma-mail/noctalia/luma.toml.in"
install -Dm644 LICENSE "$appdir/usr/share/licenses/luma-mail/LICENSE"
cp -a /usr/share/icons/Adwaita "$appdir/usr/share/icons/"
export APPIMAGE_EXTRACT_AND_RUN=1 DEPLOY_GTK_VERSION=4
"$tool" --appdir "$appdir" --executable "$appdir/usr/bin/luma-mail" --desktop-file "$appdir/usr/share/applications/io.github.lumamail.Luma.desktop" --icon-file "$appdir/usr/share/icons/hicolor/scalable/apps/io.github.lumamail.Luma.svg" --plugin gtk
hook="$appdir/apprun-hooks/linuxdeploy-plugin-gtk.sh"
test -f "$hook"
sed -i '/^export GDK_BACKEND=x11 /d; /^export GTK_THEME=/d' "$hook"
export OUTPUT="$(pwd)/dist/Luma-Mail-0.1.0-x86_64.AppImage" ARCH=x86_64
"$tool" --appdir "$appdir" --output appimage
chmod +x "$OUTPUT"
(
  cd dist
  sha256sum Luma-Mail-0.1.0-x86_64.AppImage > SHA256SUMS
)
