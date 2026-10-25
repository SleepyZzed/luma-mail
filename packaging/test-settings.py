#!/usr/bin/env python3
"""Verify the bundled dconf module and GTK cursor settings on a private bus.

This checks configuration transport. Cursor artwork and HiDPI rendering still
need testing on the actual Wayland compositor and installed cursor theme.
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

if len(sys.argv) == 2:
    app = str(Path(sys.argv[1]).resolve())
    with tempfile.TemporaryDirectory(prefix="luma-settings-test-") as root:
        env = dict(os.environ, XDG_CONFIG_HOME=root, LUMA_TEST_ROOT=str(Path(root) / "app"),
                   GSETTINGS_BACKEND="dconf", APPIMAGE_EXTRACT_AND_RUN="1")
        env.pop("XCURSOR_THEME", None)
        env.pop("XCURSOR_SIZE", None)
        subprocess.run(["dbus-run-session", "--", "xvfb-run", "-a", sys.executable,
                        __file__, "--inside", app], env=env, check=True, timeout=60)
elif len(sys.argv) == 3 and sys.argv[1] == "--inside":
    app = sys.argv[2]
    subprocess.run(["gsettings", "set", "org.gnome.desktop.interface", "cursor-theme", "LumaCiCursor"], check=True)
    subprocess.run(["gsettings", "set", "org.gnome.desktop.interface", "cursor-size", "36"], check=True)
    result = subprocess.run([app, "--smoke-test", "--desktop-diagnostics"],
                            text=True, capture_output=True, timeout=20, check=True)
    assert "GTK cursor: LumaCiCursor · 36 px" in result.stdout, result.stdout + result.stderr
    env = dict(os.environ, XCURSOR_THEME="LumaEnvironmentCursor", XCURSOR_SIZE="42")
    result = subprocess.run([app, "--smoke-test", "--desktop-diagnostics"], env=env,
                            text=True, capture_output=True, timeout=20, check=True)
    assert "GTK cursor: LumaEnvironmentCursor · 42 px" in result.stdout, result.stdout + result.stderr
    print("PASS: bundled GTK reads the host dconf cursor preference and honours session cursor overrides")
else:
    raise SystemExit("Usage: test-settings.py path/to/Luma-Mail.AppImage")
