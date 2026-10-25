#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
build_features=()
if [[ ${1:-} == --require-google-client ]]; then
  build_features=(--features require-google-client)
elif [[ $# -gt 0 ]]; then
  echo 'Usage: packaging/appimage.sh [--require-google-client]' >&2
  exit 1
fi
if [[ $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
  echo 'AppImage packaging requires x86_64 Linux.' >&2
  exit 1
fi
build_root="$(mktemp -d "${TMPDIR:-/tmp}/luma-appimage.XXXXXXXX")"
cleanup() {
  # Only remove the private staging directory returned by mktemp above.
  case "$build_root" in
    "${TMPDIR:-/tmp}"/luma-appimage.*) rm -rf -- "$build_root" ;;
    *) echo 'Refusing to clean an unexpected staging path.' >&2 ;;
  esac
}
trap cleanup EXIT
mkdir -p "$build_root/tools" dist
appdir="$build_root/Luma.AppDir"
tool="$build_root/tools/linuxdeploy-x86_64.AppImage"
plugin="$build_root/tools/linuxdeploy-plugin-gtk.sh"
curl --fail --location --retry 3 --output "$tool" https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
printf '%s  %s\n' 8aea8da0f7f7039d2a2cecb14657d752a222a5e1d3825caeef186c82f751cdd1 "$tool" | sha256sum --check
curl --fail --location --retry 3 --output "$plugin" https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/7a3fbc31a9e5075073ff8790f26effbac5f84453/linuxdeploy-plugin-gtk.sh
printf '%s  %s\n' b0f4cbc684a0103a9651f0955b635eaea0096b3a66c0f5a2c2aa337960375171 "$plugin" | sha256sum --check
# The plugin is pinned to an immutable upstream commit, and its output hook is
# adjusted below so GTK chooses Wayland and libadwaita manages its own theme.
chmod +x "$tool" "$plugin"
cargo build --locked --release -p luma-mail "${build_features[@]}"
install -Dm755 target/release/luma-mail "$appdir/usr/bin/luma-mail"
install -Dm644 data/io.github.lumamail.Luma.desktop "$appdir/usr/share/applications/io.github.lumamail.Luma.desktop"
install -Dm644 data/io.github.lumamail.Luma.svg "$appdir/usr/share/icons/hicolor/scalable/apps/io.github.lumamail.Luma.svg"
install -Dm644 data/noctalia/luma.toml.in "$appdir/usr/share/luma-mail/noctalia/luma.toml.in"
install -Dm644 LICENSE "$appdir/usr/share/licenses/luma-mail/LICENSE"
# dconf is a dynamically loaded GIO module, so ELF dependency scanning alone
# misses it. Without it the bundle sees schema defaults instead of user settings.
gio_modules="$(pkg-config --variable=giomoduledir gio-2.0)"
if [[ -z "$gio_modules" ]]; then
  gio_modules="$(pkg-config --variable=libdir gio-2.0)/gio/modules"
fi
test -f "$gio_modules/libdconfsettings.so"
install -Dm755 "$gio_modules/libdconfsettings.so" "$appdir/usr/lib/gio/modules/libdconfsettings.so"
gio-querymodules "$appdir/usr/lib/gio/modules"
cp -a /usr/share/icons/Adwaita "$appdir/usr/share/icons/"
export APPIMAGE_EXTRACT_AND_RUN=1 DEPLOY_GTK_VERSION=4
"$tool" --appdir "$appdir" --executable "$appdir/usr/bin/luma-mail" --desktop-file "$appdir/usr/share/applications/io.github.lumamail.Luma.desktop" --icon-file "$appdir/usr/share/icons/hicolor/scalable/apps/io.github.lumamail.Luma.svg" --plugin gtk
hook="$appdir/apprun-hooks/linuxdeploy-plugin-gtk.sh"
test -f "$hook"
sed -i '/^export GDK_BACKEND=x11 /d; /^export GTK_THEME=/d' "$hook"
cat >> "$hook" <<'EOF'
# Use the bundled ABI-compatible settings module to read the host dconf daemon.
export GIO_MODULE_DIR="$APPDIR/usr/lib/gio/modules"
export GIO_EXTRA_MODULES="$APPDIR/usr/lib/gio/modules${GIO_EXTRA_MODULES:+:$GIO_EXTRA_MODULES}"
# Keep host-installed cursor themes visible, including user-local themes.
export XCURSOR_PATH="${XCURSOR_PATH:-${XDG_DATA_HOME:-$HOME/.local/share}/icons:$HOME/.icons:/usr/local/share/icons:/usr/share/icons:/usr/share/pixmaps}"
EOF
version="$(cargo metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "luma-mail"))')"
export OUTPUT="$(pwd)/dist/Luma-Mail-$version-x86_64.AppImage" ARCH=x86_64
"$tool" --appdir "$appdir" --output appimage
chmod +x "$OUTPUT"
(
  cd dist
  sha256sum "$(basename "$OUTPUT")" > SHA256SUMS
)
