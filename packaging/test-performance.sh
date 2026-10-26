#!/usr/bin/env bash
set -euo pipefail
root="$(mktemp -d)"
trap 'rm -rf -- "$root"' EXIT
mkdir -p "$root/config" dist
printf '%s\n' '{"cache_mib":0,"accounts":[],"theme":"dark"}' > "$root/config/settings.json"
timeout 155s dbus-run-session -- xvfb-run -a env LUMA_TEST_ROOT="$root" \
  /usr/bin/python3 packaging/profile-processes.py "$1" dist/performance.json
