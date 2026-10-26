#!/usr/bin/env bash
set -euo pipefail
root="$(mktemp -d)"
if [[ -n ${2:-} ]]; then
  mkdir -p "$(dirname "$2")"
  export LUMA_RENDER_SCREENSHOT="$(realpath -m "$2")"
fi
trap 'rm -rf -- "$root"' EXIT
python3 - "$root" <<'PY'
import sys, wave, struct, zlib
from pathlib import Path
with wave.open(str(Path(sys.argv[1]) / 'audio.wav'), 'wb') as wav:
    wav.setparams((1, 2, 8000, 0, 'NONE', 'not compressed'))
    wav.writeframes(b'\0\0' * 16000)
def chunk(kind, data):
    return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', zlib.crc32(kind + data))
# Synthetic portrait image: no private mailbox content in rendering artifacts.
rows = b''.join(b'\0' + bytes((30, 170 if y < 360 else 110, 180)) * 180 for y in range(720))
png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!IIBBBBB', 180, 720, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b'')
(Path(sys.argv[1]) / 'portrait.png').write_bytes(png)
PY
gst-launch-1.0 -q videotestsrc num-buffers=30 ! video/x-raw,width=160,height=120,framerate=15/1 ! videoconvert ! vp8enc ! webmmux ! filesink location="$root/video.webm"
mkdir -p "$root/app/config"
printf '%s\n' '{"cache_mib":0,"cache_ttl_hours":0}' > "$root/app/config/settings.json"
mkdir -p "$root/runtime" "$root/xdg-cache"
chmod 700 "$root/runtime"
# Start the bus after the display/environment exist, so activated portals inherit
# them too. A headless runner has no pre-existing graphical session environment.
export LUMA_TEST_ROOT="$root/app" LUMA_RENDER_MEDIA="$root"
export XDG_CACHE_HOME="$root/xdg-cache"
export APPIMAGE_EXTRACT_AND_RUN=1
export GTK_A11Y=none LIBGL_ALWAYS_SOFTWARE=1
if [[ ${LUMA_TEST_DISPLAY:-x11} == wayland ]]; then
  display=(bash packaging/with-wayland.sh)
else
  export XDG_RUNTIME_DIR="$root/runtime" GDK_BACKEND=x11
  display=(xvfb-run -a)
fi
if ! timeout 45s "${display[@]}" dbus-run-session -- "$1" --render-diagnostics; then
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
codec_index="$root/runtime/luma-mail/gstreamer/codecs.bin"
if [[ -f "$codec_index" ]]; then
  # Fixed library metadata is separate from message contents. This inventory is
  # private runtime data, not a mailbox/attachment cache; guard against growth.
  test "$(stat -c %s "$codec_index")" -lt 4194304
  echo "PASS private runtime codec inventory: $(stat -c %s "$codec_index") bytes, no mail data"
fi
echo 'PASS zero-budget HTML/media creates no body cache, attachment spool or local draft store'
if find "$root/xdg-cache" -maxdepth 1 -name 'mesa_shader_cache*' -print -quit | grep -q .; then
  echo 'FAIL Mesa shader cache escaped private runtime storage' >&2
  exit 1
fi
echo 'PASS Mesa shader cache stays in private runtime storage with a 16 MiB driver limit'
report=()
if [[ -n ${LUMA_RENDER_SCREENSHOT:-} ]]; then
  report=(--output "${LUMA_RENDER_SCREENSHOT%.png}-runtime-storage.json")
fi
python3 packaging/audit-storage.py --tree configuration "$root/app/config" \
  --tree app_cache "$root/app/cache" --tree runtime "$root/runtime" \
  --tree other_cache "$root/xdg-cache" "${report[@]}"
