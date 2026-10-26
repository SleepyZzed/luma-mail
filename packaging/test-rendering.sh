#!/usr/bin/env bash
set -euo pipefail
root="$(mktemp -d)"
if [[ -n ${2:-} ]]; then
  mkdir -p "$(dirname "$2")"
  export LUMA_RENDER_SCREENSHOT="$(realpath -m "$2")"
fi
trap 'rm -rf -- "$root"' EXIT
python3 - "$root" <<'PY'
import sys, wave
from pathlib import Path
with wave.open(str(Path(sys.argv[1]) / 'audio.wav'), 'wb') as wav:
    wav.setparams((1, 2, 8000, 0, 'NONE', 'not compressed'))
    wav.writeframes(b'\0\0' * 16000)
PY
gst-launch-1.0 -q videotestsrc num-buffers=30 ! video/x-raw,width=160,height=120,framerate=15/1 ! videoconvert ! vp8enc ! webmmux ! filesink location="$root/video.webm"
mkdir -p "$root/app/config"
printf '%s\n' '{"cache_mib":0,"cache_ttl_hours":0}' > "$root/app/config/settings.json"
mkdir -p "$root/runtime" "$root/xdg-cache"
chmod 700 "$root/runtime"
# Start the bus after the display/environment exist, so activated portals inherit
# them too. A headless runner has no pre-existing graphical session environment.
export LUMA_TEST_ROOT="$root/app" LUMA_RENDER_MEDIA="$root"
export XDG_CACHE_HOME="$root/xdg-cache" XDG_RUNTIME_DIR="$root/runtime"
export APPIMAGE_EXTRACT_AND_RUN=1
export GTK_A11Y=none LIBGL_ALWAYS_SOFTWARE=1
if ! timeout 45s xvfb-run -a dbus-run-session -- "$1" --render-diagnostics; then
  echo 'Renderer launch failed; recent kernel sandbox diagnostics follow'
  sudo journalctl -k --no-pager -n 60 || true
  exit 1
fi
for unexpected in "$root/app/cache/bodies" "$root/app/config/composer" "$root/runtime/luma-mail/gstreamer/disabled-registry.bin"; do
  if [[ -e "$unexpected" ]]; then
    echo "FAIL zero-retention check: $unexpected"
    stat "$unexpected"
    exit 1
  fi
done
echo 'PASS zero-budget HTML/media creates no body cache, attachment spool or local draft store'
