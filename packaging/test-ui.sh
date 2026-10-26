#!/usr/bin/env bash
set -euo pipefail
root="$(mktemp -d)"
trap 'rm -rf -- "$root"' EXIT
mkdir -p "$root/config" dist
printf '%s\n' '{"cache_mib":0,"accounts":[],"theme":"noctalia"}' > "$root/config/settings.json"
cat > "$root/config/noctalia.toml" <<'THEME'
mode = "dark"
background = "#21191F"
surface = "#281E25"
text = "#F2EDF1"
muted = "#ADA2AC"
accent = "#F2A4CF"
on_accent = "#321426"
outline = "#44353F"
hover = "#352631"
THEME
for view in reader compose reply; do
  args=(--design-preview --smoke-test)
  [[ "$view" == reader ]] || args+=("--preview-$view")
  timeout 20s dbus-run-session -- xvfb-run -a env LUMA_TEST_ROOT="$root" \
    LUMA_SMOKE_SCREENSHOT="$(pwd)/dist/native-ui-$view.png" target/debug/luma-mail "${args[@]}" 2>"$root/$view.log"
  if grep -Ei 'Theme parser (error|warning)|Failed to load.*luma-|could not load.*luma-|Gtk-CRITICAL' "$root/$view.log"; then exit 1; fi
  test -s "dist/native-ui-$view.png"
done
echo 'PASS native Linux reader, compact compose, inline reply and zero CSS/icon errors'
