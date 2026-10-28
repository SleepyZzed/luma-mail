#!/bin/sh
# Test-only X11 desktop. Window restore/maximize assertions require an actual
# window manager; a bare Xvfb server cannot implement those desktop requests.
set -eu
exec xvfb-run -a -s '-screen 0 1920x1080x24' sh -c '
  openbox --sm-disable >/dev/null 2>&1 &
  desktop_wm=$!
  trap '\''kill "$desktop_wm" 2>/dev/null || :; wait "$desktop_wm" 2>/dev/null || :'\'' EXIT HUP INT TERM
  desktop_ready=0
  while [ "$desktop_ready" -lt 60 ]; do
    if xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null | grep -q "window id"; then break; fi
    desktop_ready=$((desktop_ready+1))
    sleep .05
  done
  if [ "$desktop_ready" -eq 60 ]; then echo "Test window manager did not start" >&2; exit 1; fi
  "$@"
' sh "$@"
