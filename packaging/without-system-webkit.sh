#!/usr/bin/env bash
# Disposable CI runner only: ensure the AppImage cannot fall back to host WebKit.
set -euo pipefail
libdir="$(pkg-config --variable=libdir webkitgtk-6.0)"
root="$(mktemp -d)"
originals=()
saved=()
restore() {
  for ((i=${#originals[@]}-1; i>=0; i--)); do
    sudo mv -- "${saved[i]}" "${originals[i]}"
  done
  rmdir -- "$root"
}
trap restore EXIT
for source in "$libdir/libwebkitgtk-6.0.so.4" "$libdir/libjavascriptcoregtk-6.0.so.1" "$libdir/webkitgtk-6.0"; do
  source="$(realpath -e "$source")"
  case "$source" in /usr/lib/*) ;; *) echo 'Unexpected system WebKit path' >&2; exit 1;; esac
  destination="$root/$(basename "$source")"
  sudo mv -- "$source" "$destination"
  originals+=("$source")
  saved+=("$destination")
done
"$@"
