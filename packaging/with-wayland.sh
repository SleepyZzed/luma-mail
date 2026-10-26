#!/usr/bin/env bash
# A real headless Wayland compositor, isolated from the runner's X11 session.
set -euo pipefail
root="$(mktemp -d)"
cleanup() {
  if [[ -n ${compositor:-} ]]; then kill "$compositor" 2>/dev/null || true; wait "$compositor" 2>/dev/null || true; fi
  rm -rf -- "$root"
}
trap cleanup EXIT
export XDG_RUNTIME_DIR="$root"
chmod 700 "$root"
export WAYLAND_DISPLAY=luma-test-wayland GDK_BACKEND=wayland
unset DISPLAY
weston --backend=headless-backend.so --renderer=pixman --socket="$WAYLAND_DISPLAY" --idle-time=0 --no-config --log="$root/weston.log" &
compositor=$!
for ((i=0; i<100; i++)); do
  if [[ -S "$root/$WAYLAND_DISPLAY" ]]; then break; fi
  if ! kill -0 "$compositor" 2>/dev/null; then cat "$root/weston.log"; exit 1; fi
  sleep 0.05
done
test -S "$root/$WAYLAND_DISPLAY"
"$@"
