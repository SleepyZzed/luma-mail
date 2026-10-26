#!/usr/bin/env bash
set -euo pipefail
root="$(mktemp -d)"
trap 'rm -rf -- "$root"' EXIT
mkdir -p "$root/app/config" "$root/runtime" "$root/xdg-cache" dist
chmod 700 "$root/runtime"
printf '%s\n' '{"cache_mib":0,"accounts":[],"theme":"dark"}' > "$root/app/config/settings.json"
export LUMA_TEST_ROOT="$root/app" XDG_RUNTIME_DIR="$root/runtime" XDG_CACHE_HOME="$root/xdg-cache"
export GTK_A11Y=none LIBGL_ALWAYS_SOFTWARE=1 GDK_BACKEND=x11
timeout 155s xvfb-run -a dbus-run-session -- \
  /usr/bin/python3 packaging/profile-processes.py "$1" dist/performance.json
