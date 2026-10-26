#!/usr/bin/env bash
# Match the AppImage's read-only application tree without requiring FUSE in CI.
set -euo pipefail
image="$(realpath -e "$1")"
screenshot="$(realpath -m "$2")"
root="$(mktemp -d -t luma-readonly.XXXXXXXX)"
cleanup() {
  case "$root" in
    "${TMPDIR:-/tmp}"/luma-readonly.*)
      chmod -R u+w "$root"
      rm -rf -- "$root"
      ;;
    *) echo 'Unexpected read-only test root; refusing cleanup' >&2 ;;
  esac
}
trap cleanup EXIT
(cd "$root"; "$image" --appimage-extract >/dev/null)
python3 packaging/audit-storage.py --tree compressed_appimage "$image" \
  --tree extracted_installation "$root/squashfs-root" \
  --output "${screenshot%.png}-package-storage.json"
chmod -R a-w "$root/squashfs-root"
bash packaging/without-system-webkit.sh bash packaging/test-rendering.sh "$root/squashfs-root/AppRun" "$screenshot"
