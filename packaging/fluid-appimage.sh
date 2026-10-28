#!/usr/bin/env bash
# Package already-built Qt/Rust binaries. This never reads account/config data.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
binary= engine= version= qt_notices= output=dist
while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary) binary="$2"; shift 2;;
    --engine) engine="$2"; shift 2;;
    --version) version="$2"; shift 2;;
    --qt-notices) qt_notices="$2"; shift 2;;
    --output) output="$2"; shift 2;;
    *) echo "Unknown option: $1" >&2; exit 1;;
  esac
done
[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]]
[[ -n "$binary" && -n "$engine" && -n "$qt_notices" && "$version" =~ ^[0-9A-Za-z][0-9A-Za-z.+_-]{0,100}$ ]]
binary="$(realpath -e "$binary")"; engine="$(realpath -e "$engine")"; qt_notices="$(realpath -e "$qt_notices")"
[[ -x "$binary" && -x "$engine" && -f "$qt_notices/QT-SOURCES.json" ]]
mkdir -p "$output"; output="$(realpath -e "$output")"
destination="$output/Luma-Mail-$version-fluid-x86_64.AppImage"
[[ ! -e "$destination" ]]
build_root="$(mktemp -d "${TMPDIR:-/tmp}/luma-fluid-appimage.XXXXXXXX")"
cleanup() {
  case "$build_root" in
    "${TMPDIR:-/tmp}"/luma-fluid-appimage.*) rm -rf -- "$build_root";;
    *) echo 'Refusing unexpected staging cleanup' >&2;;
  esac
}
trap cleanup EXIT
appdir="$build_root/Luma.AppDir"; mkdir -p "$build_root/tools"
tool="$build_root/tools/linuxdeploy-x86_64.AppImage"
plugin="$build_root/tools/linuxdeploy-plugin-qt-x86_64.AppImage"
curl --fail --location --retry 3 --output "$tool" https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
printf '%s  %s\n' 8aea8da0f7f7039d2a2cecb14657d752a222a5e1d3825caeef186c82f751cdd1 "$tool" | sha256sum --check
curl --fail --location --retry 3 --output "$plugin" https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/continuous/linuxdeploy-plugin-qt-x86_64.AppImage
printf '%s  %s\n' cfc1055b2b9dbc08412b579f20990b7b41a17b61beaa5847dc9477c96c9e9617 "$plugin" | sha256sum --check
chmod +x "$tool" "$plugin"
install -Dm755 "$binary" "$appdir/usr/bin/luma-mail"
install -Dm755 "$engine" "$appdir/usr/bin/luma-engine"
install -Dm644 data/io.github.lumamail.Luma.desktop "$appdir/usr/share/applications/io.github.lumamail.Luma.desktop"
install -Dm644 data/io.github.lumamail.Luma.svg "$appdir/usr/share/icons/hicolor/scalable/apps/io.github.lumamail.Luma.svg"
install -Dm644 data/noctalia/luma.toml.in "$appdir/usr/share/luma-mail/noctalia/luma.toml.in"
install -Dm644 LICENSE "$appdir/usr/share/licenses/luma-mail/LICENSE"
install -Dm644 docs/fluid-packaging.md "$appdir/usr/share/doc/luma-mail/START-HERE.md"
install -Dm644 Cargo.lock "$appdir/usr/share/doc/luma-mail/Cargo.lock"
export QMAKE="${QMAKE:-$(command -v qmake6 || command -v qmake)}"
qt_version="$("$QMAKE" -query QT_VERSION)"
python3 - "$qt_notices/QT-SOURCES.json" "$qt_version" <<'PY'
import json,sys
manifest=json.load(open(sys.argv[1]))
assert manifest['version']==sys.argv[2], 'Qt source notices must match the deployed binary version'
assert len(manifest['modules'])==8 and all(m['notices'] for m in manifest['modules'])
PY
mkdir -p "$appdir/usr/share/licenses/luma-mail/qt"
cp -a "$qt_notices/." "$appdir/usr/share/licenses/luma-mail/qt/"
# Require the native Wayland client plugins instead of silently shipping X11 only.
platforms="$("$QMAKE" -query QT_INSTALL_PLUGINS)/platforms"
mapfile -t wayland < <(find "$platforms" -maxdepth 1 -name 'libqwayland*.so' -printf '%f\n')
[[ ${#wayland[@]} -gt 0 ]]
export EXTRA_PLATFORM_PLUGINS="$(IFS=';'; echo "${wayland[*]}")"
export QML_SOURCES_PATHS="$PWD/crates/fluid/qml"
export EXTRA_QT_MODULES=svg APPIMAGE_EXTRACT_AND_RUN=1
"$tool" --appdir "$appdir" --executable "$appdir/usr/bin/luma-mail" --executable "$appdir/usr/bin/luma-engine" \
  --desktop-file "$appdir/usr/share/applications/io.github.lumamail.Luma.desktop" \
  --icon-file "$appdir/usr/share/icons/hicolor/scalable/apps/io.github.lumamail.Luma.svg" \
  --exclude-library 'libqgtk3.so'
# Apply plugin exclusions to the Qt deployer itself. Geolocation is disabled in
# mail documents; the unused NMEA/GPS plugin must not pull in serial-port support.
"$plugin" --appdir "$appdir" --exclude-library 'libqgtk3.so' --exclude-library 'libqtposition_nmea.so'
[[ -x "$appdir/usr/libexec/QtWebEngineProcess" ]]
[[ -f "$appdir/usr/resources/icudtl.dat" && -f "$appdir/usr/resources/qtwebengine_resources.pak" ]]
[[ -f "$appdir/usr/translations/qtwebengine_locales/en-US.pak" ]]
[[ -f "$appdir/usr/plugins/platformthemes/libqxdgdesktopportal.so" ]]
python3 - "$appdir" <<'PY'
from pathlib import Path
import re,sys
root=Path(sys.argv[1]);legacy=[]
# Qt Wayland's libadwaita.so is a Qt decoration plugin, distinct from the
# GTK4 libadwaita-1.so runtime. Frameless Luma does not request decorations.
for area in ['lib','plugins','qml']:
    for file in (root/'usr'/area).rglob('*'):
        if file.is_file() and re.match(r'(libgtk-|libadwaita-[0-9]|libwebkitgtk).*\.so',file.name,re.I):
            with file.open('rb') as stream: binary=stream.read(4)==b'\x7fELF'
            if binary:legacy.append(file.relative_to(root).as_posix())
if legacy:
    print('Unexpected legacy UI runtime dependencies: '+', '.join(legacy),file=sys.stderr)
    raise SystemExit(1)
print('Native dependency check: no GTK/Adwaita/WebKit frontend libraries')
PY
python3 packaging/native-notices.py "$appdir" "$output/FLUID-NATIVE-SOURCES.tsv"
python3 packaging/rust-notices.py --output "$appdir/usr/share/licenses/luma-mail/rust"
cat > "$appdir/usr/bin/qt.conf" <<'EOF'
[Paths]
Prefix=..
Binaries=bin
LibraryExecutables=libexec
Libraries=lib
Plugins=plugins
QmlImports=qml
Data=.
Translations=translations
EOF
# linuxdeploy makes AppRun a symlink to the executable. Unlink that entry
# before creating a wrapper; redirection through it would overwrite the ELF.
python3 - "$appdir" <<'PY'
from pathlib import Path
import sys
root=Path(sys.argv[1]);launcher=root/'AppRun';binary=root/'usr/bin/luma-mail'
with binary.open('rb') as stream:
    if stream.read(4)!=b'\x7fELF':raise SystemExit('The deployed mail application is not an ELF executable')
if launcher.is_symlink():launcher.unlink()
elif launcher.exists():raise SystemExit('Unexpected existing AppRun file; refusing to overwrite it')
PY
cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
set -eu
APPDIR="${APPDIR:-$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)}"
export APPDIR
export LD_LIBRARY_PATH="$APPDIR/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export QT_PLUGIN_PATH="$APPDIR/usr/plugins"
export QML_IMPORT_PATH="$APPDIR/usr/qml"
export QTWEBENGINEPROCESS_PATH="$APPDIR/usr/libexec/QtWebEngineProcess"
export QTWEBENGINE_RESOURCES_PATH="$APPDIR/usr/resources"
export QTWEBENGINE_LOCALES_PATH="$APPDIR/usr/translations/qtwebengine_locales"
export XCURSOR_PATH="${XCURSOR_PATH:-${XDG_DATA_HOME:-$HOME/.local/share}/icons:$HOME/.icons:/usr/local/share/icons:/usr/share/icons:/usr/share/pixmaps}"
if [ -n "${WAYLAND_DISPLAY:-}" ] && [ -z "${QT_QPA_PLATFORM:-}" ]; then export QT_QPA_PLATFORM='wayland;xcb'; fi
# Use the session's native file-chooser portal (GNOME/KDE/Hyprland as configured).
# Host-only qt5ct/qt6ct plugins cannot safely load into the bundled Qt runtime.
theme="${QT_QPA_PLATFORMTHEME:-xdgdesktopportal}"
case "$theme" in
  *[!a-zA-Z0-9_-]*|'') theme=xdgdesktopportal;;
  *) [ -f "$APPDIR/usr/plugins/platformthemes/libq${theme}.so" ] || theme=xdgdesktopportal;;
esac
export QT_QPA_PLATFORMTHEME="$theme"
exec "$APPDIR/usr/bin/luma-mail" "$@"
EOF
chmod +x "$appdir/AppRun"
python3 - "$appdir/usr/bin/luma-mail" <<'PY'
import sys
with open(sys.argv[1],'rb') as binary:
    if binary.read(4)!=b'\x7fELF':raise SystemExit('Launcher creation changed the native application')
PY
python3 packaging/audit-storage.py --tree installation "$appdir" --output "$output/FLUID-INSTALLED-STORAGE.json"
export OUTPUT="$destination" ARCH=x86_64
"$tool" --appdir "$appdir" --output appimage
chmod +x "$destination"
(cd "$output" && sha256sum "$(basename "$destination")" > "$(basename "$destination").sha256")
git archive --format=tar.gz --output="$output/Luma-Mail-$version-source.tar.gz" HEAD
