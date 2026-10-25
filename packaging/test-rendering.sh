#!/usr/bin/env bash
set -euo pipefail
root="$(mktemp -d)"
trap 'rm -rf -- "$root"' EXIT
python3 - "$root" <<'PY'
import sys, wave
from pathlib import Path
with wave.open(str(Path(sys.argv[1]) / 'audio.wav'), 'wb') as wav:
    wav.setparams((1, 2, 8000, 0, 'NONE', 'not compressed'))
    wav.writeframes(b'\0\0' * 16000)
PY
gst-launch-1.0 -q videotestsrc num-buffers=30 ! video/x-raw,width=160,height=120,framerate=15/1 ! videoconvert ! vp8enc ! webmmux ! filesink location="$root/video.webm"
timeout 45s dbus-run-session -- xvfb-run -a env LUMA_TEST_ROOT="$root/app" LUMA_RENDER_MEDIA="$root" APPIMAGE_EXTRACT_AND_RUN=1 "$1" --render-diagnostics
